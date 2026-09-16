use crate::editable_dbc::{EditableDbc, EditableMessage, FrameFormat};
use dear_imgui_rs::{Ui, WindowFlags};

/// 经典 CAN 最大 DLC
pub const MAX_CLASSIC_DLC: u64 = 8;
/// CAN FD 最大 DLC
pub const MAX_FD_DLC: u64 = 64;

#[allow(dead_code)]
pub enum MessageEditEvent {
    None,
    Apply,
    Ok,
    Cancel,
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct MessageEditWindowState {
    /// 当前消息 ID（应用 ID 修改后随之更新）
    pub current_id: u32,
    pub original_message: EditableMessage,
    pub name_buffer: String,
    pub id_buffer: String,
    pub size_buffer: String,
    pub transmitter_buffer: String,
    pub comment_buffer: String,
    pub frame_format_is_extended: bool,
    pub frame_format_is_fd: bool,
    /// 应用失败时的提示（如 ID 非法或重复）
    pub error_message: String,
}

/// 解析消息 ID 输入：支持 0x 前缀十六进制与十进制
pub fn parse_message_id(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u32>().ok()
    }
}

#[allow(dead_code)]
impl MessageEditWindowState {
    pub fn open(msg: &EditableMessage) -> Self {
        Self {
            current_id: msg.message_id(),
            original_message: msg.clone(),
            name_buffer: msg.message_name().to_string(),
            id_buffer: format!("0x{:03X}", msg.message_id()),
            size_buffer: msg.message_size().to_string(),
            transmitter_buffer: msg.transmitter().to_string(),
            comment_buffer: msg.comment().to_string(),
            frame_format_is_extended: msg.frame_format().is_extended(),
            frame_format_is_fd: msg.frame_format().is_fd(),
            error_message: String::new(),
        }
    }

    fn new_format(&self) -> FrameFormat {
        FrameFormat::compose(self.frame_format_is_extended, self.frame_format_is_fd)
    }

    /// 校验 ID 与 DLC，返回错误描述
    fn validate_input(&self, dbc: &EditableDbc) -> Option<String> {
        let new_format = self.new_format();

        let id = match parse_message_id(&self.id_buffer) {
            Some(id) => id,
            None => return Some(format!("Invalid message ID: '{}'", self.id_buffer.trim())),
        };
        let id_limit = if new_format.is_extended() {
            0x1FFF_FFFF
        } else {
            0x7FF
        };
        if id > id_limit {
            return Some(format!(
                "ID 0x{:X} exceeds {} frame limit of 0x{:X}",
                id,
                if new_format.is_extended() { "extended" } else { "standard" },
                id_limit
            ));
        }
        if id != self.current_id && dbc.get_message(id).is_some() {
            return Some(format!("Message ID 0x{:03X} is already in use", id));
        }

        if let Ok(size) = self.size_buffer.trim().parse::<u64>() {
            let max = if new_format.is_fd() {
                MAX_FD_DLC
            } else {
                MAX_CLASSIC_DLC
            };
            if size > max {
                return Some(format!(
                    "DLC {} exceeds {} limit of {}",
                    size,
                    if new_format.is_fd() { "CAN FD" } else { "classic CAN" },
                    max
                ));
            }
        }

        None
    }

    pub fn apply_edit(&mut self, dbc: &mut EditableDbc) {
        self.error_message.clear();

        if let Some(err) = self.validate_input(dbc) {
            self.error_message = err;
            return;
        }

        let msg_id = self.current_id;
        let mut change_count = 0;

        // ID 修改优先应用，后续属性编辑使用新 ID
        if let Some(new_id) = parse_message_id(&self.id_buffer) {
            if new_id != msg_id {
                dbc.set_message_id(msg_id, new_id);
                self.current_id = new_id;
                change_count += 1;
            }
        }

        let active_id = self.current_id;

        if self.name_buffer.trim() != self.original_message.message_name() {
            dbc.set_message_name(active_id, self.name_buffer.trim());
            change_count += 1;
        }

        let new_format = self.new_format();
        if new_format != self.original_message.frame_format() {
            dbc.set_message_frame_format(active_id, new_format);
            change_count += 1;
        }

        if let Ok(size) = self.size_buffer.trim().parse::<u64>() {
            if size != self.original_message.message_size() {
                dbc.set_message_size(active_id, size);
                change_count += 1;
            }
        }

        if self.transmitter_buffer.trim() != self.original_message.transmitter() {
            dbc.set_message_transmitter(active_id, self.transmitter_buffer.trim());
            change_count += 1;
        }

        if self.comment_buffer != self.original_message.comment() {
            dbc.set_message_comment(active_id, &self.comment_buffer);
            change_count += 1;
        }

        if change_count > 1 {
            dbc.merge_last_compounds(change_count);
        }

        if let Some(msg) = dbc.get_message(active_id) {
            self.original_message = msg.clone();
        }
        self.id_buffer = format!("0x{:03X}", self.current_id);
    }

    pub fn render(&mut self, ui: &Ui) -> MessageEditEvent {
        let mut event = MessageEditEvent::None;

        let title = format!("Edit Message - 0x{:03X}", self.current_id);
        let mut is_open = true;

        ui.window(&title)
            .flags(WindowFlags::ALWAYS_AUTO_RESIZE)
            .opened(&mut is_open)
            .build(|| {
                ui.input_text("ID##msg_edit", &mut self.id_buffer).build();

                ui.input_text("Name##msg_edit", &mut self.name_buffer).build();

                if ui.radio_button("Standard##ff", !self.frame_format_is_extended) {
                    self.frame_format_is_extended = false;
                }
                ui.same_line();
                if ui.radio_button("Extended##ff", self.frame_format_is_extended) {
                    self.frame_format_is_extended = true;
                }
                ui.same_line();
                ui.checkbox("CAN FD##ff", &mut self.frame_format_is_fd);

                ui.input_text("Size##msg_edit", &mut self.size_buffer).build();
                ui.input_text("Transmitter##msg_edit", &mut self.transmitter_buffer)
                    .build();
                ui.input_text("Comment##msg_edit", &mut self.comment_buffer)
                    .build();

                if !self.error_message.is_empty() {
                    ui.text_colored([1.0, 0.3, 0.3, 1.0], &self.error_message);
                }

                ui.separator();

                if ui.button("OK") {
                    event = MessageEditEvent::Ok;
                }
                ui.same_line();
                if ui.button("Cancel") {
                    event = MessageEditEvent::Cancel;
                }
                ui.same_line();
                if ui.button("Apply") {
                    event = MessageEditEvent::Apply;
                }
            });

        if !is_open {
            event = MessageEditEvent::Cancel;
        }

        event
    }
}
