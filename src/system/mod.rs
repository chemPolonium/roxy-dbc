//! 统一系统模型：一个文档里同时装 CAN 与 FlexRay 集群
//!
//! 这一层按 AUTOSAR 的对象层次建模，而不是按 DBC 或 FIBEX 的文件形状建模：
//! `SYSTEM` 下面若干 `Cluster`（`BusKind::Can` 或 `FlexRay`），帧把 PDU 按起始
//! 字节位置映射进来（`Frame::pdus`），信号住在 PDU 里。因此
//! 一帧一个 PDU、一帧多个 PDU、一个 PDU 被多帧引用这三种对应关系都能表达。
//!
//! 读进来时 [`can_read`] 顺带记下每个对象、每个字段在原文里的字节区间
//! （[`source::ArxmlSource`]）；保存只替换改过的那一段，文件里本工具不建模的
//! 内容原样留在磁盘上。FlexRay 集群经 [`Self::push_fibex`] 并入，编辑仍走
//! FIBEX 窗口那一套。

pub mod can_read;
pub mod edit;
pub mod source;
pub mod validate;
pub mod window;

pub use crate::fibex::editable_fibex::{ByteOrder, ValueType};
use crate::fibex::editable_fibex::{
    ClusterParams, EcuKeySlot, EditableFibex, FramePduMapping, FrameTriggering, PduKind,
};

/// 系统窗口中间那栏的页签。FlexRay 集群才有后两个（排程与集群参数）。
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum SysTab {
    #[default]
    Frames,
    Pdus,
    Signals,
    Schedule,
    ClusterParams,
    CommMatrix,
    SignalGroups,
}

/// I-SIGNAL-I-PDU-GROUP 的通信方向
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GroupDir {
    #[default]
    None,
    In,
    Out,
}

impl GroupDir {
    pub fn label(self) -> &'static str {
        match self {
            GroupDir::None => "-",
            GroupDir::In => "RX",
            GroupDir::Out => "TX",
        }
    }
}

/// 信号组（I-SIGNAL-I-PDU-GROUP）：一个 ECU 收发的一组 PDU
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SignalGroup {
    pub name: String,
    pub cluster: String,
    /// 关联的 ECU（ECU-INSTANCE 的 ASSOCIATED-COM-I-PDU-GROUP-REFS）
    pub ecu: String,
    pub direction: GroupDir,
    pub pdus: Vec<String>,
}

/// 总线种类
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BusKind {
    Can,
    FlexRay,
}

impl BusKind {
    pub fn label(self) -> &'static str {
        match self {
            BusKind::Can => "CAN",
            BusKind::FlexRay => "FlexRay",
        }
    }
}

/// CAN 帧的收发行为（AUTOSAR `CAN-FRAME-*-BEHAVIOR`）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CanBehavior {
    #[default]
    Can20,
    CanFd,
}

impl CanBehavior {
    pub fn as_str(self) -> &'static str {
        match self {
            CanBehavior::Can20 => "CAN-20",
            CanBehavior::CanFd => "CAN-FD",
        }
    }

    /// 读 ARXML 的 Tx/Rx 行为文本
    pub fn from_text(text: &str) -> Option<Self> {
        match text.trim().to_ascii_uppercase().as_str() {
            "CAN-20" | "CAN" => Some(CanBehavior::Can20),
            "CAN-FD" | "CANFD" => Some(CanBehavior::CanFd),
            _ => None,
        }
    }
}

/// AUTOSAR 的 PDU 种类：真实文件里一帧可以映射到不带信号的 PDU（诊断、网络管理），
/// 参考界面（Vector AUTOSAR Explorer）把它们单独列成 PDU Type 一列
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PduType {
    #[default]
    ISignalIPdu,
    UserDefined,
    Nm,
    Container,
    Secured,
    Multiplexed,
    Other,
}

impl PduType {
    pub fn label(self) -> &'static str {
        match self {
            PduType::ISignalIPdu => "I-Signal-I-PDU",
            PduType::UserDefined => "User-Defined-PDU",
            PduType::Nm => "NM-PDU",
            PduType::Container => "Container-I-PDU",
            PduType::Secured => "Secured-I-PDU",
            PduType::Multiplexed => "Multiplexed-I-PDU",
            PduType::Other => "Other",
        }
    }

    /// 元素标签到种类；不认识的标签返回 None，由调用方决定当 Other 还是跳过
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "I-SIGNAL-I-PDU" => Some(PduType::ISignalIPdu),
            "USER-DEFINED-PDU" => Some(PduType::UserDefined),
            "NM-PDU" => Some(PduType::Nm),
            "CONTAINER-I-PDU" => Some(PduType::Container),
            "SECURED-I-PDU" => Some(PduType::Secured),
            "MULTIPLEXED-I-PDU" => Some(PduType::Multiplexed),
            _ => None,
        }
    }
}

/// CAN 帧特有字段（AUTOSAR Explorer 右侧 CAN Properties 那一组）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct CanFrameProps {
    pub identifier: u32,
    pub extended: bool,
    /// 报文本身是否为 FD 帧（决定 `FrameFormat` 与默认行为取值）
    pub fd: bool,
    pub tx_behavior: CanBehavior,
    pub rx_behavior: CanBehavior,
    /// 可选项：文件里没有就留 `None`，不编造
    pub lower_can_id: Option<u32>,
    pub upper_can_id: Option<u32>,
    pub rx_mask: Option<u32>,
    pub tx_mask: Option<u32>,
}

/// 一个总线集群
#[derive(Clone, Debug, PartialEq)]
pub struct Cluster {
    pub name: String,
    pub kind: BusKind,
    /// CAN 一般一条通道；FlexRay 为 A、B
    pub channels: Vec<String>,
    /// CAN 波特率 bit/s，文件里没有就留 None
    pub baudrate: Option<f64>,
    /// FlexRay 的 g*/gd* 参数，CAN 集群用不到但保留默认值
    pub fr: ClusterParams,
    /// LONG-NAME 里的文本，文件里没有就空
    pub long_name: String,
    /// DESC 里的文本
    pub comment: String,
}

impl Default for Cluster {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: BusKind::Can,
            channels: Vec::new(),
            baudrate: None,
            fr: ClusterParams::default(),
            long_name: String::new(),
            comment: String::new(),
        }
    }
}

/// PDU 里的一条信号。起始位是 PDU 内的位位置，落到帧上还要加 PDU 的字节偏移。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Signal {
    pub name: String,
    pub start_bit: u32,
    pub length_bits: u32,
    pub byte_order: ByteOrder,
    pub value_type: ValueType,
    pub factor: f64,
    pub offset: f64,
    pub min: f64,
    pub max: f64,
    pub unit: String,
    pub senders: Vec<String>,
    pub receivers: Vec<String>,
    pub comment: String,
    pub value_descriptions: Vec<(i64, String)>,
    /// LONG-NAME 里的文本
    pub long_name: String,
    /// 在 ARXML 里的引用路径，如 `/VehicleProject/ISignal/Active`；非 ARXML 来源为空
    pub path: String,
    /// 换算方法名（COMPU-METHOD 的 SHORT-NAME），文件里没有就空
    pub compu_name: String,
    /// INIT-VALUE 的第一个数值，文件里没有就 None
    pub init_value: Option<f64>,
    /// I-SIGNAL-TRIGGERING 的名字，文件里没有就 None
    pub triggering: Option<String>,
}

/// PDU：一段独立缩放与接收关系的信号集合
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pdu {
    pub name: String,
    pub cluster: String,
    pub length_bytes: u32,
    pub kind: PduKind,
    /// 周期发送时间，毫秒；来自 AUTOSAR 的 I-PDU-TIMING 或 DBC 的 GenMsgCycleTime
    pub cycle_ms: Option<f64>,
    pub signals: Vec<Signal>,
    pub senders: Vec<String>,
    pub receivers: Vec<String>,
    pub comment: String,
    /// AUTOSAR 的 PDU 种类；DBC / FIBEX 来源没有这个概念，按带信号的静态 PDU 处理
    pub pdu_type: PduType,
    pub long_name: String,
    pub path: String,
    /// PDU-TRIGGERING 的名字，文件里没有就 None
    pub triggering: Option<String>,
}

/// 帧：按字节位置映射若干 PDU
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub name: String,
    pub cluster: String,
    pub length_bytes: u32,
    /// FlexRay 的 payload preamble 开关
    pub payload_preamble: bool,
    /// 帧与 PDU 的对应关系：一项 = 一个 PDU 及其在帧内的起始字节
    pub pdus: Vec<FramePduMapping>,
    /// FlexRay 的每通道时隙
    pub triggerings: Vec<FrameTriggering>,
    /// CAN 集群的帧才有
    pub can: Option<CanFrameProps>,
    pub senders: Vec<String>,
    pub receivers: Vec<String>,
    pub comment: String,
    pub long_name: String,
    pub path: String,
    /// CAN-FRAME-TRIGGERING 的名字，文件里没有就 None
    pub triggering: Option<String>,
}

/// ECU 的通信控制器：参考界面里 CAN Controller Requirements 那一组
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EcuController {
    pub name: String,
    /// 每比特最少 / 最多时间量子数
    pub min_quanta: Option<f64>,
    pub max_quanta: Option<f64>,
    /// 采样点百分比下限 / 上限
    pub min_sample: Option<f64>,
    pub max_sample: Option<f64>,
}

/// ECU 实例：挂在哪些集群上、有没有配置关键时隙
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ecu {
    pub name: String,
    pub clusters: Vec<String>,
    pub comment: String,
    pub key_slot: Option<EcuKeySlot>,
    pub long_name: String,
    pub path: String,
    pub controllers: Vec<EcuController>,
}

/// 一个系统描述文件
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SystemDoc {
    pub name: String,
    pub clusters: Vec<Cluster>,
    pub ecus: Vec<Ecu>,
    pub pdus: Vec<Pdu>,
    pub frames: Vec<Frame>,
    pub groups: Vec<SignalGroup>,
}

impl SystemDoc {
    pub fn clusters_of(&self, kind: BusKind) -> Vec<&Cluster> {
        self.clusters.iter().filter(|c| c.kind == kind).collect()
    }

    /// 读一份 ARXML：CAN 与 FlexRay 两边各读各的再合进同一个文档，同时把原文与
    /// 各处字节区间一起交出来（保存时只替换改过的那一段）。混合文件里两边的 PDU
    /// 标签相同，靠帧的种类分流（见两个读取器各自的过滤）。两边都读不出来才算
    /// 失败；只读出一边时另一边不报错，因为纯 CAN / 纯 FlexRay 文件本来就是合法输入。
    pub fn from_arxml_with_source(content: &str) -> Result<(Self, source::ArxmlSource), String> {
        let tree = roxmltree::Document::parse(content).map_err(|e| e.to_string())?;
        let root = tree.root_element();
        let mut src = source::ArxmlSource {
            text: content.to_string(),
            ..Default::default()
        };
        let can = can_read::parse_can(&root, &mut src);
        let mut doc = Self {
            name: "SYSTEM".to_string(),
            clusters: can.clusters,
            ecus: can.ecus,
            pdus: can.pdus,
            frames: can.frames,
            groups: can.groups,
        };
        match crate::fibex::import::arxml::parse_arxml_doc(&tree) {
            Ok(fibex) if !fibex.frames().is_empty() || !fibex.pdus().is_empty() => {
                doc.push_fibex(&fibex);
            }
            Ok(_) => {}
            Err(e) => {
                // FlexRay 侧读不出不影响纯 CAN 文件；两边都空才在下面报错
                log::debug!("FlexRay side of ARXML not read: {e}");
            }
        }
        if doc.clusters.is_empty() {
            return Err("文件里既没有 CAN 帧也没有 FlexRay 帧".to_string());
        }
        Ok((doc, src))
    }

    // ---------------------------------------------------------------- FlexRay

    /// 从 FlexRay 模型建系统文档，追加为一个新的 FlexRay 集群
    pub fn push_fibex(&mut self, fibex: &EditableFibex) {
        let cluster = fibex.cluster().name.clone();
        if !self.clusters.iter().any(|c| c.name == cluster) {
            self.clusters.push(Cluster {
                name: cluster.clone(),
                kind: BusKind::FlexRay,
                channels: vec!["A".to_string(), "B".to_string()],
                baudrate: None,
                fr: fibex.cluster().params,
                ..Default::default()
            });
        }
        for pdu in fibex.pdus() {
            self.pdus.push(Pdu {
                name: pdu.name().to_string(),
                cluster: cluster.clone(),
                length_bytes: pdu.length(),
                kind: pdu.kind(),
                cycle_ms: None,
                signals: pdu
                    .signals()
                    .iter()
                    .map(|s| Signal {
                        name: s.name().to_string(),
                        start_bit: s.start_bit(),
                        length_bits: s.length_bits(),
                        byte_order: s.byte_order(),
                        value_type: s.value_type(),
                        factor: s.factor(),
                        offset: s.offset(),
                        min: s.min(),
                        max: s.max(),
                        unit: s.unit().to_string(),
                        senders: s.senders().to_vec(),
                        receivers: s.receivers().to_vec(),
                        comment: s.comment().to_string(),
                        value_descriptions: s.value_descriptions().to_vec(),
                        ..Default::default()
                    })
                    .collect(),
                senders: pdu.senders().to_vec(),
                receivers: pdu.receivers().to_vec(),
                comment: pdu.comment().to_string(),
                ..Default::default()
            });
        }
        for frame in fibex.frames() {
            self.frames.push(Frame {
                name: frame.name().to_string(),
                cluster: cluster.clone(),
                length_bytes: frame.length(),
                payload_preamble: frame.payload_preamble(),
                pdus: frame.pdus().to_vec(),
                triggerings: frame.triggerings().to_vec(),
                can: None,
                senders: frame
                    .pdus()
                    .iter()
                    .filter_map(|m| {
                        self.pdus
                            .iter()
                            .find(|p| p.name == m.pdu_name)
                            .and_then(|p| p.senders.first().cloned())
                    })
                    .collect(),
                receivers: Vec::new(),
                comment: frame.comment().to_string(),
                ..Default::default()
            });
        }
        for ecu in fibex.ecus() {
            match self.ecus.iter_mut().find(|e| &e.name == ecu) {
                Some(existing) => {
                    if !existing.clusters.contains(&cluster) {
                        existing.clusters.push(cluster.clone());
                    }
                }
                None => self.ecus.push(Ecu {
                    name: ecu.clone(),
                    clusters: vec![cluster.clone()],
                    comment: String::new(),
                    key_slot: fibex.ecu_key_slot(ecu),
                    ..Default::default()
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(content: &str) -> SystemDoc {
        SystemDoc::from_arxml_with_source(content).expect("parse").0
    }

    fn names(doc: &SystemDoc, kind: BusKind) -> Vec<&str> {
        doc.clusters_of(kind)
            .iter()
            .map(|c| c.name.as_str())
            .collect()
    }

    /// 混合文件：CAN 与 FlexRay 集群并列，还带着本工具不建模的 NM 包与 ADMIN-DATA
    const MIXED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<AUTOSAR xmlns="http://autosar.org/schema/r4.0">
 <AR-PACKAGES><AR-PACKAGE><SHORT-NAME>Mixed</SHORT-NAME><ELEMENTS>
  <CAN-FRAME><SHORT-NAME>FrmCan</SHORT-NAME>
   <ADMIN-DATA><SDGS><SDG GID="DV:ToolName">Vector Legacy Database Converter</SDG></SDGS></ADMIN-DATA>
   <FRAME-LENGTH>8</FRAME-LENGTH>
   <PDU-TO-FRAME-MAPPINGS><PDU-TO-FRAME-MAPPING><SHORT-NAME>MapCan</SHORT-NAME>
    <PDU-REF DEST="I-SIGNAL-I-PDU">/Mixed/PduCan</PDU-REF>
   </PDU-TO-FRAME-MAPPING></PDU-TO-FRAME-MAPPINGS></CAN-FRAME>
  <FLEXRAY-FRAME><SHORT-NAME>FrmFr</SHORT-NAME><FRAME-LENGTH>8</FRAME-LENGTH>
   <FRAME-PDU-MAPPINGS><FRAME-PDU-MAPPING><SHORT-NAME>MapFr</SHORT-NAME>
    <PDU-REF DEST="I-SIGNAL-I-PDU">/Mixed/PduFr</PDU-REF><START-POSITION>0</START-POSITION>
   </FRAME-PDU-MAPPING></FRAME-PDU-MAPPINGS></FLEXRAY-FRAME>
  <I-SIGNAL-I-PDU><SHORT-NAME>PduCan</SHORT-NAME><LENGTH>8</LENGTH>
   <I-SIGNAL-TO-PDU-MAPPINGS><I-SIGNAL-TO-I-PDU-MAPPING><SHORT-NAME>SigCan</SHORT-NAME>
    <I-SIGNAL-REF DEST="I-SIGNAL">/Mixed/SigCan</I-SIGNAL-REF><START-POSITION>0</START-POSITION>
    <PACKING-BYTE-ORDER>MOST-SIGNIFICANT-BYTE-LAST</PACKING-BYTE-ORDER>
   </I-SIGNAL-TO-I-PDU-MAPPING></I-SIGNAL-TO-PDU-MAPPINGS></I-SIGNAL-I-PDU>
  <I-SIGNAL-I-PDU><SHORT-NAME>PduFr</SHORT-NAME><LENGTH>8</LENGTH>
   <I-SIGNAL-TO-PDU-MAPPINGS><I-SIGNAL-TO-I-PDU-MAPPING><SHORT-NAME>SigFr</SHORT-NAME>
    <I-SIGNAL-REF DEST="I-SIGNAL">/Mixed/SigFr</I-SIGNAL-REF><START-POSITION>0</START-POSITION>
    <PACKING-BYTE-ORDER>MOST-SIGNIFICANT-BYTE-FIRST</PACKING-BYTE-ORDER>
   </I-SIGNAL-TO-I-PDU-MAPPING></I-SIGNAL-TO-PDU-MAPPINGS></I-SIGNAL-I-PDU>
  <I-SIGNAL><SHORT-NAME>SigCan</SHORT-NAME><LENGTH>8</LENGTH></I-SIGNAL>
  <I-SIGNAL><SHORT-NAME>SigFr</SHORT-NAME><LENGTH>8</LENGTH></I-SIGNAL>
  <ECU-INSTANCE><SHORT-NAME>ECU1</SHORT-NAME>
   <CONNECTORS><COMMUNICATION-CONNECTOR><SHORT-NAME>CN1</SHORT-NAME>
    <FRAME-PORTS><FRAME-PORT><SHORT-NAME>FP_FrmCan</SHORT-NAME>
     <COMMUNICATION-DIRECTION>OUT</COMMUNICATION-DIRECTION></FRAME-PORT></FRAME-PORTS>
   </COMMUNICATION-CONNECTOR></CONNECTORS></ECU-INSTANCE>
  <CAN-CLUSTER><SHORT-NAME>CAN_Cluster</SHORT-NAME><CAN-CLUSTER-VARIANTS>
   <CAN-CLUSTER-CONDITIONAL><PHYSICAL-CHANNELS><CAN-PHYSICAL-CHANNEL><SHORT-NAME>CH</SHORT-NAME>
    <FRAME-TRIGGERINGS><CAN-FRAME-TRIGGERING><SHORT-NAME>FT_FrmCan</SHORT-NAME>
     <FRAME-PORT-REFS><FRAME-PORT-REF DEST="FRAME-PORT">/Mixed/ECU1/CN1/FP_FrmCan</FRAME-PORT-REF></FRAME-PORT-REFS>
     <FRAME-REF DEST="CAN-FRAME">/Mixed/FrmCan</FRAME-REF>
     <CAN-ADDRESSING-MODE>STANDARD</CAN-ADDRESSING-MODE>
     <CAN-FRAME-TX-BEHAVIOR>CAN-20</CAN-FRAME-TX-BEHAVIOR>
     <IDENTIFIER>291</IDENTIFIER>
    </CAN-FRAME-TRIGGERING></FRAME-TRIGGERINGS>
   </CAN-PHYSICAL-CHANNEL></PHYSICAL-CHANNELS></CAN-CLUSTER-CONDITIONAL>
  </CAN-CLUSTER-VARIANTS></CAN-CLUSTER>
 </ELEMENTS></AR-PACKAGE>
 <AR-PACKAGE><SHORT-NAME>NM</SHORT-NAME><ELEMENTS>
  <NM-CONFIG><SHORT-NAME>NmConfig</SHORT-NAME><NODES><NM-NODE><SHORT-NAME>ECU1</SHORT-NAME></NM-NODE></NODES></NM-CONFIG>
 </ELEMENTS></AR-PACKAGE></AR-PACKAGES>
</AUTOSAR>
"#;

    /// 混合文件：同一份 ARXML 里的 CAN 与 FlexRay 各归各的模型。两边的 PDU 用同一
    /// 个标签 I-SIGNAL-I-PDU，靠"被哪种帧引用"区分，漏了过滤就会互相串
    #[test]
    fn mixed_file_does_not_leak_across_protocols() {
        let doc = read(MIXED);
        assert_eq!(names(&doc, BusKind::Can), ["CAN_Cluster"]);
        assert_eq!(names(&doc, BusKind::FlexRay), ["Cluster"]);
        assert_eq!(doc.frames.len(), 2, "一帧一边，共两帧");
        let can_pdus: Vec<&Pdu> = doc
            .pdus
            .iter()
            .filter(|p| p.cluster == "CAN_Cluster")
            .collect();
        assert_eq!(can_pdus.len(), 1);
        assert_eq!(can_pdus[0].signals[0].name, "SigCan");
        let fr_pdus: Vec<&Pdu> = doc.pdus.iter().filter(|p| p.cluster == "Cluster").collect();
        assert_eq!(fr_pdus.len(), 1, "CAN 的 PDU 不该进 FlexRay 集群");
        assert_eq!(fr_pdus[0].signals[0].name, "SigFr");
    }

    /// 纯 CAN 与纯 FlexRay 文件各自读进统一模型时，不该凭空多出另一种集群
    #[test]
    fn single_protocol_files_read_cleanly() {
        // 样本是客户内部文件、不入库：机器上没有时跳过
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("can-arxml-sample/Comfort.arxml");
        if let Ok(text) = std::fs::read_to_string(&path) {
            let can = read(&text);
            assert_eq!(names(&can, BusKind::Can), ["Comfort"]);
            assert!(
                can.clusters_of(BusKind::FlexRay).is_empty(),
                "CAN 文件不该多出 FlexRay 集群"
            );
        }

        let fr = read(include_str!("../../fibex-sample/chassis.arxml"));
        assert!(
            fr.clusters_of(BusKind::Can).is_empty(),
            "FlexRay 样本不该多出 CAN 集群"
        );
        assert!(!fr.frames.is_empty() && !fr.pdus.is_empty());
        assert!(
            fr.frames.iter().all(|f| f.can.is_none()),
            "FlexRay 帧不该带 CAN 属性"
        );
    }

    /// 外科手术式保存：改一个 CAN 字段，文件里不建模的内容一个字节都不动
    #[test]
    fn surgical_save_keeps_unmodelled_content() {
        let (doc, mut src) = SystemDoc::from_arxml_with_source(MIXED).expect("mixed parses");
        assert_eq!(src.render(), src.text, "没改就该原样");

        let frame = doc
            .frames
            .iter()
            .find(|f| f.name == "FrmCan")
            .expect("frame");
        assert!(src.edit_field(
            &format!("frame/{}/identifier", frame.name),
            "<IDENTIFIER>700</IDENTIFIER>",
            "700".to_string()
        ));
        let out = src.render();
        assert!(out.contains("<IDENTIFIER>700</IDENTIFIER>"), "改的要落盘");
        assert!(out.contains("<NM-CONFIG>"), "不建模的 NM 包要原样留下");
        assert!(
            out.contains("Vector Legacy Database Converter"),
            "CAN-FRAME 里的 ADMIN-DATA 要原样留下"
        );
        assert!(out.contains("<FLEXRAY-FRAME>"), "FlexRay 帧要原样留下");

        let back = read(&out);
        assert_eq!(back.frames.len(), doc.frames.len());
        let f = back
            .frames
            .iter()
            .find(|f| f.name == "FrmCan")
            .expect("frame");
        assert_eq!(
            f.can.as_ref().unwrap().identifier,
            700,
            "改过的标识符要读回来"
        );
    }
}
