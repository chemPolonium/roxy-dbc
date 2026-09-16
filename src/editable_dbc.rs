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
    history: Vec<Operation>,
    compound_counts: Vec<usize>,
    redo_history: Vec<Operation>,
    redo_compound_counts: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameFormat {
    Standard,
    Extended,
    /// CAN FD 帧，11 位 ID
    StandardFd,
    /// CAN FD 帧，29 位扩展 ID
    ExtendedFd,
}

impl Default for FrameFormat {
    fn default() -> Self {
        FrameFormat::Standard
    }
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
}

#[allow(dead_code)]
impl EditableDbc {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            messages: Vec::new(),
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
            history: Vec::new(),
            compound_counts: Vec::new(),
            redo_history: Vec::new(),
            redo_compound_counts: Vec::new(),
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
        self.push_compound(Operation::AddNode { name: name.to_string() });
    }

    pub fn delete_node(&mut self, name: &str) {
        if !self.nodes.iter().any(|n| n == name) {
            return;
        }
        self.nodes.retain(|n| n != name);
        self.push_compound(Operation::DeleteNode { name: name.to_string() });
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
                        .map(|r| if r == old_name { new_name.to_string() } else { r.clone() })
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
        let mut seen_names: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

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
                        if msg.frame_format.is_extended() { "extended" } else { "standard" },
                        id_limit
                    ),
                });
            }

            // 发送节点应存在于 BU_ 列表中
            if !msg.transmitter.is_empty()
                && msg.transmitter != "Vector__XXX"
                && !self.nodes.iter().any(|n| *n == msg.transmitter)
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
            let mut seen_signals: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

            for (sig_idx, sig) in msg.signals.iter().enumerate() {
                // 按实际字节序计算占用的位（Motorola 起始位是 MSB，不能简单相加）
                let positions = get_signal_bit_positions(sig.start_bit, sig.signal_size, &sig.byte_order);
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

        issues
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
                }
                em
            })
            .collect();

        editable_dbc
    }

    pub fn get_message(&self, message_id: u32) -> Option<&EditableMessage> {
        self.messages.iter().find(|m| m.message_id == message_id)
    }

    pub fn find_message_index(&self, message_id: u32) -> Option<usize> {
        self.messages
            .iter()
            .position(|m| m.message_id == message_id)
    }

    fn find_index_signal_index(&self, message_idx: usize, signal_name: &str) -> Option<usize> {
        let msg = &self.messages[message_idx];
        return msg.signals.iter().position(|s| s.name == signal_name);
    }

    fn find_message_signal_index(
        &self,
        message_id: u32,
        signal_name: &str,
    ) -> Option<(usize, usize)> {
        if let Some(msg_idx) = self.find_message_index(message_id) {
            if let Some(sig_idx) = self.find_index_signal_index(msg_idx, signal_name) {
                return Some((msg_idx, sig_idx));
            }
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
                sig.multiplexer_indicator = new_indicator.clone();
                old_indicator
            } else {
                return;
            }
        };

        self.push_compound(Operation::SetSignalMultiplexerIndicator {
            message_id: message_id,
            signal_name: signal_name.to_string(),
            old_indicator: old_indicator,
            new_indicator: new_indicator.clone(),
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
            message_id: message_id,
            signal_name: signal_name.to_string(),
            old_start_bit: old_start_bit,
            new_start_bit: new_start_bit,
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
            message_id: message_id,
            signal_name: signal_name.to_string(),
            old_size: old_size,
            new_size: new_size,
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
            message_id: message_id,
            signal_name: signal_name.to_string(),
            old_byte_order: old_byte_order,
            new_byte_order: new_byte_order,
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
            message_id: message_id,
            signal_name: signal_name.to_string(),
            old_value_type: old_value_type,
            new_value_type: new_value_type,
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
            message_id: message_id,
            signal_name: signal_name.to_string(),
            old_factor: old_factor,
            new_factor: new_factor,
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
            message_id: message_id,
            signal_name: signal_name.to_string(),
            old_offset: old_offset,
            new_offset: new_offset,
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
            message_id: message_id,
            signal_name: signal_name.to_string(),
            old_min: old_min,
            new_min: new_min,
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
            message_id: message_id,
            signal_name: signal_name.to_string(),
            old_max: old_max,
            new_max: new_max,
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
            message_id: message_id,
            signal_name: signal_name.to_string(),
            old_unit: old_unit,
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
            message_id: message_id,
            signal_name: signal_name.to_string(),
            old_receivers: old_receivers,
            new_receivers: new_receivers,
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
            message_id: message_id,
            signal_name: signal_name.to_string(),
            old_comment: old_comment,
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
                message_id: message_id,
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
            message_id: message_id,
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
            Operation::SetMessageFrameFormat { message_id, old_format, new_format: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    msg.frame_format = *old_format;
                }
                Ok(())
            }
            Operation::SetMessageName { message_id, old_name, new_name: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    msg.message_name = old_name.clone();
                }
                Ok(())
            }
            Operation::SetMessageSize { message_id, old_size, new_size: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    msg.message_size = *old_size;
                }
                Ok(())
            }
            Operation::SetMessageTransmitter { message_id, old_transmitter, new_transmitter: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    msg.transmitter = old_transmitter.clone();
                }
                Ok(())
            }
            Operation::SetMessageComment { message_id, old_comment, new_comment: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    msg.comment = old_comment.clone();
                }
                Ok(())
            }
            Operation::SetSignalName { message_id, signal_old_name, signal_new_name } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_new_name) {
                        sig.name = signal_old_name.clone();
                    }
                }
                Ok(())
            }
            Operation::SetSignalMultiplexerIndicator { message_id, signal_name, old_indicator, new_indicator: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.multiplexer_indicator = old_indicator.clone();
                    }
                }
                Ok(())
            }
            Operation::SetSignalStartBit { message_id, signal_name, old_start_bit, new_start_bit: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.start_bit = *old_start_bit;
                    }
                }
                Ok(())
            }
            Operation::SetSignalSize { message_id, signal_name, old_size, new_size: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.signal_size = *old_size;
                    }
                }
                Ok(())
            }
            Operation::SetSignalByteOrder { message_id, signal_name, old_byte_order, new_byte_order: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.byte_order = *old_byte_order;
                    }
                }
                Ok(())
            }
            Operation::SetSignalValueType { message_id, signal_name, old_value_type, new_value_type: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.value_type = *old_value_type;
                    }
                }
                Ok(())
            }
            Operation::SetSignalFactor { message_id, signal_name, old_factor, new_factor: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.factor = *old_factor;
                    }
                }
                Ok(())
            }
            Operation::SetSignalOffset { message_id, signal_name, old_offset, new_offset: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.offset = *old_offset;
                    }
                }
                Ok(())
            }
            Operation::SetSignalMin { message_id, signal_name, old_min, new_min: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.min = *old_min;
                    }
                }
                Ok(())
            }
            Operation::SetSignalMax { message_id, signal_name, old_max, new_max: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.max = *old_max;
                    }
                }
                Ok(())
            }
            Operation::SetSignalUnit { message_id, signal_name, old_unit, new_unit: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.unit = old_unit.clone();
                    }
                }
                Ok(())
            }
            Operation::SetSignalReceivers { message_id, signal_name, old_receivers, new_receivers: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.receivers = old_receivers.clone();
                    }
                }
                Ok(())
            }
            Operation::SetSignalComment { message_id, signal_name, old_comment, new_comment: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.comment = old_comment.clone();
                    }
                }
                Ok(())
            }
            Operation::SetSignalValueDescriptions { message_id, signal_name, old_descriptions, new_descriptions: _ } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.value_descriptions = old_descriptions.clone();
                    }
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
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    msg.signals.retain(|s| s.name != signal.name);
                }
                Ok(())
            }
            Operation::DeleteSignal { message_id, signal } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
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
                if let Some(node) = self.nodes.iter_mut().find(|n| n.as_str() == new_name.as_str()) {
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
            Operation::SetMessageFrameFormat { message_id, old_format: _, new_format } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    msg.frame_format = *new_format;
                }
                Ok(())
            }
            Operation::SetMessageName { message_id, old_name: _, new_name } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    msg.message_name = new_name.clone();
                }
                Ok(())
            }
            Operation::SetMessageSize { message_id, old_size: _, new_size } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    msg.message_size = *new_size;
                }
                Ok(())
            }
            Operation::SetMessageTransmitter { message_id, old_transmitter: _, new_transmitter } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    msg.transmitter = new_transmitter.clone();
                }
                Ok(())
            }
            Operation::SetMessageComment { message_id, old_comment: _, new_comment } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    msg.comment = new_comment.clone();
                }
                Ok(())
            }
            Operation::SetSignalName { message_id, signal_old_name, signal_new_name } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_old_name) {
                        sig.name = signal_new_name.clone();
                    }
                }
                Ok(())
            }
            Operation::SetSignalMultiplexerIndicator { message_id, signal_name, old_indicator: _, new_indicator } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.multiplexer_indicator = new_indicator.clone();
                    }
                }
                Ok(())
            }
            Operation::SetSignalStartBit { message_id, signal_name, old_start_bit: _, new_start_bit } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.start_bit = *new_start_bit;
                    }
                }
                Ok(())
            }
            Operation::SetSignalSize { message_id, signal_name, old_size: _, new_size } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.signal_size = *new_size;
                    }
                }
                Ok(())
            }
            Operation::SetSignalByteOrder { message_id, signal_name, old_byte_order: _, new_byte_order } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.byte_order = *new_byte_order;
                    }
                }
                Ok(())
            }
            Operation::SetSignalValueType { message_id, signal_name, old_value_type: _, new_value_type } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.value_type = *new_value_type;
                    }
                }
                Ok(())
            }
            Operation::SetSignalFactor { message_id, signal_name, old_factor: _, new_factor } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.factor = *new_factor;
                    }
                }
                Ok(())
            }
            Operation::SetSignalOffset { message_id, signal_name, old_offset: _, new_offset } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.offset = *new_offset;
                    }
                }
                Ok(())
            }
            Operation::SetSignalMin { message_id, signal_name, old_min: _, new_min } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.min = *new_min;
                    }
                }
                Ok(())
            }
            Operation::SetSignalMax { message_id, signal_name, old_max: _, new_max } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.max = *new_max;
                    }
                }
                Ok(())
            }
            Operation::SetSignalUnit { message_id, signal_name, old_unit: _, new_unit } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.unit = new_unit.clone();
                    }
                }
                Ok(())
            }
            Operation::SetSignalReceivers { message_id, signal_name, old_receivers: _, new_receivers } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.receivers = new_receivers.clone();
                    }
                }
                Ok(())
            }
            Operation::SetSignalComment { message_id, signal_name, old_comment: _, new_comment } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.comment = new_comment.clone();
                    }
                }
                Ok(())
            }
            Operation::SetSignalValueDescriptions { message_id, signal_name, old_descriptions: _, new_descriptions } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    if let Some(sig) = msg.signals.iter_mut().find(|s| s.name == *signal_name) {
                        sig.value_descriptions = new_descriptions.clone();
                    }
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
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
                    msg.signals.push(signal.clone());
                }
                Ok(())
            }
            Operation::DeleteSignal { message_id, signal } => {
                if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == *message_id) {
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
                if let Some(node) = self.nodes.iter_mut().find(|n| n.as_str() == old_name.as_str()) {
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
                raw_id,
                msg.message_name,
                msg.message_size,
                msg.transmitter
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
                        .map(|(val, desc)| format!("{} \"{}\"", val, desc.replace('\\', "\\\\").replace('"', "\\\"")))
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

        out
    }
}

/// 计算 DBC 信号占用的绝对位号（起始位为字节内位号，bit N = 字节 N/8 的第 N%8 位）。
/// Intel（小端）位号线性递增；Motorola（大端）字节内递减、跨字节折行。
pub fn get_signal_bit_positions(
    start_bit: u64,
    size: u64,
    byte_order: &ByteOrder,
) -> Vec<usize> {
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
            dbc.message_attribute(msg.id, "VFrameFormat").and_then(|v| match v {
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
        }
    }

    fn from_message(msg: &Message, comment: &str, frame_format: FrameFormat) -> Self {
        let signals = msg
            .signals
            .iter()
            .map(|sig| EditableSignal::from_signal(sig))
            .collect();

        Self {
            message_id: msg.id.raw() & 0x1FFF_FFFF,
            frame_format,
            message_name: msg.name.clone(),
            message_size: msg.size,
            transmitter: msg.transmitter.clone().unwrap_or_else(|| "Vector__XXX".to_string()),
            signals: signals,
            comment: comment.to_string(),
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

    pub fn set_message_id(&mut self, id: u32) {
        self.message_id = id;
    }

    pub fn set_message_name(&mut self, name: &str) {
        self.message_name = name.to_string();
    }
}

#[allow(dead_code)]
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
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn multiplexer_indicator(&self) -> &MultiplexIndicator {
        &self.multiplexer_indicator
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

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    pub fn value_descriptions(&self) -> &[(i64, String)] {
        &self.value_descriptions
    }

    pub fn set_value_descriptions(&mut self, descs: Vec<(i64, String)>) {
        self.value_descriptions = descs;
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
        let sig = msg.signals().iter().find(|s| s.name() == "Signal_4").unwrap();
        assert!(sig.comment().contains("asaklfjlsdfjlsdfgls"), "signal comment should be imported");
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
        assert!(text.contains("BA_DEF_ BO_ \"VFrameFormat\" ENUM"), "must emit VFrameFormat enum");
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
        assert!(messages.iter().any(|m| m.contains("exceeds classic CAN limit")));
        assert!(messages.iter().any(|m| m.contains("factor 0")));
        assert!(messages.iter().any(|m| m.contains("min 10 > max 0")));
        assert!(messages.iter().any(|m| m.contains("not defined in the node list")));

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
        let out = crate::file_encoding::encode_to_bytes(&editable.to_dbc_string(), encoding_rs::GBK, false);
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
        assert!(issues.iter().any(|i| i.message.contains("exceeds message size")));

        // 2 字节消息中不越界（即使 start_bit + size = 23 > 16）
        dbc.set_message_size(0x300, 2);
        let issues = dbc.validate();
        assert!(!issues.iter().any(|i| i.message.contains("exceeds message size")));
    }
}
