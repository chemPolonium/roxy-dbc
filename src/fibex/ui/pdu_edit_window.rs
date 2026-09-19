use dear_imgui_rs::{Condition, Ui, WindowFlags};

use crate::fibex::editable_fibex::{EditablePdu, EditableFibex, PduKind};

#[derive(Clone, Debug)]
pub enum PduEditEvent {
    None,
    Apply,
    Ok,
    Cancel,
}

/// PDU 编辑窗口
#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct PduEditWindowState {
    pub show: bool,
    pub pdu_old_name: String,
    pub focus_requested: bool,

    name: String,
    length: i32,
    kind: PduKind,
    comment: String,
}

impl PduEditWindowState {
    pub fn open(pdu_name: &str, pdu: &EditablePdu) -> Self {
        Self {
            show: true,
            pdu_old_name: pdu_name.to_string(),
            focus_requested: true,
            name: pdu.name().to_string(),
            length: pdu.length() as i32,
            kind: pdu.kind(),
            comment: pdu.comment().to_string(),
        }
    }

    pub fn render(&mut self, ui: &Ui) -> PduEditEvent {
        if !self.show {
            return PduEditEvent::None;
        }

        let mut event = PduEditEvent::None;
        let title = format!("Edit PDU - {}###PDUEdit_{}", self.pdu_old_name, self.pdu_old_name);

        let mut window = ui
            .window(&title)
            .size([370.0, 230.0], Condition::Always)
            .flags(WindowFlags::NO_COLLAPSE);
        if self.focus_requested {
            window = window.focused(true);
            self.focus_requested = false;
        }

        window.build(|| {
            let label_w = 110.0;

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
            ui.text("Type");
            ui.same_line_with_pos(label_w);
            let kinds = ["Static", "Dynamic", "Event"];
            let kind_idx = match self.kind {
                PduKind::Static => 0usize,
                PduKind::Dynamic => 1,
                PduKind::Event => 2,
            };
            let mut idx = kind_idx;
            if ui.combo_simple_string("##kind", &mut idx, &kinds) {
                self.kind = match idx {
                    0 => PduKind::Static,
                    1 => PduKind::Dynamic,
                    _ => PduKind::Event,
                };
            }

            ui.align_text_to_frame_padding();
            ui.text("Comment");
            ui.same_line_with_pos(label_w);
            ui.input_text("##comment", &mut self.comment).build();

            ui.separator();
            if ui.button("OK") {
                event = PduEditEvent::Ok;
            }
            ui.same_line();
            if ui.button("Apply") {
                event = PduEditEvent::Apply;
            }
            ui.same_line();
            if ui.button("Cancel") {
                event = PduEditEvent::Cancel;
            }
        });

        event
    }

    pub fn apply_edit(&self, fibex: &mut EditableFibex) {
        let old = self.pdu_old_name.clone();

        if self.name.trim() != old && !self.name.trim().is_empty() {
            fibex.set_pdu_name(&old, self.name.trim());
        }
        let current_name = if self.name.trim().is_empty() {
            old.clone()
        } else {
            self.name.trim().to_string()
        };

        fibex.set_pdu_length(&current_name, self.length.clamp(0, 254) as u32);
        fibex.set_pdu_kind(&current_name, self.kind);
        fibex.set_pdu_comment(&current_name, &self.comment);
    }
}
