use dear_imgui_rs::{Condition, Ui, WindowFlags};

use crate::fibex::editable_fibex::{EditableFibex, EditableFrame, FrChannel, FrameTriggering};

#[derive(Clone, Debug)]
pub enum FrameEditEvent {
    None,
    Apply,
    Ok,
    Cancel,
}

/// Frame 编辑窗口
#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct FrameEditWindowState {
    pub show: bool,
    pub frame_old_name: String,
    pub focus_requested: bool,

    name: String,
    length: i32,
    payload_preamble: bool,
    channel: FrChannel,
    slot_id: i32,
    base_cycle: i32,
    cycle_repetition: i32,
    startup: bool,
    comment: String,
}

impl FrameEditWindowState {
    pub fn open(frame_name: &str, frame: &EditableFrame) -> Self {
        Self {
            show: true,
            frame_old_name: frame_name.to_string(),
            focus_requested: true,
            name: frame.name().to_string(),
            length: frame.length() as i32,
            payload_preamble: frame.payload_preamble(),
            channel: frame.triggering().channel,
            slot_id: frame.triggering().slot_id as i32,
            base_cycle: frame.triggering().base_cycle as i32,
            cycle_repetition: frame.triggering().cycle_repetition as i32,
            startup: frame.triggering().startup,
            comment: frame.comment().to_string(),
        }
    }

    pub fn render(&mut self, ui: &Ui) -> FrameEditEvent {
        if !self.show {
            return FrameEditEvent::None;
        }

        let mut event = FrameEditEvent::None;
        let title = format!("Edit Frame - {}###FRMEDIT_{}", self.frame_old_name, self.frame_old_name);

        let mut window = ui
            .window(&title)
            .size([410.0, 380.0], Condition::Always)
            .flags(WindowFlags::NO_COLLAPSE);
        if self.focus_requested {
            window = window.focused(true);
            self.focus_requested = false;
        }

        window.build(|| {
            let label_w = 130.0;

            ui.align_text_to_frame_padding();
            ui.text("Name");
            ui.same_line_with_pos(label_w);
            ui.input_text("##name", &mut self.name).build();

            ui.align_text_to_frame_padding();
            ui.text("Length (bytes)");
            ui.same_line_with_pos(label_w);
            ui.input_int("##length", &mut self.length);
            self.length = self.length.clamp(0, 254);

            ui.align_text_to_frame_padding();
            ui.text("Payload Preamble");
            ui.same_line_with_pos(label_w);
            ui.checkbox("##preamble", &mut self.payload_preamble);

            ui.separator();

            ui.align_text_to_frame_padding();
            ui.text("Channel");
            ui.same_line_with_pos(label_w);
            let channels = ["A", "B", "A+B"];
            let ch_idx = match self.channel {
                FrChannel::A => 0usize,
                FrChannel::B => 1,
                FrChannel::Both => 2,
            };
            let mut idx = ch_idx;
            if ui.combo_simple_string("##channel", &mut idx, &channels) {
                self.channel = match idx {
                    0 => FrChannel::A,
                    1 => FrChannel::B,
                    _ => FrChannel::Both,
                };
            }

            ui.align_text_to_frame_padding();
            ui.text("Slot ID");
            ui.same_line_with_pos(label_w);
            ui.input_int("##slot", &mut self.slot_id);
            self.slot_id = self.slot_id.clamp(1, 2047);

            ui.align_text_to_frame_padding();
            ui.text("Base Cycle");
            ui.same_line_with_pos(label_w);
            ui.input_int("##base", &mut self.base_cycle);
            self.base_cycle = self.base_cycle.clamp(0, 63);

            ui.align_text_to_frame_padding();
            ui.text("Cycle Repetition");
            ui.same_line_with_pos(label_w);
            let reps = [1i32, 2, 4, 8, 16, 32, 64];
            let labels = ["1", "2", "4", "8", "16", "32", "64"];
            let rep_idx = reps.iter().position(|&r| r == self.cycle_repetition).unwrap_or(0);
            let mut idx = rep_idx;
            if ui.combo_simple_string("##rep", &mut idx, &labels) {
                self.cycle_repetition = reps[idx];
            }

            ui.align_text_to_frame_padding();
            ui.text("Startup Frame");
            ui.same_line_with_pos(label_w);
            ui.checkbox("##startup", &mut self.startup);

            ui.separator();

            ui.align_text_to_frame_padding();
            ui.text("Comment");
            ui.same_line_with_pos(label_w);
            ui.input_text("##comment", &mut self.comment).build();

            ui.separator();
            if ui.button("OK") {
                event = FrameEditEvent::Ok;
            }
            ui.same_line();
            if ui.button("Apply") {
                event = FrameEditEvent::Apply;
            }
            ui.same_line();
            if ui.button("Cancel") {
                event = FrameEditEvent::Cancel;
            }
        });

        event
    }

    pub fn apply_edit(&self, fibex: &mut EditableFibex) {
        let old = self.frame_old_name.clone();

        if self.name.trim() != old && !self.name.trim().is_empty() {
            fibex.set_frame_name(&old, self.name.trim());
        }
        let current_name = if self.name.trim().is_empty() {
            old.clone()
        } else {
            self.name.trim().to_string()
        };

        fibex.set_frame_length(&current_name, self.length.clamp(0, 254) as u32);
        fibex.set_frame_payload_preamble(&current_name, self.payload_preamble);
        fibex.set_frame_triggering(
            &current_name,
            FrameTriggering {
                channel: self.channel,
                slot_id: self.slot_id.clamp(1, 2047) as u32,
                base_cycle: self.base_cycle.clamp(0, 63) as u32,
                cycle_repetition: self.cycle_repetition.max(1) as u32,
                startup: self.startup,
            },
        );
        fibex.set_frame_comment(&current_name, &self.comment);
    }
}
