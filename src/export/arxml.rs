//! 把当前 DBC 导出为 AUTOSAR 4.3 的 CAN 集群快照
//!
//! 元素形状对齐 AUTOSAR Explorer 自己导出的文件：根节点带 `xsi:schemaLocation`，
//! 内容按类别分进 `AR-PACKAGE`（ECU_INSTANCES / FRAME / PDUS / I_SIGNALS /
//! BASE_TYPES / COMPUMETHODS / UNIT / SYSTEM_SIGNALS）；信号换算挂在
//! SYSTEM-SIGNAL→COMPU-METHOD 上，符号与位宽走 SW-BASE-TYPE，收发关系用 ECU
//! 连接器上的端口加三层 triggering（FRAME→PDU→I-SIGNAL）表达。
//!
//! 不建模：多路复用（MULTIPLEXED-I-PDU）、端到端保护、诊断、波特率。

use crate::editable_dbc::{AttrValue, EditableDbc, EditableMessage, EditableSignal};
use can_dbc::{ByteOrder, ValueType};
use std::collections::HashMap;

/// 顶层 package 名，所有引用路径以它为前缀
const ROOT: &str = "CanDb";

const PKG_SYSTEM: &str = "SYSTEM";
const PKG_CLUSTER: &str = "CAN_Cluster";
const PKG_ECUS: &str = "ECU_INSTANCES";
const PKG_FRAMES: &str = "FRAME";
const PKG_PDUS: &str = "PDUS";
const PKG_ISIGNALS: &str = "I_SIGNALS";
const PKG_BASES: &str = "BASE_TYPES";
const PKG_COMPUS: &str = "COMPUMETHODS";
const PKG_UNITS: &str = "UNIT";
const PKG_SYS_SIGNALS: &str = "SYSTEM_SIGNALS";

const CLUSTER: &str = "CAN_Cluster";
const CHANNEL: &str = "CANChannel";

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// AUTOSAR Identifier 只允许字母、数字、下划线，且不能以数字开头
fn ident(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() {
        out.push('_');
    }
    if out.as_bytes()[0].is_ascii_digit() {
        out.insert(0, 'N');
    }
    out
}

fn fmt_f64(v: f64) -> String {
    if v.is_finite() && (v - v.round()).abs() < f64::EPSILON && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// 毫秒周期换成秒文本：100 → 0.1
fn fmt_secs(ms: f64) -> String {
    let mut t = format!("{:.6}", ms / 1000.0);
    while t.ends_with('0') {
        t.pop();
    }
    if t.ends_with('.') {
        t.pop();
    }
    t
}

fn path(pkg: &str, name: &str) -> String {
    format!("/{ROOT}/{pkg}/{name}")
}

/// triggering 不是 packageable 元素，路径走到通道为止
fn trigger_path(name: &str) -> String {
    format!("/{ROOT}/{PKG_CLUSTER}/{CLUSTER}/{CHANNEL}/{name}")
}

fn byte_order_tag(order: &ByteOrder) -> &'static str {
    match order {
        ByteOrder::BigEndian => "MOST-SIGNIFICANT-BYTE-FIRST",
        _ => "MOST-SIGNIFICANT-BYTE-LAST",
    }
}

/// 元素名去重：同名元素撞车时追加序号，保证引用路径唯一
#[derive(Default)]
struct Names {
    used: HashMap<String, u32>,
}

impl Names {
    fn take(&mut self, wanted: String) -> String {
        let n = self.used.entry(wanted.clone()).or_insert(0);
        *n += 1;
        if *n == 1 {
            wanted
        } else {
            format!("{wanted}_{n}")
        }
    }
}

/// 一帧用到的名字
struct FrameRefs {
    frame: String,
    pdu: String,
    frame_port: String,
    pdu_port: String,
    triggering: String,
    pdu_triggering: String,
}

/// 帧内一条信号用到的名字
struct SigRefs {
    isignal: String,
    system: String,
    compu: String,
    base: String,
    unit: Option<String>,
    tx_port: String,
}

struct BaseType {
    name: String,
    size: u64,
    signed: bool,
}

struct Unit {
    name: String,
    display: String,
}

/// 名字与类型表先整体算一遍：多处引用同一元素时拼出的路径必须完全一致
struct Plan {
    frames: Vec<FrameRefs>,
    sigs: Vec<Vec<SigRefs>>,
    bases: Vec<BaseType>,
    units: Vec<Unit>,
}

fn build_plan(dbc: &EditableDbc) -> Plan {
    let mut names = Names::default();
    let mut bases: Vec<BaseType> = Vec::new();
    let mut units: Vec<Unit> = Vec::new();
    let mut frames = Vec::new();
    let mut sigs = Vec::new();

    for msg in dbc.messages() {
        let base = ident(msg.message_name());
        let frame = names.take(base.clone());
        frames.push(FrameRefs {
            frame: frame.clone(),
            pdu: names.take(format!("{base}_PDU")),
            frame_port: format!("FP_{base}"),
            pdu_port: format!("PP_{base}"),
            triggering: format!("FT_{base}"),
            pdu_triggering: format!("PT_{base}"),
        });

        let mut row = Vec::new();
        for sig in msg.signals() {
            let isignal = names.take(format!("{}_{}", frame, ident(sig.name())));
            let signed = matches!(sig.value_type(), ValueType::Signed);
            let big = matches!(sig.byte_order(), ByteOrder::BigEndian);
            let base_name = format!(
                "BaseType_{}{}_{}",
                if signed { "Int" } else { "Uint" },
                sig.signal_size(),
                if big { "BE" } else { "LE" }
            );
            if !bases.iter().any(|b| b.name == base_name) {
                bases.push(BaseType {
                    name: base_name.clone(),
                    size: sig.signal_size(),
                    signed,
                });
            }
            let unit = if sig.unit().is_empty() {
                None
            } else {
                let name = format!("Unit_{}", ident(sig.unit()));
                if !units.iter().any(|u| u.name == name) {
                    units.push(Unit {
                        name: name.clone(),
                        display: sig.unit().to_string(),
                    });
                }
                Some(name)
            };
            row.push(SigRefs {
                isignal: isignal.clone(),
                system: names.take(format!("{isignal}_S")),
                compu: format!("Encoding_{isignal}"),
                base: base_name,
                unit,
                tx_port: format!("SP_{isignal}_Tx"),
            });
        }
        sigs.push(row);
    }
    bases.sort_by(|a, b| a.name.cmp(&b.name));
    units.sort_by(|a, b| a.name.cmp(&b.name));
    Plan {
        frames,
        sigs,
        bases,
        units,
    }
}

// ---------------------------------------------------------------- XML 写入

struct W {
    s: String,
}

impl W {
    fn indent(&mut self, d: usize) {
        self.s.push_str(&"  ".repeat(d));
    }

    fn text(&mut self, d: usize, tag: &str, value: &str) {
        self.indent(d);
        self.s.push_str(&format!("<{tag}>{value}</{tag}>\n"));
    }

    fn empty(&mut self, d: usize, tag: &str) {
        self.indent(d);
        self.s.push_str(&format!("<{tag}/>\n"));
    }

    fn open(&mut self, d: usize, tag: &str) {
        self.indent(d);
        self.s.push_str(&format!("<{tag}>\n"));
    }

    fn close(&mut self, d: usize, tag: &str) {
        self.indent(d);
        self.s.push_str(&format!("</{tag}>\n"));
    }

    fn reference(&mut self, d: usize, tag: &str, dest: &str, target: &str) {
        self.indent(d);
        self.s
            .push_str(&format!("<{tag} DEST=\"{dest}\">{target}</{tag}>\n"));
    }

    /// 注释：参考文件里的 L-2 一律用 FOR-ALL，不带语言限定
    fn desc(&mut self, d: usize, comment: &str) {
        if comment.is_empty() {
            return;
        }
        self.open(d, "DESC");
        self.indent(d + 1);
        self.s
            .push_str(&format!("<L-2 L=\"FOR-ALL\">{}</L-2>\n", escape(comment)));
        self.close(d, "DESC");
    }

    /// 一个 AR-PACKAGE：`root` 为真时内层装子 package，否则装元素
    fn package<F: FnOnce(&mut W, usize)>(&mut self, d: usize, name: &str, root: bool, body: F) {
        self.open(d, "AR-PACKAGE");
        self.text(d + 1, "SHORT-NAME", name);
        let inner = if root { "AR-PACKAGES" } else { "ELEMENTS" };
        self.open(d + 1, inner);
        body(self, d + 2);
        self.close(d + 1, inner);
        self.close(d, "AR-PACKAGE");
    }
}

// ---------------------------------------------------------------- 入口

pub fn export_arxml(dbc: &EditableDbc) -> String {
    let plan = build_plan(dbc);
    let mut w = W { s: String::new() };
    w.s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    w.s.push_str("<AUTOSAR xsi:schemaLocation=\"http://autosar.org/schema/r4.0 AUTOSAR_4-3-0.xsd\" xmlns=\"http://autosar.org/schema/r4.0\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n");
    w.open(1, "AR-PACKAGES");
    w.package(2, ROOT, true, |w, d| {
        w.open(d, "AR-PACKAGE");
        w.text(d + 1, "SHORT-NAME", PKG_SYSTEM);
        w.open(d + 1, "ELEMENTS");
        w.open(d + 2, "SYSTEM");
        w.text(d + 3, "SHORT-NAME", "SYSTEM");
        w.close(d + 2, "SYSTEM");
        w.close(d + 1, "ELEMENTS");
        w.close(d, "AR-PACKAGE");

        write_cluster(w, d, dbc, &plan);
        write_ecus(w, d, dbc, &plan);
        write_frames(w, d, dbc, &plan);
        write_pdus(w, d, dbc, &plan);
        write_isignals(w, d, dbc, &plan);
        write_base_types(w, d, &plan);
        write_compus(w, d, dbc, &plan);
        write_units(w, d, &plan);
        write_system_signals(w, d, dbc, &plan);
    });
    w.close(1, "AR-PACKAGES");
    w.s.push_str("</AUTOSAR>\n");
    w.s
}

// ---------------------------------------------------------------- 集群

fn write_cluster(w: &mut W, d: usize, dbc: &EditableDbc, plan: &Plan) {
    w.package(d, PKG_CLUSTER, false, |w, d| {
        w.open(d, "CAN-CLUSTER");
        w.text(d + 1, "SHORT-NAME", CLUSTER);
        w.open(d + 1, "CAN-CLUSTER-VARIANTS");
        w.open(d + 2, "CAN-CLUSTER-CONDITIONAL");
        w.open(d + 3, "PHYSICAL-CHANNELS");
        w.open(d + 4, "CAN-PHYSICAL-CHANNEL");
        w.text(d + 5, "SHORT-NAME", CHANNEL);

        let ecus: Vec<String> = dbc
            .nodes()
            .iter()
            .filter(|n| **n != "Vector__XXX")
            .map(|n| ident(n))
            .collect();
        if !ecus.is_empty() {
            w.open(d + 5, "COMM-CONNECTORS");
            for ecu in &ecus {
                w.open(d + 6, "COMMUNICATION-CONNECTOR-REF-CONDITIONAL");
                w.reference(
                    d + 7,
                    "COMMUNICATION-CONNECTOR-REF",
                    "CAN-COMMUNICATION-CONNECTOR",
                    &path(PKG_ECUS, &format!("{ecu}/Connector_{ecu}")),
                );
                w.close(d + 6, "COMMUNICATION-CONNECTOR-REF-CONDITIONAL");
            }
            w.close(d + 5, "COMM-CONNECTORS");
        }

        write_frame_triggerings(w, d + 5, dbc, plan);
        write_signal_triggerings(w, d + 5, dbc, plan);
        write_pdu_triggerings(w, d + 5, dbc, plan);

        w.close(d + 4, "CAN-PHYSICAL-CHANNEL");
        w.close(d + 3, "PHYSICAL-CHANNELS");
        w.close(d + 2, "CAN-CLUSTER-CONDITIONAL");
        w.close(d + 1, "CAN-CLUSTER-VARIANTS");
        w.close(d, "CAN-CLUSTER");
    });
}

fn write_frame_triggerings(w: &mut W, d: usize, dbc: &EditableDbc, plan: &Plan) {
    w.open(d, "FRAME-TRIGGERINGS");
    for (mi, msg) in dbc.messages().iter().enumerate() {
        let f = &plan.frames[mi];
        w.open(d + 1, "CAN-FRAME-TRIGGERING");
        w.text(d + 2, "SHORT-NAME", &f.triggering);
        if msg.transmitter() != "Vector__XXX" {
            let ecu = ident(msg.transmitter());
            w.open(d + 2, "FRAME-PORT-REFS");
            w.reference(
                d + 3,
                "FRAME-PORT-REF",
                "FRAME-PORT",
                &path(PKG_ECUS, &format!("{ecu}/Connector_{ecu}/{}", f.frame_port)),
            );
            w.close(d + 2, "FRAME-PORT-REFS");
        }
        w.reference(d + 2, "FRAME-REF", "CAN-FRAME", &path(PKG_FRAMES, &f.frame));
        w.open(d + 2, "PDU-TRIGGERINGS");
        w.open(d + 3, "PDU-TRIGGERING-REF-CONDITIONAL");
        w.reference(
            d + 4,
            "PDU-TRIGGERING-REF",
            "PDU-TRIGGERING",
            &trigger_path(&f.pdu_triggering),
        );
        w.close(d + 3, "PDU-TRIGGERING-REF-CONDITIONAL");
        w.close(d + 2, "PDU-TRIGGERINGS");
        w.text(
            d + 2,
            "CAN-ADDRESSING-MODE",
            if msg.frame_format().is_extended() {
                "EXTENDED"
            } else {
                "STANDARD"
            },
        );
        w.text(
            d + 2,
            "CAN-FRAME-TX-BEHAVIOR",
            if msg.frame_format().is_fd() {
                "CAN-FD"
            } else {
                "CAN-20"
            },
        );
        w.text(d + 2, "IDENTIFIER", &msg.message_id().to_string());
        w.close(d + 1, "CAN-FRAME-TRIGGERING");
    }
    w.close(d, "FRAME-TRIGGERINGS");
}

fn write_signal_triggerings(w: &mut W, d: usize, dbc: &EditableDbc, plan: &Plan) {
    w.open(d, "I-SIGNAL-TRIGGERINGS");
    for (mi, msg) in dbc.messages().iter().enumerate() {
        let tx_ecu = if msg.transmitter() == "Vector__XXX" {
            None
        } else {
            Some(ident(msg.transmitter()))
        };
        for (si, sig) in msg.signals().iter().enumerate() {
            let r = &plan.sigs[mi][si];
            // 一条信号一条 triggering：发送端口与全部接收端口都列在
            // I-SIGNAL-PORT-REFS 里。按收发拆成多条会让同一父元素下 SHORT-NAME
            // 重复，外部工具会把一条信号列成好几行
            let mut ports: Vec<(String, String)> = Vec::new();
            if let Some(ecu) = &tx_ecu {
                ports.push((ecu.clone(), r.tx_port.clone()));
            }
            for node in sig.receivers() {
                if node != "Vector__XXX" {
                    ports.push((ident(node), format!("SP_{}_Rx", r.isignal)));
                }
            }
            if ports.is_empty() {
                continue;
            }
            w.open(d + 1, "I-SIGNAL-TRIGGERING");
            w.text(d + 2, "SHORT-NAME", &format!("ST_{}", r.isignal));
            w.open(d + 2, "I-SIGNAL-PORT-REFS");
            for (ecu, port) in ports {
                w.reference(
                    d + 3,
                    "I-SIGNAL-PORT-REF",
                    "I-SIGNAL-PORT",
                    &path(PKG_ECUS, &format!("{ecu}/Connector_{ecu}/{port}")),
                );
            }
            w.close(d + 2, "I-SIGNAL-PORT-REFS");
            w.reference(
                d + 2,
                "I-SIGNAL-REF",
                "I-SIGNAL",
                &path(PKG_ISIGNALS, &r.isignal),
            );
            w.close(d + 1, "I-SIGNAL-TRIGGERING");
        }
    }
    w.close(d, "I-SIGNAL-TRIGGERINGS");
}

fn write_pdu_triggerings(w: &mut W, d: usize, dbc: &EditableDbc, plan: &Plan) {
    w.open(d, "PDU-TRIGGERINGS");
    for (mi, msg) in dbc.messages().iter().enumerate() {
        if msg.transmitter() == "Vector__XXX" {
            continue;
        }
        let f = &plan.frames[mi];
        let ecu = ident(msg.transmitter());
        w.open(d + 1, "PDU-TRIGGERING");
        w.text(d + 2, "SHORT-NAME", &f.pdu_triggering);
        w.open(d + 2, "I-PDU-PORT-REFS");
        w.reference(
            d + 3,
            "I-PDU-PORT-REF",
            "I-PDU-PORT",
            &path(PKG_ECUS, &format!("{ecu}/Connector_{ecu}/{}", f.pdu_port)),
        );
        w.close(d + 2, "I-PDU-PORT-REFS");
        w.reference(
            d + 2,
            "I-PDU-REF",
            "I-SIGNAL-I-PDU",
            &path(PKG_PDUS, &f.pdu),
        );
        w.open(d + 2, "I-SIGNAL-TRIGGERINGS");
        for si in 0..msg.signals().len() {
            let r = &plan.sigs[mi][si];
            w.open(d + 3, "I-SIGNAL-TRIGGERING-REF-CONDITIONAL");
            w.reference(
                d + 4,
                "I-SIGNAL-TRIGGERING-REF",
                "I-SIGNAL-TRIGGERING",
                &trigger_path(&format!("ST_{}", r.isignal)),
            );
            w.close(d + 3, "I-SIGNAL-TRIGGERING-REF-CONDITIONAL");
        }
        w.close(d + 2, "I-SIGNAL-TRIGGERINGS");
        w.close(d + 1, "PDU-TRIGGERING");
    }
    w.close(d, "PDU-TRIGGERINGS");
}

// ---------------------------------------------------------------- ECU

fn write_ecus(w: &mut W, d: usize, dbc: &EditableDbc, plan: &Plan) {
    w.package(d, PKG_ECUS, false, |w, d| {
        for node in dbc.nodes() {
            if node == "Vector__XXX" {
                continue;
            }
            let ecu = ident(node);
            w.open(d, "ECU-INSTANCE");
            w.text(d + 1, "SHORT-NAME", &ecu);
            w.empty(d + 1, "ASSOCIATED-COM-I-PDU-GROUP-REFS");
            w.open(d + 1, "COMM-CONTROLLERS");
            w.open(d + 2, "CAN-COMMUNICATION-CONTROLLER");
            w.text(d + 3, "SHORT-NAME", &format!("Controller_{ecu}"));
            w.open(d + 3, "CAN-COMMUNICATION-CONTROLLER-VARIANTS");
            w.open(d + 4, "CAN-COMMUNICATION-CONTROLLER-CONDITIONAL");
            w.open(d + 5, "CAN-CONTROLLER-ATTRIBUTES");
            w.empty(d + 6, "CAN-CONTROLLER-CONFIGURATION");
            w.close(d + 5, "CAN-CONTROLLER-ATTRIBUTES");
            w.close(d + 4, "CAN-COMMUNICATION-CONTROLLER-CONDITIONAL");
            w.close(d + 3, "CAN-COMMUNICATION-CONTROLLER-VARIANTS");
            w.close(d + 2, "CAN-COMMUNICATION-CONTROLLER");
            w.close(d + 1, "COMM-CONTROLLERS");

            w.open(d + 1, "CONNECTORS");
            w.open(d + 2, "CAN-COMMUNICATION-CONNECTOR");
            w.text(d + 3, "SHORT-NAME", &format!("Connector_{ecu}"));
            w.reference(
                d + 3,
                "COMM-CONTROLLER-REF",
                "CAN-COMMUNICATION-CONTROLLER",
                &path(PKG_ECUS, &format!("{ecu}/Controller_{ecu}")),
            );
            w.open(d + 3, "ECU-COMM-PORT-INSTANCES");
            for (mi, msg) in dbc.messages().iter().enumerate() {
                let f = &plan.frames[mi];
                let tx = msg.transmitter() == node;
                let rx = msg
                    .signals()
                    .iter()
                    .any(|s| s.receivers().iter().any(|r| r == node));
                if tx || rx {
                    port(w, d + 4, "FRAME-PORT", &f.frame_port, tx);
                    port(w, d + 4, "I-PDU-PORT", &f.pdu_port, tx);
                }
                for (si, sig) in msg.signals().iter().enumerate() {
                    let r = &plan.sigs[mi][si];
                    if tx {
                        port(w, d + 4, "I-SIGNAL-PORT", &r.tx_port, true);
                    }
                    if sig.receivers().iter().any(|n| n == node) {
                        port(
                            w,
                            d + 4,
                            "I-SIGNAL-PORT",
                            &format!("SP_{}_Rx", r.isignal),
                            false,
                        );
                    }
                }
            }
            w.close(d + 3, "ECU-COMM-PORT-INSTANCES");
            w.close(d + 2, "CAN-COMMUNICATION-CONNECTOR");
            w.close(d + 1, "CONNECTORS");
            w.text(d + 1, "DIAGNOSTIC-ADDRESS", "0");
            w.close(d, "ECU-INSTANCE");
        }
    });
}

fn port(w: &mut W, d: usize, tag: &str, name: &str, out: bool) {
    w.open(d, tag);
    w.text(d + 1, "SHORT-NAME", name);
    w.text(
        d + 1,
        "COMMUNICATION-DIRECTION",
        if out { "OUT" } else { "IN" },
    );
    w.close(d, tag);
}

// ---------------------------------------------------------------- 帧与 PDU

fn write_frames(w: &mut W, d: usize, dbc: &EditableDbc, plan: &Plan) {
    w.package(d, PKG_FRAMES, false, |w, d| {
        for (mi, msg) in dbc.messages().iter().enumerate() {
            let f = &plan.frames[mi];
            w.open(d, "CAN-FRAME");
            w.text(d + 1, "SHORT-NAME", &f.frame);
            w.desc(d + 1, msg.comment());
            w.text(d + 1, "FRAME-LENGTH", &msg.message_size().to_string());
            w.open(d + 1, "PDU-TO-FRAME-MAPPINGS");
            w.open(d + 2, "PDU-TO-FRAME-MAPPING");
            w.text(d + 3, "SHORT-NAME", &format!("{}_Map", f.frame));
            w.text(d + 3, "PACKING-BYTE-ORDER", "MOST-SIGNIFICANT-BYTE-FIRST");
            w.reference(d + 3, "PDU-REF", "I-SIGNAL-I-PDU", &path(PKG_PDUS, &f.pdu));
            w.text(d + 3, "START-POSITION", "7");
            w.close(d + 2, "PDU-TO-FRAME-MAPPING");
            w.close(d + 1, "PDU-TO-FRAME-MAPPINGS");
            w.close(d, "CAN-FRAME");
        }
    });
}

fn write_pdus(w: &mut W, d: usize, dbc: &EditableDbc, plan: &Plan) {
    w.package(d, PKG_PDUS, false, |w, d| {
        for (mi, msg) in dbc.messages().iter().enumerate() {
            let f = &plan.frames[mi];
            w.open(d, "I-SIGNAL-I-PDU");
            w.text(d + 1, "SHORT-NAME", &f.pdu);
            w.text(d + 1, "LENGTH", &msg.message_size().to_string());
            if let Some(ms) = cycle_ms(dbc, msg) {
                w.open(d + 1, "I-PDU-TIMING-SPECIFICATIONS");
                w.open(d + 2, "I-PDU-TIMING");
                w.open(d + 3, "TRANSMISSION-MODE-DECLARATION");
                w.open(d + 4, "TRANSMISSION-MODE-TRUE-TIMING");
                w.open(d + 5, "CYCLIC-TIMING");
                // 顺序按 AUTOSAR 的 CYCLIC-TIMING：TIME-REPETITION? TIME-OFFSET? TIME-PERIOD?
                // 先写 TIME-PERIOD 会被 MATLAB 的 schema 校验直接拒掉
                w.open(d + 6, "TIME-OFFSET");
                w.text(d + 7, "VALUE", "0");
                w.close(d + 6, "TIME-OFFSET");
                w.open(d + 6, "TIME-PERIOD");
                w.text(d + 7, "VALUE", &fmt_secs(ms));
                w.close(d + 6, "TIME-PERIOD");
                w.close(d + 5, "CYCLIC-TIMING");
                w.close(d + 4, "TRANSMISSION-MODE-TRUE-TIMING");
                w.close(d + 3, "TRANSMISSION-MODE-DECLARATION");
                w.close(d + 2, "I-PDU-TIMING");
                w.close(d + 1, "I-PDU-TIMING-SPECIFICATIONS");
            }
            w.open(d + 1, "I-SIGNAL-TO-PDU-MAPPINGS");
            for (si, sig) in msg.signals().iter().enumerate() {
                let r = &plan.sigs[mi][si];
                w.open(d + 2, "I-SIGNAL-TO-I-PDU-MAPPING");
                // 映射名用信号名：读回侧按它取 DBC 信号名，帧内不重复即可
                w.text(d + 3, "SHORT-NAME", &ident(sig.name()));
                w.reference(
                    d + 3,
                    "I-SIGNAL-REF",
                    "I-SIGNAL",
                    &path(PKG_ISIGNALS, &r.isignal),
                );
                w.text(
                    d + 3,
                    "PACKING-BYTE-ORDER",
                    byte_order_tag(sig.byte_order()),
                );
                w.text(d + 3, "START-POSITION", &sig.start_bit().to_string());
                w.close(d + 2, "I-SIGNAL-TO-I-PDU-MAPPING");
            }
            w.close(d + 1, "I-SIGNAL-TO-PDU-MAPPINGS");
            w.close(d, "I-SIGNAL-I-PDU");
        }
    });
}

fn cycle_ms(dbc: &EditableDbc, msg: &EditableMessage) -> Option<f64> {
    match dbc.message_cycle_time(msg.message_id())? {
        AttrValue::Int(v) => (v > 0).then_some(v as f64),
        AttrValue::Float(v) => (v > 0.0).then_some(v),
        AttrValue::Text(t) => t.trim().parse::<f64>().ok().filter(|v| *v > 0.0),
    }
}

// ---------------------------------------------------------------- 信号、类型与换算

fn write_isignals(w: &mut W, d: usize, dbc: &EditableDbc, plan: &Plan) {
    w.package(d, PKG_ISIGNALS, false, |w, d| {
        for (mi, msg) in dbc.messages().iter().enumerate() {
            for (si, sig) in msg.signals().iter().enumerate() {
                let r = &plan.sigs[mi][si];
                w.open(d, "I-SIGNAL");
                w.text(d + 1, "SHORT-NAME", &r.isignal);
                w.desc(d + 1, sig.comment());
                w.text(d + 1, "DATA-TYPE-POLICY", "LEGACY");
                w.text(d + 1, "LENGTH", &sig.signal_size().to_string());
                w.open(d + 1, "NETWORK-REPRESENTATION-PROPS");
                w.open(d + 2, "SW-DATA-DEF-PROPS-VARIANTS");
                w.open(d + 3, "SW-DATA-DEF-PROPS-CONDITIONAL");
                w.reference(
                    d + 4,
                    "BASE-TYPE-REF",
                    "SW-BASE-TYPE",
                    &path(PKG_BASES, &r.base),
                );
                w.close(d + 3, "SW-DATA-DEF-PROPS-CONDITIONAL");
                w.close(d + 2, "SW-DATA-DEF-PROPS-VARIANTS");
                w.close(d + 1, "NETWORK-REPRESENTATION-PROPS");
                w.reference(
                    d + 1,
                    "SYSTEM-SIGNAL-REF",
                    "SYSTEM-SIGNAL",
                    &path(PKG_SYS_SIGNALS, &r.system),
                );
                w.close(d, "I-SIGNAL");
            }
        }
    });
}

fn write_base_types(w: &mut W, d: usize, plan: &Plan) {
    w.package(d, PKG_BASES, false, |w, d| {
        for b in &plan.bases {
            w.open(d, "SW-BASE-TYPE");
            w.text(d + 1, "SHORT-NAME", &b.name);
            w.text(d + 1, "BASE-TYPE-SIZE", &b.size.to_string());
            w.text(
                d + 1,
                "BASE-TYPE-ENCODING",
                if b.signed { "2C" } else { "NONE" },
            );
            w.close(d, "SW-BASE-TYPE");
        }
    });
}

fn write_compus(w: &mut W, d: usize, dbc: &EditableDbc, plan: &Plan) {
    w.package(d, PKG_COMPUS, false, |w, d| {
        for (mi, msg) in dbc.messages().iter().enumerate() {
            for (si, sig) in msg.signals().iter().enumerate() {
                let r = &plan.sigs[mi][si];
                w.open(d, "COMPU-METHOD");
                w.text(d + 1, "SHORT-NAME", &r.compu);
                w.text(
                    d + 1,
                    "CATEGORY",
                    if sig.value_descriptions().is_empty() {
                        "LINEAR"
                    } else {
                        "TEXTTABLE"
                    },
                );
                if let Some(unit) = &r.unit {
                    w.reference(d + 1, "UNIT-REF", "UNIT", &path(PKG_UNITS, unit));
                }
                w.open(d + 1, "COMPU-INTERNAL-TO-PHYS");
                w.open(d + 2, "COMPU-SCALES");
                // 线性刻度在前：导入侧取第一条没有 COMPU-CONST 的刻度读量程与因子
                write_linear_scale(w, d + 3, sig);
                for (value, text) in sig.value_descriptions() {
                    w.open(d + 3, "COMPU-SCALE");
                    w.text(d + 4, "LOWER-LIMIT", &value.to_string());
                    w.text(d + 4, "UPPER-LIMIT", &value.to_string());
                    w.open(d + 4, "COMPU-CONST");
                    w.text(d + 5, "VT", &escape(text));
                    w.close(d + 4, "COMPU-CONST");
                    w.close(d + 3, "COMPU-SCALE");
                }
                w.close(d + 2, "COMPU-SCALES");
                w.close(d + 1, "COMPU-INTERNAL-TO-PHYS");
                w.close(d, "COMPU-METHOD");
            }
        }
    });
}

/// 线性刻度：LOWER/UPPER 写原始值范围，分子是 [偏移, 系数]，分母固定 [0, 1]
fn write_linear_scale(w: &mut W, d: usize, sig: &EditableSignal) {
    let factor = if sig.factor() == 0.0 {
        1.0
    } else {
        sig.factor()
    };
    let scaled = sig.factor() != 1.0 || sig.offset() != 0.0;
    let ranged = sig.min() != 0.0 || sig.max() != 0.0;
    if !scaled && !ranged {
        return;
    }
    w.open(d, "COMPU-SCALE");
    if ranged {
        w.text(
            d + 1,
            "LOWER-LIMIT",
            &fmt_f64((sig.min() - sig.offset()) / factor),
        );
        w.text(
            d + 1,
            "UPPER-LIMIT",
            &fmt_f64((sig.max() - sig.offset()) / factor),
        );
    }
    if scaled {
        w.open(d + 1, "COMPU-RATIONAL-COEFFS");
        w.open(d + 2, "COMPU-NUMERATOR");
        w.text(d + 3, "V", &fmt_f64(sig.offset()));
        w.text(d + 3, "V", &fmt_f64(factor));
        w.close(d + 2, "COMPU-NUMERATOR");
        w.open(d + 2, "COMPU-DENOMINATOR");
        w.text(d + 3, "V", "0");
        w.text(d + 3, "V", "1");
        w.close(d + 2, "COMPU-DENOMINATOR");
        w.close(d + 1, "COMPU-RATIONAL-COEFFS");
    }
    w.close(d, "COMPU-SCALE");
}

fn write_units(w: &mut W, d: usize, plan: &Plan) {
    w.package(d, PKG_UNITS, false, |w, d| {
        for u in &plan.units {
            w.open(d, "UNIT");
            w.text(d + 1, "SHORT-NAME", &u.name);
            w.text(d + 1, "DISPLAY-NAME", &escape(&u.display));
            w.close(d, "UNIT");
        }
    });
}

fn write_system_signals(w: &mut W, d: usize, dbc: &EditableDbc, plan: &Plan) {
    w.package(d, PKG_SYS_SIGNALS, false, |w, d| {
        for (mi, msg) in dbc.messages().iter().enumerate() {
            for (si, sig) in msg.signals().iter().enumerate() {
                let r = &plan.sigs[mi][si];
                w.open(d, "SYSTEM-SIGNAL");
                w.text(d + 1, "SHORT-NAME", &r.system);
                w.desc(d + 1, sig.comment());
                w.open(d + 1, "PHYSICAL-PROPS");
                w.open(d + 2, "SW-DATA-DEF-PROPS-VARIANTS");
                w.open(d + 3, "SW-DATA-DEF-PROPS-CONDITIONAL");
                w.reference(
                    d + 4,
                    "COMPU-METHOD-REF",
                    "COMPU-METHOD",
                    &path(PKG_COMPUS, &r.compu),
                );
                w.close(d + 3, "SW-DATA-DEF-PROPS-CONDITIONAL");
                w.close(d + 2, "SW-DATA-DEF-PROPS-VARIANTS");
                w.close(d + 1, "PHYSICAL-PROPS");
                w.close(d, "SYSTEM-SIGNAL");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::export_arxml;
    use crate::editable_dbc::{EditableDbc, EditableMessage, EditableSignal, FrameFormat};
    use can_dbc::{ByteOrder, Dbc, ValueType};

    const MOTBUS: &str = include_str!("../../dbc-sample/motbus.dbc");

    fn load(text: &str) -> EditableDbc {
        EditableDbc::from_dbc(&Dbc::try_from(text).expect("sample dbc should parse"))
    }

    fn sorted(list: &[String]) -> Vec<String> {
        let mut v = list.to_vec();
        v.sort();
        v
    }

    /// 导出 ARXML 再读回来：标识符、字节序、符号、因子/偏移、量程、单位、值表、
    /// 收发节点都不应丢
    #[test]
    fn motbus_round_trip_keeps_scaling_and_receivers() {
        let orig = load(MOTBUS);
        let xml = export_arxml(&orig);
        let back = crate::import::arxml::parse_arxml(&xml).expect("re-import should parse");

        assert_eq!(back.messages().len(), orig.messages().len(), "报文数变了");
        assert_eq!(sorted(back.nodes()), sorted(orig.nodes()), "节点表变了");
        assert!(
            orig.messages().iter().any(|m| m
                .signals()
                .iter()
                .any(|s| !s.value_descriptions().is_empty())),
            "样本应含值表，否则此测试无意义"
        );

        for msg in orig.messages() {
            let m = back
                .get_message_by_name(msg.message_name())
                .unwrap_or_else(|| panic!("丢了报文 {}", msg.message_name()));
            assert_eq!(m.message_id(), msg.message_id(), "ID 变了");
            assert_eq!(m.message_size(), msg.message_size());
            assert_eq!(m.transmitter(), msg.transmitter(), "发送节点变了");
            assert_eq!(m.signals().len(), msg.signals().len(), "信号数变了");
            for sig in msg.signals() {
                let s = m
                    .signals()
                    .iter()
                    .find(|s| s.name() == sig.name())
                    .unwrap_or_else(|| panic!("丢了信号 {}", sig.name()));
                assert_eq!(s.start_bit(), sig.start_bit(), "{} 起始位", sig.name());
                assert_eq!(s.signal_size(), sig.signal_size(), "{} 长度", sig.name());
                assert_eq!(s.byte_order(), sig.byte_order(), "{} 字节序", sig.name());
                assert_eq!(s.value_type(), sig.value_type(), "{} 符号", sig.name());
                assert!(
                    (s.factor() - sig.factor()).abs() < 1e-9,
                    "{} 因子 {} != {}",
                    sig.name(),
                    s.factor(),
                    sig.factor()
                );
                assert!(
                    (s.offset() - sig.offset()).abs() < 1e-9,
                    "{} 偏移 {} != {}",
                    sig.name(),
                    s.offset(),
                    sig.offset()
                );
                assert!(
                    (s.min() - sig.min()).abs() < 1e-9 && (s.max() - sig.max()).abs() < 1e-9,
                    "{} 量程 [{},{}] != [{},{}]",
                    sig.name(),
                    s.min(),
                    s.max(),
                    sig.min(),
                    sig.max()
                );
                assert_eq!(s.unit(), sig.unit(), "{} 单位", sig.name());
                let sort_vals = |v: &[(i64, String)]| {
                    let mut c = v.to_vec();
                    c.sort();
                    c
                };
                assert_eq!(
                    sort_vals(s.value_descriptions()),
                    sort_vals(sig.value_descriptions()),
                    "{} 值表",
                    sig.name()
                );
                assert_eq!(
                    sorted(s.receivers()),
                    sorted(sig.receivers()),
                    "{} 接收节点",
                    sig.name()
                );
            }
        }
    }

    /// 一条信号一条 I-SIGNAL-TRIGGERING：把发送方与每个接收方拆成多条，会让同一
    /// 父元素下 SHORT-NAME 重复，外部工具把一条信号列成好几行
    #[test]
    fn one_triggering_per_signal_with_all_ports() {
        let dbc = load(MOTBUS);
        let xml = export_arxml(&dbc);
        let mut expected = 0usize;
        for m in dbc.messages() {
            let has_tx = m.transmitter() != "Vector__XXX";
            for s in m.signals() {
                if has_tx || !s.receivers().is_empty() {
                    expected += 1;
                }
            }
        }
        let blocks: Vec<&str> = xml
            .split("<I-SIGNAL-TRIGGERING>")
            .skip(1)
            .filter_map(|b| b.split("</I-SIGNAL-TRIGGERING>").next())
            .collect();
        assert_eq!(blocks.len(), expected, "triggering 条数应等于信号数");
        let mut names: Vec<&str> = blocks
            .iter()
            .filter_map(|b| b.split("<SHORT-NAME>").nth(1))
            .filter_map(|b| b.split("</SHORT-NAME>").next())
            .collect();
        names.sort_unstable();
        let unique = names.iter().collect::<std::collections::HashSet<_>>().len();
        assert_eq!(unique, names.len(), "I-SIGNAL-TRIGGERING 名字重复");
        assert!(
            blocks
                .iter()
                .any(|b| b.matches("<I-SIGNAL-PORT-REF ").count() >= 3),
            "多接收方信号的端口应集中在同一条 triggering 里"
        );
    }

    /// 帧注释、扩展帧与 CAN FD 通过端口与行为标志往返
    #[test]
    fn comments_extended_and_fd_survive_round_trip() {
        let sig = EditableSignal::build(
            "Speed".to_string(),
            0,
            16,
            ByteOrder::LittleEndian,
            ValueType::Unsigned,
            0.1,
            0.0,
            0.0,
            250.0,
            "km/h".to_string(),
            vec!["ECU2".to_string()],
            vec![(0, "Off".to_string()), (1, "On".to_string())],
            "车速".to_string(),
        );
        let msg = EditableMessage::build(
            0x18FFAA00,
            FrameFormat::ExtendedFd,
            "EngineData".to_string(),
            16,
            "ECU1".to_string(),
            vec![sig],
            "发动机数据".to_string(),
        );
        let dbc =
            EditableDbc::from_imported(vec![msg], vec!["ECU1".to_string(), "ECU2".to_string()]);

        let xml = export_arxml(&dbc);
        assert!(xml.contains("<CAN-ADDRESSING-MODE>EXTENDED</CAN-ADDRESSING-MODE>"));
        assert!(xml.contains("<CAN-FRAME-TX-BEHAVIOR>CAN-FD</CAN-FRAME-TX-BEHAVIOR>"));
        assert!(xml.contains("<FRAME-PORT-REF"));
        assert!(xml.contains("<I-SIGNAL-PORT-REF"));

        let back = crate::import::arxml::parse_arxml(&xml).expect("re-import should parse");
        let m = back.get_message_by_name("EngineData").expect("EngineData");
        assert_eq!(m.message_id(), 0x18FF_AA00);
        assert!(m.is_extended());
        assert!(m.frame_format().is_fd(), "CAN FD 行为标志没读回来");
        assert_eq!(m.transmitter(), "ECU1");
        assert_eq!(m.comment(), "发动机数据");
        let s = &m.signals()[0];
        assert_eq!(s.comment(), "车速");
        assert_eq!(sorted(s.receivers()), vec!["ECU2".to_string()]);
        assert_eq!(s.value_descriptions().len(), 2);
        assert_eq!(s.unit(), "km/h");
    }

    /// CYCLIC-TIMING 的子元素必须按 schema 给的顺序写：先 TIME-OFFSET 再 TIME-PERIOD。
    /// 反过来的话 MATLAB 直接报 "element 'TIME-OFFSET' is not allowed for content model"
    #[test]
    fn cyclic_timing_writes_offset_before_period() {
        let xml = export_arxml(&load(MOTBUS));
        let offset = xml.find("<TIME-OFFSET>").expect("导出应含 TIME-OFFSET");
        let period = xml.find("<TIME-PERIOD>").expect("导出应含 TIME-PERIOD");
        assert!(
            offset < period,
            "CYCLIC-TIMING 里 TIME-OFFSET 要在 TIME-PERIOD 之前"
        );
    }
}
