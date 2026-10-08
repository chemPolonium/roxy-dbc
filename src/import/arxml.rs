//! 读 AUTOSAR 4.x 风格的 CAN 通信快照（ARXML）为可编辑的 DBC 模型
//!
//! 按引用解析：CAN-FRAME-TRIGGERING 的 FRAME-REF 找到帧、IDENTIFIER 与寻址方式给出
//! ID 与标准/扩展、CAN-FRAME-*-BEHAVIOR 给出是否 CAN FD；帧里的
//! PDU-TO-FRAME-MAPPING 的 PDU-REF 找到 I-SIGNAL-I-PDU，PDU 的
//! I-SIGNAL-TO-PDU-MAPPING 给出信号位置与字节序，I-SIGNAL-REF 找到 I-SIGNAL，
//! 再由其 COMPU-METHOD-REF 找到因子/偏移/上下限/单位/值表；收发关系来自端口名
//! 后缀 `_Tx` / `_Rx`（帧级 FRAME-PORT，信号级 I-SIGNAL-PORT）。
//! 文件里缺这些引用时回退到旧的命名约定（`{帧名}_PDU` / `{帧名}_Trigger`）。

use crate::editable_dbc::{EditableDbc, EditableMessage, EditableSignal, FrameFormat};
use can_dbc::{ByteOrder, ValueType};

/// COMPU-METHOD 里能拿到的物理换算信息
struct CompuMethod {
    factor: f64,
    offset: f64,
    min: Option<f64>,
    max: Option<f64>,
    unit: String,
    value_descriptions: Vec<(i64, String)>,
}

/// I-SIGNAL 定义
struct SignalDef {
    length_bits: u64,
    signed: bool,
    comment: String,
    compu: Option<String>,
}

/// I-SIGNAL-TO-PDU-MAPPING
struct Mapping {
    signal_name: String,
    signal_ref: Option<String>,
    start_bit: u64,
    length: u64,
    byte_order: Option<ByteOrder>,
}

/// I-SIGNAL-I-PDU
struct PduDef {
    name: String,
    length: u64,
    mappings: Vec<Mapping>,
}

/// CAN-FRAME-TRIGGERING 给出的调度与收发信息
#[derive(Clone, Default)]
struct Triggering {
    identifier: Option<u32>,
    extended: bool,
    fd: bool,
    transmitter: Option<String>,
    receivers: Vec<String>,
}

pub fn parse_arxml(content: &str) -> Result<EditableDbc, String> {
    let doc = roxmltree::Document::parse(content).map_err(|e| format!("XML parse error: {}", e))?;
    let root = doc.root_element();

    // ------------------------------------------------------------------
    // COMPU-METHOD
    // ------------------------------------------------------------------
    let mut compu_methods: std::collections::HashMap<String, CompuMethod> =
        std::collections::HashMap::new();
    for cm in collect(&root, "COMPU-METHOD") {
        let name = short_name(&cm);
        if name == "Unnamed" || compu_methods.contains_key(&name) {
            continue;
        }
        let mut method = CompuMethod {
            factor: 1.0,
            offset: 0.0,
            min: None,
            max: None,
            unit: text_in(&cm, &["UNIT", "DISPLAY-NAME", "UNIT-DISPLAY-NAME"]).unwrap_or_default(),
            value_descriptions: Vec::new(),
        };
        for scale in collect(&cm, "COMPU-SCALE") {
            if let Some(vt) = collect(&scale, "COMPU-CONST")
                .first()
                .and_then(|c| text_in(c, &["VT"]))
                .or_else(|| text_in(&scale, &["VT"]))
            {
                if let Some(lower) = text_in(&scale, &["LOWER-LIMIT"]).and_then(|s| parse_f64(&s)) {
                    method.value_descriptions.push((lower as i64, vt));
                }
                continue;
            }
            if let Some(coeffs) = collect(&scale, "COMPU-NUMERATOR").first() {
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
            if method.min.is_none() {
                method.min = text_in(&scale, &["LOWER-LIMIT"]).and_then(|s| parse_f64(&s));
                method.max = text_in(&scale, &["UPPER-LIMIT"]).and_then(|s| parse_f64(&s));
            }
        }
        compu_methods.insert(name, method);
    }

    // ------------------------------------------------------------------
    // I-SIGNAL
    // ------------------------------------------------------------------
    let mut signal_defs: std::collections::HashMap<String, SignalDef> =
        std::collections::HashMap::new();
    for sig in collect(&root, "I-SIGNAL") {
        let name = short_name(&sig);
        if name == "Unnamed" || signal_defs.contains_key(&name) {
            continue;
        }
        signal_defs.insert(
            name,
            SignalDef {
                length_bits: text_in(&sig, &["LENGTH"])
                    .and_then(|s| parse_u64(&s))
                    .unwrap_or(0),
                signed: text_in(&sig, &["SIGN-CONVENTION"])
                    .is_some_and(|t| t.contains("COMPLEMENT") || t == "SIGNED"),
                comment: desc_text(&sig),
                compu: text_in(&sig, &["COMPU-METHOD-REF"]).map(|r| ref_name(&r)),
            },
        );
    }

    // ------------------------------------------------------------------
    // I-SIGNAL-I-PDU
    // ------------------------------------------------------------------
    let mut pdus: std::collections::HashMap<String, PduDef> = std::collections::HashMap::new();
    let mut pdu_order: Vec<String> = Vec::new();
    for pdu in collect(&root, "I-SIGNAL-I-PDU") {
        let name = short_name(&pdu);
        if name == "Unnamed" || pdu_name_taken(&pdus, &name) {
            continue;
        }
        let mut mappings = Vec::new();
        for m in collect(&pdu, "I-SIGNAL-TO-PDU-MAPPING")
            .into_iter()
            .chain(collect(&pdu, "I-SIGNAL-TO-I-PDU-MAPPING"))
        {
            let signal_ref =
                text_in(&m, &["I-SIGNAL-REF", "SYSTEM-SIGNAL-REF"]).map(|r| ref_name(&r));
            let signal_name = short_name(&m);
            let signal_name = if signal_name == "Unnamed" || signal_name.is_empty() {
                signal_ref.clone().unwrap_or_default()
            } else {
                signal_name
            };
            mappings.push(Mapping {
                signal_name,
                start_bit: text_in(&m, &["START-POSITION"])
                    .and_then(|s| parse_u64(&s))
                    .unwrap_or(0),
                length: text_in(&m, &["LENGTH"])
                    .and_then(|s| parse_u64(&s))
                    .unwrap_or(0),
                byte_order: text_in(&m, &["PACKING-BYTE-ORDER"]).map(|t| {
                    if t == "MOST-SIGNIFICANT-BYTE-FIRST" {
                        ByteOrder::BigEndian
                    } else {
                        ByteOrder::LittleEndian
                    }
                }),
                signal_ref,
            });
        }
        pdu_order.push(name.clone());
        pdus.insert(
            name,
            PduDef {
                name: short_name(&pdu),
                length: collect(&pdu, "LENGTH")
                    .first()
                    .and_then(|n| n.text().and_then(parse_u64))
                    .unwrap_or(8),
                mappings,
            },
        );
    }

    // ------------------------------------------------------------------
    // 信号级收发：I-SIGNAL-TRIGGERING 的端口引用
    // ------------------------------------------------------------------
    let mut signal_receivers: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut signal_senders: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for st in collect(&root, "I-SIGNAL-TRIGGERING") {
        let Some(ref_text) = text_in(&st, &["I-SIGNAL-REF", "SYSTEM-SIGNAL-REF"]) else {
            continue;
        };
        let key = ref_name(&ref_text);
        for port in collect(&st, "I-SIGNAL-PORT-REF")
            .into_iter()
            .filter_map(|n| n.text().map(|t| t.trim().to_string()))
        {
            let Some((ecu, is_tx)) = port_direction(&port) else {
                continue;
            };
            let list = if is_tx {
                &mut signal_senders
            } else {
                &mut signal_receivers
            };
            let entry = list.entry(key.clone()).or_default();
            if !entry.contains(&ecu) {
                entry.push(ecu);
            }
        }
    }

    // ------------------------------------------------------------------
    // CAN-FRAME-TRIGGERING：标识符、寻址方式、CAN FD 行为、帧端口
    // ------------------------------------------------------------------
    let mut triggerings: std::collections::HashMap<String, Triggering> =
        std::collections::HashMap::new();
    for t in collect(&root, "CAN-FRAME-TRIGGERING") {
        let trigger_name = short_name(&t);
        let frame_name = text_in(&t, &["FRAME-REF"])
            .map(|r| ref_name(&r))
            .unwrap_or_else(|| strip_trigger_suffix(&trigger_name));
        let entry = triggerings.entry(frame_name).or_default();
        if entry.identifier.is_none() {
            entry.identifier = text_in(&t, &["IDENTIFIER"]).and_then(|s| parse_id(&s));
        }
        entry.extended |= text_in(&t, &["CAN-ADDRESSING-MODE"]).as_deref() == Some("EXTENDED");
        entry.fd |= collect(&t, "CAN-FRAME-TX-BEHAVIOR")
            .into_iter()
            .chain(collect(&t, "CAN-FRAME-RX-BEHAVIOR"))
            .any(|n| n.text().is_some_and(|t| t.contains("FD")));
        for port in collect(&t, "FRAME-PORT-REF")
            .into_iter()
            .filter_map(|n| n.text().map(|s| s.trim().to_string()))
        {
            let Some((ecu, is_tx)) = port_direction(&port) else {
                continue;
            };
            if is_tx {
                if entry.transmitter.is_none() {
                    entry.transmitter = Some(ecu);
                }
            } else if !entry.receivers.contains(&ecu) {
                entry.receivers.push(ecu);
            }
        }
    }

    // ------------------------------------------------------------------
    // CAN-FRAME -> 报文
    // ------------------------------------------------------------------
    let mut messages: Vec<EditableMessage> = Vec::new();
    let mut nodes: Vec<String> = Vec::new();
    let mut used_pdus: std::collections::HashSet<String> = std::collections::HashSet::new();

    for frame in collect(&root, "CAN-FRAME") {
        let name = short_name(&frame);
        let trig = triggerings.get(&name).cloned().unwrap_or_default();
        let identifier = trig
            .identifier
            .or_else(|| text_in(&frame, &["IDENTIFIER"]).and_then(|s| parse_id(&s)))
            .unwrap_or(0);

        // PDU：优先帧里的 PDU-TO-FRAME-MAPPING 引用，回退到旧的命名约定
        let pdu_ref = collect(&frame, "PDU-TO-FRAME-MAPPING")
            .first()
            .and_then(|m| text_in(m, &["PDU-REF"]))
            .map(|r| ref_name(&r));
        let pdu_key = pdu_ref
            .clone()
            .or_else(|| {
                let by_convention = format!("{name}_PDU");
                pdus.contains_key(&by_convention).then_some(by_convention)
            })
            .unwrap_or_else(|| pdu_ref.unwrap_or_default());
        let pdu = pdus.get(&pdu_key).inspect(|p| {
            used_pdus.insert(p.name.clone());
        });

        let length = text_in(&frame, &["FRAME-LENGTH"])
            .and_then(|s| parse_u64(&s))
            .or_else(|| pdu.map(|p| p.length))
            .unwrap_or(8);

        let extended = trig.extended || identifier > 0x7FF;
        let fd = trig.fd || length > 8;

        let mut signals: Vec<EditableSignal> = Vec::new();
        if let Some(pdu) = pdu {
            for m in &pdu.mappings {
                let def = m.signal_ref.as_ref().and_then(|r| signal_defs.get(r));
                let length_bits = if m.length > 0 {
                    m.length
                } else {
                    def.map(|d| d.length_bits).unwrap_or(1)
                };
                let compu = def
                    .and_then(|d| d.compu.as_ref())
                    .and_then(|c| compu_methods.get(c));
                let mut receivers = m
                    .signal_ref
                    .as_ref()
                    .and_then(|r| signal_receivers.get(r))
                    .cloned()
                    .unwrap_or_else(|| trig.receivers.clone());
                if receivers.is_empty() {
                    receivers = trig.receivers.clone();
                }
                for node in &receivers {
                    if !node.is_empty() && node != "Vector__XXX" && !nodes.contains(node) {
                        nodes.push(node.clone());
                    }
                }
                let mut values: Vec<(i64, String)> = compu
                    .map(|c| c.value_descriptions.clone())
                    .unwrap_or_default();
                values.sort_by_key(|(v, _)| *v);
                signals.push(EditableSignal::build(
                    m.signal_name.clone(),
                    m.start_bit,
                    length_bits,
                    m.byte_order.unwrap_or(ByteOrder::LittleEndian),
                    if def.is_some_and(|d| d.signed) {
                        ValueType::Signed
                    } else {
                        ValueType::Unsigned
                    },
                    compu.map(|c| c.factor).unwrap_or(1.0),
                    compu.map(|c| c.offset).unwrap_or(0.0),
                    compu.and_then(|c| c.min).unwrap_or(0.0),
                    compu.and_then(|c| c.max).unwrap_or(0.0),
                    compu.map(|c| c.unit.clone()).unwrap_or_default(),
                    receivers,
                    values,
                    def.map(|d| d.comment.clone()).unwrap_or_default(),
                ));
            }
        }

        let transmitter = trig
            .transmitter
            .clone()
            .unwrap_or_else(|| "Vector__XXX".to_string());
        if transmitter != "Vector__XXX" && !nodes.contains(&transmitter) {
            nodes.push(transmitter.clone());
        }
        // 只有帧端口、没有信号端口时，帧的接收者已在上面分给每个信号
        if !trig.receivers.is_empty() {
            for node in &trig.receivers {
                if !node.is_empty() && node != "Vector__XXX" && !nodes.contains(node) {
                    nodes.push(node.clone());
                }
            }
        }

        messages.push(EditableMessage::build(
            identifier,
            FrameFormat::compose(extended, fd),
            name.clone(),
            length,
            transmitter,
            signals,
            desc_text(&frame),
        ));
    }

    // 没有被任何帧引用的 PDU（外部文件）：按 PDU 名建一条无 ID 的报文
    for name in &pdu_order {
        if used_pdus.contains(name) {
            continue;
        }
        let Some(pdu) = pdus.get(name) else { continue };
        let signals = pdu
            .mappings
            .iter()
            .map(|m| {
                let def = m.signal_ref.as_ref().and_then(|r| signal_defs.get(r));
                let receivers = m
                    .signal_ref
                    .as_ref()
                    .and_then(|r| signal_receivers.get(r))
                    .cloned()
                    .unwrap_or_default();
                for node in &receivers {
                    if !node.is_empty() && node != "Vector__XXX" && !nodes.contains(node) {
                        nodes.push(node.clone());
                    }
                }
                let compu = def
                    .and_then(|d| d.compu.as_ref())
                    .and_then(|c| compu_methods.get(c));
                let mut values: Vec<(i64, String)> = compu
                    .map(|c| c.value_descriptions.clone())
                    .unwrap_or_default();
                values.sort_by_key(|(v, _)| *v);
                EditableSignal::build(
                    m.signal_name.clone(),
                    m.start_bit,
                    if m.length > 0 {
                        m.length
                    } else {
                        def.map(|d| d.length_bits).unwrap_or(1)
                    },
                    m.byte_order.unwrap_or(ByteOrder::LittleEndian),
                    if def.is_some_and(|d| d.signed) {
                        ValueType::Signed
                    } else {
                        ValueType::Unsigned
                    },
                    compu.map(|c| c.factor).unwrap_or(1.0),
                    compu.map(|c| c.offset).unwrap_or(0.0),
                    compu.and_then(|c| c.min).unwrap_or(0.0),
                    compu.and_then(|c| c.max).unwrap_or(0.0),
                    compu.map(|c| c.unit.clone()).unwrap_or_default(),
                    receivers,
                    values,
                    def.map(|d| d.comment.clone()).unwrap_or_default(),
                )
            })
            .collect();
        messages.push(EditableMessage::build(
            0,
            FrameFormat::Standard,
            pdu.name.clone(),
            pdu.length,
            "Vector__XXX".to_string(),
            signals,
            String::new(),
        ));
    }

    Ok(EditableDbc::from_imported(messages, nodes))
}

fn pdu_name_taken(pdus: &std::collections::HashMap<String, PduDef>, name: &str) -> bool {
    pdus.contains_key(name)
}

/// 收集指定标签的元素（含嵌套）
fn collect<'a, 'b>(node: &roxmltree::Node<'a, 'b>, tag: &str) -> Vec<roxmltree::Node<'a, 'b>> {
    let mut out = Vec::new();
    collect_into(node, tag, &mut out);
    out
}

fn collect_into<'a, 'b>(
    node: &roxmltree::Node<'a, 'b>,
    tag: &str,
    out: &mut Vec<roxmltree::Node<'a, 'b>>,
) {
    if node.is_element() && node.has_tag_name(tag) {
        out.push(*node);
    }
    for child in node.children() {
        collect_into(&child, tag, out);
    }
}

fn short_name(node: &roxmltree::Node) -> String {
    node.children()
        .find(|n| n.has_tag_name("SHORT-NAME"))
        .and_then(|n| n.text())
        .map(|t| t.trim().to_string())
        .unwrap_or_else(|| "Unnamed".to_string())
}

/// 直接子元素里第一个有文本的指定标签
fn text_in(node: &roxmltree::Node, tags: &[&str]) -> Option<String> {
    for tag in tags {
        if let Some(text) = collect(node, tag)
            .into_iter()
            .filter_map(|n| n.text().map(|t| t.trim().to_string()))
            .find(|t| !t.is_empty())
        {
            return Some(text);
        }
    }
    None
}

/// DESC 里的文本（L-2 / L-4 等）
fn desc_text(node: &roxmltree::Node) -> String {
    collect(node, "DESC")
        .first()
        .and_then(|d| {
            d.descendants()
                .find(|n| {
                    n.is_element() && n.has_tag_name("L-2")
                        || n.is_element() && n.has_tag_name("L-4")
                })
                .and_then(|n| n.text().map(|t| t.trim().to_string()))
        })
        .unwrap_or_default()
}

/// 引用路径的最后一段就是元素名
fn ref_name(reference: &str) -> String {
    reference
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or(reference)
        .trim()
        .to_string()
}

/// 端口路径形如 /CanDb/ECU1/CN_Cluster/FP_Foo_Tx：末段后缀给方向，倒数第 3 段是 ECU
fn port_direction(port_ref: &str) -> Option<(String, bool)> {
    let segments: Vec<&str> = port_ref.split('/').filter(|s| !s.is_empty()).collect();
    let last = segments.last()?.to_uppercase();
    let ecu = segments.iter().rev().nth(2).map(|s| s.to_string())?;
    if last.ends_with("_TX") {
        Some((ecu, true))
    } else if last.ends_with("_RX") {
        Some((ecu, false))
    } else {
        None
    }
}

/// 没有 FRAME-REF 时从触发点名字回推帧名（`{帧名}_Trigger`）
fn strip_trigger_suffix(name: &str) -> String {
    name.strip_suffix("_Trigger")
        .or_else(|| name.strip_suffix("_TRIGGER"))
        .unwrap_or(name)
        .to_string()
}

fn parse_u64(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u64>().ok()
    }
}

fn parse_f64(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok()
}

fn parse_id(s: &str) -> Option<u32> {
    parse_u64(s).map(|v| v as u32)
}
