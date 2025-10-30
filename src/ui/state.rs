//! UI 状态管理模块

use crate::editable_dbc::{EditableDbc, EditableMessage, Operation};
use crate::ui::dbc_window::DbcWindowState;
use crate::ui::message_edit_window::MessageEditWindowState;

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
    pub copied_message: Option<EditableMessage>,
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
    pub next_dbc_id: usize,
    pub error_dialog: ErrorDialog,
    pub signal_edit_dialog: SignalEditDialog,
    pub last_focused_dbc_index: Option<usize>,
    pub dbc_window_focus_request: Option<usize>,
    pub message_window_focus_request: Option<usize>,
    pub last_focused_message_window: Option<usize>,
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
            next_dbc_id: 1,
            error_dialog: ErrorDialog::default(),
            signal_edit_dialog: SignalEditDialog::default(),
            last_focused_dbc_index: None,
            dbc_window_focus_request: None,
            message_window_focus_request: None,
            last_focused_message_window: None,
            clipboard: ClipboardState::default(),
            confirm_delete_dialog: ConfirmDeleteDialog::default(),
        }
    }
}

impl UiState {
    /// 检查指定 message 是否有对应的 Signal 窗口打开
    ///
    /// 若存在打开的信号窗口则弹出错误对话框并返回 Err。
    // pub fn ensure_message_not_in_open_message_windows(
    //     &mut self,
    //     message_id: u32,
    // ) -> Result<(), ()> {
    //     if let Some(mw) = self
    //         .message_windows
    //         .iter()
    //         .find(|w| w.message.message_id() == message_id)
    //     {
    //         self.error_dialog.message = format!(
    //             "Cannot modify or delete message: '{}' (0x{:03X}) because its Message window is still open.\nPlease close the corresponding Message window first.",
    //             mw.message.message_name(),
    //             mw.message.message_id()
    //         );
    //         self.error_dialog.show = true;
    //         return Err(());
    //     }
    //     Ok(())
    // }

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
            let max_id = window
                .dbc
                .messages()
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
