//! AUTOSAR ARXML (R4.x 风格) FlexRay 数据库解析
//!
//! 宽容解析：支持 FLEXRAY-CLUSTER / FLEXRAY-PHYSICAL-CHANNEL / FLEXRAY-FRAME-TRIGGERING /
//! FLEXRAY-FRAME / I-SIGNAL-I-PDU(或 FLEXRAY-I-PDU) / I-SIGNAL / ECU-INSTANCE。

use super::{
    collect_elements_by_tag, find_text_candidates, get_short_name, parse_cycle_repetition,
    parse_f64, parse_u32, ref_short_name,
};
use crate::fibex::editable_fibex::*;

pub fn parse_arxml_doc(doc: &roxmltree::Document) -> Result<EditableFibex, String> {
    let root = doc.root_element();

    // ------------------------------------------------------------------
    // 1. I-SIGNAL 定义（名称 -> 属性）
    // ------------------------------------------------------------------
    let mut signal_nodes = Vec::new();
    for tag in ["I-SIGNAL", "SYSTEM-SIGNAL"] {
        collect_elements_by_tag(&root, tag, &mut signal_nodes);
    }

    struct SignalDef {
        length_bits: u32,
        signed: bool,
        comment: String,
    }

    let mut signal_defs: std::collections::HashMap<String, SignalDef> =
        std::collections::HashMap::new();
    for sig in &signal_nodes {
        let name = get_short_name(sig);
        let length_bits = find_text_candidates(sig, &["LENGTH"])
            .and_then(|t| parse_u32(&t))
            .unwrap_or(1);
        let signed = find_text_candidates(
            sig,
            &["SIGN-CONVENTION", "I-SIGNAL-TYPE", "NETWORK-REPRESENTATION-PROPS"],
        )
        .map(|t| {
            let u = t.to_uppercase();
            // 注意 "UNSIGNED" 也包含 "SIGNED"，需排除
            u.contains("TWOS") || u.contains("SYMMETRIC") || (u.contains("SIGNED") && !u.starts_with("UN"))
        })
        .unwrap_or(false);
        let comment = find_text_candidates(sig, &["L-2", "DESC"]).unwrap_or_default();
        signal_defs.insert(
            name,
            SignalDef {
                length_bits,
                signed,
                comment,
            },
        );
    }

    // ------------------------------------------------------------------
    // 2. PDU 定义（I-SIGNAL-I-PDU / FLEXRAY-I-PDU）
    // ------------------------------------------------------------------
    let mut pdu_nodes = Vec::new();
    for tag in ["I-SIGNAL-I-PDU", "FLEXRAY-I-PDU"] {
        collect_elements_by_tag(&root, tag, &mut pdu_nodes);
    }

    struct PduMappingRaw {
        signal_name: Option<String>,
        start_bit: u32,
        length_bits: u32,
        big_endian: bool,
    }

    struct PduRaw {
        name: String,
        length: u32,
        kind: PduKind,
        comment: String,
        mappings: Vec<PduMappingRaw>,
    }

    let mut pdu_raws: Vec<(String, PduRaw)> = Vec::new(); // (PDU short name, raw)
    for pdu in &pdu_nodes {
        let name = get_short_name(pdu);
        let length = find_text_candidates(pdu, &["LENGTH"])
            .and_then(|t| parse_u32(&t))
            .unwrap_or(4);
        let comment = find_text_candidates(pdu, &["L-2", "DESC"]).unwrap_or_default();

        // PDU 类型：FLEXRAY-I-PDU 里的 PDU-TYPE 或 I-PDU-TYPE
        let kind = match find_text_candidates(pdu, &["PDU-TYPE", "I-PDU-TYPE"])
            .unwrap_or_default()
            .to_uppercase()
            .as_str()
        {
            t if t.contains("DYNAMIC") => PduKind::Dynamic,
            t if t.contains("EVENT") || t.contains("GENERAL-PURPOSE") => PduKind::Event,
            _ => PduKind::Static,
        };

        let mut mappings = Vec::new();
        let mut mapping_nodes = Vec::new();
        // AUTOSAR 中映射元素的标准标签是 I-SIGNAL-TO-I-PDU-MAPPING（容器名才是
        // I-SIGNAL-TO-PDU-MAPPINGS）；部分导出工具直接用 I-SIGNAL-TO-PDU-MAPPING，两者都支持。
        // 此前只匹配容器名导致一个信号都解析不到（PDU 信号数为 0）。
        for tag in ["I-SIGNAL-TO-I-PDU-MAPPING", "I-SIGNAL-TO-PDU-MAPPING"] {
            collect_elements_by_tag(pdu, tag, &mut mapping_nodes);
        }
        for m in &mapping_nodes {
            let signal_name = find_text_candidates(m, &["I-SIGNAL-REF", "SYSTEM-SIGNAL-REF"])
                .map(|r| ref_short_name(&r).to_string());
            let start_bit = find_text_candidates(m, &["START-POSITION", "START-BIT-POSITION"])
                .and_then(|t| parse_u32(&t))
                .unwrap_or(0);
            let length_bits = find_text_candidates(m, &["LENGTH"])
                .and_then(|t| parse_u32(&t))
                .unwrap_or(1);
            let big_endian = matches!(
                find_text_candidates(m, &["PACKING-BYTE-ORDER"])
                    .unwrap_or_default()
                    .to_uppercase(),
                t if t.contains("MOST-SIGNIFICANT-BYTE-FIRST") || t.contains("BIG-ENDIAN")
            );
            mappings.push(PduMappingRaw {
                signal_name,
                start_bit,
                length_bits,
                big_endian,
            });
        }

        pdu_raws.push((
            name.clone(),
            PduRaw {
                name,
                length,
                kind,
                comment,
                mappings,
            },
        ));
    }

    // ------------------------------------------------------------------
    // 2.5 COMPU-METHOD（物理值换算：factor / offset / 值表）
    // ------------------------------------------------------------------
    struct CompuMethod {
        factor: f64,
        offset: f64,
        value_descriptions: Vec<(i64, String)>,
    }

    let mut compu_nodes = Vec::new();
    collect_elements_by_tag(&root, "COMPU-METHOD", &mut compu_nodes);
    let mut compu_methods: std::collections::HashMap<String, CompuMethod> =
        std::collections::HashMap::new();
    for cm in &compu_nodes {
        let name = get_short_name(cm);
        let mut method = CompuMethod {
            factor: 1.0,
            offset: 0.0,
            value_descriptions: Vec::new(),
        };

        let mut scale_nodes = Vec::new();
        collect_elements_by_tag(cm, "COMPU-SCALE", &mut scale_nodes);
        for scale in &scale_nodes {
            // 线性换算：phys = numerator[0] + numerator[1] * raw
            let mut coeff_nodes = Vec::new();
            collect_elements_by_tag(scale, "COMPU-NUMERATOR", &mut coeff_nodes);
            for coeffs in &coeff_nodes {
                let values: Vec<f64> = coeffs
                    .children()
                    .filter(|n| n.has_tag_name("V"))
                    .filter_map(|n| n.text().and_then(parse_f64))
                    .collect();
                if values.len() >= 2 {
                    method.offset = values[0];
                    method.factor = values[1];
                } else if values.len() == 1 {
                    method.factor = values[0];
                }
            }

            // 枚举值表：LOWER/UPPER 限定的 COMPU-CONST 文本
            if let Some(vt) = find_text_candidates(scale, &["COMPU-CONST", "VT"]) {
                let lower = find_text_candidates(scale, &["LOWER-LIMIT"])
                    .and_then(|t| parse_u32(&t));
                if let Some(lower) = lower {
                    method.value_descriptions.push((lower as i64, vt));
                }
            }
        }

        compu_methods.insert(name, method);
    }

    // I-SIGNAL 的 COMPU-METHOD-REF -> 名称映射
    let mut signal_compu_ref: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for sig in &signal_nodes {
        if let Some(cm_ref) = find_text_candidates(sig, &["COMPU-METHOD-REF"]) {
            signal_compu_ref.insert(get_short_name(sig), ref_short_name(&cm_ref).to_string());
        }
    }

    // ------------------------------------------------------------------
    // 2.6 端口收发关系（PDU-TRIGGERING / I-SIGNAL-TRIGGERING 的 PORT-REFS）
    //
    // 端口路径形如 /ECU/Engine/CN_xxx/PP_ABSInfo_Tx：
    //   倒数第 3 段是 ECU 名，末段后缀 _Tx / _Rx 表示方向
    // ------------------------------------------------------------------
    fn port_ecu_and_direction(port_ref: &str) -> Option<(String, bool)> {
        let segments: Vec<&str> = port_ref.split('/').filter(|s| !s.is_empty()).collect();
        let last = segments.last()?.to_uppercase();
        let ecu = segments
            .iter()
            .rev()
            .nth(2)
            .map(|s| s.to_string())
            .unwrap_or_default();
        if last.ends_with("_TX") {
            Some((ecu, true))
        } else if last.ends_with("_RX") {
            Some((ecu, false))
        } else {
            None
        }
    }

    // PDU 级：PDU-TRIGGERING 里的 I-PDU-PORT-REFS + I-PDU-REF
    let mut pdu_tx: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut pdu_rx: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut pdu_trigger_nodes = Vec::new();
    collect_elements_by_tag(&root, "PDU-TRIGGERING", &mut pdu_trigger_nodes);
    for pt in &pdu_trigger_nodes {
        let Some(pdu_ref) = find_text_candidates(pt, &["I-PDU-REF"]) else {
            continue;
        };
        let pdu_name = ref_short_name(&pdu_ref).to_string();
        let mut port_refs = Vec::new();
        collect_elements_by_tag(pt, "I-PDU-PORT-REF", &mut port_refs);
        for port in port_refs {
            if let Some((ecu, is_tx)) = port_ecu_and_direction(port.text().unwrap_or_default()) {
                if ecu.is_empty() {
                    continue;
                }
                let map = if is_tx { &mut pdu_tx } else { &mut pdu_rx };
                let entry = map.entry(pdu_name.clone()).or_default();
                if !entry.contains(&ecu) {
                    entry.push(ecu);
                }
            }
        }
    }

    // 信号级：I-SIGNAL-TRIGGERING 里的 I-SIGNAL-PORT-REFS + I-SIGNAL-REF
    let mut sig_tx: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut sig_rx: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut sig_trigger_nodes = Vec::new();
    collect_elements_by_tag(&root, "I-SIGNAL-TRIGGERING", &mut sig_trigger_nodes);
    for st in &sig_trigger_nodes {
        let Some(signal_ref) = find_text_candidates(st, &["I-SIGNAL-REF"]) else {
            continue;
        };
        let signal_name = ref_short_name(&signal_ref).to_string();
        let mut port_refs = Vec::new();
        collect_elements_by_tag(st, "I-SIGNAL-PORT-REF", &mut port_refs);
        for port in port_refs {
            if let Some((ecu, is_tx)) = port_ecu_and_direction(port.text().unwrap_or_default()) {
                if ecu.is_empty() {
                    continue;
                }
                let map = if is_tx { &mut sig_tx } else { &mut sig_rx };
                let entry = map.entry(signal_name.clone()).or_default();
                if !entry.contains(&ecu) {
                    entry.push(ecu);
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // 3. Frame 定义
    // ------------------------------------------------------------------
    let mut frame_nodes = Vec::new();
    collect_elements_by_tag(&root, "FLEXRAY-FRAME", &mut frame_nodes);

    struct FrameRaw {
        name: String,
        length: u32,
        payload_preamble: bool,
        comment: String,
        // (pdu short name, start byte)
        pdu_mappings: Vec<(String, u32)>,
    }

    let mut frames_by_name: std::collections::HashMap<String, FrameRaw> =
        std::collections::HashMap::new();
    for frame in &frame_nodes {
        let name = get_short_name(frame);
        let length = find_text_candidates(frame, &["FRAME-LENGTH"])
            .and_then(|t| parse_u32(&t))
            .unwrap_or(8);
        let payload_preamble = find_text_candidates(frame, &["PAYLOAD-PREAMBLE", "PADDING-ACTIVATION"])
            .map(|t| t.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let comment = find_text_candidates(frame, &["L-2", "DESC"]).unwrap_or_default();

        let mut pdu_mappings = Vec::new();
        let mut mapping_nodes = Vec::new();
        for tag in ["FRAME-PDU-MAPPING", "PDU-TO-FRAME-MAPPING"] {
            collect_elements_by_tag(frame, tag, &mut mapping_nodes);
        }
        for m in &mapping_nodes {
            let Some(pdu_ref) = find_text_candidates(m, &["PDU-REF"]) else {
                continue;
            };
            let start = find_text_candidates(m, &["START-POSITION", "START-BIT-POSITION"])
                .and_then(|t| parse_u32(&t))
                .unwrap_or(0);
            pdu_mappings.push((ref_short_name(&pdu_ref).to_string(), start));
        }

        frames_by_name.insert(
            name.clone(),
            FrameRaw {
                name,
                length,
                payload_preamble,
                comment,
                pdu_mappings,
            },
        );
    }

    // ------------------------------------------------------------------
    // 4. Frame-Triggering（按通道归属）
    // ------------------------------------------------------------------
    let mut channel_nodes = Vec::new();
    collect_elements_by_tag(&root, "FLEXRAY-PHYSICAL-CHANNEL", &mut channel_nodes);

    // (frame short name -> triggering)
    let mut triggerings: std::collections::HashMap<String, FrameTriggering> =
        std::collections::HashMap::new();
    for (ch_idx, channel) in channel_nodes.iter().enumerate() {
        let ch_name = get_short_name(channel).to_uppercase();
        let ch = if ch_name.contains('B') || (ch_idx == 1 && channel_nodes.len() == 2) {
            FrChannel::B
        } else {
            FrChannel::A
        };

        let mut trigger_nodes = Vec::new();
        collect_elements_by_tag(channel, "FLEXRAY-FRAME-TRIGGERING", &mut trigger_nodes);
        for ft in &trigger_nodes {
            let Some(frame_ref) = find_text_candidates(ft, &["FRAME-REF"]) else {
                continue;
            };
            let frame_name = ref_short_name(&frame_ref).to_string();
            let slot_id = find_text_candidates(ft, &["SLOT-ID"])
                .and_then(|t| parse_u32(&t))
                .unwrap_or(1);
            let base_cycle = find_text_candidates(ft, &["BASE-CYCLE"])
                .and_then(|t| parse_u32(&t))
                .unwrap_or(0);
            let cycle_repetition = find_text_candidates(ft, &["CYCLE-REPETITION"])
                .and_then(|t| parse_cycle_repetition(&t))
                .unwrap_or(1);
            let startup = find_text_candidates(ft, &["STARTUP-FRAME"])
                .map(|t| t.eq_ignore_ascii_case("true"))
                .unwrap_or(false);

            let trig = triggerings.entry(frame_name).or_insert(FrameTriggering {
                channel: ch,
                slot_id,
                base_cycle,
                cycle_repetition: cycle_repetition.max(1),
                startup,
            });
            // 同一帧在两个通道都出现 -> Both
            if trig.channel != ch {
                trig.channel = FrChannel::Both;
            }
            trig.slot_id = slot_id;
            trig.base_cycle = base_cycle;
            trig.cycle_repetition = cycle_repetition.max(1);
            trig.startup = startup;
        }
    }

    // ------------------------------------------------------------------
    // 5. Cluster 参数
    // ------------------------------------------------------------------
    let mut cluster_nodes = Vec::new();
    collect_elements_by_tag(&root, "FLEXRAY-CLUSTER", &mut cluster_nodes);

    let mut cluster = EditableCluster {
        name: "Cluster".to_string(),
        params: ClusterParams::default(),
    };
    if let Some(cluster_node) = cluster_nodes.first() {
        cluster.name = get_short_name(cluster_node);
        let p = &mut cluster.params;
        let get_u32 = |tags: &[&str]| -> Option<u32> {
            find_text_candidates(cluster_node, tags).and_then(|t| parse_u32(&t))
        };
        let get_f64 = |tags: &[&str]| -> Option<f64> {
            find_text_candidates(cluster_node, tags).and_then(|t| parse_f64(&t))
        };

        if let Some(v) = get_u32(&["FLEXRAY-COLDSTART-ATTEMPTS", "GD-COLDSTART-ATTEMPTS"]) {
            p.coldstart_attempts = v;
        }
        let macrotick = get_f64(&["FLEXRAY-GD-MACROTICK-DURATION", "GD-MACROTICK-DURATION"]);
        let cycle_mt = get_u32(&["FLEXRAY-CYCLE", "GD-CYCLE"]);
        if let Some(mt) = macrotick {
            p.macrotick_duration_us = mt;
        }
        if let Some(mt) = cycle_mt {
            p.cycle_time_ms = mt as f64 * p.macrotick_duration_us / 1000.0;
        } else if let Some(ms) = get_f64(&["FLEXRAY-CYCLE-TIME-MS"]) {
            p.cycle_time_ms = ms;
        }
        if let Some(v) = get_u32(&["FLEXRAY-GD-ACTION-POINT-OFFSET"]) {
            p.action_point_offset = v;
        }
        if let Some(v) = get_u32(&["FLEXRAY-GD-MINISLOT-ACTION-POINT-OFFSET"]) {
            p.minislot_action_point_offset = v;
        }
        if let Some(v) = get_u32(&["FLEXRAY-GD-DYNAMIC-SLOT-IDLE-PHASE"]) {
            p.dynamic_slot_idle_phase = v;
        }
        if let Some(v) = get_u32(&["FLEXRAY-GD-MINOR-VERSION"]) {
            p.minor_version = v;
        }
        if let Some(v) = get_u32(&["FLEXRAY-GD-NIT", "FLEXRAY-NETWORK-IDLE-TIME"]) {
            p.network_idle_time = v;
        }
        if let Some(v) = get_u32(&["FLEXRAY-G-NUMBER-OF-MINISLOTS", "G-NUMBER-OF-MINISLOTS"]) {
            p.number_of_minislots = v;
        }
        if let Some(v) = get_u32(&["FLEXRAY-G-NUMBER-OF-STATIC-SLOTS", "G-NUMBER-OF-STATIC-SLOTS"]) {
            p.number_of_static_slots = v;
        }
        if let Some(v) = get_u32(&["FLEXRAY-GD-MINISLOT"]) {
            p.minislot_duration = v;
        }
        if let Some(v) = get_u32(&["FLEXRAY-GD-STATIC-SLOT"]) {
            p.static_slot_duration = v;
        }
        if let Some(v) = get_u32(&["FLEXRAY-GD-SYMBOL-WINDOW"]) {
            p.symbol_window = v;
        }
        if let Some(v) = get_u32(&["FLEXRAY-GD-SYMBOL-WINDOW-IDLE-PHASE"]) {
            p.symbol_window_idle_phase = v;
        }
        if let Some(v) = get_u32(&["FLEXRAY-OFFSET-CORRECTION-START", "G-OFFSET-CORRECTION-START"]) {
            p.offset_correction_start = v;
        }
    }

    // ------------------------------------------------------------------
    // 6. ECU
    // ------------------------------------------------------------------
    let mut ecu_nodes = Vec::new();
    collect_elements_by_tag(&root, "ECU-INSTANCE", &mut ecu_nodes);
    let mut ecus: Vec<String> = ecu_nodes.iter().map(|n| get_short_name(n)).collect();
    ecus.retain(|n| !n.is_empty() && n != "Unnamed");

    // ------------------------------------------------------------------
    // 7. 组装
    // ------------------------------------------------------------------
    let mut pdus: Vec<EditablePdu> = Vec::new();
    for (_, raw) in &pdu_raws {
        let mut signals = Vec::new();
        for m in &raw.mappings {
            let Some(signal_name) = &m.signal_name else {
                continue;
            };
            let def = signal_defs.get(signal_name);
            let length_bits = def.map(|d| d.length_bits).unwrap_or(m.length_bits.max(1));
            let signed = def.map(|d| d.signed).unwrap_or(false);

            // 物理值换算（COMPU-METHOD）：factor / offset / 值表
            let compu = signal_compu_ref
                .get(signal_name)
                .and_then(|cm| compu_methods.get(cm));
            let factor = compu.map(|c| c.factor).unwrap_or(1.0);
            let offset = compu.map(|c| c.offset).unwrap_or(0.0);
            let value_descriptions = compu
                .map(|c| c.value_descriptions.clone())
                .unwrap_or_default();
            let mut value_descriptions = value_descriptions;
            value_descriptions.sort_by_key(|(v, _)| *v);

            signals.push(EditableSignal::build(
                signal_name.clone(),
                m.start_bit,
                length_bits,
                if m.big_endian {
                    ByteOrder::BigEndian
                } else {
                    ByteOrder::LittleEndian
                },
                if signed {
                    ValueType::Signed
                } else {
                    ValueType::Unsigned
                },
                factor,
                offset,
                0.0,
                0.0,
                String::new(),
                sig_rx.get(signal_name).cloned().unwrap_or_default(),
                value_descriptions,
                def.map(|d| d.comment.clone()).unwrap_or_default(),
            ));

            // 发送者不入 build（与 DBC 语义对齐 receivers 在 build 内），单独补写
            if let Some(last) = signals.last_mut() {
                last.set_senders(sig_tx.get(signal_name).cloned().unwrap_or_default());
            }
        }
        let mut pdu = EditablePdu::build(
            raw.name.clone(),
            raw.length,
            raw.kind,
            signals,
            raw.comment.clone(),
        );
        pdu.set_senders(pdu_tx.get(&raw.name).cloned().unwrap_or_default());
        pdu.set_receivers(pdu_rx.get(&raw.name).cloned().unwrap_or_default());
        pdus.push(pdu);
    }

    let mut frames: Vec<EditableFrame> = Vec::new();
    for (name, raw) in &frames_by_name {
        let mut pdus_in_frame = Vec::new();
        for (pdu_name, start) in &raw.pdu_mappings {
            pdus_in_frame.push(FramePduMapping::new(pdu_name, *start));
        }
        let triggering = triggerings.get(name).copied().unwrap_or_default();
        frames.push(EditableFrame::build(
            raw.name.clone(),
            raw.length,
            raw.payload_preamble,
            triggering,
            pdus_in_frame,
            raw.comment.clone(),
        ));
    }

    // 名字去重
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
