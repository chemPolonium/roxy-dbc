use dear_imgui_rs::{Condition, Ui, WindowFlags};

use crate::fibex::editable_fibex::{ByteOrder, EditableFibex, EditableSignal, ValueType};
use crate::fibex::ui::input_f64_auto;

#[derive(Clone, Debug)]
pub enum SignalEditEvent {
    None,
    Apply,
    Ok,
    Cancel,
}

/// 信号编辑对话框
#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct SignalEditDialog {
    pub show: bool,
    pub pdu_name: String,
    pub signal_old_name: String,
    pub focus_requested: bool,

    // 可编辑副本
    name: String,
    start_bit: i32,
    length_bits: i32,
    byte_order: ByteOrder,
    value_type: ValueType,
    factor: f64,
    offset: f64,
    min: f64,
    max: f64,
    unit: String,
    receivers_text: String,
    comment: String,
    value_descriptions: Vec<(i32, String)>,
}

impl SignalEditDialog {
    pub fn open_from_signal(&mut self, pdu_name: &str, signal: &EditableSignal) {
        self.show = true;
        self.pdu_name = pdu_name.to_string();
        self.signal_old_name = signal.name().to_string();
        self.focus_requested = true;

        self.name = signal.name().to_string();
        self.start_bit = signal.start_bit() as i32;
        self.length_bits = signal.length_bits() as i32;
        self.byte_order = signal.byte_order();
        self.value_type = signal.value_type();
        self.factor = signal.factor();
        self.offset = signal.offset();
        self.min = signal.min();
        self.max = signal.max();
        self.unit = signal.unit().to_string();
        self.receivers_text = signal.receivers().join(", ");
        self.comment = signal.comment().to_string();
        self.value_descriptions = signal
            .value_descriptions()
            .iter()
            .map(|(v, d)| (*v as i32, d.clone()))
            .collect();
    }

    pub fn render(&mut self, ui: &Ui) -> SignalEditEvent {
        if !self.show {
            return SignalEditEvent::None;
        }

        let mut event = SignalEditEvent::None;
        let title = format!(
            "Signal - {}###SIGEDIT_{}_{}",
            self.signal_old_name, self.pdu_name, self.signal_old_name
        );

        let mut window = ui
            .window(&title)
            .size([430.0, 620.0], Condition::Always)
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
            ui.text("Start Bit");
            ui.same_line_with_pos(label_w);
            ui.input_int("##start", &mut self.start_bit);
            if self.start_bit < 0 {
                self.start_bit = 0;
            }

            ui.align_text_to_frame_padding();
            ui.text("Bits");
            ui.same_line_with_pos(label_w);
            ui.input_int("##length", &mut self.length_bits);
            if self.length_bits < 1 {
                self.length_bits = 1;
            }

            ui.align_text_to_frame_padding();
            ui.text("Byte Order");
            ui.same_line_with_pos(label_w);
            let orders = ["Motorola (BE)", "Intel (LE)"];
            let order_idx = match self.byte_order {
                ByteOrder::BigEndian => 0usize,
                ByteOrder::LittleEndian => 1,
            };
            let mut idx = order_idx;
            if ui.combo_simple_string("##byte_order", &mut idx, &orders) {
                self.byte_order = match idx {
                    0 => ByteOrder::BigEndian,
                    _ => ByteOrder::LittleEndian,
                };
            }

            ui.align_text_to_frame_padding();
            ui.text("Value Type");
            ui.same_line_with_pos(label_w);
            let types = ["Unsigned", "Signed"];
            let type_idx = match self.value_type {
                ValueType::Unsigned => 0usize,
                ValueType::Signed => 1,
            };
            let mut idx = type_idx;
            if ui.combo_simple_string("##value_type", &mut idx, &types) {
                self.value_type = match idx {
                    0 => ValueType::Unsigned,
                    _ => ValueType::Signed,
                };
            }

            ui.align_text_to_frame_padding();
            ui.text("Factor");
            ui.same_line_with_pos(label_w);
            input_f64_auto(ui, "##factor", &mut self.factor);

            ui.align_text_to_frame_padding();
            ui.text("Offset");
            ui.same_line_with_pos(label_w);
            input_f64_auto(ui, "##offset", &mut self.offset);

            ui.align_text_to_frame_padding();
            ui.text("Min");
            ui.same_line_with_pos(label_w);
            input_f64_auto(ui, "##min", &mut self.min);

            ui.align_text_to_frame_padding();
            ui.text("Max");
            ui.same_line_with_pos(label_w);
            input_f64_auto(ui, "##max", &mut self.max);

            ui.align_text_to_frame_padding();
            ui.text("Unit");
            ui.same_line_with_pos(label_w);
            ui.input_text("##unit", &mut self.unit).build();

            ui.align_text_to_frame_padding();
            ui.text("Receivers");
            ui.same_line_with_pos(label_w);
            ui.input_text("##receivers", &mut self.receivers_text)
                .hint("ECU1, ECU2, ...")
                .build();

            ui.align_text_to_frame_padding();
            ui.text("Comment");
            ui.same_line_with_pos(label_w);
            ui.input_text("##comment", &mut self.comment).build();

            ui.separator_with_text("Value Descriptions");

            // 逐行编辑值描述（值 -> 描述），删除按钮在行尾
            let mut remove_idx: Option<usize> = None;
            for (i, (value, desc)) in self.value_descriptions.iter_mut().enumerate() {
                ui.set_next_item_width(80.0);
                ui.input_int(format!("##vd_v{i}").as_str(), value);
                ui.same_line();
                ui.set_next_item_width(-60.0);
                ui.input_text(format!("##vd_d{i}").as_str(), desc).build();
                ui.same_line();
                if ui.small_button(format!("Delete##vd_r{i}").as_str()) {
                    remove_idx = Some(i);
                }
            }
            if let Some(i) = remove_idx {
                self.value_descriptions.remove(i);
            }
            if ui.small_button("+ Add Value Description") {
                let next = self
                    .value_descriptions
                    .last()
                    .map(|(v, _)| v + 1)
                    .unwrap_or(0);
                self.value_descriptions.push((next, String::new()));
            }

            ui.separator();
            if ui.button("OK") {
                event = SignalEditEvent::Ok;
            }
            ui.same_line();
            if ui.button("Apply") {
                event = SignalEditEvent::Apply;
            }
            ui.same_line();
            if ui.button("Cancel") {
                event = SignalEditEvent::Cancel;
            }
        });

        event
    }

    pub fn apply_edit(&self, fibex: &mut EditableFibex) {
        let pdu = self.pdu_name.clone();
        let old = self.signal_old_name.clone();

        if self.name.trim() != old && !self.name.trim().is_empty() {
            fibex.set_signal_name(&pdu, &old, self.name.trim());
        }
        let current_name = if self.name.trim().is_empty() {
            old.clone()
        } else {
            self.name.trim().to_string()
        };

        fibex.set_signal_start_bit(&pdu, &current_name, self.start_bit.max(0) as u32);
        fibex.set_signal_length(&pdu, &current_name, self.length_bits.max(1) as u32);
        fibex.set_signal_byte_order(&pdu, &current_name, self.byte_order);
        fibex.set_signal_value_type(&pdu, &current_name, self.value_type);
        fibex.set_signal_factor(&pdu, &current_name, self.factor);
        fibex.set_signal_offset(&pdu, &current_name, self.offset);
        fibex.set_signal_min(&pdu, &current_name, self.min);
        fibex.set_signal_max(&pdu, &current_name, self.max);
        fibex.set_signal_unit(&pdu, &current_name, &self.unit);
        fibex.set_signal_receivers(
            &pdu,
            &current_name,
            self.receivers_text
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        );
        fibex.set_signal_comment(&pdu, &current_name, &self.comment);
        fibex.set_signal_value_descriptions(
            &pdu,
            &current_name,
            self.value_descriptions
                .iter()
                .map(|(v, d)| (*v as i64, d.clone()))
                .collect(),
        );
    }
}
