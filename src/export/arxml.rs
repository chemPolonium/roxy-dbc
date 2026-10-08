//! 把当前 DBC 导出为 AUTOSAR 4.x 风格的 CAN 通信快照
//!
//! 写出的元素：CAN-CLUSTER（含通道与 CAN-FRAME-TRIGGERING）、CAN-FRAME、
//! I-SIGNAL-I-PDU（含 I-SIGNAL-TO-PDU-MAPPING）、I-SIGNAL、COMPU-METHOD、
//! ECU-INSTANCE（含 FRAME-PORT / I-SIGNAL-PORT）。收发关系按端口名后缀
//! `_Tx` / `_Rx` 表达，与本仓库读 FlexRay 快照的方式一致。

use crate::editable_dbc::{EditableDbc, EditableSignal};
use can_dbc::ByteOrder;

/// 顶层 package 名，元素路径都以它为前缀
const PKG: &str = "CanDb";

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 数字文本：整数值不带小数点，其余按最短形式
fn fmt_f64(v: f64) -> String {
    if v.is_finite() && (v - v.round()).abs() < f64::EPSILON && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// 该信号是否需要写 COMPU-METHOD（物理换算、量程或值表）
fn has_compu(sig: &EditableSignal) -> bool {
    sig.factor() != 1.0
        || sig.offset() != 0.0
        || sig.min() != 0.0
        || sig.max() != 0.0
        || !sig.unit().is_empty()
        || !sig.value_descriptions().is_empty()
}

pub fn export_arxml(dbc: &EditableDbc) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<AUTOSAR xmlns=\"http://autosar.org/schema/r4.0\">\n");
    out.push_str("  <AR-PACKAGES>\n");
    out.push_str("    <AR-PACKAGE>\n");
    out.push_str(&format!("      <SHORT-NAME>{PKG}</SHORT-NAME>\n"));
    out.push_str("      <ELEMENTS>\n");

    export_cluster(&mut out, dbc);
    export_ecus(&mut out, dbc);
    export_frames(&mut out, dbc);
    export_pdus(&mut out, dbc);
    export_signals(&mut out, dbc);
    export_signal_triggerings(&mut out, dbc);

    out.push_str("      </ELEMENTS>\n");
    out.push_str("    </AR-PACKAGE>\n");
    out.push_str("  </AR-PACKAGES>\n");
    out.push_str("</AUTOSAR>\n");
    out
}

/// CAN-CLUSTER：一条 CAN 通道，帧触发点里带标识符、寻址方式与 CAN FD 行为
fn export_cluster(out: &mut String, dbc: &EditableDbc) {
    out.push_str("        <CAN-CLUSTER>\n");
    out.push_str("          <SHORT-NAME>Cluster</SHORT-NAME>\n");
    out.push_str("          <CAN-CLUSTER-VARIANTS>\n");
    out.push_str("            <CAN-CLUSTER-CONDITIONAL>\n");
    out.push_str("              <PHYSICAL-CHANNELS>\n");
    out.push_str("                <CAN-PHYSICAL-CHANNEL>\n");
    out.push_str("                  <SHORT-NAME>ChannelA</SHORT-NAME>\n");
    out.push_str("                  <FRAME-TRIGGERINGS>\n");
    for msg in dbc.messages().iter() {
        let name = escape_xml(msg.message_name());
        out.push_str("                    <CAN-FRAME-TRIGGERING>\n");
        out.push_str(&format!(
            "                      <SHORT-NAME>{name}_Trigger</SHORT-NAME>\n"
        ));
        let ports = frame_ports(dbc, msg.message_name());
        if !ports.is_empty() {
            out.push_str("                      <FRAME-PORT-REFS>\n");
            for port in ports {
                out.push_str(&format!(
                    "                        <FRAME-PORT-REF DEST=\"FRAME-PORT\">/{PKG}/{port}</FRAME-PORT-REF>\n"
                ));
            }
            out.push_str("                      </FRAME-PORT-REFS>\n");
        }
        out.push_str(&format!(
            "                      <FRAME-REF DEST=\"CAN-FRAME\">/{PKG}/{name}</FRAME-REF>\n"
        ));
        let addressing = if msg.frame_format().is_extended() {
            "EXTENDED"
        } else {
            "STANDARD"
        };
        out.push_str(&format!(
            "                      <CAN-ADDRESSING-MODE>{addressing}</CAN-ADDRESSING-MODE>\n"
        ));
        // CAN FD 帧收发行为写 CAN-FD，经典帧写 CAN
        let behavior = if msg.frame_format().is_fd() {
            "CAN-FD"
        } else {
            "CAN"
        };
        out.push_str(&format!(
            "                      <CAN-FRAME-TX-BEHAVIOR>{behavior}</CAN-FRAME-TX-BEHAVIOR>\n\
             \x20                     <CAN-FRAME-RX-BEHAVIOR>{behavior}</CAN-FRAME-RX-BEHAVIOR>\n"
        ));
        out.push_str(&format!(
            "                      <IDENTIFIER>{}</IDENTIFIER>\n",
            msg.message_id()
        ));
        out.push_str("                    </CAN-FRAME-TRIGGERING>\n");
    }
    out.push_str("                  </FRAME-TRIGGERINGS>\n");
    out.push_str("                </CAN-PHYSICAL-CHANNEL>\n");
    out.push_str("              </PHYSICAL-CHANNELS>\n");
    out.push_str("            </CAN-CLUSTER-CONDITIONAL>\n");
    out.push_str("          </CAN-CLUSTER-VARIANTS>\n");
    out.push_str("        </CAN-CLUSTER>\n");
}

/// 一帧的收发端口路径：发送者一个 `_Tx`，接收者各一个 `_Rx`
fn frame_ports(dbc: &EditableDbc, message_name: &str) -> Vec<String> {
    let Some(msg) = dbc.get_message_by_name(message_name) else {
        return Vec::new();
    };
    let mut ports = Vec::new();
    let transmitter = msg.transmitter();
    if transmitter != "Vector__XXX" {
        ports.push(format!(
            "{transmitter}/CN_Cluster/FP_{}_Tx",
            escape_xml(message_name)
        ));
    }
    for node in frame_receivers(msg) {
        ports.push(format!(
            "{node}/CN_Cluster/FP_{}_Rx",
            escape_xml(message_name)
        ));
    }
    ports
}

/// 帧的接收节点：帧内各信号接收者的并集，按节点表顺序输出
fn frame_receivers(msg: &crate::editable_dbc::EditableMessage) -> Vec<String> {
    let mut receivers: Vec<String> = Vec::new();
    for sig in msg.signals() {
        for node in sig.receivers() {
            if node != "Vector__XXX" && !receivers.iter().any(|r| r == node) {
                receivers.push(node.clone());
            }
        }
    }
    receivers
}

/// ECU-INSTANCE：FRAME-PORT 表发送关系，I-SIGNAL-PORT 表信号级接收关系
fn export_ecus(out: &mut String, dbc: &EditableDbc) {
    for node in dbc.nodes() {
        if node == "Vector__XXX" {
            continue;
        }
        let ecu = escape_xml(node);
        out.push_str("        <ECU-INSTANCE>\n");
        out.push_str(&format!("          <SHORT-NAME>{ecu}</SHORT-NAME>\n"));
        out.push_str("          <CONNECTORS>\n");
        out.push_str("            <COMM-CONNECTOR>\n");
        out.push_str("              <SHORT-NAME>CN_Cluster</SHORT-NAME>\n");

        let mut frame_ports = Vec::new();
        let mut signal_ports = Vec::new();
        for msg in dbc.messages().iter() {
            let message_name = escape_xml(msg.message_name());
            if msg.transmitter() == node {
                frame_ports.push(format!("FP_{message_name}_Tx"));
            } else if frame_receivers(msg).iter().any(|r| r == node) {
                frame_ports.push(format!("FP_{message_name}_Rx"));
            }
            for sig in msg.signals() {
                if sig.receivers().iter().any(|r| r == node) {
                    signal_ports.push(format!(
                        "PP_{}_Rx",
                        signal_port_name(msg.message_name(), sig.name())
                    ));
                }
            }
        }

        if !frame_ports.is_empty() {
            out.push_str("              <FRAME-PORTS>\n");
            for port in frame_ports {
                out.push_str("                <FRAME-PORT>\n");
                out.push_str(&format!(
                    "                  <SHORT-NAME>{port}</SHORT-NAME>\n"
                ));
                out.push_str("                </FRAME-PORT>\n");
            }
            out.push_str("              </FRAME-PORTS>\n");
        }
        if !signal_ports.is_empty() {
            out.push_str("              <I-SIGNAL-PORTS>\n");
            for port in signal_ports {
                out.push_str("                <I-SIGNAL-PORT>\n");
                out.push_str(&format!(
                    "                  <SHORT-NAME>{port}</SHORT-NAME>\n"
                ));
                out.push_str("                </I-SIGNAL-PORT>\n");
            }
            out.push_str("              </I-SIGNAL-PORTS>\n");
        }

        out.push_str("            </COMM-CONNECTOR>\n");
        out.push_str("          </CONNECTORS>\n");
        out.push_str("        </ECU-INSTANCE>\n");
    }
}

/// 端口里的信号标识：同名信号出现在多帧时带上帧名，避免端口重名
fn signal_port_name(message_name: &str, signal_name: &str) -> String {
    format!("{message_name}_{signal_name}")
}

/// CAN-FRAME：长度与 PDU 映射
fn export_frames(out: &mut String, dbc: &EditableDbc) {
    for msg in dbc.messages().iter() {
        let name = escape_xml(msg.message_name());
        out.push_str("        <CAN-FRAME>\n");
        out.push_str(&format!("          <SHORT-NAME>{name}</SHORT-NAME>\n"));
        if !msg.comment().is_empty() {
            out.push_str(&format!(
                "          <DESC><L-2 L=\"EN\">{}</L-2></DESC>\n",
                escape_xml(msg.comment())
            ));
        }
        out.push_str(&format!(
            "          <FRAME-LENGTH>{}</FRAME-LENGTH>\n",
            msg.message_size()
        ));
        out.push_str("          <PDU-TO-FRAME-MAPPINGS>\n");
        out.push_str("            <PDU-TO-FRAME-MAPPING>\n");
        out.push_str(&format!(
            "              <SHORT-NAME>{name}_PduMapping</SHORT-NAME>\n"
        ));
        out.push_str(&format!(
            "              <PDU-REF DEST=\"I-SIGNAL-I-PDU\">/{PKG}/{name}_PDU</PDU-REF>\n"
        ));
        out.push_str("            </PDU-TO-FRAME-MAPPING>\n");
        out.push_str("          </PDU-TO-FRAME-MAPPINGS>\n");
        out.push_str("        </CAN-FRAME>\n");
    }
}

/// I-SIGNAL-I-PDU：信号映射只写引用与位置，换算留给 I-SIGNAL + COMPU-METHOD
fn export_pdus(out: &mut String, dbc: &EditableDbc) {
    for msg in dbc.messages().iter() {
        let name = escape_xml(msg.message_name());
        out.push_str("        <I-SIGNAL-I-PDU>\n");
        out.push_str(&format!("          <SHORT-NAME>{name}_PDU</SHORT-NAME>\n"));
        out.push_str(&format!(
            "          <LENGTH>{}</LENGTH>\n",
            msg.message_size()
        ));
        if !msg.signals().is_empty() {
            out.push_str("          <I-SIGNAL-TO-PDU-MAPPINGS>\n");
            for sig in msg.signals() {
                out.push_str("            <I-SIGNAL-TO-PDU-MAPPING>\n");
                out.push_str(&format!(
                    "              <SHORT-NAME>{}</SHORT-NAME>\n",
                    escape_xml(sig.name())
                ));
                out.push_str(&format!(
                    "              <I-SIGNAL-REF DEST=\"I-SIGNAL\">/{PKG}/{}</I-SIGNAL-REF>\n",
                    escape_xml(&signal_port_name(msg.message_name(), sig.name()))
                ));
                out.push_str(&format!(
                    "              <START-POSITION>{}</START-POSITION>\n",
                    sig.start_bit()
                ));
                out.push_str(&format!(
                    "              <LENGTH>{}</LENGTH>\n",
                    sig.signal_size()
                ));
                let order = match sig.byte_order() {
                    ByteOrder::LittleEndian => "MOST-SIGNIFICANT-BYTE-LAST",
                    ByteOrder::BigEndian => "MOST-SIGNIFICANT-BYTE-FIRST",
                };
                out.push_str(&format!(
                    "              <PACKING-BYTE-ORDER>{order}</PACKING-BYTE-ORDER>\n"
                ));
                out.push_str("            </I-SIGNAL-TO-PDU-MAPPING>\n");
            }
            out.push_str("          </I-SIGNAL-TO-PDU-MAPPINGS>\n");
        }
        out.push_str("        </I-SIGNAL-I-PDU>\n");
    }
}

/// I-SIGNAL 与 COMPU-METHOD：每个（帧, 信号）写一份，名字带帧名前缀，
/// 这样同名信号在不同帧里的换算与接收者不会互相覆盖
fn export_signals(out: &mut String, dbc: &EditableDbc) {
    for msg in dbc.messages().iter() {
        for sig in msg.signals() {
            let key = escape_xml(&signal_port_name(msg.message_name(), sig.name()));
            out.push_str("        <I-SIGNAL>\n");
            out.push_str(&format!("          <SHORT-NAME>{key}</SHORT-NAME>\n"));
            if !sig.comment().is_empty() {
                out.push_str(&format!(
                    "          <DESC><L-2 L=\"EN\">{}</L-2></DESC>\n",
                    escape_xml(sig.comment())
                ));
            }
            out.push_str(&format!(
                "          <LENGTH>{}</LENGTH>\n",
                sig.signal_size()
            ));
            out.push_str(&format!(
                "          <SIGN-CONVENTION>{}</SIGN-CONVENTION>\n",
                match sig.value_type() {
                    can_dbc::ValueType::Signed => "TWOS_COMPLEMENT",
                    can_dbc::ValueType::Unsigned => "UNSIGNED",
                }
            ));
            if has_compu(sig) {
                out.push_str(&format!(
                    "          <COMPU-METHOD-REF DEST=\"COMPU-METHOD\">/{PKG}/COMPU_{key}</COMPU-METHOD-REF>\n"
                ));
            }
            out.push_str("        </I-SIGNAL>\n");
        }
    }

    for msg in dbc.messages().iter() {
        for sig in msg.signals() {
            if !has_compu(sig) {
                continue;
            }
            let key = escape_xml(&signal_port_name(msg.message_name(), sig.name()));
            out.push_str("        <COMPU-METHOD>\n");
            out.push_str(&format!("          <SHORT-NAME>COMPU_{key}</SHORT-NAME>\n"));
            out.push_str(&format!(
                "          <CATEGORY>{}</CATEGORY>\n",
                if sig.value_descriptions().is_empty() {
                    "LINEAR"
                } else {
                    "TEXTTABLE"
                }
            ));
            out.push_str("          <COMPU-INTERNAL-TO-PHYS>\n");
            out.push_str("            <COMPU-SCALES>\n");
            // 线性刻度在前：导入侧取第一条没有 COMPU-CONST 的刻度读量程与因子
            out.push_str("              <COMPU-SCALE>\n");
            if sig.min() != 0.0 || sig.max() != 0.0 {
                out.push_str(&format!(
                    "                <LOWER-LIMIT INTERVAL-TYPE=\"CLOSED\">{}</LOWER-LIMIT>\n",
                    fmt_f64(sig.min())
                ));
                out.push_str(&format!(
                    "                <UPPER-LIMIT INTERVAL-TYPE=\"CLOSED\">{}</UPPER-LIMIT>\n",
                    fmt_f64(sig.max())
                ));
            }
            out.push_str("                <COMPU-RATIONAL-COEFFS>\n");
            out.push_str("                  <COMPU-NUMERATOR>\n");
            out.push_str(&format!(
                "                    <V>{}</V>\n                    <V>{}</V>\n",
                fmt_f64(sig.offset()),
                fmt_f64(sig.factor())
            ));
            out.push_str("                  </COMPU-NUMERATOR>\n");
            out.push_str("                  <COMPU-DENOMINATOR>\n                    <V>1</V>\n                  </COMPU-DENOMINATOR>\n");
            out.push_str("                </COMPU-RATIONAL-COEFFS>\n");
            if !sig.unit().is_empty() {
                out.push_str(&format!(
                    "                <UNIT>{}</UNIT>\n",
                    escape_xml(sig.unit())
                ));
            }
            out.push_str("              </COMPU-SCALE>\n");
            for (value, desc) in sig.value_descriptions() {
                out.push_str("              <COMPU-SCALE>\n");
                out.push_str(&format!(
                    "                <LOWER-LIMIT INTERVAL-TYPE=\"CLOSED\">{value}</LOWER-LIMIT>\n"
                ));
                out.push_str(&format!(
                    "                <UPPER-LIMIT INTERVAL-TYPE=\"CLOSED\">{value}</UPPER-LIMIT>\n"
                ));
                out.push_str(&format!(
                    "                <COMPU-CONST><VT>{}</VT></COMPU-CONST>\n",
                    escape_xml(desc)
                ));
                out.push_str("              </COMPU-SCALE>\n");
            }
            out.push_str("            </COMPU-SCALES>\n");
            out.push_str("          </COMPU-INTERNAL-TO-PHYS>\n");
            out.push_str("        </COMPU-METHOD>\n");
        }
    }
}

/// I-SIGNAL-TRIGGERING：把信号级收发写进端口引用
fn export_signal_triggerings(out: &mut String, dbc: &EditableDbc) {
    for msg in dbc.messages().iter() {
        for sig in msg.signals() {
            let ports = signal_ports(dbc, msg.message_name(), sig.name());
            if ports.is_empty() {
                continue;
            }
            out.push_str("        <I-SIGNAL-TRIGGERING>\n");
            out.push_str(&format!(
                "          <SHORT-NAME>ST_{}</SHORT-NAME>\n",
                escape_xml(&signal_port_name(msg.message_name(), sig.name()))
            ));
            out.push_str("          <I-SIGNAL-PORT-REFS>\n");
            for port in ports {
                out.push_str(&format!(
                    "            <I-SIGNAL-PORT-REF DEST=\"I-SIGNAL-PORT\">/{PKG}/{port}</I-SIGNAL-PORT-REF>\n"
                ));
            }
            out.push_str("          </I-SIGNAL-PORT-REFS>\n");
            out.push_str(&format!(
                "          <I-SIGNAL-REF DEST=\"I-SIGNAL\">/{PKG}/{}</I-SIGNAL-REF>\n",
                escape_xml(&signal_port_name(msg.message_name(), sig.name()))
            ));
            out.push_str("        </I-SIGNAL-TRIGGERING>\n");
        }
    }
}

/// 某帧内某信号的端口路径（ECU/连接器/端口名）
fn signal_ports(dbc: &EditableDbc, message_name: &str, signal_name: &str) -> Vec<String> {
    let Some(msg) = dbc.get_message_by_name(message_name) else {
        return Vec::new();
    };
    let Some(sig) = msg.signals().iter().find(|s| s.name() == signal_name) else {
        return Vec::new();
    };
    let port = signal_port_name(message_name, signal_name);
    sig.receivers()
        .iter()
        .filter(|r| r.as_str() != "Vector__XXX")
        .map(|r| format!("{}/CN_Cluster/PP_{port}_Rx", escape_xml(r)))
        .collect()
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
                // 值表按数值排序后比较：导出侧写入 COMPU-SCALE 时已排过序
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
}
