//! DBC 文件读取和解析模块
pub use can_dbc::{ByteOrder, DBC, Message, Signal, ValueType};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// 只读的 DBC 数据（从文件解析而来）
#[derive(Debug, Clone)]
pub struct DbcData {
    pub file_path: String,
    pub dbc: Option<DBC>,
    pub error_message: String,
}

impl Default for DbcData {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            dbc: None,
            error_message: String::new(),
        }
    }
}

impl DbcData {
    pub fn new() -> Self {
        Default::default()
    }

    /// 加载 DBC 文件
    pub fn load_dbc_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let path = path.as_ref();
        self.file_path = path.to_string_lossy().to_string();

        // 检查文件是否存在
        if !path.exists() {
            return Err(format!("File not found: {}", path.display()));
        }

        // 读取文件内容
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;

        // 尝试解析 DBC 内容
        match can_dbc::DBC::from_slice(content.as_bytes()) {
            Ok(dbc) => {
                self.dbc = Some(dbc);
                self.error_message.clear();
                Ok(())
            }
            Err(e) => {
                // 清空之前的数据
                self.dbc = None;
                Err(format!("Failed to parse DBC file: {:?}", e))
            }
        }
    }

    // 获取DBC对象的引用
    // Note: direct accessors `dbc()` and `message_count()` were removed as they were
    // unused at runtime. Access to parsed DBC is kept via `EditableDbcData` and
    // conversion helpers (MessageRef / MessageView) for UI-safe usage.
}

/// 自定义 Message 结构（用于新建的 Message）
#[derive(Debug, Clone)]
pub struct MessageOverride {
    pub message_id: u32,
    pub message_name: String,
    pub message_size: u8,
    pub transmitter: Option<String>,
    pub comment: Option<String>,
    pub signals: Vec<Signal>,
}

/// 对单个 Signal 的覆盖表示（只存储可编辑字段）
#[derive(Debug, Clone)]
pub struct SignalOverride {
    pub name: String,
    pub start_bit: u64,
    pub signal_size: u64,
    pub byte_order: ByteOrder,
    pub value_type: ValueType,
    pub factor: f64,
    pub offset: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub unit: String,
    pub comment: String,
}

impl MessageOverride {
    /// 从 can_dbc::Message 创建 MessageOverride（保持与以前 MessageOverride 相同语义）
    pub fn from_message(msg: &Message) -> Self {
        Self {
            message_id: msg.message_id().raw(),
            message_name: msg.message_name().to_string(),
            message_size: (*msg.message_size()).try_into().expect(&format!(
                "message_size {} for message {} out of u8 range",
                *msg.message_size(),
                msg.message_id().raw()
            )),
            transmitter: None, // can_dbc::Message 没有暴露 transmitter
            comment: None,
            signals: msg.signals().to_vec(),
        }
    }

    /// 创建一个空的新 Message
    #[allow(dead_code)]
    pub fn new(message_id: u32) -> Self {
        Self {
            message_id,
            message_name: format!("NewMessage_{:03X}", message_id),
            message_size: 8,
            transmitter: None,
            comment: None,
            signals: Vec::new(),
        }
    }

    /// 创建副本（用于复制功能）
    pub fn duplicate(&self, new_id: u32) -> Self {
        Self {
            message_id: new_id,
            message_name: format!("{}_Copy", self.message_name),
            message_size: self.message_size,
            transmitter: self.transmitter.clone(),
            comment: self.comment.clone(),
            signals: self.signals.clone(),
        }
    }
}

/// 可编辑的 DBC 数据层
///
/// 这个结构包装了只读的 DbcData，并添加了覆盖层来支持编辑功能。
/// 所有的"编辑"操作实际上是在覆盖层进行，不修改原始 DBC 数据。
#[derive(Debug, Clone)]
pub struct EditableDbcData {
    /// 只读的基础 DBC 数据
    pub base: DbcData,

    /// Message 名称覆盖映射 (original_message_id -> new_name)
    pub message_name_overrides: HashMap<u32, String>,

    /// Message 注释覆盖映射 (original_message_id -> comment)
    pub message_comment_overrides: HashMap<u32, String>,

    /// Message ID 覆盖映射 (original_message_id -> new_message_id)
    pub message_id_overrides: HashMap<u32, u32>,

    /// Message Size 覆盖映射 (original_message_id -> new_size)
    pub message_size_overrides: HashMap<u32, u8>,

    /// Message Transmitter 覆盖映射 (original_message_id -> transmitter)
    pub message_transmitter_overrides: HashMap<u32, String>,

    /// 新建的 Message 列表 (message_id -> MessageOverride)
    pub added_messages: HashMap<u32, MessageOverride>,

    /// Signal 级别的覆盖 (message_id, signal_name) -> SignalOverride
    pub signal_overrides: HashMap<(u32, String), SignalOverride>,

    /// 被删除的 Message ID 集合
    pub deleted_message_ids: HashSet<u32>,
}

impl EditableDbcData {
    /// 创建一个新的、空的 EditableDbcData
    pub fn new() -> Self {
        Self {
            base: DbcData::new(),
            message_name_overrides: HashMap::new(),
            message_comment_overrides: HashMap::new(),
            message_id_overrides: HashMap::new(),
            message_size_overrides: HashMap::new(),
            message_transmitter_overrides: HashMap::new(),
            added_messages: HashMap::new(),
            signal_overrides: HashMap::new(),
            deleted_message_ids: HashSet::new(),
        }
    }

    /// Create EditableDbcData initialized from parsed DbcData
    pub fn from_dbc_data(dbc: DbcData) -> Self {
        let mut ed = Self::new();
        ed.base = dbc;
        ed
    }

    /// Convenience wrapper to load DBC file via inner DbcData
    pub fn load_dbc_file<P: AsRef<std::path::Path>>(&mut self, path: P) -> Result<(), String> {
        self.base.load_dbc_file(path)
    }
    /// 获取 message 的显示名称（考虑覆盖）
    pub fn get_message_name(&self, message_id: u32, original_name: &str) -> String {
        self.message_name_overrides
            .get(&message_id)
            .cloned()
            .unwrap_or_else(|| original_name.to_string())
    }

    /// 设置 message 的名称覆盖
    pub fn set_message_name(&mut self, message_id: u32, new_name: String) {
        if new_name.is_empty() {
            self.message_name_overrides.remove(&message_id);
        } else {
            self.message_name_overrides.insert(message_id, new_name);
        }
    }

    /// 获取 message 的注释（考虑覆盖）
    pub fn get_message_comment(&self, message_id: u32) -> String {
        // 优先返回覆盖的注释
        if let Some(comment) = self.message_comment_overrides.get(&message_id) {
            return comment.clone();
        }

        // 否则从原始 DBC 中获取注释
        if let Some(dbc) = self.base.dbc.as_ref() {
            // 遍历消息找到对应的 MessageId
            for msg in dbc.messages() {
                if msg.message_id().raw() == message_id {
                    if let Some(comment) = dbc.message_comment(*msg.message_id()) {
                        return comment.to_string();
                    }
                    break;
                }
            }
        }

        String::new()
    }

    /// 设置 message 的注释覆盖
    pub fn set_message_comment(&mut self, message_id: u32, comment: String) {
        if comment.is_empty() {
            self.message_comment_overrides.remove(&message_id);
        } else {
            self.message_comment_overrides.insert(message_id, comment);
        }
    }

    /// 获取 message 的显示 ID（考虑覆盖）
    pub fn get_message_id(&self, original_message_id: u32) -> u32 {
        self.message_id_overrides
            .get(&original_message_id)
            .copied()
            .unwrap_or(original_message_id)
    }

    /// 设置 message 的 ID 覆盖
    pub fn set_message_id(&mut self, original_message_id: u32, new_message_id: u32) {
        if new_message_id == original_message_id {
            self.message_id_overrides.remove(&original_message_id);
        } else {
            self.message_id_overrides
                .insert(original_message_id, new_message_id);
        }
    }

    /// 获取 message 的显示 Size（考虑覆盖）
    ///
    /// NOTE: many callers pass the parser's message_size() which is a u64,
    /// so we accept u64 here and convert to u8 at the boundary. If the
    /// original_size cannot fit into a u8 this will panic — caller should
    /// validate sizes are within 1..=8 in UI paths.
    pub fn get_message_size(&self, message_id: u32, original_size: u64) -> u8 {
        let orig_u8: u8 = original_size.try_into().expect(&format!(
            "message_size {} for message {} out of u8 range",
            original_size, message_id
        ));

        self.message_size_overrides
            .get(&message_id)
            .copied()
            .unwrap_or(orig_u8)
    }

    /// 设置 message 的 Size 覆盖
    pub fn set_message_size(&mut self, message_id: u32, new_size: u8) {
        // 注意：这里我们不验证 size 是否合法（1-8），由 UI 层验证
        self.message_size_overrides.insert(message_id, new_size);
    }

    /// 获取 message 的 Transmitter（考虑覆盖）
    pub fn get_message_transmitter(&self, message_id: u32) -> String {
        self.message_transmitter_overrides
            .get(&message_id)
            .cloned()
            .unwrap_or_default()
    }

    /// 设置 message 的 Transmitter 覆盖
    pub fn set_message_transmitter(&mut self, message_id: u32, transmitter: String) {
        if transmitter.is_empty() {
            self.message_transmitter_overrides.remove(&message_id);
        } else {
            self.message_transmitter_overrides
                .insert(message_id, transmitter);
        }
    }

    /// 添加新 Message
    pub fn add_message(&mut self, message: MessageOverride) {
        let id = message.message_id;
        self.added_messages.insert(id, message);
        // 如果之前被删除了，从删除列表中移除
        self.deleted_message_ids.remove(&id);
    }

    /// 删除 Message（标记为删除）
    pub fn delete_message(&mut self, message_id: u32) {
        // 如果是新建的 Message，直接从 added_messages 中移除
        if self.added_messages.remove(&message_id).is_some() {
            return;
        }
        // 否则标记为删除
        self.deleted_message_ids.insert(message_id);
    }

    // (Removed unused helper methods: is_message_deleted, is_message_added, get_added_message)

    /// 获取所有可见的 Message（基础 + 新建 - 删除）
    pub fn get_all_messages(&self) -> Vec<MessageRef<'_>> {
        let mut messages = Vec::new();

        // 添加基础 DBC 中未删除的 Message
        if let Some(dbc) = self.base.dbc.as_ref() {
            for msg in dbc.messages() {
                let id = msg.message_id().raw();
                if !self.deleted_message_ids.contains(&id) {
                    messages.push(MessageRef::Original(msg));
                }
            }
        }

        // 添加新建的 Message
        for custom_msg in self.added_messages.values() {
            messages.push(MessageRef::Custom(custom_msg));
        }

        messages
    }

    /// 搜索包含指定关键词的消息（考虑名称覆盖、新建和删除）
    pub fn search_messages(&self, query: &str) -> Vec<MessageRef<'_>> {
        let all_messages = self.get_all_messages();

        if query.is_empty() {
            return all_messages;
        }

        let query_lower = query.to_lowercase();

        all_messages
            .into_iter()
            .filter(|msg_ref| {
                match msg_ref {
                    MessageRef::Original(msg) => {
                        let message_id = msg.message_id().raw();

                        // 检查覆盖名称
                        if let Some(override_name) = self.message_name_overrides.get(&message_id) {
                            if override_name.to_lowercase().contains(&query_lower) {
                                return true;
                            }
                        }

                        // 检查原始名称
                        if msg.message_name().to_lowercase().contains(&query_lower) {
                            return true;
                        }

                        // 检查信号名称
                        msg.signals()
                            .iter()
                            .any(|sig| sig.name().to_lowercase().contains(&query_lower))
                    }
                    MessageRef::Custom(custom_msg) => {
                        // 检查自定义消息名称
                        custom_msg
                            .message_name
                            .to_lowercase()
                            .contains(&query_lower)
                            || custom_msg
                                .signals
                                .iter()
                                .any(|sig| sig.name().to_lowercase().contains(&query_lower))
                    }
                }
            })
            .collect()
    }

    /// 检查是否有任何修改（用于判断是否需要保存）
    pub fn has_modifications(&self) -> bool {
        !self.message_name_overrides.is_empty()
            || !self.message_comment_overrides.is_empty()
            || !self.message_id_overrides.is_empty()
            || !self.message_size_overrides.is_empty()
            || !self.message_transmitter_overrides.is_empty()
            || !self.added_messages.is_empty()
            || !self.deleted_message_ids.is_empty()
    }

    // 清空所有修改
    // clear_modifications removed (unused). Clearing of specific override maps
    // is performed where appropriate in load/reset code paths.

    /// 获取修改数量
    pub fn modification_count(&self) -> usize {
        self.message_name_overrides.len()
            + self.message_comment_overrides.len()
            + self.message_id_overrides.len()
            + self.message_size_overrides.len()
            + self.message_transmitter_overrides.len()
            + self.added_messages.len()
            + self.deleted_message_ids.len()
    }

    /// 根据 message_id 获取 Message（用于双击操作）
    /// 注意：只返回原始 Message，不返回新建的 MessageOverride
    // Note: direct parser accessors (like get_message_by_id) should be avoided at runtime.
    // Use MessageRef / MessageView conversion helpers during import to provide UI-safe
    // representations. Unused parser-dependent accessors were removed per project policy.

    /// 根据 message_id 获取 MessageRef（支持原始和新建的 Message）
    pub fn get_message_ref_by_id(&self, message_id: u32) -> Option<MessageRef<'_>> {
        // 先检查是否在原始 DBC 中
        if let Some(dbc) = self.base.dbc.as_ref() {
            for msg in dbc.messages() {
                if msg.message_id().raw() == message_id
                    && !self.deleted_message_ids.contains(&message_id)
                {
                    return Some(MessageRef::Original(msg));
                }
            }
        }

        // 检查是否是新建的 Message
        if let Some(custom_msg) = self.added_messages.get(&message_id) {
            return Some(MessageRef::Custom(custom_msg));
        }

        None
    }
}

/// Message 引用枚举（用于统一访问原始和自定义 Message）
#[derive(Clone)]
pub enum MessageRef<'a> {
    Original(&'a Message),
    Custom(&'a MessageOverride),
}

impl<'a> MessageRef<'a> {
    /// 获取 Message ID
    pub fn message_id(&self) -> u32 {
        match self {
            MessageRef::Original(msg) => msg.message_id().raw(),
            MessageRef::Custom(msg) => msg.message_id,
        }
    }

    /// 获取 Message Name
    pub fn message_name(&self) -> &str {
        match self {
            MessageRef::Original(msg) => msg.message_name(),
            MessageRef::Custom(msg) => &msg.message_name,
        }
    }

    /// 获取 Message Size
    pub fn message_size(&self) -> u64 {
        match self {
            MessageRef::Original(msg) => *msg.message_size(),
            MessageRef::Custom(msg) => msg.message_size as u64,
        }
    }

    /// 获取 Signals
    pub fn signals(&self) -> &[Signal] {
        match self {
            MessageRef::Original(msg) => msg.signals(),
            MessageRef::Custom(msg) => &msg.signals,
        }
    }

    /// 转换为 MessageOverride（用于复制）
    pub fn to_message_override(&self) -> MessageOverride {
        match self {
            MessageRef::Original(msg) => MessageOverride::from_message(msg),
            MessageRef::Custom(msg) => (*msg).clone(),
        }
    }

    // 检查是否是自定义 Message (removed)
}

impl Default for EditableDbcData {
    fn default() -> Self {
        Self::new()
    }
}

// ApplyOp implementation moved to dedicated integration module to avoid
// circular module resolution issues. See `src/edit_history_integration.rs`.

// NOTE: Snapshot-based undo was removed in favor of operation-based per-window
// History which stores full message payloads for reliable undo/redo. If you
// need a snapshot-style mechanism later, reintroduce a minimal snapshot type
// that clones only the override maps from `EditableDbcData` (keeps source code
// references minimal to avoid confusion with the current History design).
