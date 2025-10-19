//! UI 状态管理模块

use crate::dbc::{CustomMessage, EditableDbcData, MessageRef, OverridesSnapshot};
use crate::ui::view::MessageView;

/// Confirmation dialog state for delete operations
pub struct ConfirmDeleteDialog {
    pub show: bool,
    pub parent_dbc_id: usize,
    pub message_id: u32,
    pub display_name: String,
}

impl Default for ConfirmDeleteDialog {
    fn default() -> Self {
        Self {
            show: false,
            parent_dbc_id: 0,
            message_id: 0,
            display_name: String::new(),
        }
    }
}

/// DBC 窗口状态
#[derive(Clone)]
pub struct DbcWindowState {
    pub id: usize,
    pub editable_data: EditableDbcData,
    pub search_query: String,
    pub selected_message_id: Option<u32>,
    pub is_open: bool,
    // Undo/Redo 支持
    pub undo_stack: Vec<UndoEntry>,
    pub redo_stack: Vec<UndoEntry>,
    // 临时存放信号编辑请求 (message_id, signal_name)
    pub pending_signal_edit: Option<(u32, String)>,
    // pending deletion request from per-row context menu
    pub pending_delete_message: Option<u32>,
    // optional display name for pending delete (to show in confirmation dialog)
    pub pending_confirm_delete_display_name: Option<String>,
}

/// 可撤销的操作类型
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum UndoOperationKind {
    RenameMessage {
        message_id: u32,
        old_name: String,
        new_name: String,
    },
    ModifyMessageComment {
        message_id: u32,
        old_comment: String,
        new_comment: String,
    },
    ModifyMessageId {
        original_message_id: u32,
        old_id: u32,
        new_id: u32,
    },
    ModifyMessageSize {
        message_id: u32,
        old_size: u64,
        new_size: u64,
    },
    ModifyMessageTransmitter {
        message_id: u32,
        old_transmitter: String,
        new_transmitter: String,
    },
    ModifySignal {
        message_id: u32,
        signal_name: String,
        // could store before/after minimal fields if desired
    },
    AddMessage {
        message_id: u32,
    },
    DeleteMessage {
        message_id: u32,
    },
    // 预留：未来可以添加更多操作类型
    // ModifySignal { ... },
}

/// Undo 条目：使用轻量级快照策略
///
/// 只保存覆盖层数据的快照，不保存整个 DBC 对象，
/// 大幅减少内存占用（从 MB 级别降低到 KB 级别）
#[derive(Clone, Debug)]
pub struct UndoEntry {
    pub op: UndoOperationKind,
    pub before: OverridesSnapshot, // 操作前的覆盖数据快照
    pub after: OverridesSnapshot,  // 操作后的覆盖数据快照
}

impl DbcWindowState {
    const MAX_UNDO_ENTRIES: usize = 100;

    /// 创建新的 DBC 窗口状态
    pub fn new(id: usize, editable_data: EditableDbcData) -> Self {
        Self {
            id,
            editable_data,
            search_query: String::new(),
            selected_message_id: None,
            is_open: true,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            pending_signal_edit: None,
            pending_delete_message: None,
            pending_confirm_delete_display_name: None,
        }
    }

    /// 记录一次可撤销操作
    pub fn push_undo(
        &mut self,
        op: UndoOperationKind,
        before: &OverridesSnapshot,
        after: &OverridesSnapshot,
    ) {
        // 执行新操作时清空重做栈
        self.redo_stack.clear();

        self.undo_stack.push(UndoEntry {
            op,
            before: before.clone(),
            after: after.clone(),
        });

        self.limit_undo_stack_size();
    }

    /// 执行撤销操作
    pub fn undo(&mut self) {
        if let Some(entry) = self.undo_stack.pop() {
            // 保存当前状态到重做栈
            let current = OverridesSnapshot::from_editable(&self.editable_data);

            // 应用撤销前的状态
            entry.before.apply_to(&mut self.editable_data);

            // 推入重做栈
            self.redo_stack.push(UndoEntry {
                op: entry.op,
                before: entry.before,
                after: current,
            });
        }
    }

    /// 执行重做操作
    pub fn redo(&mut self) {
        if let Some(entry) = self.redo_stack.pop() {
            // 保存当前状态到撤销栈
            let current = OverridesSnapshot::from_editable(&self.editable_data);

            // 应用重做后的状态
            entry.after.apply_to(&mut self.editable_data);

            // 推入撤销栈
            self.undo_stack.push(UndoEntry {
                op: entry.op,
                before: current,
                after: entry.after,
            });
        }
    }

    /// 限制 undo 栈的大小
    fn limit_undo_stack_size(&mut self) {
        if self.undo_stack.len() > Self::MAX_UNDO_ENTRIES {
            let overflow = self.undo_stack.len() - Self::MAX_UNDO_ENTRIES;
            self.undo_stack.drain(0..overflow);
        }
    }

    /// 检查是否可以撤销
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// 检查是否可以重做
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// 获取最后一次撤销操作的描述
    pub fn last_undo_description(&self) -> Option<String> {
        self.undo_stack
            .last()
            .map(|entry| describe_undo_operation(entry))
    }

    /// 获取最后一次重做操作的描述
    pub fn last_redo_description(&self) -> Option<String> {
        self.redo_stack
            .last()
            .map(|entry| describe_undo_operation(entry))
    }
}

/// 描述撤销/重做操作
pub fn describe_undo_operation(entry: &UndoEntry) -> String {
    match &entry.op {
        UndoOperationKind::RenameMessage { .. } => "Rename Message".to_string(),
        UndoOperationKind::ModifyMessageComment { .. } => "Modify Comment".to_string(),
        UndoOperationKind::ModifyMessageId { .. } => "Modify ID".to_string(),
        UndoOperationKind::ModifyMessageSize { .. } => "Modify Size".to_string(),
        UndoOperationKind::ModifyMessageTransmitter { .. } => "Modify Transmitter".to_string(),
        UndoOperationKind::AddMessage { .. } => "Add Message".to_string(),
        UndoOperationKind::DeleteMessage { .. } => "Delete Message".to_string(),
        UndoOperationKind::ModifySignal {
            message_id: _,
            signal_name,
        } => {
            format!("Modify Signal '{}'", signal_name)
        }
    }
}

/// Signal 详细窗口状态
#[derive(Clone)]
pub struct SignalWindowState {
    pub id: usize,
    pub message: MessageView,
    pub is_open: bool,
    pub parent_dbc_id: usize,
    // 临时信号编辑请求（仅在窗口内双击某个信号时设置，主循环会处理并打开编辑对话框）
    pub pending_signal_edit: Option<String>,
    // 选中信号的名称（用于在表格中高亮整行）
    pub selected_signal_name: Option<String>,
}

/// 错误对话框状态
pub struct ErrorDialog {
    pub show: bool,
    pub message: String,
}

impl Default for ErrorDialog {
    fn default() -> Self {
        Self {
            show: false,
            message: String::new(),
        }
    }
}

/// Message 编辑对话框状态
pub struct MessageEditDialog {
    pub show: bool,
    pub parent_dbc_id: usize,
    pub message_id: u32,

    // 编辑缓冲区
    pub name_buffer: String,
    pub comment_buffer: String,
    pub id_buffer: String,
    pub size_buffer: String,
    pub transmitter_buffer: String,

    // 原始值（用于取消时恢复）
    pub original_name: String,
    pub original_comment: String,
    pub original_id: u32,
    pub original_size: u64,
    pub original_transmitter: String,
}

impl MessageEditDialog {
    pub fn new() -> Self {
        Self {
            show: false,
            parent_dbc_id: 0,
            message_id: 0,
            name_buffer: String::new(),
            comment_buffer: String::new(),
            id_buffer: String::new(),
            size_buffer: String::new(),
            transmitter_buffer: String::new(),
            original_name: String::new(),
            original_comment: String::new(),
            original_id: 0,
            original_size: 0,
            original_transmitter: String::new(),
        }
    }

    // MessageEditDialog::open (Message) removed; use open_with_ref which supports both Original and Custom messages

    /// 打开编辑对话框（使用 MessageRef，支持原始和新建的 Message）
    pub fn open_with_ref(
        &mut self,
        parent_dbc_id: usize,
        message: &MessageRef,
        editable_data: &EditableDbcData,
    ) {
        self.show = true;
        self.parent_dbc_id = parent_dbc_id;
        self.message_id = message.message_id();

        // 初始化名称缓冲区（考虑覆盖）
        let display_name = editable_data.get_message_name(self.message_id, message.message_name());
        self.name_buffer = display_name.clone();
        self.original_name = display_name;

        // 初始化注释缓冲区
        let comment = editable_data.get_message_comment(self.message_id);
        self.comment_buffer = comment.clone();
        self.original_comment = comment;

        // 初始化 ID 缓冲区（考虑覆盖）
        let display_id = editable_data.get_message_id(self.message_id);
        self.id_buffer = format!("0x{:X}", display_id);
        self.original_id = display_id;

        // 初始化 Size 缓冲区（考虑覆盖）
        let display_size = editable_data.get_message_size(self.message_id, message.message_size());
        self.size_buffer = display_size.to_string();
        self.original_size = display_size;

        // 初始化 Transmitter 缓冲区（考虑覆盖）
        let transmitter = editable_data.get_message_transmitter(self.message_id);
        self.transmitter_buffer = transmitter.clone();
        self.original_transmitter = transmitter;
    }

    /// 检查是否有修改
    pub fn has_changes(&self) -> bool {
        self.name_buffer != self.original_name
            || self.comment_buffer != self.original_comment
            || self.id_buffer != format!("0x{:X}", self.original_id)
            || self.size_buffer != self.original_size.to_string()
            || self.transmitter_buffer != self.original_transmitter
    }

    /// 重置为原始值（取消修改）
    pub fn reset_to_original(&mut self) {
        self.name_buffer = self.original_name.clone();
        self.comment_buffer = self.original_comment.clone();
        self.id_buffer = format!("0x{:X}", self.original_id);
        self.size_buffer = self.original_size.to_string();
        self.transmitter_buffer = self.original_transmitter.clone();
    }

    /// 关闭对话框
    pub fn close(&mut self) {
        self.show = false;
    }
}

impl Default for MessageEditDialog {
    fn default() -> Self {
        Self::new()
    }
}

/// Message 新建对话框状态
pub struct MessageCreateDialog {
    pub show: bool,
    pub parent_dbc_id: usize,

    // 输入缓冲区
    pub name_buffer: String,
    pub comment_buffer: String,
    pub id_buffer: String,
    pub size_buffer: String,
    pub transmitter_buffer: String,
}

impl MessageCreateDialog {
    pub fn new() -> Self {
        Self {
            show: false,
            parent_dbc_id: 0,
            name_buffer: String::new(),
            comment_buffer: String::new(),
            id_buffer: String::new(),
            size_buffer: String::from("8"), // 默认8字节
            transmitter_buffer: String::new(),
        }
    }

    /// 打开创建对话框
    pub fn open(&mut self, parent_dbc_id: usize, suggested_id: u32) {
        self.show = true;
        self.parent_dbc_id = parent_dbc_id;

        // 重置所有字段
        self.name_buffer.clear();
        self.comment_buffer.clear();
        self.id_buffer = format!("0x{:X}", suggested_id);
        self.size_buffer = String::from("8");
        self.transmitter_buffer.clear();
    }

    /// 关闭对话框
    pub fn close(&mut self) {
        self.show = false;
    }

    /// 检查输入是否有效
    pub fn is_valid(&self) -> bool {
        !self.name_buffer.trim().is_empty()
            && !self.id_buffer.trim().is_empty()
            && self.parse_id().is_some()
            && self.parse_size().is_some()
    }

    /// 解析 ID
    pub fn parse_id(&self) -> Option<u32> {
        let s = self.id_buffer.trim();
        if s.is_empty() {
            return None;
        }
        // 尝试解析十六进制（0x 或 0X 前缀）
        if s.starts_with("0x") || s.starts_with("0X") {
            if let Ok(id) = u32::from_str_radix(&s[2..], 16) {
                return Some(id);
            }
        }
        // 尝试解析十进制
        if let Ok(id) = s.parse::<u32>() {
            return Some(id);
        }
        // 尝试直接解析为十六进制（没有 0x 前缀）
        if let Ok(id) = u32::from_str_radix(s, 16) {
            return Some(id);
        }
        None
    }

    /// 解析 Size
    pub fn parse_size(&self) -> Option<u64> {
        let s = self.size_buffer.trim();
        if let Ok(size) = s.parse::<u64>() {
            if size <= 8 {
                return Some(size);
            }
        }
        None
    }
}

impl Default for MessageCreateDialog {
    fn default() -> Self {
        Self::new()
    }
}

/// Signal 编辑对话框状态
pub struct SignalEditDialog {
    pub show: bool,
    pub parent_dbc_id: usize,
    pub message_id: u32,

    // 编辑缓冲区
    pub name_buffer: String,
    pub start_bit_buffer: String,
    pub size_buffer: String,
    pub byte_order_is_little: bool,
    pub signed: bool,
    pub factor_buffer: String,
    pub offset_buffer: String,
    pub min_buffer: String,
    pub max_buffer: String,
    pub unit_buffer: String,
    pub comment_buffer: String,

    // 原始值（用于取消）
    pub original_name: String,
}

impl SignalEditDialog {
    pub fn new() -> Self {
        Self {
            show: false,
            parent_dbc_id: 0,
            message_id: 0,
            name_buffer: String::new(),
            start_bit_buffer: String::new(),
            size_buffer: String::new(),
            byte_order_is_little: true,
            signed: false,
            factor_buffer: String::from("1.0"),
            offset_buffer: String::from("0.0"),
            min_buffer: String::from("0.0"),
            max_buffer: String::from("0.0"),
            unit_buffer: String::new(),
            comment_buffer: String::new(),
            original_name: String::new(),
        }
    }

    pub fn open(&mut self, parent_dbc_id: usize, message_id: u32) {
        self.show = true;
        self.parent_dbc_id = parent_dbc_id;
        self.message_id = message_id;
        // other fields should be initialized by caller using actual signal data
    }

    pub fn close(&mut self) {
        self.show = false;
    }
}

impl Default for SignalEditDialog {
    fn default() -> Self {
        Self::new()
    }
}

/// 剪贴板状态（用于复制/粘贴）
pub struct ClipboardState {
    pub copied_message: Option<CustomMessage>,
}

impl Default for ClipboardState {
    fn default() -> Self {
        Self {
            copied_message: None,
        }
    }
}

/// 主 UI 状态管理
pub struct UiState {
    pub show_performance_window: bool,
    pub show_about_dialog: bool,
    pub dbc_windows: Vec<DbcWindowState>,
    pub signal_windows: Vec<SignalWindowState>,
    pub next_dbc_id: usize,
    pub error_dialog: ErrorDialog,
    pub message_edit_dialog: MessageEditDialog,
    pub message_create_dialog: MessageCreateDialog,
    pub signal_edit_dialog: SignalEditDialog,
    pub last_focused_dbc_index: Option<usize>,
    pub dbc_window_focus_request: Option<usize>,
    pub signal_window_focus_request: Option<usize>,
    pub last_focused_signal_window: Option<usize>,
    pub clipboard: ClipboardState,
    // confirmation dialog state for deletes
    pub confirm_delete_dialog: ConfirmDeleteDialog,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            show_performance_window: false,
            show_about_dialog: false,
            dbc_windows: Vec::new(),
            signal_windows: Vec::new(),
            next_dbc_id: 1,
            error_dialog: ErrorDialog::default(),
            message_edit_dialog: MessageEditDialog::default(),
            message_create_dialog: MessageCreateDialog::default(),
            signal_edit_dialog: SignalEditDialog::default(),
            last_focused_dbc_index: None,
            dbc_window_focus_request: None,
            signal_window_focus_request: None,
            last_focused_signal_window: None,
            clipboard: ClipboardState::default(),
            confirm_delete_dialog: ConfirmDeleteDialog::default(),
        }
    }
}

impl UiState {
    /// 检查指定 message 是否有对应的 Signal 窗口打开
    ///
    /// 若存在打开的信号窗口则弹出错误对话框并返回 Err。
    #[allow(dead_code)]
    pub fn ensure_message_not_in_open_signal_windows(&mut self, message_id: u32) -> Result<(), ()> {
        if let Some(sw) = self
            .signal_windows
            .iter()
            .find(|w| w.message.message_id() == message_id)
        {
            self.error_dialog.message = format!(
                "Cannot modify or delete message: '{}' (0x{:03X}) because its Signal window is still open.\nPlease close the corresponding Signal window first.",
                sw.message.message_name(),
                sw.message.message_id()
            );
            self.error_dialog.show = true;
            return Err(());
        }
        Ok(())
    }

    /// 获取当前聚焦的 DBC 窗口
    pub fn get_focused_dbc_window(&mut self) -> Option<&mut DbcWindowState> {
        let idx = self.last_focused_dbc_index?;
        self.dbc_windows.get_mut(idx)
    }

    // copy_message removed; use handle_copy_message in menu.rs which already performs copy and logs

    /// 检查剪贴板是否有内容
    pub fn has_clipboard_message(&self) -> bool {
        self.clipboard.copied_message.is_some()
    }

    /// 生成下一个可用的 Message ID
    pub fn generate_next_message_id(&self, dbc_window_index: usize) -> u32 {
        if let Some(window) = self.dbc_windows.get(dbc_window_index) {
            let all_messages = window.editable_data.get_all_messages();
            let max_id = all_messages
                .iter()
                .map(|m| m.message_id())
                .max()
                .unwrap_or(0);
            max_id + 1
        } else {
            0x100
        }
    }
}
