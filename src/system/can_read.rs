//! 直接读 AUTOSAR 4.x 的 CAN 部分进统一系统模型
//!
//! 不经 DBC 中转：真实文件里的集群名与通道名、波特率、PDU 名与 PDU 种类、帧与 PDU
//! 的起始字节、I-PDU-TIMING 的周期、ECU 的控制器要求、各层触发点的名字都是数据库
//! 内容；DBC 只有"帧即 PDU"，中转一圈这些就全没了。
//!
//! 读的同时把每个对象、每个可编辑字段在原文里的字节区间记进
//! [`crate::system::source::ArxmlSource`]：保存只替换改过的那一段，文件里本工具
//! 不建模的内容一个字节都不动。
//!
//! 收发关系按端口解析：CAN-FRAME-TRIGGERING 的 FRAME-PORT-REF 给帧级收发，
//! PDU-TRIGGERING 的 I-PDU-PORT-REF 给 PDU 级，I-SIGNAL-TRIGGERING 的
//! I-SIGNAL-PORT-REF 给信号级；方向取 ECU 连接器里端口定义的 COMMUNICATION-DIRECTION，
//! 名字后缀 `_Tx` / `_Rx` 只作回退。

use crate::fibex::editable_fibex::{ByteOrder, ValueType};
use crate::system::source::{self, ArxmlSource};
use crate::system::{
    BusKind, CanBehavior, CanFrameProps, Cluster, Ecu, EcuController, Frame, GroupDir, Pdu,
    PduType, Signal, SignalGroup,
};
use std::collections::HashMap;

/// 读出的 CAN 部分，由 [`crate::system::SystemDoc::from_arxml_with_source`] 合进统一文档
#[derive(Default)]
pub struct CanRead {
    pub clusters: Vec<Cluster>,
    pub ecus: Vec<Ecu>,
    pub pdus: Vec<Pdu>,
    pub frames: Vec<Frame>,
    pub groups: Vec<SignalGroup>,
}

/// COMPU-METHOD 里能拿到的物理换算信息
struct Compu {
    factor: f64,
    offset: f64,
    min: Option<f64>,
    max: Option<f64>,
    unit: String,
    unit_name: Option<String>,
    value_descriptions: Vec<(i64, String)>,
}

/// I-SIGNAL 定义
struct SignalDef {
    length_bits: u64,
    signed: bool,
    comment: String,
    long_name: String,
    path: String,
    compu: Option<String>,
    init_value: Option<f64>,
}

/// 信号在 PDU 里的映射
struct Mapping {
    signal: String,
    start_bit: u64,
    length: u64,
    byte_order: Option<ByteOrder>,
}

/// PDU 定义
struct PduDef {
    name: String,
    length: u64,
    pdu_type: PduType,
    comment: String,
    long_name: String,
    path: String,
    cycle_ms: Option<f64>,
    mappings: Vec<Mapping>,
}

/// CAN-FRAME-TRIGGERING 给出的标识符与收发
#[derive(Clone, Default)]
struct Triggering {
    name: String,
    identifier: Option<u32>,
    extended: bool,
    fd: bool,
    tx_behavior: Option<CanBehavior>,
    rx_behavior: Option<CanBehavior>,
    senders: Vec<String>,
    receivers: Vec<String>,
}

pub fn parse_can(root: &roxmltree::Node, src: &mut ArxmlSource) -> CanRead {
    let units = read_units(root, src);
    let base_signed = read_base_types(root);
    let compus = read_compu_methods(root, &units, src);
    let system_compu = read_system_signals(root);
    let signal_defs = read_signals(root, &base_signed, &system_compu, src);
    let port_dirs = read_ports(root);

    // FlexRay 帧引用的 PDU 不属于 CAN 模型：混合文件里两边共用 I-SIGNAL-I-PDU 标签
    let mut flexray_pdus: HashMap<String, ()> = HashMap::new();
    for frame in collect(root, "FLEXRAY-FRAME") {
        for m in collect(&frame, "FRAME-PDU-MAPPING")
            .into_iter()
            .chain(collect(&frame, "PDU-TO-FRAME-MAPPING"))
        {
            if let Some(r) = text_in(&m, &["PDU-REF"]) {
                flexray_pdus.insert(ref_name(&r), ());
            }
        }
    }

    let pdus = read_pdus(root, &flexray_pdus, src);
    let (pdu_senders, pdu_receivers, pdu_triggerings) = read_pdu_triggerings(root, &port_dirs, src);
    let (signal_senders, signal_receivers, signal_triggerings) =
        read_signal_triggerings(root, &port_dirs, src);

    // ---------------------------------------------------------------- 集群与触发点
    let mut clusters: Vec<Cluster> = Vec::new();
    let mut frame_cluster: HashMap<String, String> = HashMap::new();
    let mut pdu_cluster: HashMap<String, String> = HashMap::new();
    let mut triggerings: HashMap<String, Triggering> = HashMap::new();
    let mut ecu_clusters: HashMap<String, Vec<String>> = HashMap::new();

    for c in collect(root, "CAN-CLUSTER") {
        let name = short_name(&c);
        if name == "Unnamed" {
            continue;
        }
        record_object(src, "cluster", &name, &c);
        record_field(src, &format!("cluster/{name}/baudrate"), &c, &["BAUDRATE"]);
        let mut channels: Vec<String> = Vec::new();
        let mut connectors: Vec<String> = Vec::new();
        for ch in collect(&c, "CAN-PHYSICAL-CHANNEL") {
            let ch_name = short_name(&ch);
            if ch_name != "Unnamed" && !channels.contains(&ch_name) {
                channels.push(ch_name);
            }
            for r in collect(&ch, "COMMUNICATION-CONNECTOR-REF") {
                if let Some(text) = r.text() {
                    // /VehicleProject/ECU/Console/CN_CAN00：倒数第三段是 ECU
                    let seg: Vec<&str> = text.split('/').filter(|s| !s.is_empty()).collect();
                    if let Some(ecu) = seg.iter().rev().nth(2) {
                        let ecu = ecu.to_string();
                        if !connectors.contains(&ecu) {
                            connectors.push(ecu.clone());
                        }
                        ecu_clusters.entry(ecu).or_default().push(name.clone());
                    }
                }
            }
            for t in collect(&ch, "CAN-FRAME-TRIGGERING") {
                let info = read_frame_triggering(&t, &port_dirs, src);
                let frame = frame_of_triggering(&t, &info.name);
                frame_cluster
                    .entry(frame.clone())
                    .or_insert_with(|| name.clone());
                triggerings.entry(frame).or_insert(info);
            }
            if let Some(ft) = ch
                .children()
                .find(|n| n.is_element() && n.has_tag_name("FRAME-TRIGGERINGS"))
            {
                src.record_container(
                    &format!("cluster/{name}/triggerings"),
                    source::inner_span(&ft),
                );
            }
            // PDU-TRIGGERING 住在通道下：它引用的 PDU 属于这个集群。文件里定义了
            // 却既没被帧映射也没被触发的 PDU（参考界面不列）保持不属于任何集群
            for t in collect(&ch, "PDU-TRIGGERING") {
                if let Some(r) = text_in(&t, &["I-PDU-REF"]) {
                    pdu_cluster
                        .entry(ref_name(&r))
                        .or_insert_with(|| name.clone());
                }
            }
        }
        if channels.is_empty() {
            channels.push("CANChannel".to_string());
        }
        clusters.push(Cluster {
            baudrate: text_in(&c, &["BAUDRATE"]).and_then(|s| s.parse::<f64>().ok()),
            long_name: long_name_text(&c),
            comment: desc_text(&c),
            name,
            kind: BusKind::Can,
            channels,
            ..Default::default()
        });
    }

    // ---------------------------------------------------------------- 帧
    let mut frames: Vec<Frame> = Vec::new();
    for f in collect(root, "CAN-FRAME") {
        let name = short_name(&f);
        if name == "Unnamed" {
            continue;
        }
        record_object(src, "frame", &name, &f);
        record_field(src, &format!("frame/{name}/length"), &f, &["FRAME-LENGTH"]);
        let trig = triggerings.get(&name).cloned().unwrap_or_default();
        let cluster = frame_cluster
            .get(&name)
            .cloned()
            .or_else(|| clusters.first().map(|c| c.name.clone()))
            .unwrap_or_default();
        let mut mappings = Vec::new();
        for m in collect(&f, "PDU-TO-FRAME-MAPPING") {
            let Some(r) = text_in(&m, &["PDU-REF"]) else {
                continue;
            };
            let pdu_name = ref_name(&r);
            src.record_element(
                &format!("frame/{name}/map/{pdu_name}/element"),
                source::element_span(&m),
            );
            record_field(
                src,
                &format!("frame/{name}/map/{pdu_name}/ref"),
                &m,
                &["PDU-REF"],
            );
            record_field(
                src,
                &format!("frame/{name}/map/{pdu_name}/start"),
                &m,
                &["START-POSITION"],
            );
            mappings.push(crate::fibex::editable_fibex::FramePduMapping {
                pdu_name,
                start_position: text_in(&m, &["START-POSITION"])
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0) as u32,
            });
        }
        if let Some(span) = source::container_of(&f) {
            src.record_container("frames", span);
        }
        if let Some(maps) = f
            .children()
            .find(|n| n.is_element() && n.has_tag_name("PDU-TO-FRAME-MAPPINGS"))
        {
            src.record_container(&format!("frame/{name}/maps"), source::inner_span(&maps));
        }
        let length = text_in(&f, &["FRAME-LENGTH"])
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or_else(|| {
                mappings
                    .first()
                    .and_then(|m| pdus.iter().find(|p| p.name == m.pdu_name))
                    .map(|p| p.length)
                    .unwrap_or(8)
            });
        let identifier = trig
            .identifier
            .or_else(|| {
                text_in(&f, &["IDENTIFIER"])
                    .and_then(|s| s.parse::<u64>().ok())
                    .map(|v| v as u32)
            })
            .unwrap_or(0);
        let extended = trig.extended || identifier > 0x7FF;
        let fd = trig.fd || length > 8;
        frames.push(Frame {
            pdus: mappings,
            can: Some(CanFrameProps {
                identifier,
                extended,
                fd,
                tx_behavior: trig.tx_behavior.unwrap_or(if fd {
                    CanBehavior::CanFd
                } else {
                    CanBehavior::Can20
                }),
                rx_behavior: trig.rx_behavior.unwrap_or(if fd {
                    CanBehavior::CanFd
                } else {
                    CanBehavior::Can20
                }),
                ..Default::default()
            }),
            senders: trig.senders.clone(),
            receivers: trig.receivers.clone(),
            comment: desc_text(&f),
            long_name: long_name_text(&f),
            path: path_of(&f),
            triggering: (!trig.name.is_empty()).then_some(trig.name),
            name,
            cluster,
            length_bytes: length as u32,
            ..Default::default()
        });
    }

    // ---------------------------------------------------------------- PDU
    let mut out_pdus: Vec<Pdu> = Vec::new();
    for p in &pdus {
        let cluster = frames
            .iter()
            .find(|f| f.pdus.iter().any(|m| m.pdu_name == p.name))
            .map(|f| f.cluster.clone())
            .or_else(|| pdu_cluster.get(&p.name).cloned())
            .unwrap_or_default();
        let mut signals: Vec<Signal> = Vec::new();
        for m in &p.mappings {
            let def = signal_defs.get(&m.signal);
            let compu = def
                .and_then(|d| d.compu.as_ref())
                .and_then(|c| compus.get(c));
            let mut receivers = signal_receivers.get(&m.signal).cloned().unwrap_or_default();
            if receivers.is_empty() {
                receivers = pdu_receivers.get(&p.name).cloned().unwrap_or_default();
            }
            let senders = signal_senders
                .get(&m.signal)
                .cloned()
                .or_else(|| pdu_senders.get(&p.name).cloned())
                .unwrap_or_default();
            let mut values = compu
                .map(|c| c.value_descriptions.clone())
                .unwrap_or_default();
            values.sort_by_key(|(v, _)| *v);
            signals.push(Signal {
                byte_order: m.byte_order.unwrap_or(ByteOrder::LittleEndian),
                value_type: if def.is_some_and(|d| d.signed) {
                    ValueType::Signed
                } else {
                    ValueType::Unsigned
                },
                factor: compu.map(|c| c.factor).unwrap_or(1.0),
                offset: compu.map(|c| c.offset).unwrap_or(0.0),
                min: compu.and_then(|c| c.min).unwrap_or(0.0),
                max: compu.and_then(|c| c.max).unwrap_or(0.0),
                unit: compu.map(|c| c.unit.clone()).unwrap_or_default(),
                senders,
                receivers,
                comment: def.map(|d| d.comment.clone()).unwrap_or_default(),
                value_descriptions: values,
                long_name: def.map(|d| d.long_name.clone()).unwrap_or_default(),
                path: def.map(|d| d.path.clone()).unwrap_or_default(),
                compu_name: def.and_then(|d| d.compu.clone()).unwrap_or_default(),
                init_value: def.and_then(|d| d.init_value),
                triggering: signal_triggerings.get(&m.signal).cloned(),
                name: m.signal.clone(),
                start_bit: m.start_bit as u32,
                length_bits: if m.length > 0 {
                    m.length as u32
                } else {
                    def.map(|d| d.length_bits as u32).unwrap_or(1)
                },
            });
        }
        out_pdus.push(Pdu {
            cycle_ms: p.cycle_ms,
            signals,
            senders: pdu_senders.get(&p.name).cloned().unwrap_or_default(),
            receivers: pdu_receivers.get(&p.name).cloned().unwrap_or_default(),
            comment: p.comment.clone(),
            pdu_type: p.pdu_type,
            long_name: p.long_name.clone(),
            path: p.path.clone(),
            triggering: pdu_triggerings.get(&p.name).cloned(),
            name: p.name.clone(),
            cluster,
            length_bytes: p.length as u32,
            ..Default::default()
        });
    }

    // ---------------------------------------------------------------- ECU
    let mut ecus: Vec<Ecu> = Vec::new();
    for e in collect(root, "ECU-INSTANCE") {
        let name = short_name(&e);
        if name == "Unnamed" {
            continue;
        }
        record_object(src, "ecu", &name, &e);
        if let Some(span) = source::container_of(&e) {
            src.record_container("ecus", span);
        }
        let mut controllers: Vec<EcuController> = Vec::new();
        for ctrl in collect(&e, "CAN-COMMUNICATION-CONTROLLER") {
            controllers.push(EcuController {
                min_quanta: text_in(&ctrl, &["MIN-NUMBER-OF-TIME-QUANTA-PER-BIT"])
                    .and_then(|s| s.parse::<f64>().ok()),
                max_quanta: text_in(&ctrl, &["MAX-NUMBER-OF-TIME-QUANTA-PER-BIT"])
                    .and_then(|s| s.parse::<f64>().ok()),
                min_sample: text_in(&ctrl, &["MIN-SAMPLE-POINT"])
                    .and_then(|s| s.parse::<f64>().ok()),
                max_sample: text_in(&ctrl, &["MAX-SAMPLE-POINT"])
                    .and_then(|s| s.parse::<f64>().ok()),
                name: short_name(&ctrl),
            });
        }
        let clusters_of_ecu = ecu_clusters
            .get(&name)
            .cloned()
            .unwrap_or_else(|| clusters.iter().map(|c| c.name.clone()).collect());
        ecus.push(Ecu {
            clusters: clusters_of_ecu,
            comment: desc_text(&e),
            long_name: long_name_text(&e),
            path: path_of(&e),
            controllers,
            name,
            ..Default::default()
        });
    }

    // ---------------------------------------------------------------- 信号组
    let mut group_defs: HashMap<String, (GroupDir, Vec<String>)> = HashMap::new();
    for g in collect(root, "I-SIGNAL-I-PDU-GROUP") {
        let name = short_name(&g);
        if name == "Unnamed" || group_defs.contains_key(&name) {
            continue;
        }
        let direction = match text_in(&g, &["COMMUNICATION-DIRECTION"]).as_deref() {
            Some("IN") => GroupDir::In,
            Some("OUT") => GroupDir::Out,
            _ => GroupDir::None,
        };
        let pdus = collect(&g, "I-SIGNAL-I-PDU-REF")
            .into_iter()
            .filter_map(|r| r.text().map(|t| ref_name(&t)))
            .collect();
        group_defs.insert(name, (direction, pdus));
    }
    let mut ecu_groups: HashMap<String, Vec<String>> = HashMap::new();
    for e in collect(root, "ECU-INSTANCE") {
        let ecu = short_name(&e);
        for r in collect(&e, "ASSOCIATED-COM-I-PDU-GROUP-REF") {
            if let Some(t) = r.text() {
                ecu_groups
                    .entry(ecu.clone())
                    .or_default()
                    .push(ref_name(&t));
            }
        }
    }
    let mut groups: Vec<SignalGroup> = Vec::new();
    let mut covered: HashMap<String, ()> = HashMap::new();
    for e in &ecus {
        for g in ecu_groups.get(&e.name).cloned().unwrap_or_default() {
            if let Some((direction, pdus)) = group_defs.get(&g) {
                covered.insert(g.clone(), ());
                groups.push(SignalGroup {
                    cluster: e.clusters.first().cloned().unwrap_or_default(),
                    ecu: e.name.clone(),
                    direction: *direction,
                    pdus: pdus.clone(),
                    name: g,
                });
            }
        }
    }
    // 没挂到任何 ECU 的组：集群按成员 PDU 推
    for (name, (direction, pdus)) in &group_defs {
        if covered.contains_key(name) {
            continue;
        }
        let cluster = pdus
            .iter()
            .find_map(|p| out_pdus.iter().find(|pdu| pdu.name == *p))
            .map(|p| p.cluster.clone())
            .unwrap_or_default();
        groups.push(SignalGroup {
            cluster,
            ecu: String::new(),
            direction: *direction,
            pdus: pdus.clone(),
            name: name.clone(),
        });
    }
    groups.sort_by(|a, b| (&a.ecu, &a.name).cmp(&(&b.ecu, &b.name)));

    CanRead {
        clusters,
        ecus,
        pdus: out_pdus,
        frames,
        groups,
    }
}

// ------------------------------------------------------------------ 读取各部分

/// 记下对象元素本身与名字 / 注释 / 长名这几个通用字段的出处
fn record_object(src: &mut ArxmlSource, kind: &str, name: &str, node: &roxmltree::Node) {
    let key = format!("{kind}/{name}");
    src.record_element(&key, source::element_span(node));
    if let Some(anchor) = source::anchor_after_short_name(node) {
        src.record_anchor(&key, anchor.clone());
        src.record_anchor(&format!("{key}/name"), anchor.clone());
        src.record_anchor(&format!("{key}/comment"), anchor.clone());
        src.record_anchor(&format!("{key}/long_name"), anchor);
    }
    record_field(src, &format!("{key}/name"), node, &["SHORT-NAME"]);
    record_field(src, &format!("{key}/comment"), node, &["DESC"]);
    record_field(src, &format!("{key}/long_name"), node, &["LONG-NAME"]);
    if let Some(span) = source::container_of(node) {
        src.record_container(&format!("{kind}s"), span);
    }
    // 容器所属的 package：新增同类对象时引用路径要拼到它下面
    if let Some(package) = node.parent().and_then(|p| p.parent()) {
        src.record_container_path(&format!("{kind}s"), path_of(&package));
    }
}

/// 记一个字段的文本区间，返回文本
fn record_field(
    src: &mut ArxmlSource,
    key: &str,
    node: &roxmltree::Node,
    tags: &[&str],
) -> Option<String> {
    let (text, span) = source::text_span(node, tags)?;
    src.record_field(key, span);
    Some(text)
}

fn read_units(root: &roxmltree::Node, src: &mut ArxmlSource) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for u in collect(root, "UNIT") {
        let name = short_name(&u);
        if name == "Unnamed" {
            continue;
        }
        src.record_element(&format!("unit/{name}"), source::element_span(&u));
        record_field(src, &format!("unit/{name}/display"), &u, &["DISPLAY-NAME"]);
        out.entry(name)
            .or_insert_with(|| text_in(&u, &["DISPLAY-NAME"]).unwrap_or_default());
    }
    out
}

fn read_base_types(root: &roxmltree::Node) -> HashMap<String, bool> {
    let mut out = HashMap::new();
    for b in collect(root, "SW-BASE-TYPE") {
        let name = short_name(&b);
        if name == "Unnamed" {
            continue;
        }
        let encoding = text_in(&b, &["BASE-TYPE-ENCODING"]).unwrap_or_default();
        out.entry(name)
            .or_insert(matches!(encoding.as_str(), "2C" | "1C" | "SIGNED"));
    }
    out
}

fn read_system_signals(root: &roxmltree::Node) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for s in collect(root, "SYSTEM-SIGNAL") {
        let name = short_name(&s);
        if name == "Unnamed" {
            continue;
        }
        if let Some(r) = text_in(&s, &["COMPU-METHOD-REF"]) {
            out.insert(name, ref_name(&r));
        }
    }
    out
}

fn read_compu_methods(
    root: &roxmltree::Node,
    units: &HashMap<String, String>,
    src: &mut ArxmlSource,
) -> HashMap<String, Compu> {
    let mut out = HashMap::new();
    for cm in collect(root, "COMPU-METHOD") {
        let name = short_name(&cm);
        if name == "Unnamed" || out.contains_key(&name) {
            continue;
        }
        let unit_ref = text_in(&cm, &["UNIT-REF"]).map(|r| ref_name(&r));
        let unit = match &unit_ref {
            Some(r) => units.get(r).cloned().unwrap_or_default(),
            None => {
                text_in(&cm, &["UNIT", "DISPLAY-NAME", "UNIT-DISPLAY-NAME"]).unwrap_or_default()
            }
        };
        let mut method = Compu {
            factor: 1.0,
            offset: 0.0,
            min: None,
            max: None,
            unit,
            unit_name: unit_ref,
            value_descriptions: Vec::new(),
        };
        for scale in collect(&cm, "COMPU-SCALE") {
            if let Some(vt) = collect(&scale, "COMPU-CONST")
                .first()
                .and_then(|c| text_in(c, &["VT"]))
                .or_else(|| text_in(&scale, &["VT"]))
            {
                if let Some(lower) =
                    text_in(&scale, &["LOWER-LIMIT"]).and_then(|s| s.parse::<f64>().ok())
                {
                    method.value_descriptions.push((lower as i64, vt));
                }
                continue;
            }
            if let Some(coeffs) = collect(&scale, "COMPU-NUMERATOR").first() {
                // <V>偏移</V><V>系数</V>：两个值的文本区间分别记下来
                let vs: Vec<roxmltree::Node> =
                    coeffs.children().filter(|n| n.has_tag_name("V")).collect();
                if let Some(span) = vs.first().and_then(source::node_text_span) {
                    src.record_field(&format!("compu/{name}/offset"), span);
                }
                if vs.len() >= 2 {
                    if let Some(span) = source::node_text_span(&vs[1]) {
                        src.record_field(&format!("compu/{name}/factor"), span);
                    }
                }
                let values: Vec<f64> = vs
                    .iter()
                    .filter_map(|n| n.text().and_then(|s| s.parse::<f64>().ok()))
                    .collect();
                if values.len() >= 2 {
                    method.offset = values[0];
                    method.factor = values[1];
                } else if values.len() == 1 {
                    method.factor = values[0];
                }
            }
            if method.min.is_none() {
                // 刻度里的上下限是原始值，物理值 = 偏移 + 系数 × 原始值
                if let Some((_, span)) = source::text_span(&scale, &["LOWER-LIMIT"]) {
                    src.record_field(&format!("compu/{name}/min"), span);
                }
                if let Some((_, span)) = source::text_span(&scale, &["UPPER-LIMIT"]) {
                    src.record_field(&format!("compu/{name}/max"), span);
                }
                method.min = text_in(&scale, &["LOWER-LIMIT"])
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(|raw| method.offset + method.factor * raw);
                method.max = text_in(&scale, &["UPPER-LIMIT"])
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(|raw| method.offset + method.factor * raw);
            }
        }
        // 值表整块替换：记下 COMPU-SCALES 的元素区间
        if let Some(scales) = collect(&cm, "COMPU-SCALES").into_iter().next() {
            src.record_field(
                &format!("compu/{name}/values"),
                source::element_span(&scales),
            );
        }
        if let Some(unit) = &method.unit_name {
            src.compu_unit.insert(name.clone(), unit.clone());
        }
        out.insert(name, method);
    }
    out
}

fn read_signals(
    root: &roxmltree::Node,
    base_signed: &HashMap<String, bool>,
    system_compu: &HashMap<String, String>,
    src: &mut ArxmlSource,
) -> HashMap<String, SignalDef> {
    let mut out = HashMap::new();
    for sig in collect(root, "I-SIGNAL") {
        let name = short_name(&sig);
        if name == "Unnamed" || out.contains_key(&name) {
            continue;
        }
        record_object(src, "signal", &name, &sig);
        record_field(src, &format!("signal/{name}/length"), &sig, &["LENGTH"]);
        let signed = match text_in(&sig, &["SIGN-CONVENTION"]) {
            Some(t) => t.contains("COMPLEMENT") || t == "SIGNED",
            None => text_in(&sig, &["BASE-TYPE-REF"])
                .map(|r| base_signed.get(&ref_name(&r)).copied().unwrap_or(false))
                .unwrap_or(false),
        };
        let compu = text_in(&sig, &["COMPU-METHOD-REF"])
            .map(|r| ref_name(&r))
            .or_else(|| {
                text_in(&sig, &["SYSTEM-SIGNAL-REF"])
                    .map(|r| ref_name(&r))
                    .and_then(|s| system_compu.get(&s).cloned())
            });
        let init_value = collect(&sig, "INIT-VALUE")
            .first()
            .and_then(|iv| text_in(iv, &["VALUE"]))
            .and_then(|s| s.parse::<f64>().ok());
        out.insert(
            name,
            SignalDef {
                length_bits: text_in(&sig, &["LENGTH"])
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0),
                signed,
                comment: desc_text(&sig),
                long_name: long_name_text(&sig),
                path: path_of(&sig),
                compu,
                init_value,
            },
        );
    }
    out
}

/// ECU 连接器里的端口定义：完整路径 → (ECU, 是否发送)
fn read_ports(root: &roxmltree::Node) -> HashMap<String, (String, bool)> {
    let mut out = HashMap::new();
    for ecu_node in collect(root, "ECU-INSTANCE") {
        let ecu = short_name(&ecu_node);
        if ecu == "Unnamed" {
            continue;
        }
        for connector in collect(&ecu_node, "CAN-COMMUNICATION-CONNECTOR")
            .into_iter()
            .chain(collect(&ecu_node, "COMMUNICATION-CONNECTOR"))
        {
            let cn = short_name(&connector);
            for p in connector.descendants().filter(|n| {
                n.is_element()
                    && matches!(
                        n.tag_name().name(),
                        "FRAME-PORT" | "I-PDU-PORT" | "I-SIGNAL-PORT"
                    )
            }) {
                let name = short_name(&p);
                if name == "Unnamed" {
                    continue;
                }
                let out_dir = text_in(&p, &["COMMUNICATION-DIRECTION"])
                    .is_some_and(|d| d.eq_ignore_ascii_case("OUT"));
                out.insert(format!("/{ecu}/{cn}/{name}"), (ecu.clone(), out_dir));
                // 文件里的引用可能带根 package 前缀，末三段相同就能对上
                out.insert(format!("{ecu}/{cn}/{name}"), (ecu.clone(), out_dir));
            }
        }
    }
    out
}

fn read_pdus(
    root: &roxmltree::Node,
    flexray: &HashMap<String, ()>,
    src: &mut ArxmlSource,
) -> Vec<PduDef> {
    let mut out = Vec::new();
    for pdu in root
        .descendants()
        .filter(|n| n.is_element() && PduType::from_tag(n.tag_name().name()).is_some())
    {
        let name = short_name(&pdu);
        if name == "Unnamed"
            || flexray.contains_key(&name)
            || out.iter().any(|p: &PduDef| p.name == name)
        {
            continue;
        }
        record_object(src, "pdu", &name, &pdu);
        record_field(src, &format!("pdu/{name}/length"), &pdu, &["LENGTH"]);
        // 周期同时写在 true / false 两段时序里：每处都记，改的时候一起换
        for period in collect(&pdu, "TIME-PERIOD") {
            if let Some((_, span)) = source::text_span(&period, &["VALUE"]) {
                src.record_field(&format!("pdu/{name}/cycle"), span);
            }
        }
        let mut mappings = Vec::new();
        for m in collect(&pdu, "I-SIGNAL-TO-PDU-MAPPING")
            .into_iter()
            .chain(collect(&pdu, "I-SIGNAL-TO-I-PDU-MAPPING"))
        {
            let Some(r) = text_in(&m, &["I-SIGNAL-REF", "SYSTEM-SIGNAL-REF"]) else {
                continue;
            };
            let signal = ref_name(&r);
            src.record_element(
                &format!("pdu/{name}/map/{signal}/element"),
                source::element_span(&m),
            );
            record_field(
                src,
                &format!("pdu/{name}/map/{signal}/ref"),
                &m,
                &["I-SIGNAL-REF", "SYSTEM-SIGNAL-REF"],
            );
            record_field(
                src,
                &format!("pdu/{name}/map/{signal}/start"),
                &m,
                &["START-POSITION"],
            );
            record_field(
                src,
                &format!("pdu/{name}/map/{signal}/order"),
                &m,
                &["PACKING-BYTE-ORDER"],
            );
            mappings.push(Mapping {
                start_bit: text_in(&m, &["START-POSITION"])
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0),
                length: text_in(&m, &["LENGTH"])
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0),
                byte_order: text_in(&m, &["PACKING-BYTE-ORDER"]).map(|t| {
                    if t == "MOST-SIGNIFICANT-BYTE-FIRST" {
                        ByteOrder::BigEndian
                    } else {
                        ByteOrder::LittleEndian
                    }
                }),
                signal,
            });
        }
        if let Some(maps) = pdu
            .children()
            .find(|n| n.is_element() && n.has_tag_name("I-SIGNAL-TO-PDU-MAPPINGS"))
        {
            src.record_container(&format!("pdu/{name}/maps"), source::inner_span(&maps));
        }
        out.push(PduDef {
            cycle_ms: read_cycle_ms(&pdu),
            pdu_type: PduType::from_tag(pdu.tag_name().name()).unwrap_or(PduType::Other),
            comment: desc_text(&pdu),
            long_name: long_name_text(&pdu),
            path: path_of(&pdu),
            mappings,
            length: collect(&pdu, "LENGTH")
                .first()
                .and_then(|n| n.text().and_then(|s| s.parse::<u64>().ok()))
                .unwrap_or(8),
            name,
        });
    }
    out
}

/// I-PDU-TIMING 的周期：CYCLIC-TIMING 的 TIME-PERIOD 是秒，换成毫秒
fn read_cycle_ms(pdu: &roxmltree::Node) -> Option<f64> {
    let timing = collect(pdu, "I-PDU-TIMING").into_iter().next()?;
    let period = collect(&timing, "TIME-PERIOD").into_iter().next()?;
    let seconds: f64 = collect(&period, "VALUE")
        .first()
        .and_then(|v| v.text())?
        .parse()
        .ok()?;
    Some(seconds * 1000.0)
}

fn read_pdu_triggerings(
    root: &roxmltree::Node,
    ports: &HashMap<String, (String, bool)>,
    src: &mut ArxmlSource,
) -> (
    HashMap<String, Vec<String>>,
    HashMap<String, Vec<String>>,
    HashMap<String, String>,
) {
    let mut senders: HashMap<String, Vec<String>> = HashMap::new();
    let mut receivers: HashMap<String, Vec<String>> = HashMap::new();
    let mut names: HashMap<String, String> = HashMap::new();
    for t in collect(root, "PDU-TRIGGERING") {
        let Some(r) = text_in(&t, &["I-PDU-REF"]) else {
            continue;
        };
        let pdu = ref_name(&r);
        names.entry(pdu.clone()).or_insert_with(|| short_name(&t));
        record_field(src, &format!("pdu/{pdu}/pdu_ref"), &t, &["I-PDU-REF"]);
        for port in collect(&t, "I-PDU-PORT-REF")
            .into_iter()
            .filter_map(|n| n.text().map(|s| s.trim().to_string()))
        {
            let Some((ecu, is_tx)) = port_side(&port, ports) else {
                continue;
            };
            let list = if is_tx { &mut senders } else { &mut receivers };
            let entry = list.entry(pdu.clone()).or_default();
            if !entry.contains(&ecu) {
                entry.push(ecu);
            }
        }
    }
    (senders, receivers, names)
}

fn read_signal_triggerings(
    root: &roxmltree::Node,
    ports: &HashMap<String, (String, bool)>,
    src: &mut ArxmlSource,
) -> (
    HashMap<String, Vec<String>>,
    HashMap<String, Vec<String>>,
    HashMap<String, String>,
) {
    let mut senders: HashMap<String, Vec<String>> = HashMap::new();
    let mut receivers: HashMap<String, Vec<String>> = HashMap::new();
    let mut names: HashMap<String, String> = HashMap::new();
    for t in collect(root, "I-SIGNAL-TRIGGERING") {
        let Some(r) = text_in(&t, &["I-SIGNAL-REF", "SYSTEM-SIGNAL-REF"]) else {
            continue;
        };
        let signal = ref_name(&r);
        names
            .entry(signal.clone())
            .or_insert_with(|| short_name(&t));
        src.record_element(
            &format!("signal/{signal}/triggering"),
            source::element_span(&t),
        );
        record_field(
            src,
            &format!("signal/{signal}/sig_ref"),
            &t,
            &["I-SIGNAL-REF", "SYSTEM-SIGNAL-REF"],
        );
        for port in collect(&t, "I-SIGNAL-PORT-REF")
            .into_iter()
            .filter_map(|n| n.text().map(|s| s.trim().to_string()))
        {
            let Some((ecu, is_tx)) = port_side(&port, ports) else {
                continue;
            };
            let list = if is_tx { &mut senders } else { &mut receivers };
            let entry = list.entry(signal.clone()).or_default();
            if !entry.contains(&ecu) {
                entry.push(ecu);
            }
        }
    }
    (senders, receivers, names)
}

fn read_frame_triggering(
    t: &roxmltree::Node,
    ports: &HashMap<String, (String, bool)>,
    src: &mut ArxmlSource,
) -> Triggering {
    let name = short_name(t);
    // 标识符与寻址方式写在触发点上：出处记到帧的键下
    let frame = frame_of_triggering(t, &name);
    src.record_element(
        &format!("frame/{frame}/triggering"),
        source::element_span(t),
    );
    record_field(src, &format!("frame/{frame}/frame_ref"), t, &["FRAME-REF"]);
    record_field(
        src,
        &format!("frame/{frame}/identifier"),
        t,
        &["IDENTIFIER"],
    );
    record_field(
        src,
        &format!("frame/{frame}/addressing"),
        t,
        &["CAN-ADDRESSING-MODE"],
    );
    record_field(
        src,
        &format!("frame/{frame}/tx_behavior"),
        t,
        &["CAN-FRAME-TX-BEHAVIOR"],
    );
    record_field(
        src,
        &format!("frame/{frame}/rx_behavior"),
        t,
        &["CAN-FRAME-RX-BEHAVIOR"],
    );
    let mut info = Triggering {
        identifier: text_in(t, &["IDENTIFIER"])
            .and_then(|s| s.parse::<u64>().ok())
            .map(|v| v as u32),
        extended: text_in(t, &["CAN-ADDRESSING-MODE"]).as_deref() == Some("EXTENDED"),
        fd: collect(t, "CAN-FRAME-TX-BEHAVIOR")
            .into_iter()
            .chain(collect(t, "CAN-FRAME-RX-BEHAVIOR"))
            .any(|n| n.text().is_some_and(|s| s.contains("FD"))),
        name,
        ..Default::default()
    };
    info.tx_behavior = text_in(t, &["CAN-FRAME-TX-BEHAVIOR"])
        .as_deref()
        .and_then(CanBehavior::from_text);
    info.rx_behavior = text_in(t, &["CAN-FRAME-RX-BEHAVIOR"])
        .as_deref()
        .and_then(CanBehavior::from_text);
    for port in collect(t, "FRAME-PORT-REF")
        .into_iter()
        .filter_map(|n| n.text().map(|s| s.trim().to_string()))
    {
        let Some((ecu, is_tx)) = port_side(&port, ports) else {
            continue;
        };
        if is_tx {
            if !info.senders.contains(&ecu) {
                info.senders.push(ecu);
            }
        } else if !info.receivers.contains(&ecu) {
            info.receivers.push(ecu);
        }
    }
    info
}

fn frame_of_triggering(t: &roxmltree::Node, trigger_name: &str) -> String {
    text_in(t, &["FRAME-REF"])
        .map(|r| ref_name(&r))
        .unwrap_or_else(|| {
            trigger_name
                .strip_prefix("FT_")
                .or_else(|| trigger_name.strip_suffix("_Trigger"))
                .unwrap_or(trigger_name)
                .to_string()
        })
}

// ------------------------------------------------------------------ 小工具

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

fn desc_text(node: &roxmltree::Node) -> String {
    collect(node, "DESC")
        .first()
        .and_then(|d| {
            d.descendants()
                .find(|n| n.is_element() && (n.has_tag_name("L-2") || n.has_tag_name("L-4")))
                .and_then(|n| n.text().map(|t| t.trim().to_string()))
        })
        .unwrap_or_default()
}

fn long_name_text(node: &roxmltree::Node) -> String {
    collect(node, "LONG-NAME")
        .first()
        .and_then(|d| {
            d.descendants()
                .find(|n| n.is_element() && (n.has_tag_name("L-4") || n.has_tag_name("L-2")))
                .and_then(|n| n.text().map(|t| t.trim().to_string()))
        })
        .unwrap_or_default()
}

/// 引用路径：从根 package 到元素自身，如 `/VehicleProject/CanFrame/Console_1`
fn path_of(node: &roxmltree::Node) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut cur = *node;
    loop {
        if cur.is_element() {
            let name = cur
                .children()
                .find(|n| n.has_tag_name("SHORT-NAME"))
                .and_then(|n| n.text())
                .map(|t| t.trim().to_string());
            if let Some(name) = name {
                parts.push(name);
            }
        }
        match cur.parent() {
            Some(p) => cur = p,
            None => break,
        }
    }
    parts.reverse();
    format!("/{}", parts.join("/"))
}

/// 参考界面里显示的路径：去掉根 package 那一段，如 `/CanFrame/Console_1`
pub fn display_path(path: &str) -> String {
    let mut seg: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if seg.len() > 2 {
        seg.remove(0);
    }
    format!("/{}", seg.join("/"))
}

fn ref_name(reference: &str) -> String {
    reference
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or(reference)
        .trim()
        .to_string()
}

/// 端口引用 → (ECU, 是否发送)：先按连接器里的端口定义查，再回退 `_Tx` / `_Rx` 后缀
fn port_side(port_ref: &str, ports: &HashMap<String, (String, bool)>) -> Option<(String, bool)> {
    if let Some(found) = ports.get(port_ref) {
        return Some(found.clone());
    }
    let trimmed = port_ref.trim_start_matches('/');
    if let Some(found) = ports.get(trimmed) {
        return Some(found.clone());
    }
    let segments: Vec<&str> = port_ref.split('/').filter(|s| !s.is_empty()).collect();
    let last = segments.last()?.to_uppercase();
    let ecu = segments.iter().rev().nth(2)?.to_string();
    if last.ends_with("_TX") {
        Some((ecu, true))
    } else if last.ends_with("_RX") {
        Some((ecu, false))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 样本是客户内部文件、不入库：机器上没有时就跳过这几条测试
    fn read(rel: &str) -> Option<(CanRead, ArxmlSource)> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        let text = std::fs::read_to_string(&path).ok()?;
        let doc = roxmltree::Document::parse(&text).expect("xml parses");
        let mut src = ArxmlSource {
            text: text.clone(),
            ..Default::default()
        };
        let r = parse_can(&doc.root_element(), &mut src);
        Some((r, src))
    }

    #[test]
    fn comfort_reads_real_names_and_types() {
        let Some((r, _src)) = read("can-arxml-sample/Comfort.arxml") else {
            return;
        };
        assert_eq!(r.clusters.len(), 1);
        let c = &r.clusters[0];
        assert_eq!(c.name, "Comfort", "集群名取文件里的，不编 CAN_Cluster");
        assert_eq!(c.baudrate, Some(500000.0));
        assert_eq!(c.channels, vec!["CHNL".to_string()]);
        assert_eq!(c.long_name, "democomfort");
        assert!(c.comment.contains("CAN communication matrix"));

        assert_eq!(r.frames.len(), 18);
        let in_cluster: Vec<&Pdu> = r.pdus.iter().filter(|p| p.cluster == "Comfort").collect();
        assert_eq!(
            in_cluster.len(),
            17,
            "参考界面列 17 个：9 个 I-Signal-I-PDU + 4 个 User-Defined + 4 个 NM"
        );
        let orphan = r
            .pdus
            .iter()
            .find(|p| p.name == "Diag_Request")
            .expect("文件里定义了却没挂到任何帧的 PDU");
        assert_eq!(orphan.cluster, "", "不编一个集群给它");
        assert_eq!(r.ecus.len(), 5);

        let types: HashMap<&str, usize> = in_cluster.iter().fold(HashMap::new(), |mut acc, p| {
            *acc.entry(p.pdu_type.label()).or_insert(0) += 1;
            acc
        });
        assert_eq!(types.get("I-Signal-I-PDU"), Some(&9usize));
        assert_eq!(types.get("User-Defined-PDU"), Some(&4usize));
        assert_eq!(types.get("NM-PDU"), Some(&4usize));

        let console = r
            .pdus
            .iter()
            .find(|p| p.name == "Console_1")
            .expect("PDU 名取文件里的");
        assert_eq!(
            console.cycle_ms,
            Some(20.0),
            "I-PDU-TIMING 的 0.02s 换成 20ms"
        );
        assert_eq!(console.signals.len(), 12);
        assert_eq!(console.senders, vec!["Console".to_string()]);
        assert_eq!(
            console.receivers,
            vec!["DOOR_le".to_string(), "DOOR_ri".to_string()]
        );
        assert_eq!(console.triggering.as_deref(), Some("PT_Console_1"));

        let frame = r
            .frames
            .iter()
            .find(|f| f.name == "Console_1")
            .expect("frame");
        assert_eq!(frame.can.as_ref().unwrap().identifier, 416);
        assert_eq!(frame.triggering.as_deref(), Some("FT_Console_1"));
        assert_eq!(frame.senders, vec!["Console".to_string()]);
        assert_eq!(
            frame.receivers,
            vec!["DOOR_le".to_string(), "DOOR_ri".to_string()]
        );
        assert_eq!(display_path(&frame.path), "/CanFrame/Console_1");

        let diag = r
            .frames
            .iter()
            .find(|f| f.name == "Diag_Request")
            .expect("no-pdu frame");
        assert!(diag.pdus.is_empty(), "文件里这帧没有 PDU 映射，不编一个");

        let ecu = r.ecus.iter().find(|e| e.name == "Console").expect("ecu");
        assert_eq!(ecu.controllers.len(), 1);
        assert_eq!(ecu.controllers[0].name, "CT_CAN00");
        assert_eq!(ecu.controllers[0].min_quanta, Some(15.0));
        assert_eq!(ecu.controllers[0].max_sample, Some(81.25));

        let mirror = console
            .signals
            .iter()
            .find(|s| s.name == "Mirror_l2l")
            .expect("signal");
        assert_eq!(mirror.start_bit, 0);
        assert_eq!(mirror.length_bits, 1);

        // 信号组：每个 ECU 一组 Rx 一组 Tx，共 10 组
        assert_eq!(r.groups.len(), 10);
        let tx = r
            .groups
            .iter()
            .find(|g| g.name == "Console_Tx")
            .expect("Console_Tx");
        assert_eq!(tx.ecu, "Console");
        assert_eq!(tx.cluster, "Comfort");
        assert_eq!(tx.direction, crate::system::GroupDir::Out);
        assert_eq!(
            tx.pdus,
            vec![
                "Console_1".to_string(),
                "Console_2".to_string(),
                "TP_Console".to_string()
            ]
        );
        let rx = r
            .groups
            .iter()
            .find(|g| g.name == "Console_Rx")
            .expect("Console_Rx");
        assert_eq!(rx.direction, crate::system::GroupDir::In);
    }

    #[test]
    fn powertrain_reads_compu_and_units() {
        let Some((r, _src)) = read("can-arxml-sample/PowerTrain.arxml") else {
            return;
        };
        assert_eq!(r.clusters[0].name, "PowerTrain");
        assert_eq!(r.frames.len(), 12);
        let abs = r
            .pdus
            .iter()
            .find(|p| p.name == "ABSdata")
            .expect("ABSdata");
        let speed = abs
            .signals
            .iter()
            .find(|s| s.name == "CarSpeed")
            .expect("CarSpeed");
        assert!((speed.factor - 0.5).abs() < 1e-9);
        assert_eq!(speed.unit, "mph");
        assert!(speed.max > 0.0);
    }

    /// 没改任何东西时，"原文 + 补丁" 必须一个字节都不差
    #[test]
    fn untouched_source_renders_byte_identical() {
        for rel in [
            "can-arxml-sample/Comfort.arxml",
            "can-arxml-sample/PowerTrain.arxml",
        ] {
            let Some((_r, src)) = read(rel) else {
                continue;
            };
            assert!(!src.is_dirty());
            assert_eq!(src.render(), src.text, "{rel} 未改动就该原样");
        }
    }

    /// 每个记下的字段区间里确实是那个值：改一段再读回，值要变
    #[test]
    fn recorded_field_spans_point_at_the_values() {
        let Some((r, mut src)) = read("can-arxml-sample/Comfort.arxml") else {
            return;
        };
        let frame = &r.frames[0];
        let id = frame.can.as_ref().unwrap().identifier;
        assert!(src.edit_field(
            &format!("frame/{}/identifier", frame.name),
            &format!("<IDENTIFIER>{}</IDENTIFIER>", id + 1),
            (id + 1).to_string()
        ));
        let out = src.render();
        let back = roxmltree::Document::parse(&out).expect("patched xml parses");
        let mut src2 = ArxmlSource {
            text: out.clone(),
            ..Default::default()
        };
        let r2 = parse_can(&back.root_element(), &mut src2);
        assert_eq!(
            r2.frames[0].can.as_ref().unwrap().identifier,
            id + 1,
            "改过的标识符要能读回来"
        );
        // 其余部分一个字节没动：416 → 417 位数相同，总长不变
        assert_eq!(out.len(), src.text.len(), "只该换那三个数字");
    }
}
