use can_dbc::{ByteOrder, ValueType};
use imgui::Ui;

use crate::editable_dbc::{EditableDbc, EditableSignal};

#[allow(dead_code)]
pub enum SignalEditEvent {
    None,
    Apply,
    Ok,
    Cancel,
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct SignalEditDialog {
    pub show: bool,
    pub message_id: u32,
    pub original_name: String,

    pub name_buffer: String,
    pub start_bit_buffer: String,
    pub size_buffer: String,
    pub byte_order_is_little: bool,
    pub signed: bool,
    pub factor_buffer: String,
    pub offset_buffer: String,
    pub min_buffer: String,
    pub max_buffer: String,
    pub unit_buffer: String,
    pub comment_buffer: String,
}

impl SignalEditDialog {
    pub fn new() -> Self {
        Self {
            show: false,
            message_id: 0,
            original_name: String::new(),
            name_buffer: String::new(),
            start_bit_buffer: String::new(),
            size_buffer: String::new(),
            byte_order_is_little: true,
            signed: false,
            factor_buffer: String::from("1.0"),
            offset_buffer: String::from("0.0"),
            min_buffer: String::from("0.0"),
            max_buffer: String::from("0.0"),
            unit_buffer: String::new(),
            comment_buffer: String::new(),
        }
    }

    pub fn open_from_signal(&mut self, message_id: u32, signal: &EditableSignal) {
        self.show = true;
        self.message_id = message_id;
        self.original_name = signal.name().to_string();
        self.name_buffer = signal.name().to_string();
        self.start_bit_buffer = signal.start_bit().to_string();
        self.size_buffer = signal.signal_size().to_string();
        self.byte_order_is_little = matches!(signal.byte_order(), ByteOrder::LittleEndian);
        self.signed = matches!(signal.value_type(), ValueType::Signed);
        self.factor_buffer = format!("{}", signal.factor());
        self.offset_buffer = format!("{}", signal.offset());
        self.min_buffer = format!("{}", signal.min());
        self.max_buffer = format!("{}", signal.max());
        self.unit_buffer = signal.unit().to_string();
        self.comment_buffer = signal.comment().to_string();
    }

    pub fn apply_edit(&mut self, dbc: &mut EditableDbc) {
        let msg_id = self.message_id;
        let old_name = &self.original_name;
        let mut change_count = 0;

        let new_name = self.name_buffer.trim();
        if new_name != old_name {
            dbc.set_signal_name(msg_id, old_name, new_name);
            change_count += 1;
        }

        if let Ok(start_bit) = self.start_bit_buffer.trim().parse::<u64>() {
            let current = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.start_bit());
            if let Some(current) = current {
                if start_bit != current {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_start_bit(msg_id, name, start_bit);
                    change_count += 1;
                }
            }
        }

        if let Ok(size) = self.size_buffer.trim().parse::<u64>() {
            let current = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.signal_size());
            if let Some(current) = current {
                if size != current {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_size(msg_id, name, size);
                    change_count += 1;
                }
            }
        }

        {
            let current_bo = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.byte_order().clone());
            if let Some(current_bo) = current_bo {
                let new_bo = if self.byte_order_is_little {
                    ByteOrder::LittleEndian
                } else {
                    ByteOrder::BigEndian
                };
                if new_bo != current_bo {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_byte_order(msg_id, name, new_bo);
                    change_count += 1;
                }
            }
        }

        {
            let current_vt = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.value_type().clone());
            if let Some(current_vt) = current_vt {
                let new_vt = if self.signed {
                    ValueType::Signed
                } else {
                    ValueType::Unsigned
                };
                if new_vt != current_vt {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_value_type(msg_id, name, new_vt);
                    change_count += 1;
                }
            }
        }

        if let Ok(factor) = self.factor_buffer.trim().parse::<f64>() {
            let current = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.factor());
            if let Some(current) = current {
                if (factor - current).abs() > f64::EPSILON {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_factor(msg_id, name, factor);
                    change_count += 1;
                }
            }
        }

        if let Ok(offset) = self.offset_buffer.trim().parse::<f64>() {
            let current = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.offset());
            if let Some(current) = current {
                if (offset - current).abs() > f64::EPSILON {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_offset(msg_id, name, offset);
                    change_count += 1;
                }
            }
        }

        if let Ok(min) = self.min_buffer.trim().parse::<f64>() {
            let current = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.min());
            if let Some(current) = current {
                if (min - current).abs() > f64::EPSILON {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_min(msg_id, name, min);
                    change_count += 1;
                }
            }
        }

        if let Ok(max) = self.max_buffer.trim().parse::<f64>() {
            let current = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.max());
            if let Some(current) = current {
                if (max - current).abs() > f64::EPSILON {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_max(msg_id, name, max);
                    change_count += 1;
                }
            }
        }

        let new_unit = self.unit_buffer.trim();
        {
            let current = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.unit().to_string());
            if let Some(current) = current {
                if new_unit != current {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_unit(msg_id, name, new_unit);
                    change_count += 1;
                }
            }
        }

        {
            let current = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.comment().to_string());
            if let Some(current) = current {
                if self.comment_buffer != current {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_comment(msg_id, name, &self.comment_buffer);
                    change_count += 1;
                }
            }
        }

        if change_count > 1 {
            dbc.merge_last_compounds(change_count);
        }

        if let Some(msg) = dbc.get_message(msg_id) {
            let current_name = if change_count > 0 { new_name.to_string() } else { old_name.clone() };
            if let Some(sig) = msg.signals().iter().find(|s| s.name() == current_name) {
                self.original_name = sig.name().to_string();
            }
        }
    }

    pub fn render(&mut self, ui: &Ui) -> SignalEditEvent {
        let mut event = SignalEditEvent::None;

        let title = format!("Edit Signal - {}", self.original_name);
        let mut is_open = true;

        ui.window(&title)
            .always_auto_resize(true)
            .opened(&mut is_open)
            .build(|| {
                ui.input_text("Name##sig_edit", &mut self.name_buffer).build();

                ui.input_text("Start Bit##sig_edit", &mut self.start_bit_buffer)
                    .build();
                ui.input_text("Length##sig_edit", &mut self.size_buffer).build();

                if ui.radio_button("Intel (LE)##bo", &mut self.byte_order_is_little, true) {}
                ui.same_line();
                if ui.radio_button("Motorola (BE)##bo", &mut self.byte_order_is_little, false) {}

                if ui.radio_button("Unsigned##vt", &mut self.signed, false) {}
                ui.same_line();
                if ui.radio_button("Signed##vt", &mut self.signed, true) {}

                ui.input_text("Factor##sig_edit", &mut self.factor_buffer).build();
                ui.input_text("Offset##sig_edit", &mut self.offset_buffer).build();
                ui.input_text("Min##sig_edit", &mut self.min_buffer).build();
                ui.input_text("Max##sig_edit", &mut self.max_buffer).build();
                ui.input_text("Unit##sig_edit", &mut self.unit_buffer).build();
                ui.input_text("Comment##sig_edit", &mut self.comment_buffer)
                    .build();

                ui.separator();

                if ui.button("OK") {
                    event = SignalEditEvent::Ok;
                }
                ui.same_line();
                if ui.button("Cancel") {
                    event = SignalEditEvent::Cancel;
                }
                ui.same_line();
                if ui.button("Apply") {
                    event = SignalEditEvent::Apply;
                }
            });

        if !is_open {
            event = SignalEditEvent::Cancel;
        }

        event
    }
}

impl Default for SignalEditDialog {
    fn default() -> Self {
        Self::new()
    }
}
