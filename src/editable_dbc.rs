use can_dbc::{
    AttributeDefinition, AttributeValueType, ByteOrder, Dbc, Message, MessageId,
    MultiplexIndicator, Signal, ValueType,
};

fn numeric_to_f64(v: &can_dbc::NumericValue) -> f64 {
    match v {
        can_dbc::NumericValue::Uint(n) => *n as f64,
        can_dbc::NumericValue::Int(n) => *n as f64,
        can_dbc::NumericValue::Double(n) => *n,
    }
}

fn numeric_to_i64(v: &can_dbc::NumericValue) -> i64 {
    match v {
        can_dbc::NumericValue::Uint(n) => *n as i64,
        can_dbc::NumericValue::Int(n) => *n,
        can_dbc::NumericValue::Double(n) => *n as i64,
    }
}

/// 帧格式与总线类型由 FrameFormat 自己写出 BA_，不走通用属性通道，避免重复声明
fn is_format_attribute(name: &str) -> bool {
    name == "VFrameFormat" || name == "BusType"
}

fn convert_attr_type(t: &AttributeValueType) -> AttrType {
    match t {
        AttributeValueType::Int(min, max) => AttrType::Int {
            min: numeric_to_i64(min),
            max: numeric_to_i64(max),
        },
        AttributeValueType::Hex(min, max) => AttrType::Hex {
            min: numeric_to_i64(min),
            max: numeric_to_i64(max),
        },
        AttributeValueType::Float(min, max) => AttrType::Float {
            min: numeric_to_f64(min),
            max: numeric_to_f64(max),
        },
        AttributeValueType::String => AttrType::String,
        AttributeValueType::Enum(values) => AttrType::Enum(values.clone()),
    }
}

fn convert_attr_value(v: &can_dbc::AttributeValue) -> AttrValue {
    match v {
        can_dbc::AttributeValue::Uint(n) => AttrValue::Int(*n as i64),
        can_dbc::AttributeValue::Int(n) => AttrValue::Int(*n),
        can_dbc::AttributeValue::Double(n) => AttrValue::Float(*n),
        can_dbc::AttributeValue::String(s) => AttrValue::Text(s.clone()),
    }
}

#[derive(Clone)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone)]
pub struct ValidationIssue {
    pub severity: Severity,
    pub message: String,
}

// 这个文件实现了一个可编辑的 Dbc 数据结构，支持基本的编辑操作和历史记录管理
// 外部可以读取里面的属性，但是不可以编辑
// 所有的编辑都是通过 EditableDbc 提供的方法来进行的，这些方法会记录操作历史以支持撤销和重做功能
// EditableDbc 直接通过 Dbc 进行初始化，转换方式等价于 String -> Dbc -> EditableDbc
// 将来会实现输出 Dbc 文件字符串的功能，即 EditableDbc -> String

// 整体的操作流程：先使用 can-dbc 库实现 String -> DBC
// 然后通过 EditableDbc::from_dbc 将 DBC 转换为 EditableDbc
// 然后通过 EditableDbc 提供的各种 set_xxx 方法进行编辑
// 编辑过程中允许撤回和重做
// 最后通过 EditableDbc 提供的 to_string 方法将结果转换回 DBC 文件字符串

// 不会实现 File 相关的功能
// 也不会有文件名的记录等数据
// 文件的读写交给上层管理

// 这些都是原子化的操作
// 在外部使用的时候，如一个窗口的更改
// 需要外部记录每个复合操作有多少次
// 然后在需要撤销的时候，调用多次 undo 即可

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum Operation {
    SetMessageId {
        old_id: u32,
        new_id: u32,
    },
    SetMessageFrameFormat {
        message_id: u32,
        old_format: FrameFormat,
        new_format: FrameFormat,
    },
    SetMessageName {
        message_id: u32,
        old_name: String,
        new_name: String,
    },
    SetMessageSize {
        message_id: u32,
        old_size: u64,
        new_size: u64,
    },
    SetMessageTransmitter {
        message_id: u32,
        old_transmitter: String,
        new_transmitter: String,
    },
    SetMessageComment {
        message_id: u32,
        old_comment: String,
        new_comment: String,
    },
    SetSignalName {
        message_id: u32,
        signal_old_name: String,
        signal_new_name: String,
    },
    SetSignalMultiplexerIndicator {
        message_id: u32,
        signal_name: String,
        old_indicator: MultiplexIndicator,
        new_indicator: MultiplexIndicator,
    },
    SetSignalStartBit {
        message_id: u32,
        signal_name: String,
        old_start_bit: u64,
        new_start_bit: u64,
    },
    SetSignalSize {
        message_id: u32,
        signal_name: String,
        old_size: u64,
        new_size: u64,
    },
    SetSignalByteOrder {
        message_id: u32,
        signal_name: String,
        old_byte_order: ByteOrder,
        new_byte_order: ByteOrder,
    },
    SetSignalValueType {
        message_id: u32,
        signal_name: String,
        old_value_type: ValueType,
        new_value_type: ValueType,
    },
    SetSignalFactor {
        message_id: u32,
        signal_name: String,
        old_factor: f64,
        new_factor: f64,
    },
    SetSignalOffset {
        message_id: u32,
        signal_name: String,
        old_offset: f64,
        new_offset: f64,
    },
    SetSignalMin {
        message_id: u32,
        signal_name: String,
        old_min: f64,
        new_min: f64,
    },
    SetSignalMax {
        message_id: u32,
        signal_name: String,
        old_max: f64,
        new_max: f64,
    },
    SetSignalUnit {
        message_id: u32,
        signal_name: String,
        old_unit: String,
        new_unit: String,
    },
    SetSignalReceivers {
        message_id: u32,
        signal_name: String,
        old_receivers: Vec<String>,
        new_receivers: Vec<String>,
    },
    SetSignalComment {
        message_id: u32,
        signal_name: String,
        old_comment: String,
        new_comment: String,
    },
    SetSignalValueDescriptions {
        message_id: u32,
        signal_name: String,
        old_descriptions: Vec<(i64, String)>,
        new_descriptions: Vec<(i64, String)>,
    },
    SetMessageAttribute {
        message_id: u32,
        name: String,
        old_value: Option<AttrValue>,
        new_value: Option<AttrValue>,
    },
    SetSignalAttribute {
        message_id: u32,
        signal_name: String,
        name: String,
        old_value: Option<AttrValue>,
        new_value: Option<AttrValue>,
    },
    AddMessage {
        message: EditableMessage,
    },
    AddSignal {
        message_id: u32,
        signal: EditableSignal,
    },
    DeleteMessage {
        message: EditableMessage,
    },
    DeleteSignal {
        message_id: u32,
        signal: EditableSignal,
    },
    AddNode {
        name: String,
    },
    DeleteNode {
        name: String,
    },
    RenameNode {
        old_name: String,
        new_name: String,
    },
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct EditableDbc {
    nodes: Vec<String>,
    messages: Vec<EditableMessage>,
    /// 文件里声明过的属性（`BA_DEF_` / `BA_DEF_DEF_`），保存时原样写回
    attribute_definitions: Vec<AttrDef>,
    history: Vec<Operation>,
    compound_counts: Vec<usize>,
    redo_history: Vec<Operation>,
    redo_compound_counts: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum FrameFormat {
    #[default]
    Standard,
    Extended,
    /// CAN FD 帧，11 位 ID
    StandardFd,
    /// CAN FD 帧，29 位扩展 ID
    ExtendedFd,
}

impl FrameFormat {
    /// 是否为 29 位扩展 ID 寻址
    pub fn is_extended(&self) -> bool {
        matches!(self, FrameFormat::Extended | FrameFormat::ExtendedFd)
    }

    /// 是否为 CAN FD 帧（DLC 可达 64 字节）
    pub fn is_fd(&self) -> bool {
        matches!(self, FrameFormat::StandardFd | FrameFormat::ExtendedFd)
    }

    /// 组合寻址模式与 FD 标志
    pub fn compose(extended: bool, fd: bool) -> Self {
        match (extended, fd) {
            (false, false) => FrameFormat::Standard,
            (true, false) => FrameFormat::Extended,
            (false, true) => FrameFormat::StandardFd,
            (true, true) => FrameFormat::ExtendedFd,
        }
    }

    /// raw id（含 0x80000000 扩展标志），用于 BO_/CM_/VAL_ 段
    pub fn raw_id(&self, id: u32) -> u32 {
        if self.is_extended() {
            id | 0x8000_0000
        } else {
            id
        }
    }
}

/// Vector 工具链约定的 VFrameFormat 枚举值顺序
pub const VFRAME_FORMAT_ENUM: [&str; 5] = [
    "StandardCAN",
    "ExtendedCAN",
    "res",
    "StandardCAN_FD",
    "ExtendedCAN_FD",
];

impl FrameFormat {
    /// 在 VFrameFormat 枚举中的索引
    pub fn vframe_format_index(&self) -> u64 {
        match self {
            FrameFormat::Standard => 0,
            FrameFormat::Extended => 1,
            FrameFormat::StandardFd => 3,
            FrameFormat::ExtendedFd => 4,
        }
    }

    /// 从枚举字符串解析（含 "CAN_FD" 即视为 FD）
    pub fn from_vframe_format(value: &str) -> Option<Self> {
        let extended = value.starts_with("Extended");
        let fd = value.contains("CAN_FD") || value.contains("CANFD");
        Some(FrameFormat::compose(extended, fd))
    }
}

/// 属性挂在什么对象上（DBC 里 `BA_DEF_` 的目标关键字）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttrTarget {
    /// `BO_` 报文属性
    Message,
    /// `SG_` 信号属性
    Signal,
    /// `BU_` 节点属性
    Node,
    /// 没有目标，作用于整个数据库
    Network,
}

impl AttrTarget {
    /// `BA_DEF_` / `BA_` 里写的对象关键字；数据库级为空
    pub fn keyword(self) -> &'static str {
        match self {
            AttrTarget::Message => "BO_",
            AttrTarget::Signal => "SG_",
            AttrTarget::Node => "BU_",
            AttrTarget::Network => "",
        }
    }
}

/// `BA_DEF_` 声明的属性类型
#[derive(Clone, Debug, PartialEq)]
pub enum AttrType {
    Int { min: i64, max: i64 },
    Hex { min: i64, max: i64 },
    Float { min: f64, max: f64 },
    Enum(Vec<String>),
    String,
}

/// 一个属性取值。枚举值就是列表里的一个字符串
#[derive(Clone, Debug, PartialEq)]
pub enum AttrValue {
    Int(i64),
    Float(f64),
    Text(String),
}

impl AttrValue {
    /// 写进 DBC 文本：文本带引号，数字不带
    pub fn to_dbc_text(&self) -> String {
        match self {
            AttrValue::Int(v) => v.to_string(),
            AttrValue::Float(v) => {
                if v.is_finite() && (v - v.round()).abs() < f64::EPSILON {
                    format!("{:.1}", v)
                } else {
                    format!("{}", v)
                }
            }
            AttrValue::Text(v) => format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"")),
        }
    }

    /// 界面与 Excel 里显示的文本
    pub fn display(&self) -> String {
        match self {
            AttrValue::Int(v) => v.to_string(),
            AttrValue::Float(v) => {
                if v.is_finite() && (v - v.round()).abs() < f64::EPSILON {
                    format!("{}", *v as i64)
                } else {
                    format!("{}", v)
                }
            }
            AttrValue::Text(v) => v.clone(),
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            AttrValue::Int(v) => Some(*v as f64),
            AttrValue::Float(v) => Some(*v),
            AttrValue::Text(v) => v.trim().parse::<f64>().ok(),
        }
    }
}

/// 一条属性声明（`BA_DEF_` + 可选的 `BA_DEF_DEF_` 默认值）
#[derive(Clone, Debug, PartialEq)]
pub struct AttrDef {
    pub name: String,
    pub target: AttrTarget,
    pub kind: AttrType,
    pub default: Option<AttrValue>,
}

/// Vector 工具链常用的属性名，界面与 Excel 模板按这些名字读写
pub mod attr_names {
    pub const MSG_TYPE: &str = "GenMsgType";
    pub const MSG_SEND_TYPE: &str = "GenMsgSendType";
    pub const MSG_CYCLE_TIME: &str = "GenMsgCycleTime";
    pub const MSG_CYCLE_TIME_FAST: &str = "GenMsgCycleTimeFast";
    pub const MSG_NR_OF_REPETITION: &str = "GenMsgNrOfRepetition";
    pub const MSG_DELAY_TIME: &str = "GenMsgDelayTime";
    pub const SIG_SEND_TYPE: &str = "GenSigSendType";
    pub const SIG_START_VALUE: &str = "GenSigStartValue";
    pub const SIG_INVALID_VALUE: &str = "GenSigInvalidValue";
    pub const SIG_INACTIVE_VALUE: &str = "GenSigInactiveValue";
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct EditableMessage {
    message_id: u32,
    frame_format: FrameFormat,
    message_name: String,
    message_size: u64,
    transmitter: String,
    signals: Vec<EditableSignal>,
    comment: String,
    /// 报文级属性：属性名 -> 值（`BA_ "x" BO_ <id> ...`）
    attributes: Vec<(String, AttrValue)>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct EditableSignal {
    name: String,
    multiplexer_indicator: MultiplexIndicator,
    start_bit: u64,
    signal_size: u64,
    byte_order: ByteOrder,
    value_type: ValueType,
    factor: f64,
    offset: f64,
    min: f64,
    max: f64,
    unit: String,
    receivers: Vec<String>,
    comment: String,
    value_descriptions: Vec<(i64, String)>,
    /// 信号级属性：属性名 -> 值（`BA_ "x" SG_ <id> <信号名> ...`）
    attributes: Vec<(String, AttrValue)>,
}

#[allow(dead_code)]
impl EditableDbc {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            messages: Vec::new(),
            attribute_definitions: Vec::new(),
            history: Vec::new(),
            compound_counts: Vec::new(),
            redo_history: Vec::new(),
            redo_compound_counts: Vec::new(),
        }
    }

    pub fn from_imported(messages: Vec<EditableMessage>, nodes: Vec<String>) -> Self {
        Self {
            nodes,
            messages,
            attribute_definitions: Vec::new(),
            history: Vec::new(),
            compound_counts: Vec::new(),
            redo_history: Vec::new(),
            redo_compound_counts: Vec::new(),
        }
    }

    /// 登记一批属性声明（导入时用，不进撤销历史）；同名同目标的定义只保留第一条
    pub fn import_attribute_definitions(&mut self, defs: Vec<AttrDef>) {
        for def in defs {
            self.upsert_attribute_definition(&def.name, def.target, def.kind, def.default);
        }
    }

    pub fn attribute_definitions(&self) -> &[AttrDef] {
        &self.attribute_definitions
    }

    /// 某类对象上的全部定义，按名字排序稳定输出
    pub fn attribute_definitions_for(&self, target: AttrTarget) -> Vec<&AttrDef> {
        let mut out: Vec<&AttrDef> = self
            .attribute_definitions
            .iter()
            .filter(|d| d.target == target)
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    pub fn attribute_definition(&self, name: &str, target: AttrTarget) -> Option<&AttrDef> {
        self.attribute_definitions
            .iter()
            .find(|d| d.name == name && d.target == target)
    }

    /// 新增或替换一条属性定义。界面与导入侧都走这里，保证类型与枚举列表不丢。
    pub fn upsert_attribute_definition(
        &mut self,
        name: &str,
        target: AttrTarget,
        kind: AttrType,
        default: Option<AttrValue>,
    ) {
        match self
            .attribute_definitions
            .iter_mut()
            .find(|d| d.name == name && d.target == target)
        {
            Some(existing) => {
                existing.kind = kind;
                if default.is_some() {
                    existing.default = default;
                }
            }
            None => self.attribute_definitions.push(AttrDef {
                name: name.to_string(),
                target,
                kind,
                default,
            }),
        }
    }

    /// 按取值推断类型：整数用 INT，带小数用 FLOAT，其余用 STRING。
    /// 范围取到能容下这个值，避免自己造出来的声明反过来报越界。
    pub fn default_type_for(value: &AttrValue) -> AttrType {
        match value {
            AttrValue::Int(v) => AttrType::Int {
                min: (*v).min(0),
                max: (*v).max(1),
            },
            AttrValue::Float(v) => AttrType::Float {
                min: if *v < 0.0 { *v } else { 0.0 },
                max: if *v > 1.0 { *v } else { 1.0 },
            },
            AttrValue::Text(_) => AttrType::String,
        }
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    pub fn nodes(&self) -> &Vec<String> {
        &self.nodes
    }

    pub fn add_node(&mut self, name: &str) {
        if self.nodes.iter().any(|n| n == name) {
            return;
        }
        self.nodes.push(name.to_string());
        self.push_compound(Operation::AddNode {
            name: name.to_string(),
        });
    }

    pub fn delete_node(&mut self, name: &str) {
        if !self.nodes.iter().any(|n| n == name) {
            return;
        }
        self.nodes.retain(|n| n != name);
        self.push_compound(Operation::DeleteNode {
            name: name.to_string(),
        });
    }

    pub fn rename_node(&mut self, old_name: &str, new_name: &str) {
        if let Some(node) = self.nodes.iter_mut().find(|n| *n == old_name) {
            *node = new_name.to_string();
            self.push_compound(Operation::RenameNode {
                old_name: old_name.to_string(),
                new_name: new_name.to_string(),
            });
        } else {
            return;
        }

        // 传播到发送节点与信号接收节点，保持引用一致（撤销时一并恢复）
        let mut changes = 1;
        let snapshot: Vec<(u32, String, Vec<(String, Vec<String>)>)> = self
            .messages
            .iter()
            .map(|m| {
                (
                    m.message_id,
                    m.transmitter.clone(),
                    m.signals
                        .iter()
                        .map(|s| (s.name.clone(), s.receivers.clone()))
                        .collect(),
                )
            })
            .collect();

        for (msg_id, transmitter, signals) in snapshot {
            if transmitter == old_name {
                self.set_message_transmitter(msg_id, new_name);
                changes += 1;
            }
            for (sig_name, receivers) in signals {
                if receivers.iter().any(|r| r == old_name) {
                    let new_receivers: Vec<String> = receivers
                        .iter()
                        .map(|r| {
                            if r == old_name {
                                new_name.to_string()
                            } else {
                                r.clone()
                            }
                        })
                        .collect();
                    self.set_signal_receivers(msg_id, &sig_name, new_receivers);
                    changes += 1;
                }
            }
        }

        if changes > 1 {
            self.merge_last_compounds(changes);
        }
    }

    pub fn messages(&self) -> &Vec<EditableMessage> {
        &self.messages
    }

    fn push_compound(&mut self, op: Operation) {
        self.history.push(op);
        self.compound_counts.push(1);
        self.redo_history.clear();
        self.redo_compound_counts.clear();
    }

    pub fn merge_last_compounds(&mut self, n: usize) {
        if n <= 1 || self.compound_counts.len() < n {
            return;
        }
        let start = self.compound_counts.len() - n;
        let total: usize = self.compound_counts[start..].iter().sum();
        self.compound_counts.truncate(start);
        self.compound_counts.push(total);
    }

    pub fn can_undo(&self) -> bool {
        !self.compound_counts.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_compound_counts.is_empty()
    }

    pub fn validate(&self) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        let mut seen_ids: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
        let mut seen_names: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();

        // 节点名须为合法 DBC 标识符
        for node in &self.nodes {
            if !is_valid_dbc_identifier(node) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!("Node name '{}' is not a valid DBC identifier", node),
                });
            }
        }

        for msg in &self.messages {
            if msg.message_name.is_empty() {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!("Message 0x{:03X} has empty name", msg.message_id),
                });
            } else if !is_valid_dbc_identifier(&msg.message_name) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!(
                        "Message name '{}' is not a valid DBC identifier",
                        msg.message_name
                    ),
                });
            }

            // DLC 校验：经典 CAN 最大 8 字节，CAN FD 最大 64 字节
            if msg.frame_format.is_fd() {
                if msg.message_size > 64 {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        message: format!(
                            "Message '{}' (0x{:03X}) DLC {} exceeds CAN FD limit of 64",
                            msg.message_name, msg.message_id, msg.message_size
                        ),
                    });
                } else if !matches!(
                    msg.message_size,
                    0 | 1..=8 | 12 | 16 | 20 | 24 | 32 | 48 | 64
                ) {
                    issues.push(ValidationIssue {
                        severity: Severity::Warning,
                        message: format!(
                            "Message '{}' (0x{:03X}) CAN FD DLC {} is not a valid CAN FD length (valid: 1-8, 12, 16, 20, 24, 32, 48, 64)",
                            msg.message_name, msg.message_id, msg.message_size
                        ),
                    });
                }
            } else {
                if msg.message_size > 8 {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        message: format!(
                            "Message '{}' (0x{:03X}) DLC {} exceeds classic CAN limit of 8 (enable CAN FD for larger DLC)",
                            msg.message_name, msg.message_id, msg.message_size
                        ),
                    });
                }
                if msg.message_size == 0 {
                    issues.push(ValidationIssue {
                        severity: Severity::Warning,
                        message: format!(
                            "Message '{}' (0x{:03X}) has zero size",
                            msg.message_name, msg.message_id
                        ),
                    });
                }
            }

            if let Some(prev_name) = seen_names.get(&msg.message_name) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!(
                        "Duplicate message name '{}': 0x{:03X} and 0x{:03X}",
                        msg.message_name, *prev_name, msg.message_id
                    ),
                });
            } else {
                seen_names.insert(msg.message_name.clone(), msg.message_id);
            }

            if let Some(prev_name) = seen_ids.get(&msg.message_id) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!(
                        "Duplicate message ID 0x{:03X}: '{}' and '{}'",
                        msg.message_id, prev_name, msg.message_name
                    ),
                });
            } else {
                seen_ids.insert(msg.message_id, msg.message_name.clone());
            }

            // ID 范围：标准帧 11 位，扩展帧 29 位
            let id_limit = if msg.frame_format.is_extended() {
                0x1FFF_FFFF
            } else {
                0x7FF
            };
            if msg.message_id > id_limit {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!(
                        "Message '{}' ID 0x{:03X} exceeds {} frame limit of 0x{:X}",
                        msg.message_name,
                        msg.message_id,
                        if msg.frame_format.is_extended() {
                            "extended"
                        } else {
                            "standard"
                        },
                        id_limit
                    ),
                });
            }

            // 发送节点应存在于 BU_ 列表中
            if !msg.transmitter.is_empty()
                && msg.transmitter != "Vector__XXX"
                && !self.nodes.contains(&msg.transmitter)
            {
                issues.push(ValidationIssue {
                    severity: Severity::Warning,
                    message: format!(
                        "Message '{}' transmitter '{}' is not defined in the node list",
                        msg.message_name, msg.transmitter
                    ),
                });
            }

            let max_bits = msg.message_size * 8;
            let mut seen_signals: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();

            for (sig_idx, sig) in msg.signals.iter().enumerate() {
                // 按实际字节序计算占用的位（Motorola 起始位是 MSB，不能简单相加）
                let positions =
                    get_signal_bit_positions(sig.start_bit, sig.signal_size, &sig.byte_order);
                let overflow = positions.iter().any(|&b| b >= max_bits as usize);
                if overflow {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        message: format!(
                            "Signal '{}' in '{}' exceeds message size: start bit {} length {} (DLC={} bytes)",
                            sig.name, msg.message_name, sig.start_bit, sig.signal_size, msg.message_size
                        ),
                    });
                }

                if sig.signal_size == 0 {
                    issues.push(ValidationIssue {
                        severity: Severity::Warning,
                        message: format!(
                            "Signal '{}' in '{}' has zero size",
                            sig.name, msg.message_name
                        ),
                    });
                }

                if !is_valid_dbc_identifier(&sig.name) {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        message: format!(
                            "Signal name '{}' in '{}' is not a valid DBC identifier",
                            sig.name, msg.message_name
                        ),
                    });
                }

                if sig.factor == 0.0 {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        message: format!(
                            "Signal '{}' in '{}' has factor 0 (physical values cannot be decoded)",
                            sig.name, msg.message_name
                        ),
                    });
                }

                if sig.min > sig.max {
                    issues.push(ValidationIssue {
                        severity: Severity::Warning,
                        message: format!(
                            "Signal '{}' in '{}' has min {} > max {}",
                            sig.name, msg.message_name, sig.min, sig.max
                        ),
                    });
                }

                for r in &sig.receivers {
                    if r == "Vector__XXX" {
                        continue;
                    }
                    if !is_valid_dbc_identifier(r) {
                        issues.push(ValidationIssue {
                            severity: Severity::Warning,
                            message: format!(
                                "Signal '{}' receiver '{}' is not a valid DBC identifier",
                                sig.name, r
                            ),
                        });
                    } else if !self.nodes.iter().any(|n| n == r) {
                        issues.push(ValidationIssue {
                            severity: Severity::Warning,
                            message: format!(
                                "Signal '{}' receiver '{}' is not defined in the node list",
                                sig.name, r
                            ),
                        });
                    }
                }

                if let Some(prev_idx) = seen_signals.get(&sig.name) {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        message: format!(
                            "Duplicate signal name '{}' in message '{}' (index {} and {})",
                            sig.name, msg.message_name, prev_idx, sig_idx
                        ),
                    });
                } else {
                    seen_signals.insert(sig.name.clone(), sig_idx);
                }
            }
        }

        // 属性取值是否落在声明的范围内 / 枚举列表里
        for msg in &self.messages {
            self.validate_attribute_values(
                &mut issues,
                &format!("message '{}'", msg.message_name),
                AttrTarget::Message,
                msg.attributes(),
            );
            for sig in msg.signals() {
                self.validate_attribute_values(
                    &mut issues,
                    &format!("signal '{}' of message '{}'", sig.name, msg.message_name),
                    AttrTarget::Signal,
                    sig.attributes(),
                );
            }
        }

        issues
    }

    fn validate_attribute_values(
        &self,
        issues: &mut Vec<ValidationIssue>,
        what: &str,
        target: AttrTarget,
        values: &[(String, AttrValue)],
    ) {
        for (name, value) in values {
            let Some(def) = self.attribute_definition(name, target) else {
                continue;
            };
            match (&def.kind, value) {
                (AttrType::Int { min, max } | AttrType::Hex { min, max }, AttrValue::Int(v)) => {
                    if *v < *min || *v > *max {
                        issues.push(ValidationIssue {
                            severity: Severity::Warning,
                            message: format!(
                                "Attribute '{}' of {} is {}, outside the declared {}..{}",
                                name, what, v, min, max
                            ),
                        });
                    }
                }
                (AttrType::Float { min, max }, v @ (AttrValue::Int(_) | AttrValue::Float(_))) => {
                    if let Some(x) = v.as_f64()
                        && (x < *min || x > *max)
                    {
                        issues.push(ValidationIssue {
                            severity: Severity::Warning,
                            message: format!(
                                "Attribute '{}' of {} is {}, outside the declared {}..{}",
                                name,
                                what,
                                v.display(),
                                AttrValue::Float(*min).display(),
                                AttrValue::Float(*max).display()
                            ),
                        });
                    }
                }
                (AttrType::Enum(options), AttrValue::Text(v))
                    if !options.iter().any(|o| o == v) =>
                {
                    issues.push(ValidationIssue {
                        severity: Severity::Warning,
                        message: format!(
                            "Attribute '{}' of {} is \"{}\", which is not one of the declared values {}",
                            name,
                            what,
                            v,
                            options.join("/")
                        ),
                    });
                }
                _ => {}
            }
        }
    }

    pub fn from_dbc(dbc: &Dbc) -> Self {
        let mut editable_dbc = Self::new();

        editable_dbc.nodes = dbc.nodes.iter().map(|x| x.0.clone()).collect();

        editable_dbc.messages = dbc
            .messages
            .iter()
            .map(|msg| {
                let mut em = EditableMessage::from_message(
                    msg,
                    dbc.message_comment(msg.id).unwrap_or(""),
                    // 从 VFrameFormat 属性解析 CAN FD；DLC > 8 视为 FD 兜底
                    parse_frame_format(dbc, msg),
                );
                for sig in &mut em.signals {
                    if let Some(val_descs) = dbc.value_descriptions_for_signal(msg.id, &sig.name) {
                        sig.value_descriptions = val_descs
                            .iter()
                            .map(|vd| (vd.id, vd.description.clone()))
                            .collect();
                    }
                    // 信号注释（CM_ SG_）
                    if let Some(comment) = dbc.signal_comment(msg.id, &sig.name) {
                        sig.comment = comment.to_string();
                    }
                    let sig_attrs: Vec<(String, AttrValue)> = dbc
                        .attribute_values_signal
                        .iter()
                        .filter(|a| a.message_id == msg.id && a.signal_name == sig.name)
                        .map(|a| (a.name.clone(), convert_attr_value(&a.value)))
                        .collect();
                    for (name, value) in sig_attrs {
                        sig.apply_attribute(&name, Some(value));
                    }
                }
                for a in dbc
                    .attribute_values_message
                    .iter()
                    .filter(|a| a.message_id == msg.id)
                {
                    if is_format_attribute(&a.name) {
                        continue;
                    }
                    em.apply_attribute(&a.name, Some(convert_attr_value(&a.value)));
                }
                em
            })
            .collect();

        // 属性声明与默认值：VFrameFormat / BusType 由帧格式自己写出，不重复登记
        editable_dbc.attribute_definitions = dbc
            .attribute_definitions
            .iter()
            .filter_map(|def| match def {
                AttributeDefinition::Message(name, t) => Some(AttrDef {
                    name: name.clone(),
                    target: AttrTarget::Message,
                    kind: convert_attr_type(t),
                    default: None,
                }),
                AttributeDefinition::Signal(name, t) => Some(AttrDef {
                    name: name.clone(),
                    target: AttrTarget::Signal,
                    kind: convert_attr_type(t),
                    default: None,
                }),
                AttributeDefinition::Node(name, t) => Some(AttrDef {
                    name: name.clone(),
                    target: AttrTarget::Node,
                    kind: convert_attr_type(t),
                    default: None,
                }),
                AttributeDefinition::Plain(name, t) => Some(AttrDef {
                    name: name.clone(),
                    target: AttrTarget::Network,
                    kind: convert_attr_type(t),
                    default: None,
                }),
                AttributeDefinition::EnvironmentVariable(_, _) => None,
            })
            .filter(|d| !is_format_attribute(&d.name))
            .collect();
        for d in &dbc.attribute_defaults {
            if is_format_attribute(&d.name) {
                continue;
            }
            let value = convert_attr_value(&d.value);
            let target = editable_dbc
                .attribute_definitions
                .iter()
                .find(|a| a.name == d.name)
                .map(|a| a.target)
                .unwrap_or(AttrTarget::Network);
            if let Some(def) = editable_dbc
                .attribute_definitions
                .iter_mut()
                .find(|a| a.name == d.name && a.target == target)
            {
                def.default = Some(value);
            } else {
                editable_dbc.attribute_definitions.push(AttrDef {
                    name: d.name.clone(),
                    target,
                    kind: EditableDbc::default_type_for(&value),
                    default: Some(value),
                });
            }
        }

        editable_dbc
    }

    pub fn get_message(&self, message_id: u32) -> Option<&EditableMessage> {
        self.messages.iter().find(|m| m.message_id == message_id)
    }

    /// 按名称查找消息
    pub fn get_message_by_name(&self, name: &str) -> Option<&EditableMessage> {
        self.messages.iter().find(|m| m.message_name() == name)
    }

    pub fn find_message_index(&self, message_id: u32) -> Option<usize> {
        self.messages
            .iter()
            .position(|m| m.message_id == message_id)
    }

    fn find_index_signal_index(&self, message_idx: usize, signal_name: &str) -> Option<usize> {
        let msg = &self.messages[message_idx];
        msg.signals.iter().position(|s| s.name == signal_name)
    }

    fn find_message_signal_index(
        &self,
        message_id: u32,
        signal_name: &str,
    ) -> Option<(usize, usize)> {
        if let Some(msg_idx) = self.find_message_index(message_id)
            && let Some(sig_idx) = self.find_index_signal_index(msg_idx, signal_name)
        {
            return Some((msg_idx, sig_idx));
        }
        None
    }

    pub fn find_signal_index(&self, message_id: u32, signal_name: &str) -> Option<usize> {
        if let Some(msg_idx) = self.find_message_index(message_id) {
            return self.find_index_signal_index(msg_idx, signal_name);
        }
        None
    }

    fn get_message_mut(&mut self, message_id: u32) -> Option<&mut EditableMessage> {
        self.messages
            .iter_mut()
            .find(|m| m.message_id == message_id)
    }

    fn get_signal_mut(
        &mut self,
        message_id: u32,
        signal_name: &str,
    ) -> Option<&mut EditableSignal> {
        if let Some(msg) = self.get_message_mut(message_id) {
            return msg.signals.iter_mut().find(|s| s.name == signal_name);
        }
        None
    }

    pub fn set_message_id(&mut self, old_message_id: u32, new_message_id: u32) {
        if let Some(msg) = self.get_message_mut(old_message_id) {
            msg.message_id = new_message_id;
        } else {
            return;
        }

        self.push_compound(Operation::SetMessageId {
            old_id: old_message_id,
            new_id: new_message_id,
        });
    }

    pub fn set_message_frame_format(&mut self, message_id: u32, new_format: FrameFormat) {
        let old_format = {
            if let Some(msg) = self.get_message_mut(message_id) {
                let old_format = msg.frame_format;
                msg.frame_format = new_format;
                old_format
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetMessageFrameFormat {
            message_id,
            old_format,
            new_format,
        });
    }

    pub fn set_message_name(&mut self, message_id: u32, new_name: &str) {
        let old_name = {
            if let Some(msg) = self.get_message_mut(message_id) {
                let old_name = msg.message_name.clone();
                msg.message_name = new_name.to_string();
                old_name
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetMessageName {
            message_id,
            old_name,
            new_name: new_name.to_string(),
        });
    }

    pub fn set_message_size(&mut self, message_id: u32, new_size: u64) {
        let old_size = {
            if let Some(msg) = self.get_message_mut(message_id) {
                let old_size = msg.message_size;
                msg.message_size = new_size;
                old_size
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetMessageSize {
            message_id,
            old_size,
            new_size,
        });
    }

    pub fn set_message_transmitter(&mut self, message_id: u32, new_transmitter: &str) {
        let old_transmitter = {
            if let Some(msg) = self.get_message_mut(message_id) {
                let old_transmitter = msg.transmitter.clone();
                msg.transmitter = new_transmitter.to_string();
                old_transmitter
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetMessageTransmitter {
            message_id,
            old_transmitter,
            new_transmitter: new_transmitter.to_string(),
        });
    }

    pub fn set_message_comment(&mut self, message_id: u32, new_comment: &str) {
        let old_comment = {
            if let Some(msg) = self.get_message_mut(message_id) {
                let old_comment = msg.comment.clone();
                msg.comment = new_comment.to_string();
                old_comment
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetMessageComment {
            message_id,
            old_comment,
            new_comment: new_comment.to_string(),
        });
    }

    pub fn set_signal_name(
        &mut self,
        message_id: u32,
        signal_old_name: &str,
        signal_new_name: &str,
    ) {
        let old_name = {
            if let Some(sig) = self.get_signal_mut(message_id, signal_old_name) {
                let old_name = sig.name.clone();
                sig.name = signal_new_name.to_string();
                old_name
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetSignalName {
            message_id,
            signal_old_name: old_name,
            signal_new_name: signal_new_name.to_string(),
        });
    }

    pub fn set_signal_multiplexer_indicator(
        &mut self,
        message_id: u32,
        signal_name: &str,
        new_indicator: &MultiplexIndicator,
    ) {
        let old_indicator = {
            if let Some(sig) = self.get_signal_mut(message_id, signal_name) {
                let old_indicator = sig.multiplexer_indicator;
                sig.multiplexer_indicator = *new_indicator;
                old_indicator
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetSignalMultiplexerIndicator {
            message_id,
            signal_name: signal_name.to_string(),
            old_indicator,
            new_indicator: *new_indicator,
        });
    }

    pub fn set_signal_start_bit(&mut self, message_id: u32, signal_name: &str, new_start_bit: u64) {
        let old_start_bit = {
            if let Some(sig) = self.get_signal_mut(message_id, signal_name) {
                let old_start_bit = sig.start_bit;
                sig.start_bit = new_start_bit;
                old_start_bit
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetSignalStartBit {
            message_id,
            signal_name: signal_name.to_string(),
            old_start_bit,
            new_start_bit,
        });
    }

    pub fn set_signal_size(&mut self, message_id: u32, signal_name: &str, new_size: u64) {
        let old_size = {
            if let Some(sig) = self.get_signal_mut(message_id, signal_name) {
                let old_size = sig.signal_size;
                sig.signal_size = new_size;
                old_size
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetSignalSize {
            message_id,
            signal_name: signal_name.to_string(),
            old_size,
            new_size,
        });
    }

    pub fn set_signal_byte_order(
        &mut self,
        message_id: u32,
        signal_name: &str,
        new_byte_order: ByteOrder,
    ) {
        let old_byte_order = {
            if let Some(sig) = self.get_signal_mut(message_id, signal_name) {
                let old_byte_order = sig.byte_order;
                sig.byte_order = new_byte_order;
                old_byte_order
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetSignalByteOrder {
            message_id,
            signal_name: signal_name.to_string(),
            old_byte_order,
            new_byte_order,
        });
    }

    pub fn set_signal_value_type(
        &mut self,
        message_id: u32,
        signal_name: &str,
        new_value_type: ValueType,
    ) {
        let old_value_type = {
            if let Some(sig) = self.get_signal_mut(message_id, signal_name) {
                let old_value_type = sig.value_type;
                sig.value_type = new_value_type;
                old_value_type
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetSignalValueType {
            message_id,
            signal_name: signal_name.to_string(),
            old_value_type,
            new_value_type,
        });
    }

    pub fn set_signal_factor(&mut self, message_id: u32, signal_name: &str, new_factor: f64) {
        let old_factor = {
            if let Some(sig) = self.get_signal_mut(message_id, signal_name) {
                let old_factor = sig.factor;
                sig.factor = new_factor;
                old_factor
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetSignalFactor {
            message_id,
            signal_name: signal_name.to_string(),
            old_factor,
            new_factor,
        });
    }

    pub fn set_signal_offset(&mut self, message_id: u32, signal_name: &str, new_offset: f64) {
        let old_offset = {
            if let Some(sig) = self.get_signal_mut(message_id, signal_name) {
                let old_offset = sig.offset;
                sig.offset = new_offset;
                old_offset
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetSignalOffset {
            message_id,
            signal_name: signal_name.to_string(),
            old_offset,
            new_offset,
        });
    }

    pub fn set_signal_min(&mut self, message_id: u32, signal_name: &str, new_min: f64) {
        let old_min = {
            if let Some(sig) = self.get_signal_mut(message_id, signal_name) {
                let old_min = sig.min;
                sig.min = new_min;
                old_min
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetSignalMin {
            message_id,
            signal_name: signal_name.to_string(),
            old_min,
            new_min,
        });
    }

    pub fn set_signal_max(&mut self, message_id: u32, signal_name: &str, new_max: f64) {
        let old_max = {
            if let Some(sig) = self.get_signal_mut(message_id, signal_name) {
                let old_max = sig.max;
                sig.max = new_max;
                old_max
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetSignalMax {
            message_id,
            signal_name: signal_name.to_string(),
            old_max,
            new_max,
        });
    }

    pub fn set_signal_unit(&mut self, message_id: u32, signal_name: &str, new_unit: &str) {
        let old_unit = {
            if let Some(sig) = self.get_signal_mut(message_id, signal_name) {
                let old_unit = sig.unit.clone();
                sig.unit = new_unit.to_string();
                old_unit
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetSignalUnit {
            message_id,
            signal_name: signal_name.to_string(),
            old_unit,
            new_unit: new_unit.to_string(),
        });
    }

    pub fn set_signal_receivers(
        &mut self,
        message_id: u32,
        signal_name: &str,
        new_receivers: Vec<String>,
    ) {
        let old_receivers = {
            if let Some(sig) = self.get_signal_mut(message_id, signal_name) {
                let old_receivers = sig.receivers.clone();
                sig.receivers = new_receivers.clone();
                old_receivers
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetSignalReceivers {
            message_id,
            signal_name: signal_name.to_string(),
            old_receivers,
            new_receivers,
        });
    }

    pub fn set_signal_comment(&mut self, message_id: u32, signal_name: &str, new_comment: &str) {
        let old_comment = {
            if let Some(sig) = self.get_signal_mut(message_id, signal_name) {
                let old_comment = sig.comment.clone();
                sig.comment = new_comment.to_string();
                old_comment
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetSignalComment {
            message_id,
            signal_name: signal_name.to_string(),
            old_comment,
            new_comment: new_comment.to_string(),
        });
    }

    pub fn set_signal_value_descriptions(
        &mut self,
        message_id: u32,
        signal_name: &str,
        new_descriptions: Vec<(i64, String)>,
    ) {
        let old_descriptions = {
            if let Some(sig) = self.get_signal_mut(message_id, signal_name) {
                let old = sig.value_descriptions.clone();
                sig.value_descriptions = new_descriptions.clone();
                old
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetSignalValueDescriptions {
            message_id,
            signal_name: signal_name.to_string(),
            old_descriptions,
            new_descriptions,
        });
    }

    /// 报文属性取值；没有显式赋值时回落到 `BA_DEF_DEF_` 的默认值
    pub fn message_attribute(&self, message_id: u32, name: &str) -> Option<AttrValue> {
        let explicit = self
            .messages
            .iter()
            .find(|m| m.message_id == message_id)
            .and_then(|m| m.attribute(name))
            .cloned();
        explicit.or_else(|| {
            self.attribute_definition(name, AttrTarget::Message)
                .and_then(|d| d.default.clone())
        })
    }

    /// 周期时间的属性名，按优先级：Vector 约定的 `GenMsgCycleTime`，
    /// 有些矩阵用 `CycleTime`
    pub const CYCLE_TIME_ALIASES: [&'static str; 2] = [attr_names::MSG_CYCLE_TIME, "CycleTime"];

    /// 报文的周期时间：在几个常见名字里取第一个文件声明过且有取值的
    pub fn message_cycle_time(&self, message_id: u32) -> Option<AttrValue> {
        Self::CYCLE_TIME_ALIASES
            .iter()
            .filter(|name| {
                self.attribute_definition(name, AttrTarget::Message)
                    .is_some()
            })
            .find_map(|name| self.message_attribute(message_id, name))
    }

    /// 信号属性取值，同样回落到默认值
    pub fn signal_attribute(
        &self,
        message_id: u32,
        signal_name: &str,
        name: &str,
    ) -> Option<AttrValue> {
        let explicit = self
            .messages
            .iter()
            .find(|m| m.message_id == message_id)
            .and_then(|m| m.signals().iter().find(|s| s.name() == signal_name))
            .and_then(|s| s.attribute(name))
            .cloned();
        explicit.or_else(|| {
            self.attribute_definition(name, AttrTarget::Signal)
                .and_then(|d| d.default.clone())
        })
    }

    /// 设置报文属性并记一步撤销；没有该定义时按取值补一条声明。
    /// 传 None 等于删除该属性。
    pub fn set_message_attribute(&mut self, message_id: u32, name: &str, value: Option<AttrValue>) {
        let old = {
            let Some(msg) = self
                .messages
                .iter_mut()
                .find(|m| m.message_id == message_id)
            else {
                return;
            };
            let old = msg.attribute(name).cloned();
            if old == value {
                return;
            }
            msg.apply_attribute(name, value.clone());
            old
        };
        if let Some(v) = &value {
            self.ensure_definition(name, AttrTarget::Message, v);
        }
        self.push_compound(Operation::SetMessageAttribute {
            message_id,
            name: name.to_string(),
            old_value: old,
            new_value: value,
        });
    }

    pub fn set_signal_attribute(
        &mut self,
        message_id: u32,
        signal_name: &str,
        name: &str,
        value: Option<AttrValue>,
    ) {
        let old = {
            let Some(msg) = self
                .messages
                .iter_mut()
                .find(|m| m.message_id == message_id)
            else {
                return;
            };
            let Some(sig) = msg.signals.iter_mut().find(|s| s.name == signal_name) else {
                return;
            };
            let old = sig.attribute(name).cloned();
            if old == value {
                return;
            }
            sig.apply_attribute(name, value.clone());
            old
        };
        if let Some(v) = &value {
            self.ensure_definition(name, AttrTarget::Signal, v);
        }
        self.push_compound(Operation::SetSignalAttribute {
            message_id,
            signal_name: signal_name.to_string(),
            name: name.to_string(),
            old_value: old,
            new_value: value,
        });
    }

    /// 属性值要能写回文件，就得有声明；缺了按取值类型补一条
    fn ensure_definition(&mut self, name: &str, target: AttrTarget, value: &AttrValue) {
        let known = self
            .attribute_definitions
            .iter()
            .any(|d| d.name == name && d.target == target);
        if !known {
            let kind = Self::default_type_for(value);
            self.upsert_attribute_definition(name, target, kind, None);
        }
    }

    /// 给所有已挂上的属性补齐声明。绕过 setter 直接写值的导入路径（Excel 模板、
    /// 外部构造）调用它，否则保存时只有 `BA_` 没有 `BA_DEF_`。
    pub fn ensure_definitions_for_used_attributes(&mut self) {
        let used: Vec<(String, AttrTarget, AttrValue)> = self
            .messages
            .iter()
            .flat_map(|m| {
                m.attributes()
                    .iter()
                    .map(|(n, v)| (n.clone(), AttrTarget::Message, v.clone()))
                    .chain(m.signals().iter().flat_map(|s| {
                        s.attributes()
                            .iter()
                            .map(|(n, v)| (n.clone(), AttrTarget::Signal, v.clone()))
                    }))
            })
            .collect();
        for (name, target, value) in used {
            self.ensure_definition(&name, target, &value);
        }
    }

    pub fn add_message(&mut self, message: &EditableMessage) {
        self.messages.push(message.clone());
        self.push_compound(Operation::AddMessage {
            message: message.clone(),
        });
    }

    pub fn new_message(&mut self) {
        let message = EditableMessage::new();
        self.add_message(&message);
    }

    pub fn delete_message(&mut self, message_id: u32) {
        let msg = {
            if let Some(msg_idx) = self.find_message_index(message_id) {
                self.messages.swap_remove(msg_idx)
            } else {
                return;
            }
        };

        self.push_compound(Operation::DeleteMessage { message: msg });
    }

    pub fn add_signal(&mut self, message_id: u32, signal: &EditableSignal) {
        if let Some(msg) = self.get_message_mut(message_id) {
            msg.signals.push(signal.clone());
            self.push_compound(Operation::AddSignal {
                message_id,
                signal: signal.clone(),
            });
        }
    }

    pub fn new_signal(&mut self, message_id: u32) {
        let signal = EditableSignal::new();
        self.add_signal(message_id, &signal);
    }

    pub fn delete_signal(&mut self, message_id: u32, signal_name: &str) {
        let sig = {
            if let Some((msg_idx, sig_idx)) =
                self.find_message_signal_index(message_id, signal_name)
            {
                self.messages[msg_idx].signals.swap_remove(sig_idx)
            } else {
                return;
            }
        };

        self.push_compound(Operation::DeleteSignal {
            message_id,
            signal: sig,
        });
    }

    pub fn undo(&mut self) -> Result<(), String> {
        let count = *self.compound_counts.last().ok_or("Nothing to undo")?;
        self.compound_counts.pop();
        let start = self.history.len() - count;
        let ops: Vec<Operation> = self.history.drain(start..).collect();
        for op in ops.iter().rev() {
            self.undo_operation(op)?;
        }
        self.redo_history.extend(ops);
        self.redo_compound_counts.push(count);
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), String> {
        let count = *self.redo_compound_counts.last().ok_or("Nothing to redo")?;
        self.redo_compound_counts.pop();
        let start = self.redo_history.len() - count;
        let ops: Vec<Operation> = self.redo_history.drain(start..).collect();
        for op in ops.iter() {
            self.redo_operation(op)?;
        }
        self.history.extend(ops);
        self.compound_counts.push(count);
        Ok(())
    }

    fn undo_operation(&mut self, op: &Operation) -> Result<(), String> {
        match op {
            Operation::SetMessageId { old_id, new_id } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *new_id) {
                    msg.message_id = *old_id;
                }
                Ok(())
            }
            Operation::SetMessageFrameFormat {
                message_id,
                old_format,
                new_format: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                {
                    msg.frame_format = *old_format;
                }
                Ok(())
            }
            Operation::SetMessageName {
                message_id,
                old_name,
                new_name: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                {
                    msg.message_name = old_name.clone();
                }
                Ok(())
            }
            Operation::SetMessageSize {
                message_id,
                old_size,
                new_size: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                {
                    msg.message_size = *old_size;
                }
                Ok(())
            }
            Operation::SetMessageTransmitter {
                message_id,
                old_transmitter,
                new_transmitter: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                {
                    msg.transmitter = old_transmitter.clone();
                }
                Ok(())
            }
            Operation::SetMessageComment {
                message_id,
                old_comment,
                new_comment: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                {
                    msg.comment = old_comment.clone();
                }
                Ok(())
            }
            Operation::SetSignalName {
                message_id,
                signal_old_name,
                signal_new_name,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_new_name)
                {
                    sig.name = signal_old_name.clone();
                }
                Ok(())
            }
            Operation::SetSignalMultiplexerIndicator {
                message_id,
                signal_name,
                old_indicator,
                new_indicator: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.multiplexer_indicator = *old_indicator;
                }
                Ok(())
            }
            Operation::SetSignalStartBit {
                message_id,
                signal_name,
                old_start_bit,
                new_start_bit: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.start_bit = *old_start_bit;
                }
                Ok(())
            }
            Operation::SetSignalSize {
                message_id,
                signal_name,
                old_size,
                new_size: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.signal_size = *old_size;
                }
                Ok(())
            }
            Operation::SetSignalByteOrder {
                message_id,
                signal_name,
                old_byte_order,
                new_byte_order: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.byte_order = *old_byte_order;
                }
                Ok(())
            }
            Operation::SetSignalValueType {
                message_id,
                signal_name,
                old_value_type,
                new_value_type: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.value_type = *old_value_type;
                }
                Ok(())
            }
            Operation::SetSignalFactor {
                message_id,
                signal_name,
                old_factor,
                new_factor: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.factor = *old_factor;
                }
                Ok(())
            }
            Operation::SetSignalOffset {
                message_id,
                signal_name,
                old_offset,
                new_offset: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.offset = *old_offset;
                }
                Ok(())
            }
            Operation::SetSignalMin {
                message_id,
                signal_name,
                old_min,
                new_min: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.min = *old_min;
                }
                Ok(())
            }
            Operation::SetSignalMax {
                message_id,
                signal_name,
                old_max,
                new_max: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.max = *old_max;
                }
                Ok(())
            }
            Operation::SetSignalUnit {
                message_id,
                signal_name,
                old_unit,
                new_unit: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.unit = old_unit.clone();
                }
                Ok(())
            }
            Operation::SetSignalReceivers {
                message_id,
                signal_name,
                old_receivers,
                new_receivers: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.receivers = old_receivers.clone();
                }
                Ok(())
            }
            Operation::SetSignalComment {
                message_id,
                signal_name,
                old_comment,
                new_comment: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.comment = old_comment.clone();
                }
                Ok(())
            }
            Operation::SetSignalValueDescriptions {
                message_id,
                signal_name,
                old_descriptions,
                new_descriptions: _,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.value_descriptions = old_descriptions.clone();
                }
                Ok(())
            }
            Operation::SetMessageAttribute {
                message_id,
                name,
                old_value,
                ..
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                {
                    msg.apply_attribute(name, old_value.clone());
                }
                Ok(())
            }
            Operation::SetSignalAttribute {
                message_id,
                signal_name,
                name,
                old_value,
                ..
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.apply_attribute(name, old_value.clone());
                }
                Ok(())
            }
            Operation::AddMessage { message } => {
                self.messages.retain(|m| m.message_id != message.message_id);
                Ok(())
            }
            Operation::DeleteMessage { message } => {
                self.messages.push(message.clone());
                Ok(())
            }
            Operation::AddSignal { message_id, signal } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                {
                    msg.signals.retain(|s| s.name != signal.name);
                }
                Ok(())
            }
            Operation::DeleteSignal { message_id, signal } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                {
                    msg.signals.push(signal.clone());
                }
                Ok(())
            }
            Operation::AddNode { name } => {
                self.nodes.retain(|n| n != name);
                Ok(())
            }
            Operation::DeleteNode { name } => {
                self.nodes.push(name.clone());
                Ok(())
            }
            Operation::RenameNode { old_name, new_name } => {
                if let Some(node) = self
                    .nodes
                    .iter_mut()
                    .find(|n| n.as_str() == new_name.as_str())
                {
                    *node = old_name.clone();
                }
                Ok(())
            }
        }
    }

    fn redo_operation(&mut self, op: &Operation) -> Result<(), String> {
        match op {
            Operation::SetMessageId { old_id, new_id } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *old_id) {
                    msg.message_id = *new_id;
                }
                Ok(())
            }
            Operation::SetMessageFrameFormat {
                message_id,
                old_format: _,
                new_format,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                {
                    msg.frame_format = *new_format;
                }
                Ok(())
            }
            Operation::SetMessageName {
                message_id,
                old_name: _,
                new_name,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                {
                    msg.message_name = new_name.clone();
                }
                Ok(())
            }
            Operation::SetMessageSize {
                message_id,
                old_size: _,
                new_size,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                {
                    msg.message_size = *new_size;
                }
                Ok(())
            }
            Operation::SetMessageTransmitter {
                message_id,
                old_transmitter: _,
                new_transmitter,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                {
                    msg.transmitter = new_transmitter.clone();
                }
                Ok(())
            }
            Operation::SetMessageComment {
                message_id,
                old_comment: _,
                new_comment,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                {
                    msg.comment = new_comment.clone();
                }
                Ok(())
            }
            Operation::SetSignalName {
                message_id,
                signal_old_name,
                signal_new_name,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_old_name)
                {
                    sig.name = signal_new_name.clone();
                }
                Ok(())
            }
            Operation::SetSignalMultiplexerIndicator {
                message_id,
                signal_name,
                old_indicator: _,
                new_indicator,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.multiplexer_indicator = *new_indicator;
                }
                Ok(())
            }
            Operation::SetSignalStartBit {
                message_id,
                signal_name,
                old_start_bit: _,
                new_start_bit,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.start_bit = *new_start_bit;
                }
                Ok(())
            }
            Operation::SetSignalSize {
                message_id,
                signal_name,
                old_size: _,
                new_size,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.signal_size = *new_size;
                }
                Ok(())
            }
            Operation::SetSignalByteOrder {
                message_id,
                signal_name,
                old_byte_order: _,
                new_byte_order,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.byte_order = *new_byte_order;
                }
                Ok(())
            }
            Operation::SetSignalValueType {
                message_id,
                signal_name,
                old_value_type: _,
                new_value_type,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.value_type = *new_value_type;
                }
                Ok(())
            }
            Operation::SetSignalFactor {
                message_id,
                signal_name,
                old_factor: _,
                new_factor,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.factor = *new_factor;
                }
                Ok(())
            }
            Operation::SetSignalOffset {
                message_id,
                signal_name,
                old_offset: _,
                new_offset,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.offset = *new_offset;
                }
                Ok(())
            }
            Operation::SetSignalMin {
                message_id,
                signal_name,
                old_min: _,
                new_min,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.min = *new_min;
                }
                Ok(())
            }
            Operation::SetSignalMax {
                message_id,
                signal_name,
                old_max: _,
                new_max,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.max = *new_max;
                }
                Ok(())
            }
            Operation::SetSignalUnit {
                message_id,
                signal_name,
                old_unit: _,
                new_unit,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.unit = new_unit.clone();
                }
                Ok(())
            }
            Operation::SetSignalReceivers {
                message_id,
                signal_name,
                old_receivers: _,
                new_receivers,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.receivers = new_receivers.clone();
                }
                Ok(())
            }
            Operation::SetSignalComment {
                message_id,
                signal_name,
                old_comment: _,
                new_comment,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.comment = new_comment.clone();
                }
                Ok(())
            }
            Operation::SetSignalValueDescriptions {
                message_id,
                signal_name,
                old_descriptions: _,
                new_descriptions,
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.value_descriptions = new_descriptions.clone();
                }
                Ok(())
            }
            Operation::SetMessageAttribute {
                message_id,
                name,
                new_value,
                ..
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                {
                    msg.apply_attribute(name, new_value.clone());
                }
                Ok(())
            }
            Operation::SetSignalAttribute {
                message_id,
                signal_name,
                name,
                new_value,
                ..
            } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                    && let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name)
                {
                    sig.apply_attribute(name, new_value.clone());
                }
                Ok(())
            }
            Operation::AddMessage { message } => {
                self.messages.push(message.clone());
                Ok(())
            }
            Operation::DeleteMessage { message } => {
                self.messages.retain(|m| m.message_id != message.message_id);
                Ok(())
            }
            Operation::AddSignal { message_id, signal } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                {
                    msg.signals.push(signal.clone());
                }
                Ok(())
            }
            Operation::DeleteSignal { message_id, signal } => {
                if let Some(msg) = self
                    .messages
                    .iter_mut()
                    .find(|m| m.message_id == *message_id)
                {
                    msg.signals.retain(|s| s.name != signal.name);
                }
                Ok(())
            }
            Operation::AddNode { name } => {
                self.nodes.push(name.clone());
                Ok(())
            }
            Operation::DeleteNode { name } => {
                self.nodes.retain(|n| n != name);
                Ok(())
            }
            Operation::RenameNode { old_name, new_name } => {
                if let Some(node) = self
                    .nodes
                    .iter_mut()
                    .find(|n| n.as_str() == old_name.as_str())
                {
                    *node = new_name.clone();
                }
                Ok(())
            }
        }
    }

    pub fn to_dbc_string(&self) -> String {
        let mut out = String::new();

        out.push_str("VERSION \"\"\n\n");

        out.push_str("NS_ :\n\n");

        out.push_str("BS_:\n\n");

        out.push_str("BU_:");
        for node in &self.nodes {
            out.push(' ');
            out.push_str(node);
        }
        out.push('\n');
        out.push('\n');

        for msg in &self.messages {
            let raw_id = msg.frame_format.raw_id(msg.message_id);
            out.push_str(&format!(
                "BO_ {} {}: {} {}\n",
                raw_id, msg.message_name, msg.message_size, msg.transmitter
            ));

            for sig in &msg.signals {
                let byte_order_char = match sig.byte_order {
                    ByteOrder::LittleEndian => '1',
                    ByteOrder::BigEndian => '0',
                };
                let value_type_char = match sig.value_type {
                    ValueType::Unsigned => '+',
                    ValueType::Signed => '-',
                };
                let mux = match &sig.multiplexer_indicator {
                    MultiplexIndicator::Plain => String::new(),
                    MultiplexIndicator::Multiplexor => " M".to_string(),
                    MultiplexIndicator::MultiplexedSignal(n) => format!(" m{}", n),
                    MultiplexIndicator::MultiplexorAndMultiplexedSignal(n) => {
                        format!(" m{}M", n)
                    }
                };
                let receivers = if sig.receivers.is_empty() {
                    "Vector__XXX".to_string()
                } else {
                    sig.receivers.join(",")
                };

                out.push_str(&format!(
                    " SG_ {}{} : {}|{}@{}{} ({},{}) [{}|{}] \"{}\" {}\n",
                    sig.name,
                    mux,
                    sig.start_bit,
                    sig.signal_size,
                    byte_order_char,
                    value_type_char,
                    sig.factor,
                    sig.offset,
                    sig.min,
                    sig.max,
                    sig.unit,
                    receivers
                ));
            }
            out.push('\n');
        }

        for msg in &self.messages {
            let raw_id = msg.frame_format.raw_id(msg.message_id);
            if !msg.comment.is_empty() {
                out.push_str(&format!(
                    "CM_ BO_ {} \"{}\";\n",
                    raw_id,
                    msg.comment.replace('\\', "\\\\").replace('"', "\\\"")
                ));
            }
            for sig in &msg.signals {
                if !sig.comment.is_empty() {
                    out.push_str(&format!(
                        "CM_ SG_ {} {} \"{}\";\n",
                        raw_id,
                        sig.name,
                        sig.comment.replace('\\', "\\\\").replace('"', "\\\"")
                    ));
                }
            }
        }

        for msg in &self.messages {
            let raw_id = msg.frame_format.raw_id(msg.message_id);
            for sig in &msg.signals {
                if !sig.value_descriptions.is_empty() {
                    let entries: Vec<String> = sig
                        .value_descriptions
                        .iter()
                        .map(|(val, desc)| {
                            format!(
                                "{} \"{}\"",
                                val,
                                desc.replace('\\', "\\\\").replace('"', "\\\"")
                            )
                        })
                        .collect();
                    out.push_str(&format!(
                        "VAL_ {} {} {} ;\n",
                        raw_id,
                        sig.name,
                        entries.join(" ")
                    ));
                }
            }
        }

        // 存在 CAN FD 消息时，写出 Vector 风格的 VFrameFormat / BusType 属性
        if self.messages.iter().any(|m| m.frame_format.is_fd()) {
            let enum_values = VFRAME_FORMAT_ENUM
                .iter()
                .map(|s| format!("\"{}\"", s))
                .collect::<Vec<_>>()
                .join(",");
            out.push_str(&format!(
                "BA_DEF_ BO_ \"VFrameFormat\" ENUM {};\n",
                enum_values
            ));
            out.push_str("BA_DEF_DEF_ \"VFrameFormat\" \"StandardCAN\";\n");
            out.push_str("BA_DEF_ \"BusType\" STRING ;\n");
            out.push_str("BA_DEF_DEF_ \"BusType\" \"CAN\";\n");
            out.push_str("BA_ \"BusType\" \"CAN FD\";\n");
            for msg in &self.messages {
                out.push_str(&format!(
                    "BA_ \"VFrameFormat\" BO_ {} {};\n",
                    msg.frame_format.raw_id(msg.message_id),
                    msg.frame_format.vframe_format_index()
                ));
            }
        }

        // 属性：BA_DEF_ 声明 -> BA_DEF_DEF_ 默认值 -> BA_ 取值。
        // 只写用到的声明（有取值或有默认值），没被引用的空声明不落盘。
        let used_names = |target: AttrTarget| -> std::collections::HashSet<String> {
            let mut names = std::collections::HashSet::new();
            match target {
                AttrTarget::Message => {
                    for m in &self.messages {
                        for (name, _) in m.attributes() {
                            names.insert(name.clone());
                        }
                    }
                }
                AttrTarget::Signal => {
                    for m in &self.messages {
                        for s in m.signals() {
                            for (name, _) in s.attributes() {
                                names.insert(name.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
            names
        };
        let mut wrote_any = false;
        for target in [
            AttrTarget::Message,
            AttrTarget::Signal,
            AttrTarget::Node,
            AttrTarget::Network,
        ] {
            let used = used_names(target);
            for def in self
                .attribute_definitions
                .iter()
                .filter(|d| d.target == target && (used.contains(&d.name) || d.default.is_some()))
            {
                wrote_any = true;
                let keyword = if def.target == AttrTarget::Network {
                    String::new()
                } else {
                    format!("{} ", def.target.keyword())
                };
                let type_text = match &def.kind {
                    AttrType::Int { min, max } => format!("INT {min} {max}"),
                    AttrType::Hex { min, max } => format!("HEX {min} {max}"),
                    AttrType::Float { min, max } => format!(
                        "FLOAT {} {}",
                        AttrValue::Float(*min).display(),
                        AttrValue::Float(*max).display()
                    ),
                    AttrType::String => "STRING".to_string(),
                    AttrType::Enum(values) => format!(
                        "ENUM {}",
                        values
                            .iter()
                            .map(|v| format!("\"{}\"", v.replace('"', "\\\"")))
                            .collect::<Vec<_>>()
                            .join(",")
                    ),
                };
                out.push_str(&format!(
                    "BA_DEF_ {}\"{}\" {};\n",
                    keyword, def.name, type_text
                ));
            }
        }
        for def in self
            .attribute_definitions
            .iter()
            .filter(|d| d.default.is_some())
        {
            out.push_str(&format!(
                "BA_DEF_DEF_ \"{}\" {};\n",
                def.name,
                def.default
                    .as_ref()
                    .map(|v| v.to_dbc_text())
                    .unwrap_or_default()
            ));
        }
        if wrote_any {
            for msg in &self.messages {
                let raw_id = msg.frame_format.raw_id(msg.message_id);
                for (name, value) in msg.attributes() {
                    out.push_str(&format!(
                        "BA_ \"{}\" BO_ {} {};\n",
                        name,
                        raw_id,
                        value.to_dbc_text()
                    ));
                }
                for sig in msg.signals() {
                    for (name, value) in sig.attributes() {
                        out.push_str(&format!(
                            "BA_ \"{}\" SG_ {} {} {};\n",
                            name,
                            raw_id,
                            sig.name(),
                            value.to_dbc_text()
                        ));
                    }
                }
            }
        }

        out
    }
}

/// 计算 DBC 信号占用的绝对位号（起始位为字节内位号，bit N = 字节 N/8 的第 N%8 位）。
/// Intel（小端）位号线性递增；Motorola（大端）字节内递减、跨字节折行。
pub fn get_signal_bit_positions(start_bit: u64, size: u64, byte_order: &ByteOrder) -> Vec<usize> {
    match byte_order {
        ByteOrder::LittleEndian => (start_bit..start_bit + size).map(|b| b as usize).collect(),
        ByteOrder::BigEndian => {
            let mut out = Vec::with_capacity(size as usize);
            let mut bit = start_bit as i64;
            for _ in 0..size {
                if bit < 0 {
                    break;
                }
                out.push(bit as usize);
                if bit % 8 == 0 {
                    bit += 15;
                } else {
                    bit -= 1;
                }
            }
            out
        }
    }
}

/// 从 `start` 起找第一个未占用的消息 ID；超过 29 位扩展上限则从 1 开始找空洞
pub fn next_free_message_id(dbc: &EditableDbc, start: u32) -> u32 {
    const MAX_ID: u32 = 0x1FFF_FFFF;
    let used: std::collections::HashSet<u32> = dbc.messages.iter().map(|m| m.message_id).collect();
    for cand in start..=MAX_ID {
        if !used.contains(&cand) {
            return cand;
        }
    }
    for cand in 1..start.min(MAX_ID) {
        if !used.contains(&cand) {
            return cand;
        }
    }
    1
}

/// DBC 标识符规则（C 风格）：字母或下划线开头，仅含字母、数字、下划线
pub fn is_valid_dbc_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// 从 DBC 属性解析消息的帧格式（含 CAN FD 标志）
///
/// 优先读取 VFrameFormat 消息属性（Vector 风格 ENUM）；
/// 缺失时若 DLC > 8 则视为 CAN FD，否则按 ID 高位判断标准/扩展。
fn parse_frame_format(dbc: &Dbc, msg: &Message) -> FrameFormat {
    let vframe = dbc
        .attribute_definitions
        .iter()
        .find_map(|d| match d {
            AttributeDefinition::Message(name, AttributeValueType::Enum(values))
                if name == "VFrameFormat" =>
            {
                Some(values.clone())
            }
            _ => None,
        })
        .and_then(|values| {
            dbc.message_attribute(msg.id, "VFrameFormat")
                .and_then(|v| match v {
                    can_dbc::AttributeValue::Uint(idx) => values.get(*idx as usize).cloned(),
                    _ => None,
                })
        })
        .and_then(|s| FrameFormat::from_vframe_format(&s));

    vframe.unwrap_or_else(|| {
        let extended = matches!(msg.id, MessageId::Extended(_));
        FrameFormat::compose(extended, msg.size > 8)
    })
}

#[allow(dead_code)]
impl EditableMessage {
    pub fn new() -> Self {
        Self {
            message_id: 0,
            frame_format: FrameFormat::Standard,
            message_name: String::new(),
            message_size: 0,
            transmitter: "Vector__XXX".to_string(),
            signals: Vec::new(),
            comment: String::new(),
            attributes: Vec::new(),
        }
    }

    pub fn build(
        message_id: u32,
        frame_format: FrameFormat,
        message_name: String,
        message_size: u64,
        transmitter: String,
        signals: Vec<EditableSignal>,
        comment: String,
    ) -> Self {
        Self {
            message_id,
            frame_format,
            message_name,
            message_size,
            transmitter,
            signals,
            comment,
            attributes: Vec::new(),
        }
    }

    pub fn copy_without_signals(&self) -> Self {
        Self {
            message_id: self.message_id,
            frame_format: self.frame_format,
            message_name: self.message_name.clone(),
            message_size: self.message_size,
            transmitter: self.transmitter.clone(),
            signals: Vec::new(),
            comment: self.comment.clone(),
            attributes: self.attributes.clone(),
        }
    }

    fn from_message(msg: &Message, comment: &str, frame_format: FrameFormat) -> Self {
        let signals = msg
            .signals
            .iter()
            .map(EditableSignal::from_signal)
            .collect();

        Self {
            message_id: msg.id.raw() & 0x1FFF_FFFF,
            frame_format,
            message_name: msg.name.clone(),
            message_size: msg.size,
            transmitter: msg
                .transmitter
                .clone()
                .unwrap_or_else(|| "Vector__XXX".to_string()),
            signals,
            comment: comment.to_string(),
            attributes: Vec::new(),
        }
    }

    pub fn signals_count(&self) -> usize {
        self.signals.len()
    }

    pub fn message_id(&self) -> u32 {
        self.message_id
    }
    pub fn frame_format(&self) -> FrameFormat {
        self.frame_format
    }
    pub fn is_extended(&self) -> bool {
        self.frame_format.is_extended()
    }
    pub fn message_name(&self) -> &str {
        &self.message_name
    }
    pub fn message_size(&self) -> u64 {
        self.message_size
    }
    pub fn transmitter(&self) -> &str {
        &self.transmitter
    }
    pub fn signals(&self) -> &Vec<EditableSignal> {
        &self.signals
    }
    pub fn comment(&self) -> &str {
        &self.comment
    }

    /// 报文级属性列表（属性名 -> 值）
    pub fn attributes(&self) -> &[(String, AttrValue)] {
        &self.attributes
    }

    pub fn attribute(&self, name: &str) -> Option<&AttrValue> {
        self.attributes
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v)
    }

    /// 直接写入属性值，不进撤销历史（撤销由 EditableDbc 的 setter 负责）
    pub(crate) fn apply_attribute(&mut self, name: &str, value: Option<AttrValue>) {
        self.attributes.retain(|(k, _)| k != name);
        if let Some(v) = value {
            self.attributes.push((name.to_string(), v));
        }
    }

    pub fn set_message_id(&mut self, id: u32) {
        self.message_id = id;
    }

    pub fn set_message_name(&mut self, name: &str) {
        self.message_name = name.to_string();
    }
}

#[allow(dead_code)]
impl Default for EditableSignal {
    fn default() -> Self {
        Self::new()
    }
}

impl EditableSignal {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            multiplexer_indicator: MultiplexIndicator::Plain,
            start_bit: 0,
            signal_size: 0,
            byte_order: ByteOrder::LittleEndian,
            value_type: ValueType::Unsigned,
            factor: 1.0,
            offset: 0.0,
            min: 0.0,
            max: 0.0,
            unit: String::new(),
            receivers: Vec::new(),
            comment: String::new(),
            value_descriptions: Vec::new(),
            attributes: Vec::new(),
        }
    }

    pub fn build(
        name: String,
        start_bit: u64,
        signal_size: u64,
        byte_order: ByteOrder,
        value_type: ValueType,
        factor: f64,
        offset: f64,
        min: f64,
        max: f64,
        unit: String,
        receivers: Vec<String>,
        value_descriptions: Vec<(i64, String)>,
        comment: String,
    ) -> Self {
        Self {
            name,
            multiplexer_indicator: MultiplexIndicator::Plain,
            start_bit,
            signal_size,
            byte_order,
            value_type,
            factor,
            offset,
            min,
            max,
            unit,
            receivers,
            comment,
            value_descriptions,
            attributes: Vec::new(),
        }
    }

    fn from_signal(sig: &Signal) -> Self {
        Self {
            name: sig.name.to_string(),
            multiplexer_indicator: sig.multiplexer_indicator,
            start_bit: sig.start_bit,
            signal_size: sig.size,
            byte_order: sig.byte_order,
            value_type: sig.value_type,
            factor: sig.factor,
            offset: sig.offset,
            min: numeric_to_f64(&sig.min),
            max: numeric_to_f64(&sig.max),
            unit: sig.unit.clone(),
            receivers: sig.receivers.clone(),
            comment: String::new(),
            value_descriptions: Vec::new(),
            attributes: Vec::new(),
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn start_bit(&self) -> u64 {
        self.start_bit
    }
    pub fn signal_size(&self) -> u64 {
        self.signal_size
    }
    pub fn byte_order(&self) -> &ByteOrder {
        &self.byte_order
    }
    pub fn value_type(&self) -> &ValueType {
        &self.value_type
    }
    pub fn factor(&self) -> f64 {
        self.factor
    }
    pub fn offset(&self) -> f64 {
        self.offset
    }
    pub fn min(&self) -> f64 {
        self.min
    }
    pub fn max(&self) -> f64 {
        self.max
    }
    pub fn unit(&self) -> &str {
        &self.unit
    }
    pub fn receivers(&self) -> &Vec<String> {
        &self.receivers
    }
    pub fn comment(&self) -> &str {
        &self.comment
    }

    /// 信号级属性列表（属性名 -> 值）
    pub fn attributes(&self) -> &[(String, AttrValue)] {
        &self.attributes
    }

    pub fn attribute(&self, name: &str) -> Option<&AttrValue> {
        self.attributes
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v)
    }

    pub(crate) fn apply_attribute(&mut self, name: &str, value: Option<AttrValue>) {
        self.attributes.retain(|(k, _)| k != name);
        if let Some(v) = value {
            self.attributes.push((name.to_string(), v));
        }
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    pub fn value_descriptions(&self) -> &[(i64, String)] {
        &self.value_descriptions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_DBC: &str = r#"
VERSION "0.1"
NS_ :
    NS_DESC_
    CM_
    BA_DEF_
    BA_
    VAL_
    CAT_DEF_
    CAT_
    FILTER
    BA_DEF_DEF_
    EV_DATA_
    ENVVAR_DATA_
    SGTYPE_
    SGTYPE_VAL_
    BA_DEF_SGTYPE_
    BA_SGTYPE_
    SIG_TYPE_REF_
    VAL_TABLE_
    SIG_GROUP_
    SIG_VALTYPE_
    SIGTYPE_VALTYPE_
    BO_TX_BU_
    BA_DEF_REL_
    BA_REL_
    BA_DEF_DEF_REL_
    BU_SG_REL_
    BU_EV_REL_
    BU_BO_REL_
    SG_MUL_VAL_
BS_:
BU_: PC
BO_ 2000 WebData_2000: 4 Vector__XXX
    SG_ Signal_8 : 24|8@1+ (1,0) [0|255] "" Vector__XXX
    SG_ Signal_7 : 16|8@1+ (1,0) [0|255] "" Vector__XXX
    SG_ Signal_6 : 8|8@1+ (1,0) [0|255] "" Vector__XXX
    SG_ Signal_5 : 0|8@1+ (1,0) [0|255] "" Vector__XXX
BO_ 1840 WebData_1840: 4 PC
    SG_ Signal_4 : 24|8@1+ (1,0) [0|255] "" Vector__XXX
    SG_ Signal_3 : 16|8@1+ (1,0) [0|255] "" Vector__XXX
    SG_ Signal_2 : 8|8@1+ (1,0) [0|255] "" Vector__XXX
    SG_ Signal_1 : 0|8@1+ (1,0) [0|0] "" Vector__XXX

BO_ 3040 WebData_3040: 8 Vector__XXX
    SG_ Signal_6 m2 : 0|4@1+ (1,0) [0|15] "" Vector__XXX
    SG_ Signal_5 m3 : 16|8@1+ (1,0) [0|255] "kmh" Vector__XXX
    SG_ Signal_4 m3 : 8|8@1+ (1,0) [0|255] "" Vector__XXX
    SG_ Signal_3 m3 : 0|4@1+ (1,0) [0|3] "" Vector__XXX
    SG_ Signal_2 m1 : 3|12@0+ (1,0) [0|4095] "Byte" Vector__XXX
    SG_ Signal_1 m0 : 0|4@1+ (1,0) [0|7] "Byte" Vector__XXX
    SG_ Switch M : 4|4@1+ (1,0) [0|3] "" Vector__XXX

EV_ Environment1: 0 [0|220] "" 0 6 DUMMY_NODE_VECTOR0 DUMMY_NODE_VECTOR2;
EV_ Environment2: 0 [0|177] "" 0 7 DUMMY_NODE_VECTOR1 DUMMY_NODE_VECTOR2;
ENVVAR_DATA_ SomeEnvVarData: 399;

CM_ BO_ 1840 "Some Message comment";
CM_ SG_ 1840 Signal_4 "asaklfjlsdfjlsdfgls
HH?=(%)/&KKDKFSDKFKDFKSDFKSDFNKCnvsdcvsvxkcv";
CM_ SG_ 5 TestSigLittleUnsigned1 "asaklfjlsdfjlsdfgls
=0943503450KFSDKFKDFKSDFKSDFNKCnvsdcvsvxkcv";

BA_DEF_DEF_ "BusType" "AS";

BA_ "Attr" BO_ 4358435 283;
BA_ "Attr" BO_ 56949545 344;

VAL_ 2000 Signal_3 255 "NOP";

SIG_VALTYPE_ 2000 Signal_8 : 1;
"#;

    #[test]
    fn test_from_dbc() {
        let dbc = Dbc::try_from(SAMPLE_DBC).unwrap();
        let editable_dbc = EditableDbc::from_dbc(&dbc);

        // println!("{:#?}", editable_dbc);

        assert_eq!(editable_dbc.message_count(), 3);

        let msg_2000 = editable_dbc.get_message(2000).unwrap();
        assert_eq!(msg_2000.message_name(), "WebData_2000");
        assert_eq!(msg_2000.signals_count(), 4);

        let sig_signal_8 = msg_2000
            .signals()
            .iter()
            .find(|s| s.name() == "Signal_8")
            .unwrap();
        assert_eq!(sig_signal_8.start_bit(), 24);
        assert_eq!(sig_signal_8.signal_size(), 8);
        assert_eq!(sig_signal_8.byte_order(), &ByteOrder::LittleEndian);
        assert_eq!(sig_signal_8.value_type(), &ValueType::Unsigned);
        assert_eq!(sig_signal_8.factor(), 1.0);
        assert_eq!(sig_signal_8.offset(), 0.0);
        assert_eq!(sig_signal_8.min(), 0.0);
        assert_eq!(sig_signal_8.max(), 255.0);
        assert_eq!(sig_signal_8.unit(), "");
        assert_eq!(sig_signal_8.receivers(), &Vec::<String>::new());
    }

    #[test]
    fn signal_comments_are_imported() {
        let dbc = Dbc::try_from(SAMPLE_DBC).unwrap();
        let editable = EditableDbc::from_dbc(&dbc);

        let msg = editable.get_message(1840).unwrap();
        let sig = msg
            .signals()
            .iter()
            .find(|s| s.name() == "Signal_4")
            .unwrap();
        assert!(
            sig.comment().contains("asaklfjlsdfjlsdfgls"),
            "signal comment should be imported"
        );
    }

    /// EditableDbc -> String -> Dbc -> EditableDbc 回环
    fn round_trip(dbc: &EditableDbc) -> EditableDbc {
        let text = dbc.to_dbc_string();
        let parsed = Dbc::try_from(text.as_str())
            .unwrap_or_else(|e| panic!("to_dbc_string output must re-parse: {e:?}"));
        EditableDbc::from_dbc(&parsed)
    }

    #[test]
    fn extended_frame_comment_survives_round_trip() {
        let msg = EditableMessage::build(
            0x1F337,
            FrameFormat::Extended,
            "ExtMsg".to_string(),
            8,
            "Vector__XXX".to_string(),
            Vec::new(),
            "extended comment".to_string(),
        );
        let mut dbc = EditableDbc::new();
        dbc.add_message(&msg);

        let raw_id: u32 = 0x1F337 | 0x8000_0000;
        let text = dbc.to_dbc_string();
        assert!(
            text.contains(&format!("CM_ BO_ {} \"extended comment\"", raw_id)),
            "CM_ BO_ must use the raw id for extended frames"
        );

        let re = round_trip(&dbc);
        let m = re.get_message(0x1F337).unwrap();
        assert_eq!(m.comment(), "extended comment");
        assert_eq!(m.frame_format(), FrameFormat::Extended);
    }

    #[test]
    fn can_fd_frame_round_trips_with_vframe_format() {
        let sig = EditableSignal::build(
            "FdSig".to_string(),
            0,
            8,
            ByteOrder::LittleEndian,
            ValueType::Unsigned,
            1.0,
            0.0,
            0.0,
            255.0,
            String::new(),
            Vec::new(),
            Vec::new(),
            String::new(),
        );
        let std_fd = EditableMessage::build(
            0x123,
            FrameFormat::StandardFd,
            "FdMsg".to_string(),
            32,
            "Vector__XXX".to_string(),
            vec![sig],
            "fd comment".to_string(),
        );
        let mut dbc = EditableDbc::new();
        dbc.add_node("ECU1".into());
        dbc.add_message(&std_fd);

        let text = dbc.to_dbc_string();
        assert!(
            text.contains("BA_DEF_ BO_ \"VFrameFormat\" ENUM"),
            "must emit VFrameFormat enum"
        );
        assert!(text.contains("BA_ \"BusType\" \"CAN FD\";"));

        let re = round_trip(&dbc);
        let m = re.get_message(0x123).unwrap();
        assert_eq!(m.frame_format(), FrameFormat::StandardFd);
        assert_eq!(m.message_size(), 32);
        assert_eq!(m.comment(), "fd comment");
    }

    #[test]
    fn node_rename_propagates_and_undoes() {
        let sig = EditableSignal::build(
            "Sig".to_string(),
            0,
            8,
            ByteOrder::LittleEndian,
            ValueType::Unsigned,
            1.0,
            0.0,
            0.0,
            0.0,
            String::new(),
            vec!["OldNode".to_string()],
            Vec::new(),
            String::new(),
        );
        let msg = EditableMessage::build(
            0x100,
            FrameFormat::Standard,
            "Msg".to_string(),
            8,
            "OldNode".to_string(),
            vec![sig],
            String::new(),
        );
        let mut dbc = EditableDbc::new();
        dbc.add_node("OldNode".into());
        dbc.add_message(&msg);

        dbc.rename_node("OldNode", "NewNode");
        assert_eq!(dbc.nodes()[0], "NewNode");
        let m = dbc.get_message(0x100).unwrap();
        assert_eq!(m.transmitter(), "NewNode");
        assert_eq!(m.signals()[0].receivers(), &vec!["NewNode".to_string()]);
        // RenameNode + transmitter + receivers 合并为单步撤销
        assert!(dbc.can_undo());

        dbc.undo().unwrap();
        assert_eq!(dbc.nodes()[0], "OldNode");
        let m = dbc.get_message(0x100).unwrap();
        assert_eq!(m.transmitter(), "OldNode");
        assert_eq!(m.signals()[0].receivers(), &vec!["OldNode".to_string()]);
    }

    #[test]
    fn validation_flags_fd_and_range_issues() {
        let bad_sig = EditableSignal::build(
            "BadSig".to_string(),
            0,
            8,
            ByteOrder::LittleEndian,
            ValueType::Unsigned,
            0.0, // factor 0 -> error
            0.0,
            10.0, // min > max -> warning
            0.0,
            String::new(),
            vec!["Ghost".to_string()], // 未定义节点 -> warning
            Vec::new(),
            String::new(),
        );
        let classic_oversize = EditableMessage::build(
            0x200,
            FrameFormat::Standard,
            "TooBig".to_string(),
            12, // 经典 CAN 超过 8 -> error
            "Vector__XXX".to_string(),
            vec![bad_sig],
            String::new(),
        );
        let mut dbc = EditableDbc::new();
        dbc.add_message(&classic_oversize);

        let issues = dbc.validate();
        let messages: Vec<&str> = issues.iter().map(|i| i.message.as_str()).collect();
        assert!(
            messages
                .iter()
                .any(|m| m.contains("exceeds classic CAN limit"))
        );
        assert!(messages.iter().any(|m| m.contains("factor 0")));
        assert!(messages.iter().any(|m| m.contains("min 10 > max 0")));
        assert!(
            messages
                .iter()
                .any(|m| m.contains("not defined in the node list"))
        );

        // 改为 CAN FD 后 DLC 12 合法
        dbc.set_message_frame_format(0x200, FrameFormat::StandardFd);
        let issues = dbc.validate();
        assert!(!issues.iter().any(|i| i.message.contains("exceeds")));
    }

    #[test]
    fn gbk_dbc_content_survives_decode_and_round_trip() {
        // 模拟 CANdb++（ANSI/GBK）导出的 DBC：中文消息注释与值表
        let utf8_text = r#"VERSION ""

NS_ :

BS_:

BU_: PC

BO_ 100 Msg1: 8 PC
 SG_ Sig1 : 0|8@1+ (1,0) [0|255] "" PC

CM_ BO_ 100 "测试消息1";
CM_ SG_ 100 Sig1 "测试信号1";

VAL_ 100 Sig1 1 "开启" 0 "关闭";
"#;
        let (gbk_bytes, _, _) = encoding_rs::GBK.encode(utf8_text);
        // 解码后应还原为 UTF-8 文本并标注 GBK
        let decoded = crate::file_encoding::decode_file_bytes(&gbk_bytes);
        assert_eq!(decoded.encoding, encoding_rs::GBK);
        assert!(decoded.text.contains("测试消息1"));

        // can-dbc 语法接受 UTF-8 中文注释
        let parsed = Dbc::try_from(decoded.text.as_str())
            .unwrap_or_else(|e| panic!("UTF-8 Chinese DBC must parse: {e:?}"));
        let editable = EditableDbc::from_dbc(&parsed);
        let msg = editable.get_message(100).unwrap();
        assert_eq!(msg.comment(), "测试消息1");
        let sig = msg.signals()[0].clone();
        assert_eq!(sig.comment(), "测试信号1");
        assert_eq!(
            sig.value_descriptions(),
            &vec![(1i64, "开启".to_string()), (0i64, "关闭".to_string())]
        );

        // 保存回 GBK 应能还原为原始字节
        let out = crate::file_encoding::encode_to_bytes(
            &editable.to_dbc_string(),
            encoding_rs::GBK,
            false,
        );
        let redecoded = crate::file_encoding::decode_file_bytes(&out);
        assert!(redecoded.text.contains("测试消息1"));
        assert_eq!(decoded.encoding, redecoded.encoding);
    }

    #[test]
    fn motorola_overflow_is_detected_via_bit_positions() {
        // Motorola 信号：起始位 7（MSB），长度 16，在 1 字节消息中越界
        let sig = EditableSignal::build(
            "MotorolaSig".to_string(),
            7,
            16,
            ByteOrder::BigEndian,
            ValueType::Unsigned,
            1.0,
            0.0,
            0.0,
            0.0,
            String::new(),
            Vec::new(),
            Vec::new(),
            String::new(),
        );
        let msg = EditableMessage::build(
            0x300,
            FrameFormat::Standard,
            "MMsg".to_string(),
            1,
            "Vector__XXX".to_string(),
            vec![sig],
            String::new(),
        );
        let mut dbc = EditableDbc::new();
        dbc.add_message(&msg);

        // start_bit + size = 23 > 8 会误报；按实际位号 7..0 + 15..8 判断同样越界
        let issues = dbc.validate();
        assert!(
            issues
                .iter()
                .any(|i| i.message.contains("exceeds message size"))
        );

        // 2 字节消息中不越界（即使 start_bit + size = 23 > 16）
        dbc.set_message_size(0x300, 2);
        let issues = dbc.validate();
        assert!(
            !issues
                .iter()
                .any(|i| i.message.contains("exceeds message size"))
        );
    }

    /// 带属性声明与取值的 DBC 文本
    const ATTR_DBC: &str = r#"
VERSION "0.1"
NS_ :
    CM_
    BA_DEF_
    BA_
    VAL_
    BA_DEF_DEF_
BS_:
BU_: ECU1 ECU2
BO_ 100 Msg1: 1 ECU1
    SG_ Sig1 : 0|8@1+ (1,0) [0|255] "" ECU2

BA_DEF_ BO_  "CycleTime" INT 5 1000;
BA_DEF_ BO_  "GenMsgSendType" ENUM  "CeaseTransmission","Cycle","Event","IfActive","NotUsed";
BA_DEF_ SG_  "GenSigSendType" ENUM  "ACCOND","CYCLE","EVENT","NOT_USED";
BA_DEF_DEF_  "CycleTime" 100;
BA_ "CycleTime" BO_ 100 133;
BA_ "GenMsgSendType" BO_ 100 "Cycle";
BA_ "GenSigSendType" SG_ 100 Sig1 "EVENT";
"#;

    fn load(text: &str) -> EditableDbc {
        EditableDbc::from_dbc(&Dbc::try_from(text).expect("dbc text should parse"))
    }

    #[test]
    fn reads_attribute_definitions_and_values() {
        let dbc = load(ATTR_DBC);

        let def = dbc
            .attribute_definition("CycleTime", AttrTarget::Message)
            .expect("CycleTime 声明应被读到");
        assert_eq!(def.kind, AttrType::Int { min: 5, max: 1000 });
        assert_eq!(def.default, Some(AttrValue::Int(100)));

        // 没显式赋值的报文回落到默认值
        assert_eq!(
            dbc.message_attribute(100, "CycleTime"),
            Some(AttrValue::Int(133))
        );
        assert_eq!(
            dbc.signal_attribute(100, "Sig1", "GenSigSendType"),
            Some(AttrValue::Text("EVENT".to_string()))
        );
        assert_eq!(
            dbc.attribute_definition("GenMsgSendType", AttrTarget::Message)
                .map(|d| d.kind.clone()),
            Some(AttrType::Enum(vec![
                "CeaseTransmission".to_string(),
                "Cycle".to_string(),
                "Event".to_string(),
                "IfActive".to_string(),
                "NotUsed".to_string()
            ]))
        );
    }

    #[test]
    fn attributes_survive_save_and_reload() {
        let dbc = load(ATTR_DBC);
        let text = dbc.to_dbc_string();
        assert!(text.contains("BA_DEF_ BO_ \"CycleTime\" INT 5 1000;"));
        assert!(text.contains("BA_ \"CycleTime\" BO_ 100 133;"));
        assert!(text.contains("BA_ \"GenMsgSendType\" BO_ 100 \"Cycle\";"));
        assert!(text.contains("BA_ \"GenSigSendType\" SG_ 100 Sig1 \"EVENT\";"));

        let back = load(&text);
        assert_eq!(
            back.message_attribute(100, "CycleTime"),
            Some(AttrValue::Int(133))
        );
        assert_eq!(
            back.message_attribute(100, "GenMsgSendType"),
            Some(AttrValue::Text("Cycle".to_string()))
        );
        assert_eq!(
            back.signal_attribute(100, "Sig1", "GenSigSendType"),
            Some(AttrValue::Text("EVENT".to_string()))
        );
        assert_eq!(
            back.attribute_definition("CycleTime", AttrTarget::Message)
                .map(|d| d.default.clone()),
            Some(Some(AttrValue::Int(100))),
            "BA_DEF_DEF_ 的默认值要留住"
        );
    }

    /// 两个周期属性名同时声明时，取 Vector 约定的 GenMsgCycleTime
    const BOTH_CYCLE_DBC: &str = r#"
VERSION "0.1"
NS_ :
    CM_
    BA_DEF_
    BA_
    VAL_
    BA_DEF_DEF_
BS_:
BU_: ECU1
BO_ 100 Msg1: 1 ECU1
    SG_ Sig1 : 0|8@1+ (1,0) [0|255] "" Vector__XXX

BA_DEF_ BO_  "GenMsgCycleTime" INT 0 100000;
BA_DEF_ BO_  "CycleTime" INT 5 1000;
BA_ "GenMsgCycleTime" BO_ 100 20;
BA_ "CycleTime" BO_ 100 133;
"#;

    #[test]
    fn cycle_time_prefers_genmsgcycletime_then_falls_back_to_cycletime() {
        // 只声明 CycleTime 的文件（motbus.dbc 那种）也能取到
        let only_legacy = load(ATTR_DBC);
        assert_eq!(
            only_legacy.message_cycle_time(100),
            Some(AttrValue::Int(133)),
            "文件只有 CycleTime 时应退到它"
        );

        let both = load(BOTH_CYCLE_DBC);
        assert_eq!(
            both.message_cycle_time(100),
            Some(AttrValue::Int(20)),
            "两个都声明时应取 GenMsgCycleTime"
        );

        // 谁都没声明时不给值，界面上显示 "-"
        let none = load(SAMPLE_DBC);
        assert_eq!(none.message_cycle_time(2000), None);
    }

    #[test]
    fn setting_an_attribute_synthesizes_a_definition_and_undoes() {
        let mut dbc = load(SAMPLE_DBC);
        dbc.set_message_attribute(
            2000,
            attr_names::MSG_CYCLE_TIME,
            Some(AttrValue::Float(20.0)),
        );
        assert_eq!(
            dbc.message_attribute(2000, attr_names::MSG_CYCLE_TIME),
            Some(AttrValue::Float(20.0))
        );
        assert!(
            dbc.attribute_definition(attr_names::MSG_CYCLE_TIME, AttrTarget::Message)
                .is_some(),
            "没声明过的属性被赋值时应补一条 BA_DEF_"
        );
        assert!(
            dbc.to_dbc_string()
                .contains("BA_DEF_ BO_ \"GenMsgCycleTime\"")
        );

        dbc.undo().unwrap();
        assert_eq!(
            dbc.message_attribute(2000, attr_names::MSG_CYCLE_TIME),
            None
        );
        dbc.redo().unwrap();
        assert_eq!(
            dbc.message_attribute(2000, attr_names::MSG_CYCLE_TIME),
            Some(AttrValue::Float(20.0))
        );
    }

    #[test]
    fn validate_flags_attribute_values_outside_the_declaration() {
        let mut dbc = load(ATTR_DBC);
        // 2000 超出 CycleTime 声明的 5..1000，"Nope" 不在 GenMsgSendType 枚举里
        dbc.set_message_attribute(100, "CycleTime", Some(AttrValue::Int(2000)));
        dbc.set_message_attribute(
            100,
            "GenMsgSendType",
            Some(AttrValue::Text("Nope".to_string())),
        );
        let issues = dbc.validate();
        assert!(
            issues
                .iter()
                .any(|i| i.message.contains("CycleTime") && i.message.contains("outside")),
            "范围外的整数属性值应报出来: {:?}",
            issues.iter().map(|i| &i.message).collect::<Vec<_>>()
        );
        assert!(
            issues
                .iter()
                .any(|i| i.message.contains("GenMsgSendType") && i.message.contains("not one of")),
            "枚举外的取值应报出来"
        );
    }
}
