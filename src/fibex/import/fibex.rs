//! ASAM FIBEX (2.x/3.x) FlexRay 数据库解析
//!
//! 采用宽容解析策略：忽略命名空间，兼容不同厂商对方言的使用差异。已验证的方言：
//! - ASAM 样例风格：SIGNAL-INSTANCE/START-BIT-POSITION、FRAME/PDU-MAPPING、
//!   CHANNEL/SLOT/FRAME-TRIGGERING 三层包装
//! - Vector DaVinci/CANoe 风格（FIBEX 2.0.0d / 3.0）：SIGNAL-INSTANCE/BIT-POSITION、
//!   FRAME/PDU-INSTANCES/PDU-INSTANCE、FRAME-TRIGGERING 直挂 CHANNEL（无 SLOT 层，
//!   SLOT-ID 位于 TIMINGS/ABSOLUTELY-SCHEDULED-TIMING 内）、BYTE-LENGTH 表长度、
//!   flexray:FLEXRAY-CHANNEL-NAME 标识通道、fx:ECU/fx:CONTROLLER 层级
//!   结构：PROJECT/ECUS + ELEMENTS(CLUSTER/CHANNEL/FRAME-TRIGGERING/FRAME/PDU/SIGNAL/CODING)

use super::{
    collect_elements_by_tag, find_text_candidates, get_short_name, parse_cycle_repetition,
    parse_f64, parse_u32,
};
use crate::fibex::editable_fibex::*;

/// 读取子元素中的引用：优先 ID-REF 属性，回退到元素文本
fn get_ref(node: &roxmltree::Node, tag: &str) -> Option<String> {
    let child = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == tag)?;
    if let Some(id_ref) = child.attribute("ID-REF") {
        return Some(id_ref.to_string());
    }
    child.text().map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

/// 解析节点下的 SIGNAL-INSTANCE 列表：返回 (SIGNAL-REF ID, 起始位, 大端)
///
/// PDU 内的实例和"信号直挂 FRAME"（无 PDU 层的老方言）都走这里。
fn parse_signal_instances(node: &roxmltree::Node) -> Vec<(String, u32, bool)> {
    let mut sig_instances = Vec::new();
    collect_elements_by_tag(node, "SIGNAL-INSTANCE", &mut sig_instances);
    let mut out = Vec::new();
    for si in &sig_instances {
        let sig_ref = get_ref(si, "SIGNAL-REF").unwrap_or_default();
        if sig_ref.is_empty() {
            continue;
        }
        // FIBEX 标准元素为 BIT-POSITION；START-BIT-POSITION 为部分工具方言
        let start_bit = find_text_candidates(
            si,
            &["BIT-POSITION", "START-BIT-POSITION", "START-POSITION"],
        )
        .and_then(|t| parse_u32(&t))
        .unwrap_or(0);
        let is_high_low = find_text_candidates(si, &["IS-HIGH-LOW-BYTE-ORDER"])
            .map(|t| t.eq_ignore_ascii_case("true"))
            .unwrap_or(true);
        out.push((sig_ref, start_bit, is_high_low));
    }
    out
}

/// 解析 FRAME 下的 PDU 挂载：返回 (PDU ID, 起始字节)
///
/// 兼容两种方言：
/// - fx:PDU-MAPPING + START-BIT-POSITION（按字节计）
/// - fx:PDU-INSTANCE + BIT-POSITION（按位计、字节对齐，除以 8 转字节）
fn parse_pdu_placements(frame: &roxmltree::Node) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    for tag in ["PDU-MAPPING", "PDU-INSTANCE"] {
        let mut nodes = Vec::new();
        collect_elements_by_tag(frame, tag, &mut nodes);
        for m in &nodes {
            let pdu_ref = get_ref(m, "PDU-REF").unwrap_or_default();
            if pdu_ref.is_empty() {
                continue;
            }
            let start = match find_text_candidates(m, &["BIT-POSITION"]).and_then(|t| parse_u32(&t))
            {
                Some(bits) => bits / 8, // BIT-POSITION 以位计，FlexRay PDU 字节对齐
                None => find_text_candidates(m, &["START-BIT-POSITION", "START-POSITION"])
                    .and_then(|t| parse_u32(&t))
                    .unwrap_or(0),
            };
            out.push((pdu_ref, start));
        }
        if !out.is_empty() {
            break; // 两种方言不会同时出现
        }
    }
    out
}

/// 通道归属判定：优先显式标识元素（flexray:FLEXRAY-CHANNEL-NAME / CHANNEL-IDENTIFIER），
/// 回退到 SHORT-NAME 含 "B" 或双通道取第二个
fn classify_channel(channel: &roxmltree::Node, ch_idx: usize, total: usize) -> FrChannel {
    for tag in ["FLEXRAY-CHANNEL-NAME", "CHANNEL-IDENTIFIER"] {
        if let Some(t) = find_text_candidates(channel, &[tag]) {
            let t = t.trim().to_uppercase();
            if t.contains('B') {
                return FrChannel::B;
            }
            if t.contains('A') {
                return FrChannel::A;
            }
        }
    }
    let name = get_short_name(channel).to_uppercase();
    if name.contains('B') || (ch_idx == 1 && total == 2) {
        FrChannel::B
    } else {
        FrChannel::A
    }
}

pub fn parse_fibex_doc(doc: &roxmltree::Document) -> Result<EditableFibex, String> {
    let root = doc.root_element();

    // ------------------------------------------------------------------
    // 1. 收集信号（fx:SIGNAL）与编码（fx:CODING）
    // ------------------------------------------------------------------
    let mut signal_nodes = Vec::new();
    collect_elements_by_tag(&root, "SIGNAL", &mut signal_nodes);

    let mut coding_nodes = Vec::new();
    collect_elements_by_tag(&root, "CODING", &mut coding_nodes);

    // 编码 ID -> (位长, 符号, 因子, 偏移, min, max, 单位)
    struct CodingInfo {
        bit_length: u32,
        signed: bool,
        factor: f64,
        offset: f64,
        min: f64,
        max: f64,
        unit: String,
        value_descriptions: Vec<(i64, String)>,
    }

    let mut codings: std::collections::HashMap<String, CodingInfo> =
        std::collections::HashMap::new();
    for coding in &coding_nodes {
        let id = coding.attribute("ID").unwrap_or_default().to_string();
        let bit_length = find_text_candidates(coding, &["BIT-LENGTH"])
            .and_then(|t| parse_u32(&t))
            .unwrap_or(1);
        let mut signed = find_text_candidates(coding, &["SIGN-CONVENTION"])
            .map(|t| {
                let u = t.to_uppercase();
                // 注意 "UNSIGNED" 也包含 "SIGNED"，需排除
                u.contains("TWOS") || u.contains("SYMMETRIC") || (u.contains("SIGNED") && !u.starts_with("UN"))
            })
            .unwrap_or(false);
        if !signed {
            // Vector 方言无 SIGN-CONVENTION，用 CODED-TYPE 的 BASE-DATA-TYPE=A_INT* 判定
            let mut coded_types = Vec::new();
            collect_elements_by_tag(coding, "CODED-TYPE", &mut coded_types);
            signed = coded_types.iter().any(|ct| {
                ct.attributes()
                    .any(|a| a.name().contains("BASE-DATA-TYPE") && a.value().contains("A_INT"))
            });
        }
        let mut factor = 1.0f64;
        let mut offset = 0.0f64;
        let mut min = 0.0f64;
        let mut max = 0.0f64;
        let mut unit = String::new();
        let mut value_descriptions = Vec::new();

        // COMPU-RATIONAL-COEFFS: NUMERATOR/V-DENOMINATOR/V
        // 在整个 CODING 子树内查找（SCALE 与 COMPU-SCALE 两种容器方言都覆盖）
        let mut coeffs = Vec::new();
        collect_elements_by_tag(coding, "COMPU-RATIONAL-COEFFS", &mut coeffs);
        if let Some(coeff) = coeffs.first() {
            let mut numerators = Vec::new();
            collect_elements_by_tag(coeff, "V", &mut numerators);
            let values: Vec<f64> = numerators
                .iter()
                .filter_map(|n| n.text().and_then(parse_f64))
                .collect();
            // FIBEX: numerator = [offset, factor], denominator = [1]
            if values.len() >= 2 {
                offset = values[0];
                factor = values[1];
            } else if values.len() == 1 {
                factor = values[0];
            }
        }

        // 量程：优先 SCALE-CONSTR（Vector，位于 PHYS-CONSTRS 下），回退 SCALE；
        // 跳过带 VT 的枚举项
        'limits: for tag in ["SCALE-CONSTR", "SCALE"] {
            let mut scales = Vec::new();
            collect_elements_by_tag(coding, tag, &mut scales);
            for scale in &scales {
                if find_text_candidates(scale, &["VT"]).is_some() {
                    continue;
                }
                let lo = find_text_candidates(scale, &["LOWER-LIMIT"]).and_then(|t| parse_f64(&t));
                let hi = find_text_candidates(scale, &["UPPER-LIMIT"]).and_then(|t| parse_f64(&t));
                if lo.is_some() || hi.is_some() {
                    if let Some(lo) = lo {
                        min = lo;
                    }
                    if let Some(hi) = hi {
                        max = hi;
                    }
                    break 'limits;
                }
            }
        }

        // 单位：UNIT-REF 引用独立 UNIT 元素（按 ID 查 DISPLAY-NAME），
        // 回退到内联 UNIT（FUNCTION-CHANNEL/UNIT/DISPLAY-NAME）
        let mut unit_refs = Vec::new();
        collect_elements_by_tag(coding, "UNIT-REF", &mut unit_refs);
        if let Some(unit_ref) = unit_refs.first() {
            let unit_id = unit_ref.attribute("ID-REF").unwrap_or_default();
            if !unit_id.is_empty() {
                let mut unit_nodes = Vec::new();
                collect_elements_by_tag(&root, "UNIT", &mut unit_nodes);
                if let Some(u) = unit_nodes
                    .iter()
                    .find(|n| n.attribute("ID") == Some(unit_id))
                    && let Some(dn) = find_text_candidates(u, &["DISPLAY-NAME"])
                {
                    unit = dn;
                }
            }
        } else if let Some(dn) = find_text_candidates(coding, &["DISPLAY-NAME"]) {
            unit = dn;
        }

        // 值描述（带 VT 的枚举项；容器可能是 COMPU-SCALE / SCALE-CONSTR / SCALE）
        let mut seen_vt: std::collections::HashSet<(i64, String)> =
            std::collections::HashSet::new();
        for tag in ["COMPU-SCALE", "SCALE-CONSTR", "SCALE"] {
            let mut scales = Vec::new();
            collect_elements_by_tag(coding, tag, &mut scales);
            for cs in &scales {
                let Some(vt) = find_text_candidates(cs, &["VT"]) else {
                    continue;
                };
                let Some(lo) =
                    find_text_candidates(cs, &["LOWER-LIMIT"]).and_then(|t| parse_u32(&t))
                else {
                    continue;
                };
                if seen_vt.insert((lo as i64, vt.clone())) {
                    value_descriptions.push((lo as i64, vt));
                }
            }
        }

        codings.insert(
            id,
            CodingInfo {
                bit_length,
                signed,
                factor,
                offset,
                min,
                max,
                unit,
                value_descriptions,
            },
        );
    }

    // 信号 ID -> (名称, CODING-REF, 注释)
    let mut signal_defs: std::collections::HashMap<String, (String, String, String)> =
        std::collections::HashMap::new();
    for sig in &signal_nodes {
        let Some(id) = sig.attribute("ID") else {
            continue;
        };
        let name = get_short_name(sig);
        let coding_ref = get_ref(sig, "CODING-REF").unwrap_or_default();
        let comment = find_text_candidates(sig, &["L-2", "DESC"]).unwrap_or_default();
        signal_defs.insert(id.to_string(), (name, coding_ref, comment));
    }

    // ------------------------------------------------------------------
    // 2. 收集 PDU
    // ------------------------------------------------------------------
    let mut pdu_nodes = Vec::new();
    collect_elements_by_tag(&root, "PDU", &mut pdu_nodes);

    struct PduRaw {
        name: String,
        length: u32,
        kind: PduKind,
        comment: String,
        // (signal_ref_id, start_bit, is_high_low)
        instances: Vec<(String, u32, bool)>,
    }

    let mut pdu_raws: Vec<(String, PduRaw)> = Vec::new(); // (PDU ID, raw)
    for pdu in &pdu_nodes {
        let Some(id) = pdu.attribute("ID") else {
            continue;
        };
        let name = get_short_name(pdu);
        let length = find_text_candidates(pdu, &["PDU-LENGTH", "BYTE-LENGTH"])
            .and_then(|t| parse_u32(&t))
            .unwrap_or(4);
        let kind = match find_text_candidates(pdu, &["PDU-TYPE"])
            .unwrap_or_default()
            .to_uppercase()
            .as_str()
        {
            "DYNAMIC-PDU" | "EVENT-PDU" => PduKind::Dynamic,
            _ => PduKind::Static,
        };
        let comment = find_text_candidates(pdu, &["DESC", "DESCRIPTION"]).unwrap_or_default();

        pdu_raws.push((
            id.to_string(),
            PduRaw {
                name,
                length,
                kind,
                comment,
                instances: parse_signal_instances(pdu),
            },
        ));
    }

    // ------------------------------------------------------------------
    // 3. 收集 Frame（含 PDU 挂载与"信号直挂帧"的实例）
    // ------------------------------------------------------------------
    let mut frame_nodes = Vec::new();
    collect_elements_by_tag(&root, "FRAME", &mut frame_nodes);

    struct FrameRaw {
        name: String,
        length: u32,
        payload_preamble: bool,
        comment: String,
        // (pdu_ref_id, start_byte)
        pdu_mappings: Vec<(String, u32)>,
        // 信号直挂帧方言（无 PDU 层）的 SIGNAL-INSTANCE
        instances: Vec<(String, u32, bool)>,
    }

    let mut frame_raws: Vec<(String, FrameRaw)> = Vec::new(); // (FRAME ID, raw)
    for frame in &frame_nodes {
        let Some(id) = frame.attribute("ID") else {
            continue;
        };
        let name = get_short_name(frame);
        let length = find_text_candidates(frame, &["FRAME-LENGTH", "BYTE-LENGTH"])
            .and_then(|t| parse_u32(&t))
            .unwrap_or(8);
        let payload_preamble = find_text_candidates(frame, &["PAYLOAD-PREAMBLE"])
            .map(|t| t.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let comment = find_text_candidates(frame, &["DESC", "DESCRIPTION"]).unwrap_or_default();
        let pdu_mappings = parse_pdu_placements(frame);
        let instances = parse_signal_instances(frame);

        frame_raws.push((
            id.to_string(),
            FrameRaw {
                name,
                length,
                payload_preamble,
                comment,
                pdu_mappings,
                instances,
            },
        ));
    }

    // ------------------------------------------------------------------
    // 4. 收集 Frame-Triggering，按通道归属
    //    兼容两种层级：
    //    - CHANNEL/SLOT/FRAME-TRIGGERINGS/FRAME-TRIGGERING（SLOT-ID 在 SLOT 上）
    //    - CHANNEL/FRAME-TRIGGERINGS/FRAME-TRIGGERING（Vector，SLOT-ID 在触发内部的
    //      TIMINGS/ABSOLUTELY-SCHEDULED-TIMING 里）
    //    同一帧在两个通道都出现 -> FrChannel::Both
    // ------------------------------------------------------------------
    let mut triggerings: std::collections::HashMap<String, FrameTriggering> =
        std::collections::HashMap::new(); // FRAME-REF ID -> triggering

    let mut channel_nodes = Vec::new();
    collect_elements_by_tag(&root, "CHANNEL", &mut channel_nodes);

    for (ch_idx, channel) in channel_nodes.iter().enumerate() {
        let ch = classify_channel(channel, ch_idx, channel_nodes.len());

        let mut ft_nodes = Vec::new();
        collect_elements_by_tag(channel, "FRAME-TRIGGERING", &mut ft_nodes);
        for ft in &ft_nodes {
            let frame_ref = get_ref(ft, "FRAME-REF").unwrap_or_default();
            if frame_ref.is_empty() {
                continue;
            }
            // SLOT-ID：优先触发子树内部（Vector 方言），回退到外层 SLOT 包装元素
            let slot_id = find_text_candidates(ft, &["SLOT-ID"])
                .and_then(|t| parse_u32(&t))
                .or_else(|| {
                    ft.ancestors()
                        .find(|n| n.is_element() && n.tag_name().name() == "SLOT")
                        .and_then(|slot| {
                            find_text_candidates(&slot, &["SLOT-ID"]).and_then(|t| parse_u32(&t))
                        })
                })
                .unwrap_or(0);
            let rep = find_text_candidates(ft, &["CYCLE-REPETITION"])
                .and_then(|t| parse_cycle_repetition(&t))
                .unwrap_or(1);
            let base = find_text_candidates(ft, &["BASE-CYCLE"])
                .and_then(|t| parse_u32(&t))
                .unwrap_or(0);
            let startup = find_text_candidates(ft, &["STARTUP-FRAME", "IS-STARTUP-FRAME"])
                .map(|t| t.eq_ignore_ascii_case("true"))
                .unwrap_or(false);

            let trig = triggerings.entry(frame_ref).or_insert(FrameTriggering {
                channel: ch,
                slot_id,
                base_cycle: base,
                cycle_repetition: rep.max(1),
                startup,
            });
            // 同一帧在另一个通道也出现 -> Both
            if trig.channel != ch {
                trig.channel = FrChannel::Both;
            }
            trig.slot_id = slot_id;
            trig.base_cycle = base;
            trig.cycle_repetition = rep.max(1);
            trig.startup = startup;
        }
    }

    // ------------------------------------------------------------------
    // 5. 收集 ECU：优先 fx:ECU（Vector，ECU/CONTROLLERS/CONTROLLER 层级），
    //    回退顶层 CONTROLLER（其 SHORT-NAME 即 ECU 名）
    // ------------------------------------------------------------------
    let mut ecu_nodes = Vec::new();
    collect_elements_by_tag(&root, "ECU", &mut ecu_nodes);
    if ecu_nodes.is_empty() {
        collect_elements_by_tag(&root, "CONTROLLER", &mut ecu_nodes);
    }
    let mut ecus: Vec<String> = ecu_nodes.iter().map(|n| get_short_name(n)).collect();
    ecus.retain(|n| !n.is_empty() && n != "Unnamed");

    // ------------------------------------------------------------------
    // 6. 收集 Cluster 参数
    // ------------------------------------------------------------------
    let mut cluster_nodes = Vec::new();
    collect_elements_by_tag(&root, "CLUSTER", &mut cluster_nodes);

    let mut cluster = EditableCluster {
        name: "Cluster".to_string(),
        params: ClusterParams::default(),
    };
    if let Some(cluster_node) = cluster_nodes.first() {
        cluster.name = get_short_name(cluster_node);
        let p = &mut cluster.params;
        if let Some(v) = find_text_candidates(cluster_node, &["SPEED"]).and_then(|t| parse_u32(&t)) {
            // 单位不统一：Vector 按 bit/s 写 10000000，也有工具按 kbit/s 写 10000
            p.speed_kbps = if v >= 100_000 { v / 1000 } else { v };
        }
        if let Some(v) = find_text_candidates(
            cluster_node,
            &["CYCLE-TIME-ms", "CYCLE-TIME-MS", "CYCLE-TIME"],
        )
        .and_then(|t| parse_f64(&t))
        {
            p.cycle_time_ms = v;
        } else if let Some(v) = find_text_candidates(cluster_node, &["CYCLE"])
            .and_then(|t| parse_f64(&t))
        {
            // flexray:CYCLE 以微秒计（如 5000 = 5ms）
            p.cycle_time_ms = v / 1000.0;
        }
        if let Some(v) = find_text_candidates(
            cluster_node,
            &["MACROTICK", "MACROTICK-DURATION", "GD-MACROTICK-DURATION"],
        )
        .and_then(|t| parse_f64(&t))
        {
            // MACROTICK-DURATION 通常为毫秒（如 0.005）；flexray:MACROTICK 本身为微秒（如 1.375）
            p.macrotick_duration_us = if v < 0.1 { v * 1000.0 } else { v };
        }
        if let Some(v) = find_text_candidates(
            cluster_node,
            &[
                "COLD-START-ATTEMPTS",
                "COLDSTART-ATTEMPTS",
                "GD-COLDSTART-ATTEMPTS",
                "G-COLDSTART-ATTEMPTS",
            ],
        )
        .and_then(|t| parse_u32(&t))
        {
            p.coldstart_attempts = v;
        }
        if let Some(v) = find_text_candidates(
            cluster_node,
            &["ACTION-POINT-OFFSET", "GD-ACTION-POINT-OFFSET"],
        )
        .and_then(|t| parse_u32(&t))
        {
            p.action_point_offset = v;
        }
        if let Some(v) = find_text_candidates(
            cluster_node,
            &[
                "MINISLOT-ACTION-POINT-OFFSET",
                "GD-MINISLOT-ACTION-POINT-OFFSET",
            ],
        )
        .and_then(|t| parse_u32(&t))
        {
            p.minislot_action_point_offset = v;
        }
        if let Some(v) = find_text_candidates(
            cluster_node,
            &["DYNAMIC-SLOT-IDLE-PHASE", "GD-DYNAMIC-SLOT-IDLE-PHASE"],
        )
        .and_then(|t| parse_u32(&t))
        {
            p.dynamic_slot_idle_phase = v;
        }
        if let Some(v) = find_text_candidates(
            cluster_node,
            &["MINOR-VERSION", "GD-MINOR-VERSION"],
        )
        .and_then(|t| parse_u32(&t))
        {
            p.minor_version = v;
        }
        if let Some(v) = find_text_candidates(
            cluster_node,
            &["NETWORK-IDLE-TIME", "N-I-T", "NIT", "GD-NIT"],
        )
        .and_then(|t| parse_u32(&t))
        {
            p.network_idle_time = v;
        }
        if let Some(v) = find_text_candidates(
            cluster_node,
            &["NUMBER-OF-MINISLOTS", "G-NUMBER-OF-MINISLOTS"],
        )
        .and_then(|t| parse_u32(&t))
        {
            p.number_of_minislots = v;
        }
        if let Some(v) = find_text_candidates(
            cluster_node,
            &["NUMBER-OF-STATIC-SLOTS", "G-NUMBER-OF-STATIC-SLOTS"],
        )
        .and_then(|t| parse_u32(&t))
        {
            p.number_of_static_slots = v;
        }
        if let Some(v) = find_text_candidates(
            cluster_node,
            &["MINISLOT-DURATION", "GD-MINISLOT", "MINISLOT"],
        )
        .and_then(|t| parse_u32(&t))
        {
            p.minislot_duration = v;
        }
        if let Some(v) = find_text_candidates(
            cluster_node,
            &["STATIC-SLOT-DURATION", "GD-STATIC-SLOT", "STATIC-SLOT"],
        )
        .and_then(|t| parse_u32(&t))
        {
            p.static_slot_duration = v;
        }
        if let Some(v) = find_text_candidates(
            cluster_node,
            &["SYMBOL-WINDOW", "GD-SYMBOL-WINDOW"],
        )
        .and_then(|t| parse_u32(&t))
        {
            p.symbol_window = v;
        }
        if let Some(v) = find_text_candidates(
            cluster_node,
            &["SYMBOL-WINDOW-IDLE-PHASE", "GD-SYMBOL-WINDOW-IDLE-PHASE"],
        )
        .and_then(|t| parse_u32(&t))
        {
            p.symbol_window_idle_phase = v;
        }
        if let Some(v) = find_text_candidates(
            cluster_node,
            &["OFFSET-CORRECTION-START", "G-OFFSET-CORRECTION-START"],
        )
        .and_then(|t| parse_u32(&t))
        {
            p.offset_correction_start = v;
        }
    }

    // ------------------------------------------------------------------
    // 7. 组装 PDU / Frame 对象（解析引用）
    // ------------------------------------------------------------------
    // 信号直挂帧的方言（无 PDU 层）：为每个含 SIGNAL-INSTANCE 的帧合成同名 PDU
    for (id, raw) in frame_raws.iter_mut() {
        if !raw.pdu_mappings.is_empty() || raw.instances.is_empty() {
            continue;
        }
        let synth_id = format!("__synth_{}", id);
        pdu_raws.push((
            synth_id.clone(),
            PduRaw {
                name: raw.name.clone(),
                length: raw.length,
                kind: PduKind::Static,
                comment: raw.comment.clone(),
                instances: raw.instances.clone(),
            },
        ));
        raw.pdu_mappings.push((synth_id, 0));
    }

    fn build_signals(
        instances: &[(String, u32, bool)],
        signal_defs: &std::collections::HashMap<String, (String, String, String)>,
        codings: &std::collections::HashMap<String, CodingInfo>,
    ) -> Vec<EditableSignal> {
        let mut signals = Vec::new();
        for (sig_ref, start_bit, is_high_low) in instances {
            let Some((sig_name, coding_ref, sig_comment)) = signal_defs.get(sig_ref) else {
                continue;
            };
            let coding = codings.get(coding_ref);
            let bit_length = coding.map(|c| c.bit_length).unwrap_or(1);
            let signed = coding.map(|c| c.signed).unwrap_or(false);
            let byte_order = if *is_high_low {
                ByteOrder::BigEndian
            } else {
                ByteOrder::LittleEndian
            };
            signals.push(EditableSignal::build(
                sig_name.clone(),
                *start_bit,
                bit_length,
                byte_order,
                if signed {
                    ValueType::Signed
                } else {
                    ValueType::Unsigned
                },
                coding.map(|c| c.factor).unwrap_or(1.0),
                coding.map(|c| c.offset).unwrap_or(0.0),
                coding.map(|c| c.min).unwrap_or(0.0),
                coding.map(|c| c.max).unwrap_or(0.0),
                coding.map(|c| c.unit.clone()).unwrap_or_default(),
                Vec::new(),
                coding.map(|c| c.value_descriptions.clone()).unwrap_or_default(),
                sig_comment.clone(),
            ));
        }
        signals
    }

    let mut pdus: Vec<EditablePdu> = Vec::new();
    for (_, raw) in &pdu_raws {
        let signals = build_signals(&raw.instances, &signal_defs, &codings);
        pdus.push(EditablePdu::build(
            raw.name.clone(),
            raw.length,
            raw.kind,
            signals,
            raw.comment.clone(),
        ));
    }

    let mut frames: Vec<EditableFrame> = Vec::new();
    for (frame_id, raw) in &frame_raws {
        let mut pdus_in_frame = Vec::new();
        for (pdu_ref, start) in &raw.pdu_mappings {
            let Some((_, pdu_raw)) = pdu_raws.iter().find(|(id, _)| id == pdu_ref) else {
                continue;
            };
            pdus_in_frame.push(FramePduMapping::new(&pdu_raw.name, *start));
        }
        let triggering = triggerings.get(frame_id).copied().unwrap_or_default();
        frames.push(EditableFrame::build(
            raw.name.clone(),
            raw.length,
            raw.payload_preamble,
            triggering,
            pdus_in_frame,
            raw.comment.clone(),
        ));
    }

    // 名字去重（同名的保留第一个，避免编辑器中的键冲突）
    dedup_names(&mut pdus, |p| &p.name);
    dedup_names(&mut frames, |f| &f.name);

    Ok(EditableFibex::from_imported(cluster, ecus, pdus, frames))
}

fn dedup_names<T, F: Fn(&T) -> &str>(items: &mut Vec<T>, name_of: F) {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    items.retain(|item| {
        let name = name_of(item).to_string();
        if seen.contains(&name) {
            eprintln!("Warning: dropping duplicate name '{}'", name);
            false
        } else {
            seen.insert(name);
            true
        }
    });
}

#[cfg(test)]
mod tests {
    use super::super::parse_content;
    use crate::fibex::editable_fibex::{ByteOrder, FrChannel, ValueType};

    /// Vector DaVinci/CANoe 风格的 FIBEX（2.0.0d/3.0 方言）最小样例：
    /// - FRAME-TRIGGERING 直挂 CHANNEL，SLOT-ID 在 TIMINGS 内
    /// - FLEXRAY-CHANNEL-NAME 标识通道 A/B
    /// - PDU-INSTANCE/BIT-POSITION 挂 PDU 到帧（位计，字节对齐）
    /// - SIGNAL-INSTANCE/BIT-POSITION 信号位置
    /// - BYTE-LENGTH 长度、ECU/CONTROLLERS 层级、CODING 的 SCALE-CONSTR/COMPU-SCALE
    /// - 另含一个"信号直挂帧"（frame2，无 PDU 层）
    const VECTOR_STYLE_FIBEX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<fx:FIBEX VERSION="2.0.0d"
          xmlns:fx="http://www.asam.net/xml/fbx"
          xmlns:ho="http://www.asam.net/xml"
          xmlns:flexray="http://www.asam.net/xml/fbx/flexray"
          xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <fx:PROJECT ID="Project_1">
    <ho:SHORT-NAME>VectorStyleDemo</ho:SHORT-NAME>
  </fx:PROJECT>
  <fx:ELEMENTS>
    <fx:CLUSTERS>
      <fx:CLUSTER ID="Cluster_1" xsi:type="flexray:CLUSTER-TYPE">
        <ho:SHORT-NAME>FrCluster</ho:SHORT-NAME>
        <fx:SPEED>10000000</fx:SPEED>
        <flexray:MACROTICK>1.250000</flexray:MACROTICK>
        <flexray:CYCLE>5000</flexray:CYCLE>
        <flexray:COLD-START-ATTEMPTS>8</flexray:COLD-START-ATTEMPTS>
        <flexray:NUMBER-OF-STATIC-SLOTS>91</flexray:NUMBER-OF-STATIC-SLOTS>
        <flexray:NUMBER-OF-MINISLOTS>210</flexray:NUMBER-OF-MINISLOTS>
        <flexray:STATIC-SLOT>24</flexray:STATIC-SLOT>
        <flexray:MINISLOT>5</flexray:MINISLOT>
        <flexray:N-I-T>75</flexray:N-I-T>
        <flexray:OFFSET-CORRECTION-START>3632</flexray:OFFSET-CORRECTION-START>
      </fx:CLUSTER>
    </fx:CLUSTERS>
    <fx:CHANNELS>
      <fx:CHANNEL ID="Channel_A" xsi:type="flexray:CHANNEL-TYPE">
        <ho:SHORT-NAME>FrCh1</ho:SHORT-NAME>
        <flexray:FLEXRAY-CHANNEL-NAME>A</flexray:FLEXRAY-CHANNEL-NAME>
        <fx:FRAME-TRIGGERINGS>
          <fx:FRAME-TRIGGERING ID="ft1">
            <fx:TIMINGS>
              <fx:ABSOLUTELY-SCHEDULED-TIMING>
                <fx:SLOT-ID xsi:type="flexray:SLOT-ID-TYPE">13</fx:SLOT-ID>
                <fx:BASE-CYCLE xsi:type="flexray:CYCLE-COUNTER-TYPE">0</fx:BASE-CYCLE>
                <fx:CYCLE-REPETITION xsi:type="flexray:CYCLE-REPETITION-TYPE">2</fx:CYCLE-REPETITION>
              </fx:ABSOLUTELY-SCHEDULED-TIMING>
            </fx:TIMINGS>
            <fx:FRAME-REF ID-REF="frame1"/>
          </fx:FRAME-TRIGGERING>
          <fx:FRAME-TRIGGERING ID="ft2">
            <fx:TIMINGS>
              <fx:ABSOLUTELY-SCHEDULED-TIMING>
                <fx:SLOT-ID xsi:type="flexray:SLOT-ID-TYPE">14</fx:SLOT-ID>
                <fx:BASE-CYCLE xsi:type="flexray:CYCLE-COUNTER-TYPE">0</fx:BASE-CYCLE>
                <fx:CYCLE-REPETITION xsi:type="flexray:CYCLE-REPETITION-TYPE">1</fx:CYCLE-REPETITION>
              </fx:ABSOLUTELY-SCHEDULED-TIMING>
            </fx:TIMINGS>
            <fx:FRAME-REF ID-REF="frame2"/>
          </fx:FRAME-TRIGGERING>
        </fx:FRAME-TRIGGERINGS>
      </fx:CHANNEL>
      <fx:CHANNEL ID="Channel_B" xsi:type="flexray:CHANNEL-TYPE">
        <ho:SHORT-NAME>FrCh2</ho:SHORT-NAME>
        <flexray:FLEXRAY-CHANNEL-NAME>B</flexray:FLEXRAY-CHANNEL-NAME>
        <fx:FRAME-TRIGGERINGS>
          <fx:FRAME-TRIGGERING ID="ft3">
            <fx:TIMINGS>
              <fx:ABSOLUTELY-SCHEDULED-TIMING>
                <fx:SLOT-ID xsi:type="flexray:SLOT-ID-TYPE">13</fx:SLOT-ID>
                <fx:BASE-CYCLE xsi:type="flexray:CYCLE-COUNTER-TYPE">0</fx:BASE-CYCLE>
                <fx:CYCLE-REPETITION xsi:type="flexray:CYCLE-REPETITION-TYPE">2</fx:CYCLE-REPETITION>
              </fx:ABSOLUTELY-SCHEDULED-TIMING>
            </fx:TIMINGS>
            <fx:FRAME-REF ID-REF="frame1"/>
          </fx:FRAME-TRIGGERING>
        </fx:FRAME-TRIGGERINGS>
      </fx:CHANNEL>
    </fx:CHANNELS>
    <fx:ECUS>
      <fx:ECU ID="ECU_1">
        <ho:SHORT-NAME>ESP</ho:SHORT-NAME>
        <fx:CONTROLLERS>
          <fx:CONTROLLER ID="Ctrl_1">
            <ho:SHORT-NAME>ESP_Controller</ho:SHORT-NAME>
          </fx:CONTROLLER>
        </fx:CONTROLLERS>
      </fx:ECU>
      <fx:ECU ID="ECU_2">
        <ho:SHORT-NAME>Engine</ho:SHORT-NAME>
        <fx:CONTROLLERS>
          <fx:CONTROLLER ID="Ctrl_2">
            <ho:SHORT-NAME>Engine_Controller</ho:SHORT-NAME>
          </fx:CONTROLLER>
        </fx:CONTROLLERS>
      </fx:ECU>
    </fx:ECUS>
    <fx:PDUS>
      <fx:PDU ID="pdu1">
        <ho:SHORT-NAME>PDU_CrankTorque</ho:SHORT-NAME>
        <fx:BYTE-LENGTH>4</fx:BYTE-LENGTH>
        <fx:PDU-TYPE>APPLICATION</fx:PDU-TYPE>
        <fx:SIGNAL-INSTANCES>
          <fx:SIGNAL-INSTANCE ID="si1">
            <fx:BIT-POSITION>0</fx:BIT-POSITION>
            <fx:IS-HIGH-LOW-BYTE-ORDER>false</fx:IS-HIGH-LOW-BYTE-ORDER>
            <fx:SIGNAL-REF ID-REF="signal1"/>
          </fx:SIGNAL-INSTANCE>
          <fx:SIGNAL-INSTANCE ID="si2">
            <fx:BIT-POSITION>16</fx:BIT-POSITION>
            <fx:IS-HIGH-LOW-BYTE-ORDER>false</fx:IS-HIGH-LOW-BYTE-ORDER>
            <fx:SIGNAL-REF ID-REF="signal2"/>
          </fx:SIGNAL-INSTANCE>
        </fx:SIGNAL-INSTANCES>
      </fx:PDU>
      <fx:PDU ID="pdu2">
        <ho:SHORT-NAME>PDU_Second</ho:SHORT-NAME>
        <fx:BYTE-LENGTH>2</fx:BYTE-LENGTH>
        <fx:PDU-TYPE>APPLICATION</fx:PDU-TYPE>
        <fx:SIGNAL-INSTANCES>
          <fx:SIGNAL-INSTANCE ID="si3">
            <fx:BIT-POSITION>0</fx:BIT-POSITION>
            <fx:IS-HIGH-LOW-BYTE-ORDER>false</fx:IS-HIGH-LOW-BYTE-ORDER>
            <fx:SIGNAL-REF ID-REF="signal3"/>
          </fx:SIGNAL-INSTANCE>
        </fx:SIGNAL-INSTANCES>
      </fx:PDU>
    </fx:PDUS>
    <fx:FRAMES>
      <fx:FRAME ID="frame1">
        <ho:SHORT-NAME>CrankTorqueInfo</ho:SHORT-NAME>
        <fx:BYTE-LENGTH>6</fx:BYTE-LENGTH>
        <fx:PDU-INSTANCES>
          <fx:PDU-INSTANCE ID="pinst1">
            <fx:PDU-REF ID-REF="pdu1"/>
            <fx:BIT-POSITION>0</fx:BIT-POSITION>
            <fx:IS-HIGH-LOW-BYTE-ORDER>false</fx:IS-HIGH-LOW-BYTE-ORDER>
          </fx:PDU-INSTANCE>
          <fx:PDU-INSTANCE ID="pinst2">
            <fx:PDU-REF ID-REF="pdu2"/>
            <fx:BIT-POSITION>32</fx:BIT-POSITION>
            <fx:IS-HIGH-LOW-BYTE-ORDER>false</fx:IS-HIGH-LOW-BYTE-ORDER>
          </fx:PDU-INSTANCE>
        </fx:PDU-INSTANCES>
      </fx:FRAME>
      <fx:FRAME ID="frame2">
        <ho:SHORT-NAME>BrakeInfo</ho:SHORT-NAME>
        <fx:BYTE-LENGTH>4</fx:BYTE-LENGTH>
        <fx:SIGNAL-INSTANCES>
          <fx:SIGNAL-INSTANCE ID="si4">
            <fx:BIT-POSITION>8</fx:BIT-POSITION>
            <fx:IS-HIGH-LOW-BYTE-ORDER>true</fx:IS-HIGH-LOW-BYTE-ORDER>
            <fx:SIGNAL-REF ID-REF="signal4"/>
          </fx:SIGNAL-INSTANCE>
        </fx:SIGNAL-INSTANCES>
      </fx:FRAME>
    </fx:FRAMES>
    <fx:SIGNALS>
      <fx:SIGNAL ID="signal1">
        <ho:SHORT-NAME>TorqueReq</ho:SHORT-NAME>
        <fx:CODING-REF ID-REF="coding1"/>
      </fx:SIGNAL>
      <fx:SIGNAL ID="signal2">
        <ho:SHORT-NAME>CrankAngle</ho:SHORT-NAME>
        <fx:CODING-REF ID-REF="coding2"/>
      </fx:SIGNAL>
      <fx:SIGNAL ID="signal3">
        <ho:SHORT-NAME>SecondSig</ho:SHORT-NAME>
      </fx:SIGNAL>
      <fx:SIGNAL ID="signal4">
        <ho:SHORT-NAME>BrakePressure</ho:SHORT-NAME>
        <fx:CODING-REF ID-REF="coding3"/>
      </fx:SIGNAL>
    </fx:SIGNALS>
    <fx:CODINGS>
      <fx:CODING ID="coding1">
        <ho:SHORT-NAME>Coding_TorqueReq</ho:SHORT-NAME>
        <ho:CODED-TYPE CATEGORY="STANDARD-LENGTH-TYPE" ho:BASE-DATA-TYPE="A_UINT16">
          <ho:BIT-LENGTH>16</ho:BIT-LENGTH>
        </ho:CODED-TYPE>
        <ho:COMPU-METHODS>
          <ho:COMPU-METHOD>
            <ho:SHORT-NAME>TorqueReq</ho:SHORT-NAME>
            <ho:CATEGORY>LINEAR</ho:CATEGORY>
            <ho:UNIT-REF ID-REF="Unit_1"/>
            <ho:PHYS-CONSTRS>
              <ho:SCALE-CONSTR VALIDITY="VALID">
                <ho:LOWER-LIMIT INTERVAL-TYPE="CLOSED">0.000000</ho:LOWER-LIMIT>
                <ho:UPPER-LIMIT INTERVAL-TYPE="CLOSED">6553.500000</ho:UPPER-LIMIT>
              </ho:SCALE-CONSTR>
            </ho:PHYS-CONSTRS>
            <ho:COMPU-INTERNAL-TO-PHYS>
              <ho:COMPU-SCALES>
                <ho:COMPU-SCALE>
                  <ho:COMPU-RATIONAL-COEFFS>
                    <ho:COMPU-NUMERATOR>
                      <ho:V>-1.000000</ho:V>
                      <ho:V>0.100000</ho:V>
                    </ho:COMPU-NUMERATOR>
                    <ho:COMPU-DENOMINATOR>
                      <ho:V>1</ho:V>
                    </ho:COMPU-DENOMINATOR>
                  </ho:COMPU-RATIONAL-COEFFS>
                </ho:COMPU-SCALE>
              </ho:COMPU-SCALES>
            </ho:COMPU-INTERNAL-TO-PHYS>
          </ho:COMPU-METHOD>
        </ho:COMPU-METHODS>
      </fx:CODING>
      <fx:CODING ID="coding2">
        <ho:SHORT-NAME>Coding_CrankAngle</ho:SHORT-NAME>
        <ho:CODED-TYPE CATEGORY="STANDARD-LENGTH-TYPE" ho:BASE-DATA-TYPE="A_INT16">
          <ho:BIT-LENGTH>16</ho:BIT-LENGTH>
        </ho:CODED-TYPE>
        <ho:COMPU-METHODS>
          <ho:COMPU-METHOD>
            <ho:CATEGORY>IDENTICAL</ho:CATEGORY>
          </ho:COMPU-METHOD>
        </ho:COMPU-METHODS>
      </fx:CODING>
      <fx:CODING ID="coding3">
        <ho:SHORT-NAME>Coding_BrakePressure</ho:SHORT-NAME>
        <ho:CODED-TYPE CATEGORY="STANDARD-LENGTH-TYPE" ho:BASE-DATA-TYPE="A_UINT32">
          <ho:BIT-LENGTH>32</ho:BIT-LENGTH>
        </ho:CODED-TYPE>
        <ho:COMPU-METHODS>
          <ho:COMPU-METHOD>
            <ho:CATEGORY>TEXTTABLE</ho:CATEGORY>
            <ho:COMPU-INTERNAL-TO-PHYS>
              <ho:COMPU-SCALES>
                <ho:COMPU-SCALE>
                  <ho:LOWER-LIMIT INTERVAL-TYPE="CLOSED">0</ho:LOWER-LIMIT>
                  <ho:UPPER-LIMIT INTERVAL-TYPE="CLOSED">0</ho:UPPER-LIMIT>
                  <ho:COMPU-CONST>
                    <ho:VT>OK</ho:VT>
                  </ho:COMPU-CONST>
                </ho:COMPU-SCALE>
                <ho:COMPU-SCALE>
                  <ho:LOWER-LIMIT INTERVAL-TYPE="CLOSED">1</ho:LOWER-LIMIT>
                  <ho:UPPER-LIMIT INTERVAL-TYPE="CLOSED">1</ho:UPPER-LIMIT>
                  <ho:COMPU-CONST>
                    <ho:VT>Warning</ho:VT>
                  </ho:COMPU-CONST>
                </ho:COMPU-SCALE>
                <ho:COMPU-SCALE>
                  <ho:LOWER-LIMIT INTERVAL-TYPE="CLOSED">2</ho:LOWER-LIMIT>
                  <ho:UPPER-LIMIT INTERVAL-TYPE="CLOSED">2</ho:UPPER-LIMIT>
                  <ho:COMPU-CONST>
                    <ho:VT>Error</ho:VT>
                  </ho:COMPU-CONST>
                </ho:COMPU-SCALE>
              </ho:COMPU-SCALES>
            </ho:COMPU-INTERNAL-TO-PHYS>
          </ho:COMPU-METHOD>
        </ho:COMPU-METHODS>
      </fx:CODING>
    </fx:CODINGS>
    <ho:UNITS>
      <ho:UNIT ID="Unit_1">
        <ho:SHORT-NAME>Nm</ho:SHORT-NAME>
        <ho:DISPLAY-NAME>Nm</ho:DISPLAY-NAME>
      </ho:UNIT>
    </ho:UNITS>
  </fx:ELEMENTS>
</fx:FIBEX>
"#;

    #[test]
    fn parse_vector_style_fibex() {
        let fibex = parse_content(VECTOR_STYLE_FIBEX).expect("Vector-style FIBEX should parse");

        // 集群参数：SPEED bit/s 归一化、CYCLE 微秒、MACROTICK 微秒、flexray 元素名
        let p = &fibex.cluster().params;
        assert_eq!(fibex.cluster().name, "FrCluster");
        assert_eq!(p.speed_kbps, 10000);
        assert_eq!(p.cycle_time_ms, 5.0);
        assert_eq!(p.macrotick_duration_us, 1.25);
        assert_eq!(p.coldstart_attempts, 8);
        assert_eq!(p.number_of_static_slots, 91);
        assert_eq!(p.number_of_minislots, 210);
        assert_eq!(p.static_slot_duration, 24);
        assert_eq!(p.minislot_duration, 5);
        assert_eq!(p.network_idle_time, 75);
        assert_eq!(p.offset_correction_start, 3632);

        // ECU：取 fx:ECU 的 SHORT-NAME（而非内层 CONTROLLER 名）
        assert_eq!(fibex.ecus(), &vec!["ESP".to_string(), "Engine".to_string()]);

        // 触发：无 SLOT 包装层，SLOT-ID 在 TIMINGS 内；双通道 -> Both
        let frame1 = fibex.get_frame("CrankTorqueInfo").unwrap();
        assert_eq!(frame1.triggering().slot_id, 13);
        assert_eq!(frame1.triggering().channel, FrChannel::Both);
        assert_eq!(frame1.triggering().cycle_repetition, 2);
        assert_eq!(frame1.length(), 6);

        // PDU-INSTANCE/BIT-POSITION（位计）挂帧：32 bit -> 字节 4
        assert_eq!(frame1.pdus().len(), 2);
        assert_eq!(frame1.pdus()[0].pdu_name, "PDU_CrankTorque");
        assert_eq!(frame1.pdus()[0].start_position, 0);
        assert_eq!(frame1.pdus()[1].pdu_name, "PDU_Second");
        assert_eq!(frame1.pdus()[1].start_position, 4);

        // 信号：BIT-POSITION 起始位；CODING 属性解析
        let pdu1 = fibex.get_pdu("PDU_CrankTorque").unwrap();
        assert_eq!(pdu1.length(), 4);
        let torque = pdu1
            .signals()
            .iter()
            .find(|s| s.name() == "TorqueReq")
            .unwrap();
        assert_eq!(torque.start_bit(), 0);
        assert_eq!(torque.length_bits(), 16);
        assert_eq!(torque.byte_order(), ByteOrder::LittleEndian);
        assert_eq!(torque.factor(), 0.1);
        assert_eq!(torque.offset(), -1.0);
        assert_eq!(torque.min(), 0.0);
        assert_eq!(torque.max(), 6553.5);
        assert_eq!(torque.unit(), "Nm");
        let angle = pdu1
            .signals()
            .iter()
            .find(|s| s.name() == "CrankAngle")
            .unwrap();
        assert_eq!(angle.start_bit(), 16);
        assert_eq!(angle.value_type(), ValueType::Signed);

        // 信号直挂帧（无 PDU 层）：合成同名 PDU，起始位来自 BIT-POSITION
        let frame2 = fibex.get_frame("BrakeInfo").unwrap();
        assert_eq!(frame2.triggering().channel, FrChannel::A);
        assert_eq!(frame2.triggering().slot_id, 14);
        assert_eq!(frame2.pdus().len(), 1);
        assert_eq!(frame2.pdus()[0].pdu_name, "BrakeInfo");
        assert_eq!(frame2.pdus()[0].start_position, 0);
        let synth = fibex.get_pdu("BrakeInfo").unwrap();
        assert_eq!(synth.length(), 4);
        let pressure = synth
            .signals()
            .iter()
            .find(|s| s.name() == "BrakePressure")
            .unwrap();
        assert_eq!(pressure.start_bit(), 8);
        assert_eq!(pressure.length_bits(), 32);
        assert_eq!(pressure.byte_order(), ByteOrder::BigEndian);
        assert_eq!(
            pressure.value_descriptions(),
            &[
                (0, "OK".to_string()),
                (1, "Warning".to_string()),
                (2, "Error".to_string())
            ]
        );
    }
}
