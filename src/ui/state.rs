//! UI 状态管理模块

use crate::editable_dbc::EditableMessage;
use crate::ui::dbc_window::DbcWindow;
use crate::ui::signal_edit_window::SignalEditDialog;

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
/// 主 UI 状态管理
pub struct UiState {
    pub show_performance_window: bool,
    pub show_about_dialog: bool,
    pub dbc_windows: Vec<DbcWindow>,
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

#[allow(dead_code)]
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
    pub fn get_focused_dbc_window(&mut self) -> Option<&mut DbcWindow> {
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
