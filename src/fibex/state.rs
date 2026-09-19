//! FIBEX 侧 UI 状态管理
//!
//! 从 roxy-fibex 整合而来，作为 roxy-dbc `UiState` 的一个组成部分。
//! 最近文件列表由上层（roxy-dbc 的 `UiState`）统一管理，这里不再维护。

use crate::fibex::editable_fibex::{EditableFrame, EditableSignal, ValidationIssue};
use crate::fibex::ui::ecu_window::EcuWindow;
use crate::fibex::ui::fibex_window::FibexWindow;
use crate::ui::state::normalize_path;

#[allow(dead_code)]
pub enum DeleteTarget {
    Frames(Vec<String>),
    Pdus(Vec<String>),
    /// 按 PDU 分组的信号删除：(PDU 名, 信号名列表)
    Signals(Vec<(String, Vec<String>)>),
}

#[allow(dead_code)]
#[derive(Default)]
pub struct ConfirmDeleteDialog {
    pub show: bool,
    pub target: Option<DeleteTarget>,
    pub display_name: String,
    pub was_open: bool,
}

#[derive(Default)]
pub struct CloseConfirmDialog {
    pub show: bool,
    pub fibex_window_index: Option<usize>,
}

#[allow(dead_code)]
/// Error对话框状态
#[derive(Default)]
pub struct ErrorDialog {
    pub show: bool,
    pub message: String,
}

#[derive(Default)]
pub struct ValidationDialog {
    pub show: bool,
    pub issues: Vec<ValidationIssue>,
}

/// 剪贴板状态（用于复制/粘贴）
#[derive(Default)]
pub struct ClipboardState {
    pub copied_frames: Vec<EditableFrame>,
    pub copied_signals: Vec<EditableSignal>,
}

/// FIBEX 侧 UI 状态
pub struct FibexUiState {
    pub fibex_windows: Vec<FibexWindow>,
    pub next_fibex_id: usize,
    pub error_dialog: ErrorDialog,
    pub validation_dialog: ValidationDialog,
    pub last_focused_fibex_index: Option<usize>,
    pub fibex_window_focus_request: Option<usize>,
    pub clipboard: ClipboardState,
    pub confirm_delete_dialog: ConfirmDeleteDialog,
    pub close_confirm_dialog: CloseConfirmDialog,
    pub ecu_window: EcuWindow,
    /// 本帧是否有 FIBEX 窗口获得焦点（由 render_fibex_windows 设置，每帧由上层读取）
    pub focus_claimed: bool,
}

impl Default for FibexUiState {
    fn default() -> Self {
        Self {
            fibex_windows: Vec::new(),
            next_fibex_id: 1,
            error_dialog: ErrorDialog::default(),
            validation_dialog: ValidationDialog::default(),
            last_focused_fibex_index: None,
            fibex_window_focus_request: None,
            clipboard: ClipboardState::default(),
            confirm_delete_dialog: ConfirmDeleteDialog::default(),
            close_confirm_dialog: CloseConfirmDialog::default(),
            ecu_window: EcuWindow::default(),
            focus_claimed: false,
        }
    }
}

impl FibexUiState {
    pub fn has_clipboard_frame(&self) -> bool {
        !self.clipboard.copied_frames.is_empty()
    }

    /// 打开文件：已打开（按规范化路径比较）则聚焦对应窗口，否则载入新窗口。
    /// 解析失败时弹出Error对话框。是否加入最近文件由上层决定。
    pub fn open_file_path(&mut self, path: &std::path::Path) {
        let path_str = normalize_path(&path.to_string_lossy());

        if let Some(existing_idx) = self
            .fibex_windows
            .iter()
            .position(|w| w.file_path == path_str)
        {
            if let Some(window) = self.fibex_windows.get_mut(existing_idx) {
                window.is_open = true;
                self.fibex_window_focus_request = Some(existing_idx);
            }
            self.last_focused_fibex_index = Some(existing_idx);
            return;
        }

        match FibexWindow::from_path(std::path::Path::new(&path_str)) {
            Ok(mut fibex_window) => {
                fibex_window.fibex_id = self.next_fibex_id;
                self.next_fibex_id += 1;
                self.fibex_windows.push(fibex_window);
                self.last_focused_fibex_index = Some(self.fibex_windows.len() - 1);
            }
            Err(e) => {
                self.error_dialog.message = e;
                self.error_dialog.show = true;
            }
        }
    }
}
