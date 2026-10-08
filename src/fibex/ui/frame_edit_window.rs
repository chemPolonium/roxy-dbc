use dear_imgui_rs::{Condition, TableFlags, Ui, WindowFlags};

use crate::fibex::editable_fibex::{EditableFibex, EditableFrame, FrChannel, FrameTriggering};

#[derive(Clone, Debug)]
pub enum FrameEditEvent {
    None,
    Apply,
    Ok,
    Cancel,
}

/// 一个通道上的排程字段
#[derive(Clone, Copy, Debug)]
struct ChannelEdit {
    used: bool,
    slot_id: i32,
    base_cycle: i32,
    cycle_repetition: i32,
    startup: bool,
}

impl ChannelEdit {
    fn from_triggering(t: Option<FrameTriggering>) -> Self {
        match t {
            Some(t) => Self {
                used: true,
                slot_id: t.slot_id as i32,
                base_cycle: t.base_cycle as i32,
                cycle_repetition: t.cycle_repetition as i32,
                startup: t.startup,
            },
            None => Self {
                used: false,
                slot_id: 1,
                base_cycle: 0,
                cycle_repetition: 1,
                startup: false,
            },
        }
    }

    fn triggering(&self, channel: FrChannel) -> Option<FrameTriggering> {
        self.used.then(|| FrameTriggering {
            channel,
            slot_id: self.slot_id.clamp(0, 2047) as u32,
            base_cycle: self.base_cycle.clamp(0, 63) as u32,
            cycle_repetition: self.cycle_repetition.max(1) as u32,
            startup: self.startup,
        })
    }
}

/// Frame 编辑窗口
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct FrameEditWindowState {
    pub show: bool,
    pub frame_old_name: String,
    pub focus_requested: bool,

    name: String,
    length: i32,
    payload_preamble: bool,
    a: ChannelEdit,
    b: ChannelEdit,
    comment: String,
}

impl Default for FrameEditWindowState {
    fn default() -> Self {
        Self {
            show: false,
            frame_old_name: String::new(),
            focus_requested: false,
            name: String::new(),
            length: 8,
            payload_preamble: false,
            a: ChannelEdit::from_triggering(None),
            b: ChannelEdit::from_triggering(None),
            comment: String::new(),
        }
    }
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
            a: ChannelEdit::from_triggering(frame.channel_triggering(FrChannel::A)),
            b: ChannelEdit::from_triggering(frame.channel_triggering(FrChannel::B)),
            comment: frame.comment().to_string(),
        }
    }

    pub fn render(&mut self, ui: &Ui) -> FrameEditEvent {
        if !self.show {
            return FrameEditEvent::None;
        }

        let mut event = FrameEditEvent::None;
        let title = format!(
            "Edit Frame - {}###FRMEDIT_{}",
            self.frame_old_name, self.frame_old_name
        );

        let mut window = ui
            .window(&title)
            .size([620.0, 330.0], Condition::Always)
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
            ui.text_disabled("Schedule - one slot per channel");

            let mut a = self.a;
            let mut b = self.b;
            Self::schedule_table(ui, &mut a, &mut b);
            self.a = a;
            self.b = b;

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

    /// 两通道的排程字段。勾掉 Send 即该通道不再发送这帧；
    /// 未勾选时后面的格子留空，避免误以为填了值。
    fn schedule_table(ui: &Ui, a: &mut ChannelEdit, b: &mut ChannelEdit) {
        let reps = [1i32, 2, 4, 8, 16, 32, 64];
        let rep_labels = ["1", "2", "4", "8", "16", "32", "64"];

        ui.table("frame_schedule_table")
            .flags(TableFlags::BORDERS | TableFlags::ROW_BG)
            .column("Channel")
            .done()
            .column("Send")
            .done()
            .column("Slot")
            .done()
            .column("Base Cycle")
            .done()
            .column("Repetition")
            .done()
            .column("Startup")
            .done()
            .headers(true)
            .build(|ui| {
                for (label, edit) in [("A", a), ("B", b)] {
                    ui.table_next_row();

                    ui.table_set_column_index(0);
                    ui.text(label);

                    ui.table_set_column_index(1);
                    ui.checkbox(format!("##send_{label}").as_str(), &mut edit.used);

                    ui.table_set_column_index(2);
                    if edit.used {
                        ui.set_next_item_width(70.0);
                        if ui.input_int(format!("##slot_{label}").as_str(), &mut edit.slot_id) {
                            edit.slot_id = edit.slot_id.clamp(1, 2047);
                        }
                    } else {
                        ui.text_disabled("-");
                    }

                    ui.table_set_column_index(3);
                    if edit.used {
                        ui.set_next_item_width(70.0);
                        if ui.input_int(format!("##base_{label}").as_str(), &mut edit.base_cycle) {
                            edit.base_cycle = edit.base_cycle.clamp(0, 63);
                        }
                    } else {
                        ui.text_disabled("-");
                    }

                    ui.table_set_column_index(4);
                    if edit.used {
                        let rep_idx = reps
                            .iter()
                            .position(|&r| r == edit.cycle_repetition)
                            .unwrap_or(0);
                        let mut idx = rep_idx;
                        ui.set_next_item_width(64.0);
                        if ui.combo_simple_string(
                            format!("##rep_{label}").as_str(),
                            &mut idx,
                            &rep_labels,
                        ) {
                            edit.cycle_repetition = reps[idx];
                        }
                    } else {
                        ui.text_disabled("-");
                    }

                    ui.table_set_column_index(5);
                    if edit.used {
                        ui.checkbox(format!("##startup_{label}").as_str(), &mut edit.startup);
                    } else {
                        ui.text_disabled("-");
                    }
                }
            });
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
        for (channel, edit) in [(FrChannel::A, self.a), (FrChannel::B, self.b)] {
            fibex.set_channel_triggering(&current_name, channel, edit.triggering(channel));
        }
        fibex.set_frame_comment(&current_name, &self.comment);
    }
}
