use crate::editable_dbc::{EditableDbc, EditableMessage, FrameFormat};
use imgui::Ui;

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
    pub original_message: EditableMessage,
    pub name_buffer: String,
    pub id_display: String,
    pub size_buffer: String,
    pub transmitter_buffer: String,
    pub comment_buffer: String,
    pub frame_format_is_extended: bool,
}

#[allow(dead_code)]
impl MessageEditWindowState {
    pub fn open(msg: &EditableMessage) -> Self {
        Self {
            original_message: msg.clone(),
            name_buffer: msg.message_name().to_string(),
            id_display: format!("0x{:03X}", msg.message_id()),
            size_buffer: msg.message_size().to_string(),
            transmitter_buffer: msg.transmitter().to_string(),
            comment_buffer: msg.comment().to_string(),
            frame_format_is_extended: matches!(msg.frame_format(), FrameFormat::Extended),
        }
    }

    pub fn apply_edit(&mut self, dbc: &mut EditableDbc) {
        let msg_id = self.original_message.message_id();
        let mut change_count = 0;

        if self.name_buffer.trim() != self.original_message.message_name() {
            dbc.set_message_name(msg_id, self.name_buffer.trim());
            change_count += 1;
        }

        let new_format = if self.frame_format_is_extended {
            FrameFormat::Extended
        } else {
            FrameFormat::Standard
        };
        if new_format != self.original_message.frame_format() {
            dbc.set_message_frame_format(msg_id, new_format);
            change_count += 1;
        }

        if let Ok(size) = self.size_buffer.trim().parse::<u64>() {
            if size != self.original_message.message_size() {
                dbc.set_message_size(msg_id, size);
                change_count += 1;
            }
        }

        if self.transmitter_buffer.trim() != self.original_message.transmitter() {
            dbc.set_message_transmitter(msg_id, self.transmitter_buffer.trim());
            change_count += 1;
        }

        if self.comment_buffer != self.original_message.comment() {
            dbc.set_message_comment(msg_id, &self.comment_buffer);
            change_count += 1;
        }

        if change_count > 1 {
            dbc.merge_last_compounds(change_count);
        }

        self.original_message = dbc.get_message(msg_id).unwrap().clone();
    }

    pub fn render(&mut self, ui: &Ui) -> MessageEditEvent {
        let mut event = MessageEditEvent::None;

        let title = format!("Edit Message - {}", self.id_display);
        let mut is_open = true;

        ui.window(&title)
            .always_auto_resize(true)
            .opened(&mut is_open)
            .build(|| {
                ui.text(&self.id_display);

                ui.input_text("Name##msg_edit", &mut self.name_buffer).build();

                if ui.radio_button("Standard##ff", &mut self.frame_format_is_extended, false) {
                }
                ui.same_line();
                if ui.radio_button("Extended##ff", &mut self.frame_format_is_extended, true) {
                }

                ui.input_text("Size##msg_edit", &mut self.size_buffer).build();
                ui.input_text("Transmitter##msg_edit", &mut self.transmitter_buffer)
                    .build();
                ui.input_text("Comment##msg_edit", &mut self.comment_buffer)
                    .build();

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
