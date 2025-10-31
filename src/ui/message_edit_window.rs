use crate::editable_dbc::EditableMessage;
use crate::ui::signal_edit_window::SignalEditWindowState;

#[allow(dead_code)]
#[derive(Clone)]
pub struct MessageEditWindowState {
    // 这里并不需要 is_open 字段
    // Message Edit Window 必然依附于 Dbc Window 存在
    // 关闭窗口的时候这个状态直接就被删除了

    // 通过 message id 反向索引回 Dbc Window
    pub original_message: EditableMessage,
    pub edited_message: EditableMessage,

    pub pending_changes: bool,
    pub apply_requested: bool,
    pub close_requested: bool,

    pub signal_edit_windows: Vec<SignalEditWindowState>,
}

#[allow(dead_code)]
impl MessageEditWindowState {
    pub fn new(msg: EditableMessage) -> Self {
        Self {
            original_message: msg.copy_without_signals(),
            edited_message: msg.copy_without_signals(),
            pending_changes: false,
            apply_requested: false,
            close_requested: false,
            signal_edit_windows: Vec::new(),
        }
    }
}
