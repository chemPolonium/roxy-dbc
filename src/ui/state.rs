//! UI 状态管理模块

use crate::editable_dbc::{EditableMessage, EditableSignal, ValidationIssue};
use crate::ui::dbc_window::DbcWindow;

#[allow(dead_code)]
pub enum DeleteTarget {
    Messages(Vec<u32>),
    Signals(u32, Vec<String>),
}

#[allow(dead_code)]
pub struct ConfirmDeleteDialog {
    pub show: bool,
    pub target: Option<DeleteTarget>,
    pub display_name: String,
}

impl Default for ConfirmDeleteDialog {
    fn default() -> Self {
        Self {
            show: false,
            target: None,
            display_name: String::new(),
        }
    }
}

pub struct CloseConfirmDialog {
    pub show: bool,
    pub dbc_window_index: Option<usize>,
}

impl Default for CloseConfirmDialog {
    fn default() -> Self {
        Self {
            show: false,
            dbc_window_index: None,
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

pub struct ValidationDialog {
    pub show: bool,
    pub issues: Vec<ValidationIssue>,
}

impl Default for ValidationDialog {
    fn default() -> Self {
        Self {
            show: false,
            issues: Vec::new(),
        }
    }
}

/// 剪贴板状态（用于复制/粘贴）
pub struct ClipboardState {
    pub copied_messages: Vec<EditableMessage>,
    pub copied_signals: Vec<EditableSignal>,
}

impl Default for ClipboardState {
    fn default() -> Self {
        Self {
            copied_messages: Vec::new(),
            copied_signals: Vec::new(),
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
    pub validation_dialog: ValidationDialog,
    pub last_focused_dbc_index: Option<usize>,
    pub dbc_window_focus_request: Option<usize>,
    pub message_window_focus_request: Option<usize>,
    pub last_focused_message_window: Option<usize>,
    pub clipboard: ClipboardState,
    pub confirm_delete_dialog: ConfirmDeleteDialog,
    pub close_confirm_dialog: CloseConfirmDialog,
    pub recent_files: Vec<String>,
    pub file_hovering: bool,
    pub node_dialog: crate::ui::node_window::NodeDialog,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            show_performance_window: false,
            show_about_dialog: false,
            dbc_windows: Vec::new(),
            next_dbc_id: 1,
            error_dialog: ErrorDialog::default(),
            validation_dialog: ValidationDialog::default(),
            last_focused_dbc_index: None,
            dbc_window_focus_request: None,
            message_window_focus_request: None,
            last_focused_message_window: None,
            clipboard: ClipboardState::default(),
            confirm_delete_dialog: ConfirmDeleteDialog::default(),
            close_confirm_dialog: CloseConfirmDialog::default(),
            recent_files: Vec::new(),
            file_hovering: false,
            node_dialog: crate::ui::node_window::NodeDialog::default(),
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
        !self.clipboard.copied_messages.is_empty()
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

    pub fn add_recent_file(&mut self, path: &str) {
        self.recent_files.retain(|p| p != path);
        self.recent_files.insert(0, path.to_string());
        if self.recent_files.len() > 10 {
            self.recent_files.truncate(10);
        }
        self.save_recent_files();
    }

    pub fn load_recent_files(&mut self) {
        if let Some(path) = recent_files_path() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                self.recent_files = content
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(|l| l.to_string())
                    .take(10)
                    .collect();
            }
        }
    }

    pub fn save_recent_files(&self) {
        if let Some(path) = recent_files_path() {
            let content = self.recent_files.join("\n");
            let _ = std::fs::write(path, content);
        }
    }
}

fn recent_files_path() -> Option<std::path::PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .map(|p| p.join("recent_files.txt"))
}
