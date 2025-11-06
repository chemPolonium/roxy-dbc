use imgui::Ui;

use crate::editable_dbc::EditableMessage;

/// Message 详细窗口状态（包含 Signal 表格）
#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct MessageWindow {
    pub message: EditableMessage,
    pub is_open: bool,
    pub parent_dbc_id: usize,
    // 临时信号编辑请求（仅在窗口内双击某个信号时设置，主循环会处理并打开编辑对话框）
    pub pending_signal_edit: Option<String>,
    // 选中信号的名称（用于在表格中高亮整行）
    pub selected_signal_name: Option<String>,
}

impl MessageWindow {
    pub fn new(message: EditableMessage, parent_dbc_id: usize) -> Self {
        Self {
            message,
            is_open: true,
            parent_dbc_id,
            pending_signal_edit: None,
            selected_signal_name: None,
        }
    }

    pub fn render(&mut self, ui: &Ui) {
        println!(
            "Rendering MessageWindow for message ID: {}",
            self.message.message_id()
        );
        ui.separator();
    }
}
