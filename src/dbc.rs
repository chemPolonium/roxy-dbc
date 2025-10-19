//! DBC 文件读取和解析模块
pub use can_dbc::{DBC, Message};
use std::collections::HashMap;
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

    /// 获取DBC对象的引用
    #[allow(dead_code)]
    pub fn dbc(&self) -> Option<&DBC> {
        self.dbc.as_ref()
    }

    /// 获取消息数量
    #[allow(dead_code)]
    pub fn message_count(&self) -> usize {
        self.dbc.as_ref().map_or(0, |dbc| dbc.messages().len())
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
    pub message_size_overrides: HashMap<u32, u64>,

    /// Message Transmitter 覆盖映射 (original_message_id -> transmitter)
    pub message_transmitter_overrides: HashMap<u32, String>,
    // 未来可以添加更多覆盖层：
    // pub signal_name_overrides: HashMap<(u32, String), String>,
    // pub signal_value_overrides: HashMap<(u32, String), SignalValues>,
}

impl EditableDbcData {
    /// 从 DbcData 创建可编辑的数据
    pub fn from_dbc_data(dbc_data: DbcData) -> Self {
        Self {
            base: dbc_data,
            message_name_overrides: HashMap::new(),
            message_comment_overrides: HashMap::new(),
            message_id_overrides: HashMap::new(),
            message_size_overrides: HashMap::new(),
            message_transmitter_overrides: HashMap::new(),
        }
    }

    /// 创建新的空数据
    pub fn new() -> Self {
        Self::from_dbc_data(DbcData::new())
    }

    /// 加载 DBC 文件
    #[allow(dead_code)]
    pub fn load_dbc_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        self.base.load_dbc_file(path)?;
        // 清空所有覆盖数据
        self.message_name_overrides.clear();
        self.message_comment_overrides.clear();
        self.message_id_overrides.clear();
        self.message_size_overrides.clear();
        self.message_transmitter_overrides.clear();
        Ok(())
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
        self.message_comment_overrides
            .get(&message_id)
            .cloned()
            .unwrap_or_default()
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
    pub fn get_message_size(&self, message_id: u32, original_size: u64) -> u64 {
        self.message_size_overrides
            .get(&message_id)
            .copied()
            .unwrap_or(original_size)
    }

    /// 设置 message 的 Size 覆盖
    pub fn set_message_size(&mut self, message_id: u32, new_size: u64) {
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

    /// 搜索包含指定关键词的消息（考虑名称覆盖）
    pub fn search_messages(&self, query: &str) -> Vec<&Message> {
        let Some(dbc) = self.base.dbc.as_ref() else {
            return Vec::new();
        };

        if query.is_empty() {
            return dbc.messages().iter().collect();
        }

        let query_lower = query.to_lowercase();

        dbc.messages()
            .iter()
            .filter(|msg| {
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
    }

    /// 清空所有修改
    #[allow(dead_code)]
    pub fn clear_modifications(&mut self) {
        self.message_name_overrides.clear();
        self.message_comment_overrides.clear();
        self.message_id_overrides.clear();
        self.message_size_overrides.clear();
        self.message_transmitter_overrides.clear();
    }

    /// 获取修改数量
    pub fn modification_count(&self) -> usize {
        self.message_name_overrides.len()
            + self.message_comment_overrides.len()
            + self.message_id_overrides.len()
            + self.message_size_overrides.len()
            + self.message_transmitter_overrides.len()
    }
}

impl Default for EditableDbcData {
    fn default() -> Self {
        Self::new()
    }
}

/// 用于 Undo/Redo 的覆盖数据快照
///
/// 这个结构只包含覆盖层的数据，不包含完整的 DBC 数据，
/// 从而大幅减少内存占用。
#[derive(Debug, Clone)]
pub struct OverridesSnapshot {
    pub message_name_overrides: HashMap<u32, String>,
    pub message_comment_overrides: HashMap<u32, String>,
    pub message_id_overrides: HashMap<u32, u32>,
    pub message_size_overrides: HashMap<u32, u64>,
    pub message_transmitter_overrides: HashMap<u32, String>,
}

impl OverridesSnapshot {
    /// 从 EditableDbcData 创建快照
    pub fn from_editable(data: &EditableDbcData) -> Self {
        Self {
            message_name_overrides: data.message_name_overrides.clone(),
            message_comment_overrides: data.message_comment_overrides.clone(),
            message_id_overrides: data.message_id_overrides.clone(),
            message_size_overrides: data.message_size_overrides.clone(),
            message_transmitter_overrides: data.message_transmitter_overrides.clone(),
        }
    }

    /// 应用快照到 EditableDbcData
    pub fn apply_to(&self, data: &mut EditableDbcData) {
        data.message_name_overrides = self.message_name_overrides.clone();
        data.message_comment_overrides = self.message_comment_overrides.clone();
        data.message_id_overrides = self.message_id_overrides.clone();
        data.message_size_overrides = self.message_size_overrides.clone();
        data.message_transmitter_overrides = self.message_transmitter_overrides.clone();
    }
}
