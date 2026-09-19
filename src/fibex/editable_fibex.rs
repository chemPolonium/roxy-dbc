// 这个文件实现了一个可编辑的 FlexRay (FIBEX) 数据结构，支持基本的编辑Actions和历史记录管理
// 外部可以读取里面的属性，但是不可以编辑
// 所有的编辑都是通过 EditableFibex 提供的方法来进行的，这些方法会记录Actions历史以支持撤销和重做功能
//
// 整体的Actions流程：
// 先使用 import 模块实现 文件字符串 -> 数据
// 然后通过 EditableFibex::from_imported 构造可编辑对象
// 然后通过 EditableFibex 提供的各种 set_xxx 方法进行编辑
// 编辑过程中允许撤回和重做
// 最后通过 export 模块将结果转换回 FIBEX 或 ARXML 文件字符串
//
// FlexRay 的层次结构：
//   Cluster（含协议参数） / ECU 列表 / PDU（含信号）/ Frame（含 PDU 映射与时隙调度）
// Frame 通过 FrameTriggering（通道、Slot、基础周期、Cycle Repetition）映射到Static Segment调度表

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

/// FlexRay 通道。A / B / 双通道同时发送
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FrChannel {
    #[default]
    A,
    B,
    Both,
}

impl FrChannel {
    pub fn label(self) -> &'static str {
        match self {
            FrChannel::A => "A",
            FrChannel::B => "B",
            FrChannel::Both => "A+B",
        }
    }

    /// 该通道选择是否覆盖通道 `ch`
    pub fn covers(self, ch: FrChannel) -> bool {
        self == FrChannel::Both || self == ch
    }
}

/// PDU 类型：Static Segment PDU / Dynamic Segment PDU / 事件型 PDU
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PduKind {
    #[default]
    Static,
    Dynamic,
    Event,
}

impl PduKind {
    pub fn label(self) -> &'static str {
        match self {
            PduKind::Static => "Static",
            PduKind::Dynamic => "Dynamic",
            PduKind::Event => "Event",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ByteOrder {
    /// Motorola / 大端（FlexRay 常用，FIBEX is-high-low-byte-order=true）
    #[default]
    BigEndian,
    /// Intel / 小端
    LittleEndian,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ValueType {
    #[default]
    Unsigned,
    Signed,
}

/// 帧的Static Segment调度属性（FIBEX FRAME-TRIGGERING / AUTOSAR FLEXRAY-FRAME-TRIGGERING）
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameTriggering {
    pub channel: FrChannel,
    /// Static SegmentSlot ID 1..=2047
    pub slot_id: u32,
    /// 基础周期 0..=63
    pub base_cycle: u32,
    /// Cycle Repetition 1/2/4/8/16/32/64
    pub cycle_repetition: u32,
    /// 是否为Startup Frame（startup frame）
    pub startup: bool,
}

impl Default for FrameTriggering {
    fn default() -> Self {
        Self {
            channel: FrChannel::A,
            slot_id: 1,
            base_cycle: 0,
            cycle_repetition: 1,
            startup: false,
        }
    }
}

impl FrameTriggering {
    /// 该触发是否在指定周期上激活
    pub fn is_active_at_cycle(&self, cycle: u32) -> bool {
        cycle % self.cycle_repetition.max(1) == self.base_cycle % self.cycle_repetition.max(1)
    }
}

/// Frame 内的 PDU 映射（FIBEX PDU-MAPPING / AUTOSAR FRAME-PDU-MAPPING）
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub struct FramePduMapping {
    pub pdu_name: String,
    /// 起始字节位置（按字节对齐，FlexRay PDU 在帧内按字节排列）
    pub start_position: u32,
}

impl FramePduMapping {
    pub fn new(pdu_name: &str, start_position: u32) -> Self {
        Self {
            pdu_name: pdu_name.to_string(),
            start_position,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct EditableSignal {
    pub(crate) name: String,
    start_bit: u32,
    /// 位长度
    length_bits: u32,
    byte_order: ByteOrder,
    value_type: ValueType,
    factor: f64,
    offset: f64,
    min: f64,
    max: f64,
    unit: String,
    receivers: Vec<String>,
    /// 发送该信号的 ECU（来自 I-SIGNAL-PORT 的 Tx 方向）
    senders: Vec<String>,
    comment: String,
    value_descriptions: Vec<(i64, String)>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct EditablePdu {
    pub(crate) name: String,
    /// 字节长度
    length: u32,
    kind: PduKind,
    signals: Vec<EditableSignal>,
    /// 发送该 PDU 的 ECU（来自 I-PDU-PORT 的 Tx 方向）
    senders: Vec<String>,
    /// 接收该 PDU 的 ECU（来自 I-PDU-PORT 的 Rx 方向）
    receivers: Vec<String>,
    comment: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct EditableFrame {
    pub(crate) name: String,
    /// 字节长度 0..=254
    length: u32,
    payload_preamble: bool,
    triggering: FrameTriggering,
    pdus: Vec<FramePduMapping>,
    comment: String,
}

/// Cluster 级协议参数（g* / gd* 参数）
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClusterParams {
    /// fx:SPEED kbit/s，通常 5000 或 10000
    pub speed_kbps: u32,
    /// gdCycle 毫秒（0.001..=0.006? 实际 1..16ms），FIBEX CYCLE-TIME-ms
    pub cycle_time_ms: f64,
    /// gdMacrotickDuration 微秒（1..=6）
    pub macrotick_duration_us: f64,
    /// gColdstartAttempts
    pub coldstart_attempts: u32,
    /// gdActionPointOffset
    pub action_point_offset: u32,
    /// gdMinislotActionPointOffset
    pub minislot_action_point_offset: u32,
    /// gdDynamicSlotIdlePhase
    pub dynamic_slot_idle_phase: u32,
    /// gdMinorVersion
    pub minor_version: u32,
    /// gdNIT (network idle time)
    pub network_idle_time: u32,
    /// gNumberOfMinislots
    pub number_of_minislots: u32,
    /// gNumberOfStaticSlots
    pub number_of_static_slots: u32,
    /// gdMinislot
    pub minislot_duration: u32,
    /// gdStaticSlot
    pub static_slot_duration: u32,
    /// gdSymbolWindow
    pub symbol_window: u32,
    /// gdSymbolWindowIdlePhase
    pub symbol_window_idle_phase: u32,
    /// gOffsetCorrectionStart
    pub offset_correction_start: u32,
}

impl Default for ClusterParams {
    fn default() -> Self {
        // FlexRay 10 Mbit/s、5ms cycle 的典型配置
        Self {
            speed_kbps: 10000,
            cycle_time_ms: 5.0,
            macrotick_duration_us: 5.0,
            coldstart_attempts: 8,
            action_point_offset: 3,
            minislot_action_point_offset: 2,
            dynamic_slot_idle_phase: 1,
            minor_version: 3,
            network_idle_time: 75,
            number_of_minislots: 292,
            number_of_static_slots: 10,
            minislot_duration: 9,
            static_slot_duration: 67,
            symbol_window: 1,
            symbol_window_idle_phase: 1,
            offset_correction_start: 233,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct EditableCluster {
    pub name: String,
    pub params: ClusterParams,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
pub struct EditableFibex {
    cluster: EditableCluster,
    ecus: Vec<String>,
    pdus: Vec<EditablePdu>,
    frames: Vec<EditableFrame>,
    history: Vec<Operation>,
    compound_counts: Vec<usize>,
    redo_history: Vec<Operation>,
    redo_compound_counts: Vec<usize>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum Operation {
    SetClusterParam {
        param: ClusterParam,
        old: ClusterValue,
        new: ClusterValue,
    },
    AddEcu {
        name: String,
    },
    DeleteEcu {
        name: String,
    },
    RenameEcu {
        old_name: String,
        new_name: String,
    },
    AddPdu {
        pdu: EditablePdu,
    },
    DeletePdu {
        pdu: EditablePdu,
        /// 删除时从各帧中一并移除的 PDU 映射（撤销时恢复）
        frame_mappings: Vec<(String, FramePduMapping)>,
    },
    SetPduName {
        old_name: String,
        new_name: String,
    },
    SetPduLength {
        pdu_name: String,
        old_length: u32,
        new_length: u32,
    },
    SetPduKind {
        pdu_name: String,
        old_kind: PduKind,
        new_kind: PduKind,
    },
    SetPduComment {
        pdu_name: String,
        old_comment: String,
        new_comment: String,
    },
    AddSignal {
        pdu_name: String,
        signal: EditableSignal,
    },
    DeleteSignal {
        pdu_name: String,
        signal: EditableSignal,
    },
    SetSignalName {
        pdu_name: String,
        old_name: String,
        new_name: String,
    },
    SetSignalStartBit {
        pdu_name: String,
        signal_name: String,
        old_start_bit: u32,
        new_start_bit: u32,
    },
    SetSignalLength {
        pdu_name: String,
        signal_name: String,
        old_length: u32,
        new_length: u32,
    },
    SetSignalByteOrder {
        pdu_name: String,
        signal_name: String,
        old_byte_order: ByteOrder,
        new_byte_order: ByteOrder,
    },
    SetSignalValueType {
        pdu_name: String,
        signal_name: String,
        old_value_type: ValueType,
        new_value_type: ValueType,
    },
    SetSignalFactor {
        pdu_name: String,
        signal_name: String,
        old_factor: f64,
        new_factor: f64,
    },
    SetSignalOffset {
        pdu_name: String,
        signal_name: String,
        old_offset: f64,
        new_offset: f64,
    },
    SetSignalMin {
        pdu_name: String,
        signal_name: String,
        old_min: f64,
        new_min: f64,
    },
    SetSignalMax {
        pdu_name: String,
        signal_name: String,
        old_max: f64,
        new_max: f64,
    },
    SetSignalUnit {
        pdu_name: String,
        signal_name: String,
        old_unit: String,
        new_unit: String,
    },
    SetSignalReceivers {
        pdu_name: String,
        signal_name: String,
        old_receivers: Vec<String>,
        new_receivers: Vec<String>,
    },
    SetSignalComment {
        pdu_name: String,
        signal_name: String,
        old_comment: String,
        new_comment: String,
    },
    SetSignalValueDescriptions {
        pdu_name: String,
        signal_name: String,
        old_descriptions: Vec<(i64, String)>,
        new_descriptions: Vec<(i64, String)>,
    },
    AddFrame {
        frame: EditableFrame,
    },
    DeleteFrame {
        frame: EditableFrame,
    },
    SetFrameName {
        old_name: String,
        new_name: String,
    },
    SetFrameLength {
        frame_name: String,
        old_length: u32,
        new_length: u32,
    },
    SetFramePayloadPreamble {
        frame_name: String,
        old_preamble: bool,
        new_preamble: bool,
    },
    SetFrameComment {
        frame_name: String,
        old_comment: String,
        new_comment: String,
    },
    SetFrameTriggering {
        frame_name: String,
        old_triggering: FrameTriggering,
        new_triggering: FrameTriggering,
    },
    AddFramePdu {
        frame_name: String,
        mapping: FramePduMapping,
    },
    DeleteFramePdu {
        frame_name: String,
        mapping: FramePduMapping,
    },
    SetFramePduStart {
        frame_name: String,
        pdu_name: String,
        old_start: u32,
        new_start: u32,
    },
}

/// Cluster 可编辑参数的标识
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClusterParam {
    Name,
    SpeedKbps,
    CycleTimeMs,
    MacrotickDurationUs,
    ColdstartAttempts,
    ActionPointOffset,
    MinislotActionPointOffset,
    DynamicSlotIdlePhase,
    MinorVersion,
    NetworkIdleTime,
    NumberOfMinislots,
    NumberOfStaticSlots,
    MinislotDuration,
    StaticSlotDuration,
    SymbolWindow,
    SymbolWindowIdlePhase,
    OffsetCorrectionStart,
}

/// Cluster 参数的值（名称为文本，cycle/macrotick 为浮点，其余为 u32）
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum ClusterValue {
    Text(String),
    U32(u32),
    F64(f64),
}

#[allow(dead_code)]
impl EditableFibex {
    pub fn new() -> Self {
        Self {
            cluster: EditableCluster {
                name: "Cluster".to_string(),
                params: ClusterParams::default(),
            },
            ecus: Vec::new(),
            pdus: Vec::new(),
            frames: Vec::new(),
            history: Vec::new(),
            compound_counts: Vec::new(),
            redo_history: Vec::new(),
            redo_compound_counts: Vec::new(),
        }
    }

    pub fn from_imported(
        cluster: EditableCluster,
        ecus: Vec<String>,
        pdus: Vec<EditablePdu>,
        frames: Vec<EditableFrame>,
    ) -> Self {
        Self {
            cluster,
            ecus,
            pdus,
            frames,
            history: Vec::new(),
            compound_counts: Vec::new(),
            redo_history: Vec::new(),
            redo_compound_counts: Vec::new(),
        }
    }

    pub fn cluster(&self) -> &EditableCluster {
        &self.cluster
    }

    pub fn ecus(&self) -> &Vec<String> {
        &self.ecus
    }

    pub fn pdus(&self) -> &Vec<EditablePdu> {
        &self.pdus
    }

    pub fn frames(&self) -> &Vec<EditableFrame> {
        &self.frames
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub fn pdu_count(&self) -> usize {
        self.pdus.len()
    }

    // ------------------------------------------------------------------
    // 历史记录管理
    // ------------------------------------------------------------------

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
            Operation::SetClusterParam { param, old, new: _ } => {
                self.apply_cluster_param(*param, old.clone());
                Ok(())
            }
            Operation::AddEcu { name } => {
                self.ecus.retain(|n| n != name);
                Ok(())
            }
            Operation::DeleteEcu { name } => {
                if !self.ecus.iter().any(|n| n == name) {
                    self.ecus.push(name.clone());
                }
                Ok(())
            }
            Operation::RenameEcu { old_name, new_name } => {
                self.rename_ecu_internal(new_name, old_name);
                Ok(())
            }
            Operation::AddPdu { pdu } => {
                // AddPdu 恒定追加到末尾，按名字移除
                self.pdus.retain(|p| p.name != pdu.name);
                Ok(())
            }
            Operation::DeletePdu { pdu, frame_mappings } => {
                self.pdus.push(pdu.clone());
                // 恢复各帧中被一并移除的映射
                for (frame_name, mapping) in frame_mappings {
                    if let Some(frame) = self.frames.iter_mut().find(|f| f.name == *frame_name)
                        && !frame.pdus.iter().any(|m| m.pdu_name == mapping.pdu_name)
                    {
                        frame.pdus.push(mapping.clone());
                    }
                }
                Ok(())
            }
            Operation::SetPduName { old_name, new_name } => {
                self.rename_pdu_internal(new_name, old_name);
                Ok(())
            }
            Operation::SetPduLength {
                pdu_name,
                old_length,
                new_length: _,
            } => {
                if let Some(pdu) = self.get_pdu_mut(pdu_name) {
                    pdu.length = *old_length;
                }
                Ok(())
            }
            Operation::SetPduKind {
                pdu_name,
                old_kind,
                new_kind: _,
            } => {
                if let Some(pdu) = self.get_pdu_mut(pdu_name) {
                    pdu.kind = *old_kind;
                }
                Ok(())
            }
            Operation::SetPduComment {
                pdu_name,
                old_comment,
                new_comment: _,
            } => {
                if let Some(pdu) = self.get_pdu_mut(pdu_name) {
                    pdu.comment = old_comment.clone();
                }
                Ok(())
            }
            Operation::AddSignal { pdu_name, signal } => {
                if let Some(pdu) = self.get_pdu_mut(pdu_name) {
                    pdu.signals.retain(|s| s.name != signal.name);
                }
                Ok(())
            }
            Operation::DeleteSignal { pdu_name, signal } => {
                if let Some(pdu) = self.get_pdu_mut(pdu_name) {
                    pdu.signals.push(signal.clone());
                }
                Ok(())
            }
            Operation::SetSignalName {
                pdu_name,
                old_name,
                new_name,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, new_name) {
                    sig.name = old_name.clone();
                }
                Ok(())
            }
            Operation::SetSignalStartBit {
                pdu_name,
                signal_name,
                old_start_bit,
                new_start_bit: _,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.start_bit = *old_start_bit;
                }
                Ok(())
            }
            Operation::SetSignalLength {
                pdu_name,
                signal_name,
                old_length,
                new_length: _,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.length_bits = *old_length;
                }
                Ok(())
            }
            Operation::SetSignalByteOrder {
                pdu_name,
                signal_name,
                old_byte_order,
                new_byte_order: _,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.byte_order = *old_byte_order;
                }
                Ok(())
            }
            Operation::SetSignalValueType {
                pdu_name,
                signal_name,
                old_value_type,
                new_value_type: _,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.value_type = *old_value_type;
                }
                Ok(())
            }
            Operation::SetSignalFactor {
                pdu_name,
                signal_name,
                old_factor,
                new_factor: _,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.factor = *old_factor;
                }
                Ok(())
            }
            Operation::SetSignalOffset {
                pdu_name,
                signal_name,
                old_offset,
                new_offset: _,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.offset = *old_offset;
                }
                Ok(())
            }
            Operation::SetSignalMin {
                pdu_name,
                signal_name,
                old_min,
                new_min: _,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.min = *old_min;
                }
                Ok(())
            }
            Operation::SetSignalMax {
                pdu_name,
                signal_name,
                old_max,
                new_max: _,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.max = *old_max;
                }
                Ok(())
            }
            Operation::SetSignalUnit {
                pdu_name,
                signal_name,
                old_unit,
                new_unit: _,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.unit = old_unit.clone();
                }
                Ok(())
            }
            Operation::SetSignalReceivers {
                pdu_name,
                signal_name,
                old_receivers,
                new_receivers: _,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.receivers = old_receivers.clone();
                }
                Ok(())
            }
            Operation::SetSignalComment {
                pdu_name,
                signal_name,
                old_comment,
                new_comment: _,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.comment = old_comment.clone();
                }
                Ok(())
            }
            Operation::SetSignalValueDescriptions {
                pdu_name,
                signal_name,
                old_descriptions,
                new_descriptions: _,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.value_descriptions = old_descriptions.clone();
                }
                Ok(())
            }
            Operation::AddFrame { frame } => {
                let name = frame.name.clone();
                self.frames.retain(|f| f.name != name);
                Ok(())
            }
            Operation::DeleteFrame { frame } => {
                self.frames.push(frame.clone());
                Ok(())
            }
            Operation::SetFrameName { old_name, new_name } => {
                self.rename_frame_internal(new_name, old_name);
                Ok(())
            }
            Operation::SetFrameLength {
                frame_name,
                old_length,
                new_length: _,
            } => {
                if let Some(frame) = self.get_frame_mut(frame_name) {
                    frame.length = *old_length;
                }
                Ok(())
            }
            Operation::SetFramePayloadPreamble {
                frame_name,
                old_preamble,
                new_preamble: _,
            } => {
                if let Some(frame) = self.get_frame_mut(frame_name) {
                    frame.payload_preamble = *old_preamble;
                }
                Ok(())
            }
            Operation::SetFrameComment {
                frame_name,
                old_comment,
                new_comment: _,
            } => {
                if let Some(frame) = self.get_frame_mut(frame_name) {
                    frame.comment = old_comment.clone();
                }
                Ok(())
            }
            Operation::SetFrameTriggering {
                frame_name,
                old_triggering,
                new_triggering: _,
            } => {
                if let Some(frame) = self.get_frame_mut(frame_name) {
                    frame.triggering = *old_triggering;
                }
                Ok(())
            }
            Operation::AddFramePdu { frame_name, mapping } => {
                if let Some(frame) = self.get_frame_mut(frame_name) {
                    frame.pdus.retain(|m| m.pdu_name != mapping.pdu_name);
                }
                Ok(())
            }
            Operation::DeleteFramePdu { frame_name, mapping } => {
                if let Some(frame) = self.get_frame_mut(frame_name) {
                    frame.pdus.push(mapping.clone());
                }
                Ok(())
            }
            Operation::SetFramePduStart {
                frame_name,
                pdu_name,
                old_start,
                new_start: _,
            } => {
                if let Some(frame) = self.get_frame_mut(frame_name)
                    && let Some(m) = frame.pdus.iter_mut().find(|m| m.pdu_name == *pdu_name)
                {
                    m.start_position = *old_start;
                }
                Ok(())
            }
        }
    }

    fn redo_operation(&mut self, op: &Operation) -> Result<(), String> {
        match op {
            Operation::SetClusterParam { param, old: _, new } => {
                self.apply_cluster_param(*param, new.clone());
                Ok(())
            }
            Operation::AddEcu { name } => {
                if !self.ecus.iter().any(|n| n == name) {
                    self.ecus.push(name.clone());
                }
                Ok(())
            }
            Operation::DeleteEcu { name } => {
                self.ecus.retain(|n| n != name);
                Ok(())
            }
            Operation::RenameEcu { old_name, new_name } => {
                self.rename_ecu_internal(old_name, new_name);
                Ok(())
            }
            Operation::AddPdu { pdu } => {
                self.pdus.push(pdu.clone());
                Ok(())
            }
            Operation::DeletePdu { pdu, frame_mappings } => {
                self.pdus.retain(|p| p.name != pdu.name);
                for (frame_name, mapping) in frame_mappings {
                    if let Some(frame) = self.frames.iter_mut().find(|f| f.name == *frame_name) {
                        frame.pdus.retain(|m| m.pdu_name != mapping.pdu_name);
                    }
                }
                Ok(())
            }
            Operation::SetPduName { old_name, new_name } => {
                self.rename_pdu_internal(old_name, new_name);
                Ok(())
            }
            Operation::SetPduLength {
                pdu_name,
                old_length: _,
                new_length,
            } => {
                if let Some(pdu) = self.get_pdu_mut(pdu_name) {
                    pdu.length = *new_length;
                }
                Ok(())
            }
            Operation::SetPduKind {
                pdu_name,
                old_kind: _,
                new_kind,
            } => {
                if let Some(pdu) = self.get_pdu_mut(pdu_name) {
                    pdu.kind = *new_kind;
                }
                Ok(())
            }
            Operation::SetPduComment {
                pdu_name,
                old_comment: _,
                new_comment,
            } => {
                if let Some(pdu) = self.get_pdu_mut(pdu_name) {
                    pdu.comment = new_comment.clone();
                }
                Ok(())
            }
            Operation::AddSignal { pdu_name, signal } => {
                if let Some(pdu) = self.get_pdu_mut(pdu_name) {
                    pdu.signals.push(signal.clone());
                }
                Ok(())
            }
            Operation::DeleteSignal { pdu_name, signal } => {
                if let Some(pdu) = self.get_pdu_mut(pdu_name) {
                    pdu.signals.retain(|s| s.name != signal.name);
                }
                Ok(())
            }
            Operation::SetSignalName {
                pdu_name,
                old_name,
                new_name,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, old_name) {
                    sig.name = new_name.clone();
                }
                Ok(())
            }
            Operation::SetSignalStartBit {
                pdu_name,
                signal_name,
                old_start_bit: _,
                new_start_bit,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.start_bit = *new_start_bit;
                }
                Ok(())
            }
            Operation::SetSignalLength {
                pdu_name,
                signal_name,
                old_length: _,
                new_length,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.length_bits = *new_length;
                }
                Ok(())
            }
            Operation::SetSignalByteOrder {
                pdu_name,
                signal_name,
                old_byte_order: _,
                new_byte_order,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.byte_order = *new_byte_order;
                }
                Ok(())
            }
            Operation::SetSignalValueType {
                pdu_name,
                signal_name,
                old_value_type: _,
                new_value_type,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.value_type = *new_value_type;
                }
                Ok(())
            }
            Operation::SetSignalFactor {
                pdu_name,
                signal_name,
                old_factor: _,
                new_factor,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.factor = *new_factor;
                }
                Ok(())
            }
            Operation::SetSignalOffset {
                pdu_name,
                signal_name,
                old_offset: _,
                new_offset,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.offset = *new_offset;
                }
                Ok(())
            }
            Operation::SetSignalMin {
                pdu_name,
                signal_name,
                old_min: _,
                new_min,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.min = *new_min;
                }
                Ok(())
            }
            Operation::SetSignalMax {
                pdu_name,
                signal_name,
                old_max: _,
                new_max,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.max = *new_max;
                }
                Ok(())
            }
            Operation::SetSignalUnit {
                pdu_name,
                signal_name,
                old_unit: _,
                new_unit,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.unit = new_unit.clone();
                }
                Ok(())
            }
            Operation::SetSignalReceivers {
                pdu_name,
                signal_name,
                old_receivers: _,
                new_receivers,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.receivers = new_receivers.clone();
                }
                Ok(())
            }
            Operation::SetSignalComment {
                pdu_name,
                signal_name,
                old_comment: _,
                new_comment,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.comment = new_comment.clone();
                }
                Ok(())
            }
            Operation::SetSignalValueDescriptions {
                pdu_name,
                signal_name,
                old_descriptions: _,
                new_descriptions,
            } => {
                if let Some(sig) = self.get_signal_mut(pdu_name, signal_name) {
                    sig.value_descriptions = new_descriptions.clone();
                }
                Ok(())
            }
            Operation::AddFrame { frame } => {
                self.frames.push(frame.clone());
                Ok(())
            }
            Operation::DeleteFrame { frame } => {
                self.frames.retain(|f| f.name != frame.name);
                Ok(())
            }
            Operation::SetFrameName { old_name, new_name } => {
                self.rename_frame_internal(old_name, new_name);
                Ok(())
            }
            Operation::SetFrameLength {
                frame_name,
                old_length: _,
                new_length,
            } => {
                if let Some(frame) = self.get_frame_mut(frame_name) {
                    frame.length = *new_length;
                }
                Ok(())
            }
            Operation::SetFramePayloadPreamble {
                frame_name,
                old_preamble: _,
                new_preamble,
            } => {
                if let Some(frame) = self.get_frame_mut(frame_name) {
                    frame.payload_preamble = *new_preamble;
                }
                Ok(())
            }
            Operation::SetFrameComment {
                frame_name,
                old_comment: _,
                new_comment,
            } => {
                if let Some(frame) = self.get_frame_mut(frame_name) {
                    frame.comment = new_comment.clone();
                }
                Ok(())
            }
            Operation::SetFrameTriggering {
                frame_name,
                old_triggering: _,
                new_triggering,
            } => {
                if let Some(frame) = self.get_frame_mut(frame_name) {
                    frame.triggering = *new_triggering;
                }
                Ok(())
            }
            Operation::AddFramePdu { frame_name, mapping } => {
                if let Some(frame) = self.get_frame_mut(frame_name) {
                    frame.pdus.push(mapping.clone());
                }
                Ok(())
            }
            Operation::DeleteFramePdu { frame_name, mapping } => {
                if let Some(frame) = self.get_frame_mut(frame_name) {
                    frame.pdus.retain(|m| m.pdu_name != mapping.pdu_name);
                }
                Ok(())
            }
            Operation::SetFramePduStart {
                frame_name,
                pdu_name,
                old_start: _,
                new_start,
            } => {
                if let Some(frame) = self.get_frame_mut(frame_name)
                    && let Some(m) = frame.pdus.iter_mut().find(|m| m.pdu_name == *pdu_name)
                {
                    m.start_position = *new_start;
                }
                Ok(())
            }
        }
    }

    // ------------------------------------------------------------------
    // 查找
    // ------------------------------------------------------------------

    pub fn get_pdu(&self, name: &str) -> Option<&EditablePdu> {
        self.pdus.iter().find(|p| p.name == name)
    }

    fn get_pdu_mut(&mut self, name: &str) -> Option<&mut EditablePdu> {
        self.pdus.iter_mut().find(|p| p.name == name)
    }

    pub fn get_frame(&self, name: &str) -> Option<&EditableFrame> {
        self.frames.iter().find(|f| f.name == name)
    }

    fn get_frame_mut(&mut self, name: &str) -> Option<&mut EditableFrame> {
        self.frames.iter_mut().find(|f| f.name == name)
    }

    pub fn find_signal_index(&self, pdu_name: &str, signal_name: &str) -> Option<usize> {
        self.get_pdu(pdu_name)?
            .signals
            .iter()
            .position(|s| s.name == signal_name)
    }

    fn get_signal_mut(&mut self, pdu_name: &str, signal_name: &str) -> Option<&mut EditableSignal> {
        self.get_pdu_mut(pdu_name)?
            .signals
            .iter_mut()
            .find(|s| s.name == signal_name)
    }

    // ------------------------------------------------------------------
    // Cluster 编辑
    // ------------------------------------------------------------------

    pub fn set_cluster_param(&mut self, param: ClusterParam, new: ClusterValue) {
        let old = self.read_cluster_param(param);
        if old == new {
            return;
        }
        self.apply_cluster_param(param, new.clone());
        self.push_compound(Operation::SetClusterParam { param, old, new });
    }

    fn read_cluster_param(&self, param: ClusterParam) -> ClusterValue {
        let p = &self.cluster.params;
        match param {
            ClusterParam::Name => ClusterValue::Text(self.cluster.name.clone()),
            ClusterParam::SpeedKbps => ClusterValue::U32(p.speed_kbps),
            ClusterParam::CycleTimeMs => ClusterValue::F64(p.cycle_time_ms),
            ClusterParam::MacrotickDurationUs => ClusterValue::F64(p.macrotick_duration_us),
            ClusterParam::ColdstartAttempts => ClusterValue::U32(p.coldstart_attempts),
            ClusterParam::ActionPointOffset => ClusterValue::U32(p.action_point_offset),
            ClusterParam::MinislotActionPointOffset => {
                ClusterValue::U32(p.minislot_action_point_offset)
            }
            ClusterParam::DynamicSlotIdlePhase => ClusterValue::U32(p.dynamic_slot_idle_phase),
            ClusterParam::MinorVersion => ClusterValue::U32(p.minor_version),
            ClusterParam::NetworkIdleTime => ClusterValue::U32(p.network_idle_time),
            ClusterParam::NumberOfMinislots => ClusterValue::U32(p.number_of_minislots),
            ClusterParam::NumberOfStaticSlots => ClusterValue::U32(p.number_of_static_slots),
            ClusterParam::MinislotDuration => ClusterValue::U32(p.minislot_duration),
            ClusterParam::StaticSlotDuration => ClusterValue::U32(p.static_slot_duration),
            ClusterParam::SymbolWindow => ClusterValue::U32(p.symbol_window),
            ClusterParam::SymbolWindowIdlePhase => ClusterValue::U32(p.symbol_window_idle_phase),
            ClusterParam::OffsetCorrectionStart => ClusterValue::U32(p.offset_correction_start),
        }
    }

    fn apply_cluster_param(&mut self, param: ClusterParam, value: ClusterValue) {
        match param {
            ClusterParam::Name => {
                if let ClusterValue::Text(t) = value {
                    self.cluster.name = t;
                }
            }
            _ => {
                let p = &mut self.cluster.params;
                let u32v = |v: &ClusterValue| match v {
                    ClusterValue::U32(x) => *x,
                    ClusterValue::F64(x) => *x as u32,
                    ClusterValue::Text(_) => 0,
                };
                let f64v = |v: &ClusterValue| match v {
                    ClusterValue::F64(x) => *x,
                    ClusterValue::U32(x) => *x as f64,
                    ClusterValue::Text(_) => 0.0,
                };
                match param {
                    ClusterParam::SpeedKbps => p.speed_kbps = u32v(&value),
                    ClusterParam::CycleTimeMs => p.cycle_time_ms = f64v(&value),
                    ClusterParam::MacrotickDurationUs => p.macrotick_duration_us = f64v(&value),
                    ClusterParam::ColdstartAttempts => p.coldstart_attempts = u32v(&value),
                    ClusterParam::ActionPointOffset => p.action_point_offset = u32v(&value),
                    ClusterParam::MinislotActionPointOffset => {
                        p.minislot_action_point_offset = u32v(&value)
                    }
                    ClusterParam::DynamicSlotIdlePhase => p.dynamic_slot_idle_phase = u32v(&value),
                    ClusterParam::MinorVersion => p.minor_version = u32v(&value),
                    ClusterParam::NetworkIdleTime => p.network_idle_time = u32v(&value),
                    ClusterParam::NumberOfMinislots => p.number_of_minislots = u32v(&value),
                    ClusterParam::NumberOfStaticSlots => p.number_of_static_slots = u32v(&value),
                    ClusterParam::MinislotDuration => p.minislot_duration = u32v(&value),
                    ClusterParam::StaticSlotDuration => p.static_slot_duration = u32v(&value),
                    ClusterParam::SymbolWindow => p.symbol_window = u32v(&value),
                    ClusterParam::SymbolWindowIdlePhase => p.symbol_window_idle_phase = u32v(&value),
                    ClusterParam::OffsetCorrectionStart => {
                        p.offset_correction_start = u32v(&value)
                    }
                    ClusterParam::Name => {}
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // ECU 编辑
    // ------------------------------------------------------------------

    pub fn add_ecu(&mut self, name: &str) {
        if self.ecus.iter().any(|n| n == name) {
            return;
        }
        self.ecus.push(name.to_string());
        self.push_compound(Operation::AddEcu { name: name.to_string() });
    }

    pub fn delete_ecu(&mut self, name: &str) {
        if !self.ecus.iter().any(|n| n == name) {
            return;
        }
        self.ecus.retain(|n| n != name);
        self.push_compound(Operation::DeleteEcu { name: name.to_string() });
    }

    pub fn rename_ecu(&mut self, old_name: &str, new_name: &str) {
        if !self.ecus.iter().any(|n| n == old_name) {
            return;
        }
        // 目标名已被其他 ECU 占用时拒绝重命名
        if self.ecus.iter().any(|n| n == new_name) {
            return;
        }
        self.rename_ecu_internal(old_name, new_name);
        self.push_compound(Operation::RenameEcu {
            old_name: old_name.to_string(),
            new_name: new_name.to_string(),
        });
    }

    /// 重命名 ECU 并同步所有信号的接收者列表
    fn rename_ecu_internal(&mut self, old_name: &str, new_name: &str) {
        for ecu in self.ecus.iter_mut() {
            if ecu == old_name {
                *ecu = new_name.to_string();
            }
        }
        for pdu in self.pdus.iter_mut() {
            for sig in pdu.signals.iter_mut() {
                for r in sig.receivers.iter_mut() {
                    if r == old_name {
                        *r = new_name.to_string();
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // PDU 编辑
    // ------------------------------------------------------------------

    pub fn add_pdu(&mut self, pdu: &EditablePdu) {
        if self.pdus.iter().any(|p| p.name == pdu.name) {
            return;
        }
        self.pdus.push(pdu.clone());
        self.push_compound(Operation::AddPdu { pdu: pdu.clone() });
    }

    pub fn new_pdu(&mut self) {
        let mut idx = self.pdus.len();
        let mut name = format!("PDU_{}", idx);
        while self.pdus.iter().any(|p| p.name == name) {
            idx += 1;
            name = format!("PDU_{}", idx);
        }
        let pdu = EditablePdu {
            name,
            length: 4,
            kind: PduKind::Static,
            signals: Vec::new(),
            senders: Vec::new(),
            receivers: Vec::new(),
            comment: String::new(),
        };
        self.add_pdu(&pdu);
    }

    pub fn delete_pdu(&mut self, name: &str) {
        let Some(pos) = self.pdus.iter().position(|p| p.name == name) else {
            return;
        };
        let pdu = self.pdus.remove(pos);
        // 同步删除所有帧中对这个 PDU 的引用（撤销时随Actions一并恢复）
        let mut frame_mappings = Vec::new();
        for frame in self.frames.iter_mut() {
            let removed: Vec<usize> = frame
                .pdus
                .iter()
                .enumerate()
                .filter(|(_, m)| m.pdu_name == name)
                .map(|(i, _)| i)
                .collect();
            for i in removed.into_iter().rev() {
                frame_mappings.push((frame.name.clone(), frame.pdus.remove(i)));
            }
        }
        self.push_compound(Operation::DeletePdu { pdu, frame_mappings });
    }

    pub fn set_pdu_name(&mut self, old_name: &str, new_name: &str) {
        if !self.pdus.iter().any(|p| p.name == old_name) {
            return;
        }
        // 目标名已被其他 PDU 占用时拒绝重命名，避免产生重复键
        if self.pdus.iter().any(|p| p.name == new_name) {
            return;
        }
        self.rename_pdu_internal(old_name, new_name);
        self.push_compound(Operation::SetPduName {
            old_name: old_name.to_string(),
            new_name: new_name.to_string(),
        });
    }

    /// 重命名 PDU 并同步所有帧中的映射引用
    fn rename_pdu_internal(&mut self, old_name: &str, new_name: &str) {
        for pdu in self.pdus.iter_mut() {
            if pdu.name == old_name {
                pdu.name = new_name.to_string();
            }
        }
        for frame in self.frames.iter_mut() {
            for m in frame.pdus.iter_mut() {
                if m.pdu_name == old_name {
                    m.pdu_name = new_name.to_string();
                }
            }
        }
    }

    pub fn set_pdu_length(&mut self, pdu_name: &str, new_length: u32) {
        let Some(pdu) = self.get_pdu_mut(pdu_name) else {
            return;
        };
        if pdu.length == new_length {
            return;
        }
        let old_length = pdu.length;
        pdu.length = new_length;
        self.push_compound(Operation::SetPduLength {
            pdu_name: pdu_name.to_string(),
            old_length,
            new_length,
        });
    }

    pub fn set_pdu_kind(&mut self, pdu_name: &str, new_kind: PduKind) {
        let Some(pdu) = self.get_pdu_mut(pdu_name) else {
            return;
        };
        if pdu.kind == new_kind {
            return;
        }
        let old_kind = pdu.kind;
        pdu.kind = new_kind;
        self.push_compound(Operation::SetPduKind {
            pdu_name: pdu_name.to_string(),
            old_kind,
            new_kind,
        });
    }

    pub fn set_pdu_comment(&mut self, pdu_name: &str, new_comment: &str) {
        let Some(pdu) = self.get_pdu_mut(pdu_name) else {
            return;
        };
        if pdu.comment == new_comment {
            return;
        }
        let old_comment = pdu.comment.clone();
        pdu.comment = new_comment.to_string();
        self.push_compound(Operation::SetPduComment {
            pdu_name: pdu_name.to_string(),
            old_comment,
            new_comment: new_comment.to_string(),
        });
    }

    // ------------------------------------------------------------------
    // 信号编辑
    // ------------------------------------------------------------------

    pub fn add_signal(&mut self, pdu_name: &str, signal: &EditableSignal) {
        if let Some(pdu) = self.get_pdu_mut(pdu_name) {
            if pdu.signals.iter().any(|s| s.name == signal.name) {
                return;
            }
            pdu.signals.push(signal.clone());
        } else {
            return;
        }
        self.push_compound(Operation::AddSignal {
            pdu_name: pdu_name.to_string(),
            signal: signal.clone(),
        });
    }

    pub fn new_signal(&mut self, pdu_name: &str) {
        let Some(pdu) = self.get_pdu(pdu_name) else {
            return;
        };
        let mut idx = pdu.signals.len();
        let mut name = format!("Signal_{}", idx);
        while pdu.signals.iter().any(|s| s.name == name) {
            idx += 1;
            name = format!("Signal_{}", idx);
        }
        let signal = EditableSignal {
            name: name.clone(),
            start_bit: 0,
            length_bits: 1,
            byte_order: ByteOrder::BigEndian,
            value_type: ValueType::Unsigned,
            factor: 1.0,
            offset: 0.0,
            min: 0.0,
            max: 0.0,
            unit: String::new(),
            receivers: Vec::new(),
            senders: Vec::new(),
            comment: String::new(),
            value_descriptions: Vec::new(),
        };
        self.add_signal(pdu_name, &signal);
    }

    pub fn delete_signal(&mut self, pdu_name: &str, signal_name: &str) {
        let Some(pdu) = self.get_pdu_mut(pdu_name) else {
            return;
        };
        let Some(pos) = pdu.signals.iter().position(|s| s.name == signal_name) else {
            return;
        };
        let signal = pdu.signals.remove(pos);
        self.push_compound(Operation::DeleteSignal {
            pdu_name: pdu_name.to_string(),
            signal,
        });
    }

    pub fn set_signal_name(&mut self, pdu_name: &str, old_signal_name: &str, new_name: &str) {
        // 目标名已被同 PDU 内其他信号占用时拒绝重命名
        if self
            .get_pdu(pdu_name)
            .map(|p| p.signals.iter().any(|s| s.name == new_name))
            .unwrap_or(true)
        {
            return;
        }
        let Some(sig) = self.get_signal_mut(pdu_name, old_signal_name) else {
            return;
        };
        let old_name = sig.name.clone();
        sig.name = new_name.to_string();
        self.push_compound(Operation::SetSignalName {
            pdu_name: pdu_name.to_string(),
            old_name,
            new_name: new_name.to_string(),
        });
    }

    pub fn set_signal_start_bit(&mut self, pdu_name: &str, signal_name: &str, new_start_bit: u32) {
        let Some(sig) = self.get_signal_mut(pdu_name, signal_name) else {
            return;
        };
        if sig.start_bit == new_start_bit {
            return;
        }
        let old = sig.start_bit;
        sig.start_bit = new_start_bit;
        self.push_compound(Operation::SetSignalStartBit {
            pdu_name: pdu_name.to_string(),
            signal_name: signal_name.to_string(),
            old_start_bit: old,
            new_start_bit,
        });
    }

    pub fn set_signal_length(&mut self, pdu_name: &str, signal_name: &str, new_length: u32) {
        let Some(sig) = self.get_signal_mut(pdu_name, signal_name) else {
            return;
        };
        if sig.length_bits == new_length {
            return;
        }
        let old = sig.length_bits;
        sig.length_bits = new_length;
        self.push_compound(Operation::SetSignalLength {
            pdu_name: pdu_name.to_string(),
            signal_name: signal_name.to_string(),
            old_length: old,
            new_length,
        });
    }

    pub fn set_signal_byte_order(&mut self, pdu_name: &str, signal_name: &str, new: ByteOrder) {
        let Some(sig) = self.get_signal_mut(pdu_name, signal_name) else {
            return;
        };
        if sig.byte_order == new {
            return;
        }
        let old = sig.byte_order;
        sig.byte_order = new;
        self.push_compound(Operation::SetSignalByteOrder {
            pdu_name: pdu_name.to_string(),
            signal_name: signal_name.to_string(),
            old_byte_order: old,
            new_byte_order: new,
        });
    }

    pub fn set_signal_value_type(&mut self, pdu_name: &str, signal_name: &str, new: ValueType) {
        let Some(sig) = self.get_signal_mut(pdu_name, signal_name) else {
            return;
        };
        if sig.value_type == new {
            return;
        }
        let old = sig.value_type;
        sig.value_type = new;
        self.push_compound(Operation::SetSignalValueType {
            pdu_name: pdu_name.to_string(),
            signal_name: signal_name.to_string(),
            old_value_type: old,
            new_value_type: new,
        });
    }

    pub fn set_signal_factor(&mut self, pdu_name: &str, signal_name: &str, new_factor: f64) {
        let Some(sig) = self.get_signal_mut(pdu_name, signal_name) else {
            return;
        };
        if sig.factor == new_factor {
            return;
        }
        let old = sig.factor;
        sig.factor = new_factor;
        self.push_compound(Operation::SetSignalFactor {
            pdu_name: pdu_name.to_string(),
            signal_name: signal_name.to_string(),
            old_factor: old,
            new_factor,
        });
    }

    pub fn set_signal_offset(&mut self, pdu_name: &str, signal_name: &str, new_offset: f64) {
        let Some(sig) = self.get_signal_mut(pdu_name, signal_name) else {
            return;
        };
        if sig.offset == new_offset {
            return;
        }
        let old = sig.offset;
        sig.offset = new_offset;
        self.push_compound(Operation::SetSignalOffset {
            pdu_name: pdu_name.to_string(),
            signal_name: signal_name.to_string(),
            old_offset: old,
            new_offset,
        });
    }

    pub fn set_signal_min(&mut self, pdu_name: &str, signal_name: &str, new_min: f64) {
        let Some(sig) = self.get_signal_mut(pdu_name, signal_name) else {
            return;
        };
        if sig.min == new_min {
            return;
        }
        let old = sig.min;
        sig.min = new_min;
        self.push_compound(Operation::SetSignalMin {
            pdu_name: pdu_name.to_string(),
            signal_name: signal_name.to_string(),
            old_min: old,
            new_min,
        });
    }

    pub fn set_signal_max(&mut self, pdu_name: &str, signal_name: &str, new_max: f64) {
        let Some(sig) = self.get_signal_mut(pdu_name, signal_name) else {
            return;
        };
        if sig.max == new_max {
            return;
        }
        let old = sig.max;
        sig.max = new_max;
        self.push_compound(Operation::SetSignalMax {
            pdu_name: pdu_name.to_string(),
            signal_name: signal_name.to_string(),
            old_max: old,
            new_max,
        });
    }

    pub fn set_signal_unit(&mut self, pdu_name: &str, signal_name: &str, new_unit: &str) {
        let Some(sig) = self.get_signal_mut(pdu_name, signal_name) else {
            return;
        };
        if sig.unit == new_unit {
            return;
        }
        let old = sig.unit.clone();
        sig.unit = new_unit.to_string();
        self.push_compound(Operation::SetSignalUnit {
            pdu_name: pdu_name.to_string(),
            signal_name: signal_name.to_string(),
            old_unit: old,
            new_unit: new_unit.to_string(),
        });
    }

    pub fn set_signal_receivers(&mut self, pdu_name: &str, signal_name: &str, new: Vec<String>) {
        let Some(sig) = self.get_signal_mut(pdu_name, signal_name) else {
            return;
        };
        if sig.receivers == new {
            return;
        }
        let old = sig.receivers.clone();
        sig.receivers = new.clone();
        self.push_compound(Operation::SetSignalReceivers {
            pdu_name: pdu_name.to_string(),
            signal_name: signal_name.to_string(),
            old_receivers: old,
            new_receivers: new,
        });
    }

    pub fn set_signal_comment(&mut self, pdu_name: &str, signal_name: &str, new_comment: &str) {
        let Some(sig) = self.get_signal_mut(pdu_name, signal_name) else {
            return;
        };
        if sig.comment == new_comment {
            return;
        }
        let old = sig.comment.clone();
        sig.comment = new_comment.to_string();
        self.push_compound(Operation::SetSignalComment {
            pdu_name: pdu_name.to_string(),
            signal_name: signal_name.to_string(),
            old_comment: old,
            new_comment: new_comment.to_string(),
        });
    }

    pub fn set_signal_value_descriptions(
        &mut self,
        pdu_name: &str,
        signal_name: &str,
        new: Vec<(i64, String)>,
    ) {
        let Some(sig) = self.get_signal_mut(pdu_name, signal_name) else {
            return;
        };
        if sig.value_descriptions == new {
            return;
        }
        let old = sig.value_descriptions.clone();
        sig.value_descriptions = new.clone();
        self.push_compound(Operation::SetSignalValueDescriptions {
            pdu_name: pdu_name.to_string(),
            signal_name: signal_name.to_string(),
            old_descriptions: old,
            new_descriptions: new,
        });
    }

    // ------------------------------------------------------------------
    // Frame 编辑
    // ------------------------------------------------------------------

    pub fn add_frame(&mut self, frame: &EditableFrame) {
        if self.frames.iter().any(|f| f.name == frame.name) {
            return;
        }
        self.frames.push(frame.clone());
        self.push_compound(Operation::AddFrame { frame: frame.clone() });
    }

    /// 新建默认帧，返回新帧名
    pub fn new_frame(&mut self) -> String {
        let mut idx = self.frames.len();
        let mut name = format!("Frame_{}", idx);
        while self.frames.iter().any(|f| f.name == name) {
            idx += 1;
            name = format!("Frame_{}", idx);
        }
        // 选择第一个未被占用的 Slot（通道 A）
        let used: Vec<u32> = self
            .frames
            .iter()
            .filter(|f| f.triggering.channel != FrChannel::B)
            .map(|f| f.triggering.slot_id)
            .collect();
        let mut slot = 1u32;
        while used.contains(&slot) {
            slot += 1;
        }
        let frame = EditableFrame {
            name: name.clone(),
            length: 8,
            payload_preamble: false,
            triggering: FrameTriggering {
                channel: FrChannel::A,
                slot_id: slot,
                base_cycle: 0,
                cycle_repetition: 1,
                startup: false,
            },
            pdus: Vec::new(),
            comment: String::new(),
        };
        self.add_frame(&frame);
        name
    }

    pub fn delete_frame(&mut self, name: &str) {
        let Some(pos) = self.frames.iter().position(|f| f.name == name) else {
            return;
        };
        let frame = self.frames.remove(pos);
        self.push_compound(Operation::DeleteFrame { frame });
    }

    pub fn set_frame_name(&mut self, old_name: &str, new_name: &str) {
        if !self.frames.iter().any(|f| f.name == old_name) {
            return;
        }
        // 目标名已被其他帧占用时拒绝重命名
        if self.frames.iter().any(|f| f.name == new_name) {
            return;
        }
        self.rename_frame_internal(old_name, new_name);
        self.push_compound(Operation::SetFrameName {
            old_name: old_name.to_string(),
            new_name: new_name.to_string(),
        });
    }

    fn rename_frame_internal(&mut self, old_name: &str, new_name: &str) {
        for frame in self.frames.iter_mut() {
            if frame.name == old_name {
                frame.name = new_name.to_string();
            }
        }
    }

    pub fn set_frame_length(&mut self, frame_name: &str, new_length: u32) {
        let Some(frame) = self.get_frame_mut(frame_name) else {
            return;
        };
        if frame.length == new_length {
            return;
        }
        let old = frame.length;
        frame.length = new_length;
        self.push_compound(Operation::SetFrameLength {
            frame_name: frame_name.to_string(),
            old_length: old,
            new_length,
        });
    }

    pub fn set_frame_payload_preamble(&mut self, frame_name: &str, new_preamble: bool) {
        let Some(frame) = self.get_frame_mut(frame_name) else {
            return;
        };
        if frame.payload_preamble == new_preamble {
            return;
        }
        let old = frame.payload_preamble;
        frame.payload_preamble = new_preamble;
        self.push_compound(Operation::SetFramePayloadPreamble {
            frame_name: frame_name.to_string(),
            old_preamble: old,
            new_preamble,
        });
    }

    pub fn set_frame_comment(&mut self, frame_name: &str, new_comment: &str) {
        let Some(frame) = self.get_frame_mut(frame_name) else {
            return;
        };
        if frame.comment == new_comment {
            return;
        }
        let old = frame.comment.clone();
        frame.comment = new_comment.to_string();
        self.push_compound(Operation::SetFrameComment {
            frame_name: frame_name.to_string(),
            old_comment: old,
            new_comment: new_comment.to_string(),
        });
    }

    pub fn set_frame_triggering(
        &mut self,
        frame_name: &str,
        new_triggering: FrameTriggering,
    ) {
        let Some(frame) = self.get_frame_mut(frame_name) else {
            return;
        };
        if frame.triggering == new_triggering {
            return;
        }
        let old = frame.triggering;
        frame.triggering = new_triggering;
        self.push_compound(Operation::SetFrameTriggering {
            frame_name: frame_name.to_string(),
            old_triggering: old,
            new_triggering,
        });
    }

    pub fn add_frame_pdu(&mut self, frame_name: &str, mapping: FramePduMapping) {
        let Some(frame) = self.get_frame_mut(frame_name) else {
            return;
        };
        if frame.pdus.iter().any(|m| m.pdu_name == mapping.pdu_name) {
            return;
        }
        frame.pdus.push(mapping.clone());
        self.push_compound(Operation::AddFramePdu {
            frame_name: frame_name.to_string(),
            mapping,
        });
    }

    pub fn delete_frame_pdu(&mut self, frame_name: &str, pdu_name: &str) {
        let Some(frame) = self.get_frame_mut(frame_name) else {
            return;
        };
        let Some(pos) = frame.pdus.iter().position(|m| m.pdu_name == pdu_name) else {
            return;
        };
        let mapping = frame.pdus.remove(pos);
        self.push_compound(Operation::DeleteFramePdu {
            frame_name: frame_name.to_string(),
            mapping,
        });
    }

    pub fn set_frame_pdu_start(&mut self, frame_name: &str, pdu_name: &str, new_start: u32) {
        let Some(frame) = self.get_frame_mut(frame_name) else {
            return;
        };
        let Some(m) = frame.pdus.iter_mut().find(|m| m.pdu_name == pdu_name) else {
            return;
        };
        if m.start_position == new_start {
            return;
        }
        let old = m.start_position;
        m.start_position = new_start;
        self.push_compound(Operation::SetFramePduStart {
            frame_name: frame_name.to_string(),
            pdu_name: pdu_name.to_string(),
            old_start: old,
            new_start,
        });
    }

    // ------------------------------------------------------------------
    // 校验
    // ------------------------------------------------------------------

    pub fn validate(&self) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        let cluster = &self.cluster;

        // Cluster 参数范围检查（FlexRay 协议规范）
        let p = &cluster.params;
        if p.speed_kbps != 5000 && p.speed_kbps != 10000 {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                message: format!("Invalid cluster speed {} kbit/s (must be 5000 or 10000)", p.speed_kbps),
            });
        }
        if !(1.0..=16.0).contains(&p.cycle_time_ms) {
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                message: format!("Cycle time {} ms outside common range 1..16 ms", p.cycle_time_ms),
            });
        }
        if !(2..=31).contains(&p.coldstart_attempts) {
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                message: format!("gColdstartAttempts {} out of range 2..31", p.coldstart_attempts),
            });
        }
        if p.number_of_static_slots > 62 {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                message: format!("gNumberOfStaticSlots {} exceeds limit 62", p.number_of_static_slots),
            });
        }
        if p.number_of_minislots > 7998 {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                message: format!("gNumberOfMinislots {} exceeds limit 7998", p.number_of_minislots),
            });
        }

        // PDU 检查
        let mut seen_pdu_names: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (pdu_idx, pdu) in self.pdus.iter().enumerate() {
            if pdu.name.is_empty() {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!("PDU name empty at index {}", pdu_idx),
                });
            }
            if let Some(prev_idx) = seen_pdu_names.get(&pdu.name) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!(
                        "Duplicate PDU name '{}' (index {} and {})",
                        pdu.name, prev_idx, pdu_idx
                    ),
                });
            } else {
                seen_pdu_names.insert(pdu.name.clone(), pdu_idx);
            }
            if pdu.length == 0 {
                issues.push(ValidationIssue {
                    severity: Severity::Warning,
                    message: format!("PDU '{}' has zero length", pdu.name),
                });
            } else if pdu.length > 254 {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!("PDU '{}' length {} exceeds 254 bytes", pdu.name, pdu.length),
                });
            }

            let max_bits = pdu.length.saturating_mul(8);
            let mut seen_sig_names: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            for (sig_idx, sig) in pdu.signals.iter().enumerate() {
                if sig.name.is_empty() {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        message: format!("Signal name empty at index {} in PDU '{}'", pdu.name, sig_idx),
                    });
                }
                if let Some(prev_idx) = seen_sig_names.get(&sig.name) {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        message: format!(
                            "Duplicate signal name '{}' in PDU '{}' (index {} and {})",
                            pdu.name, sig.name, prev_idx, sig_idx
                        ),
                    });
                } else {
                    seen_sig_names.insert(sig.name.clone(), sig_idx);
                }
                if sig.length_bits == 0 {
                    issues.push(ValidationIssue {
                        severity: Severity::Warning,
                        message: format!("Signal '{}' in PDU '{}' has zero bit length", pdu.name, sig.name),
                    });
                }
                if sig.start_bit + sig.length_bits > max_bits {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        message: format!(
                            "Signal '{}' in PDU '{}' exceeds PDU size: bits {}..{} > {} (length = {} bytes)",
                            sig.name, pdu.name, sig.start_bit, sig.start_bit + sig.length_bits, max_bits, pdu.length
                        ),
                    });
                }
                for r in &sig.receivers {
                    if !r.is_empty() && !self.ecus.contains(r) {
                        issues.push(ValidationIssue {
                            severity: Severity::Warning,
                            message: format!(
                                "Signal '{}' in PDU '{}' references unknown ECU '{}'",
                                pdu.name, sig.name, r
                            ),
                        });
                    }
                }
            }
        }

        // Frame 检查
        let mut seen_frame_names: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (frame_idx, frame) in self.frames.iter().enumerate() {
            if frame.name.is_empty() {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!("Frame name empty at index {}", frame_idx),
                });
            }
            if let Some(prev_idx) = seen_frame_names.get(&frame.name) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!(
                        "Duplicate frame name '{}' (index {} and {})",
                        frame.name, prev_idx, frame_idx
                    ),
                });
            } else {
                seen_frame_names.insert(frame.name.clone(), frame_idx);
            }
            if frame.length == 0 {
                issues.push(ValidationIssue {
                    severity: Severity::Warning,
                    message: format!("Frame '{}' has zero length", frame.name),
                });
            } else if frame.length > 254 {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!(
                        "Frame '{}' length {} exceeds 254 bytes",
                        frame.name, frame.length
                    ),
                });
            }

            let t = &frame.triggering;
            if t.slot_id == 0 || t.slot_id > 2047 {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!(
                        "Frame '{}' slot id {} out of range 1..2047",
                        frame.name, t.slot_id
                    ),
                });
            } else if t.slot_id > p.number_of_static_slots {
                issues.push(ValidationIssue {
                    severity: Severity::Warning,
                    message: format!(
                        "Frame '{}' slot id {} exceeds gNumberOfStaticSlots ({})",
                        frame.name, t.slot_id, p.number_of_static_slots
                    ),
                });
            }
            if !matches!(
                t.cycle_repetition,
                1 | 2 | 4 | 8 | 16 | 32 | 64
            ) {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!(
                        "Frame '{}' cycle repetition {} invalid (must be 1/2/4/8/16/32/64)",
                        frame.name, t.cycle_repetition
                    ),
                });
            }
            if t.base_cycle >= t.cycle_repetition {
                issues.push(ValidationIssue {
                    severity: Severity::Error,
                    message: format!(
                        "Frame '{}' base cycle {} must be less than cycle repetition {}",
                        frame.name, t.base_cycle, t.cycle_repetition
                    ),
                });
            }

            // Frame 内 PDU 映射检查
            let mut total_end = 0u32;
            for m in &frame.pdus {
                if self.get_pdu(&m.pdu_name).is_none() {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        message: format!(
                            "Frame '{}' maps unknown PDU '{}'",
                            frame.name, m.pdu_name
                        ),
                    });
                    continue;
                }
                let pdu_len = self.get_pdu(&m.pdu_name).map(|p| p.length).unwrap_or(0);
                let end = m.start_position + pdu_len;
                if end > frame.length {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        message: format!(
                            "PDU '{}' occupies bytes {}..{} beyond frame '{}' length {}",
                            m.pdu_name, m.start_position, end, frame.name, frame.length
                        ),
                    });
                }
                total_end = total_end.max(end);
            }
            if !frame.pdus.is_empty() && total_end < frame.length {
                issues.push(ValidationIssue {
                    severity: Severity::Warning,
                    message: format!(
                        "Frame '{}' has {} unused trailing bytes",
                        frame.name,
                        frame.length - total_end
                    ),
                });
            }
        }

        // Slot/周期冲突检查
        for i in 0..self.frames.len() {
            for j in (i + 1)..self.frames.len() {
                let f1 = &self.frames[i];
                let f2 = &self.frames[j];
                let t1 = &f1.triggering;
                let t2 = &f2.triggering;
                if t1.slot_id != t2.slot_id {
                    continue;
                }
                let channel_overlap = (t1.channel.covers(FrChannel::A) && t2.channel.covers(FrChannel::A))
                    || (t1.channel.covers(FrChannel::B) && t2.channel.covers(FrChannel::B));
                if !channel_overlap {
                    continue;
                }
                // 检查 0..64 周期内是否同时激活
                for cycle in 0..64u32 {
                    if t1.is_active_at_cycle(cycle) && t2.is_active_at_cycle(cycle) {
                        issues.push(ValidationIssue {
                            severity: Severity::Error,
                            message: format!(
                                "Slot conflict: frames '{}' and '{}' both occupy slot {} cycle {} (channel {})",
                                f1.name, f2.name, t1.slot_id, cycle, t1.channel.label()
                            ),
                        });
                        break;
                    }
                }
            }
        }

        // Startup Frame数量提示（每个通道 FlexRay 冷启动需要恰好 2 个Startup Frame）
        for (ch_name, ch) in [("A", FrChannel::A), ("B", FrChannel::B)] {
            let count = self
                .frames
                .iter()
                .filter(|f| f.triggering.startup && f.triggering.channel.covers(ch))
                .count();
            if count != 0 && count != 2 {
                issues.push(ValidationIssue {
                    severity: Severity::Warning,
                    message: format!(
                        "Channel {} has {} startup frames; FlexRay coldstart requires exactly 2",
                        ch_name, count
                    ),
                });
            }
        }

        issues
    }
}

#[allow(dead_code)]
impl EditableSignal {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        name: String,
        start_bit: u32,
        length_bits: u32,
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
            start_bit,
            length_bits,
            byte_order,
            value_type,
            factor,
            offset,
            min,
            max,
            unit,
            receivers,
            senders: Vec::new(),
            comment,
            value_descriptions,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn start_bit(&self) -> u32 {
        self.start_bit
    }

    pub fn length_bits(&self) -> u32 {
        self.length_bits
    }

    pub fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    pub fn value_type(&self) -> ValueType {
        self.value_type
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

    pub fn senders(&self) -> &Vec<String> {
        &self.senders
    }

    pub fn set_senders(&mut self, senders: Vec<String>) {
        self.senders = senders;
    }

    pub fn comment(&self) -> &str {
        &self.comment
    }

    pub fn value_descriptions(&self) -> &[(i64, String)] {
        &self.value_descriptions
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }
}

#[allow(dead_code)]
impl EditablePdu {
    pub fn build(
        name: String,
        length: u32,
        kind: PduKind,
        signals: Vec<EditableSignal>,
        comment: String,
    ) -> Self {
        Self {
            name,
            length,
            kind,
            signals,
            comment,
            senders: Vec::new(),
            receivers: Vec::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn length(&self) -> u32 {
        self.length
    }

    pub fn kind(&self) -> PduKind {
        self.kind
    }

    pub fn signals(&self) -> &Vec<EditableSignal> {
        &self.signals
    }

    pub fn senders(&self) -> &Vec<String> {
        &self.senders
    }

    pub fn receivers(&self) -> &Vec<String> {
        &self.receivers
    }

    pub fn set_senders(&mut self, senders: Vec<String>) {
        self.senders = senders;
    }

    pub fn set_receivers(&mut self, receivers: Vec<String>) {
        self.receivers = receivers;
    }

    pub fn comment(&self) -> &str {
        &self.comment
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }
}

#[allow(dead_code)]
impl EditableFrame {
    pub fn build(
        name: String,
        length: u32,
        payload_preamble: bool,
        triggering: FrameTriggering,
        pdus: Vec<FramePduMapping>,
        comment: String,
    ) -> Self {
        Self {
            name,
            length,
            payload_preamble,
            triggering,
            pdus,
            comment,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn length(&self) -> u32 {
        self.length
    }

    pub fn payload_preamble(&self) -> bool {
        self.payload_preamble
    }

    pub fn triggering(&self) -> &FrameTriggering {
        &self.triggering
    }

    pub fn pdus(&self) -> &Vec<FramePduMapping> {
        &self.pdus
    }

    pub fn comment(&self) -> &str {
        &self.comment
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> EditableFibex {
        let mut fibex = EditableFibex::new();
        fibex.add_ecu("ECU_A");
        fibex.add_ecu("ECU_B");

        let sig = EditableSignal::build(
            "Speed".to_string(),
            0,
            16,
            ByteOrder::BigEndian,
            ValueType::Unsigned,
            0.1,
            0.0,
            0.0,
            500.0,
            "km/h".to_string(),
            vec!["ECU_B".to_string()],
            Vec::new(),
            String::new(),
        );
        let pdu = EditablePdu::build(
            "Pdu1".to_string(),
            4,
            PduKind::Static,
            vec![sig],
            String::new(),
        );
        fibex.add_pdu(&pdu);

        let frame = EditableFrame::build(
            "Frame1".to_string(),
            8,
            false,
            FrameTriggering {
                channel: FrChannel::A,
                slot_id: 1,
                base_cycle: 0,
                cycle_repetition: 1,
                startup: false,
            },
            vec![FramePduMapping::new("Pdu1", 0)],
            String::new(),
        );
        fibex.add_frame(&frame);
        fibex
    }

    #[test]
    fn set_and_undo_signal_factor() {
        let mut fibex = sample();
        fibex.set_signal_factor("Pdu1", "Speed", 0.5);
        assert_eq!(fibex.get_pdu("Pdu1").unwrap().signals()[0].factor(), 0.5);

        fibex.undo().unwrap();
        assert_eq!(fibex.get_pdu("Pdu1").unwrap().signals()[0].factor(), 0.1);

        fibex.redo().unwrap();
        assert_eq!(fibex.get_pdu("Pdu1").unwrap().signals()[0].factor(), 0.5);
    }

    #[test]
    fn rename_pdu_updates_frame_mapping() {
        let mut fibex = sample();
        fibex.set_pdu_name("Pdu1", "Powertrain");
        assert!(fibex.get_pdu("Pdu1").is_none());
        assert!(fibex.get_pdu("Powertrain").is_some());
        assert_eq!(
            fibex.get_frame("Frame1").unwrap().pdus()[0].pdu_name,
            "Powertrain"
        );

        fibex.undo().unwrap();
        assert_eq!(
            fibex.get_frame("Frame1").unwrap().pdus()[0].pdu_name,
            "Pdu1"
        );
    }

    #[test]
    fn rename_ecu_updates_receivers() {
        let mut fibex = sample();
        fibex.rename_ecu("ECU_B", "ECU_C");
        assert_eq!(
            fibex.get_pdu("Pdu1").unwrap().signals()[0].receivers(),
            &vec!["ECU_C".to_string()]
        );

        fibex.undo().unwrap();
        assert_eq!(
            fibex.get_pdu("Pdu1").unwrap().signals()[0].receivers(),
            &vec!["ECU_B".to_string()]
        );
    }

    #[test]
    fn delete_frame_and_undo_restores() {
        let mut fibex = sample();
        fibex.delete_frame("Frame1");
        assert_eq!(fibex.frames().len(), 0);

        fibex.undo().unwrap();
        assert_eq!(fibex.frames().len(), 1);
        assert_eq!(fibex.get_frame("Frame1").unwrap().pdus().len(), 1);
    }

    #[test]
    fn delete_pdu_undo_restores_frame_mappings() {
        let mut fibex = sample();
        fibex.delete_pdu("Pdu1");
        assert!(fibex.get_pdu("Pdu1").is_none());
        // 帧中的映射应被一并移除
        assert!(fibex.get_frame("Frame1").unwrap().pdus().is_empty());

        // 撤销后 PDU 与帧映射都应恢复
        fibex.undo().unwrap();
        assert!(fibex.get_pdu("Pdu1").is_some());
        let pdus_in_frame = &fibex.get_frame("Frame1").unwrap().pdus();
        assert_eq!(pdus_in_frame.len(), 1);
        assert_eq!(pdus_in_frame[0].pdu_name, "Pdu1");

        // 重做后再次全部移除
        fibex.redo().unwrap();
        assert!(fibex.get_pdu("Pdu1").is_none());
        assert!(fibex.get_frame("Frame1").unwrap().pdus().is_empty());
    }

    #[test]
    fn rename_rejects_duplicate_names() {
        let mut fibex = sample();
        let frame2 = EditableFrame::build(
            "Frame2".to_string(),
            8,
            false,
            FrameTriggering::default(),
            Vec::new(),
            String::new(),
        );
        fibex.add_frame(&frame2);

        // Frame1 -> Frame2 与现有帧冲突，应被拒绝
        fibex.set_frame_name("Frame1", "Frame2");
        assert!(fibex.get_frame("Frame1").is_some());
        assert!(fibex.get_frame("Frame2").is_some());
    }

    #[test]
    fn merge_compounds_then_undo() {
        let mut fibex = sample();
        // sample() 已经包含 4 个Actions（2 ECU + 1 PDU + 1 Frame）
        assert_eq!(fibex.compound_counts.len(), 4);
        fibex.add_ecu("ECU_C");
        fibex.add_ecu("ECU_D");
        fibex.merge_last_compounds(2);
        assert_eq!(fibex.compound_counts.len(), 5);

        fibex.undo().unwrap();
        // 两个 AddECU 作为一次撤销被整体回退
        assert_eq!(fibex.ecus().len(), 2);
    }

    #[test]
    fn validation_detects_slot_conflict() {
        let mut fibex = sample();
        let frame2 = EditableFrame::build(
            "Frame2".to_string(),
            8,
            false,
            FrameTriggering {
                channel: FrChannel::A,
                slot_id: 1,
                base_cycle: 0,
                cycle_repetition: 2,
                startup: false,
            },
            Vec::new(),
            String::new(),
        );
        fibex.add_frame(&frame2);

        let issues = fibex.validate();
        assert!(issues.iter().any(|i| i.message.contains("Slot conflict")));
    }

    #[test]
    fn validation_detects_signal_overflow() {
        let mut fibex = sample();
        fibex.set_signal_start_bit("Pdu1", "Speed", 24);
        let issues = fibex.validate();
        assert!(issues
            .iter()
            .any(|i| i.message.contains("exceeds PDU size")));
    }

    #[test]
    fn triggering_cycle_activity() {
        let t = FrameTriggering {
            channel: FrChannel::A,
            slot_id: 1,
            base_cycle: 1,
            cycle_repetition: 4,
            startup: false,
        };
        assert!(t.is_active_at_cycle(1));
        assert!(t.is_active_at_cycle(5));
        assert!(!t.is_active_at_cycle(0));
        assert!(!t.is_active_at_cycle(2));
    }
}
