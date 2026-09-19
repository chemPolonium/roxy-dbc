use can_dbc::{ByteOrder, ValueType};
use dear_imgui_rs::{Ui, WindowFlags};

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
    /// 所属 DBC 窗口的路径，用作窗口标题 ID（不同 DBC 的同名信号对话框不冲突）
    pub window_key: String,
    pub focus_requested: bool,

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
    pub receivers_buffer: String,
    pub comment_buffer: String,
    pub val_desc_buffer: Vec<(String, String)>,
}

impl SignalEditDialog {
    pub fn new() -> Self {
        Self {
            show: false,
            message_id: 0,
            original_name: String::new(),
            window_key: String::new(),
            focus_requested: false,
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
            receivers_buffer: String::new(),
            comment_buffer: String::new(),
            val_desc_buffer: Vec::new(),
        }
    }

    pub fn open_from_signal(&mut self, message_id: u32, signal: &EditableSignal, window_key: &str) {
        self.show = true;
        self.focus_requested = true;
        self.window_key = window_key.to_string();
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
        self.receivers_buffer = signal.receivers().join(",");
        self.comment_buffer = signal.comment().to_string();
        self.val_desc_buffer = signal
            .value_descriptions()
            .iter()
            .map(|(v, d)| (v.to_string(), d.clone()))
            .collect();
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
            if let Some(current) = current
                && start_bit != current {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_start_bit(msg_id, name, start_bit);
                    change_count += 1;
                }
        }

        if let Ok(size) = self.size_buffer.trim().parse::<u64>() {
            let current = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.signal_size());
            if let Some(current) = current
                && size != current {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_size(msg_id, name, size);
                    change_count += 1;
                }
        }

        {
            let current_bo = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| *s.byte_order());
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
                .map(|s| *s.value_type());
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
            if let Some(current) = current
                && (factor - current).abs() > f64::EPSILON {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_factor(msg_id, name, factor);
                    change_count += 1;
                }
        }

        if let Ok(offset) = self.offset_buffer.trim().parse::<f64>() {
            let current = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.offset());
            if let Some(current) = current
                && (offset - current).abs() > f64::EPSILON {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_offset(msg_id, name, offset);
                    change_count += 1;
                }
        }

        if let Ok(min) = self.min_buffer.trim().parse::<f64>() {
            let current = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.min());
            if let Some(current) = current
                && (min - current).abs() > f64::EPSILON {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_min(msg_id, name, min);
                    change_count += 1;
                }
        }

        if let Ok(max) = self.max_buffer.trim().parse::<f64>() {
            let current = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.max());
            if let Some(current) = current
                && (max - current).abs() > f64::EPSILON {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_max(msg_id, name, max);
                    change_count += 1;
                }
        }

        let new_unit = self.unit_buffer.trim();
        {
            let current = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.unit().to_string());
            if let Some(current) = current
                && new_unit != current {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_unit(msg_id, name, new_unit);
                    change_count += 1;
                }
        }

        // 接收节点：逗号分隔
        {
            let new_receivers: Vec<String> = self
                .receivers_buffer
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let current = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.receivers().clone());
            if let Some(current) = current
                && new_receivers != current {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_receivers(msg_id, name, new_receivers);
                    change_count += 1;
                }
        }

        {
            let current = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.comment().to_string());
            if let Some(current) = current
                && self.comment_buffer != current {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_comment(msg_id, name, &self.comment_buffer);
                    change_count += 1;
                }
        }

        {
            let mut new_descs: Vec<(i64, String)> = self
                .val_desc_buffer
                .iter()
                .filter_map(|(v, d)| {
                    v.trim().parse::<i64>().ok().map(|val| (val, d.clone()))
                })
                .collect();
            new_descs.sort_by_key(|(v, _)| *v);
            let current = dbc.get_message(msg_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == self.original_name || s.name() == new_name))
                .map(|s| s.value_descriptions().to_vec());
            if let Some(current) = current
                && new_descs != current {
                    let name = if change_count > 0 { new_name } else { old_name };
                    dbc.set_signal_value_descriptions(msg_id, name, new_descs);
                    change_count += 1;
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

        let title = format!("Edit Signal - {}##{}", self.original_name, self.window_key);
        let mut is_open = true;

        let mut window = ui
            .window(&title)
            .flags(WindowFlags::ALWAYS_AUTO_RESIZE)
            .opened(&mut is_open);
        if self.focus_requested {
            window = window.focused(true);
            self.focus_requested = false;
        }

        window.build(|| {
            ui.input_text("Name##sig_edit", &mut self.name_buffer).build();

            ui.input_text("Start Bit##sig_edit", &mut self.start_bit_buffer)
                .build();
            ui.input_text("Length##sig_edit", &mut self.size_buffer).build();

            if ui.radio_button("Intel (LE)##bo", self.byte_order_is_little) {
                self.byte_order_is_little = true;
            }
            ui.same_line();
            if ui.radio_button("Motorola (BE)##bo", !self.byte_order_is_little) {
                self.byte_order_is_little = false;
            }

            if ui.radio_button("Unsigned##vt", !self.signed) {
                self.signed = false;
            }
            ui.same_line();
            if ui.radio_button("Signed##vt", self.signed) {
                self.signed = true;
            }

            ui.input_text("Factor##sig_edit", &mut self.factor_buffer).build();
            ui.input_text("Offset##sig_edit", &mut self.offset_buffer).build();
            ui.input_text("Min##sig_edit", &mut self.min_buffer).build();
            ui.input_text("Max##sig_edit", &mut self.max_buffer).build();
            ui.input_text("Unit##sig_edit", &mut self.unit_buffer).build();
            ui.input_text("Receivers##sig_edit", &mut self.receivers_buffer)
                .hint("e.g. ECU1, ECU2")
                .build();
            ui.input_text("Comment##sig_edit", &mut self.comment_buffer)
                .build();

            ui.separator();
            ui.text("Value Descriptions (VAL_)");

            if ui.button("Add##val_desc") {
                self.val_desc_buffer.push((String::new(), String::new()));
            }
            ui.same_line();
            if ui.button("Import from clipboard##val_desc")
                && let Some(text) = read_clipboard_text() {
                    let parsed = parse_val_desc_text(&text);
                    self.val_desc_buffer.extend(parsed);
                }

            let mut to_remove = None;
            for (i, (val, desc)) in self.val_desc_buffer.iter_mut().enumerate() {
                let invalid = val.trim().parse::<i64>().is_err();
                ui.input_text(format!("Value##val_desc_{}", i), val).build();
                ui.same_line();
                ui.input_text(format!("Description##val_desc_{}", i), desc).build();
                ui.same_line();
                if ui.button(format!("X##val_desc_rm_{}", i)) {
                    to_remove = Some(i);
                }
                if invalid {
                    ui.same_line();
                    ui.text_colored([1.0, 0.3, 0.3, 1.0], "invalid integer");
                }
            }
            if let Some(idx) = to_remove {
                self.val_desc_buffer.remove(idx);
            }

            let mut counts = std::collections::HashMap::new();
            let mut duplicates = Vec::new();
            for (val, _) in &self.val_desc_buffer {
                if let Ok(v) = val.trim().parse::<i64>() {
                    let entry = counts.entry(v).or_insert(0usize);
                    *entry += 1;
                    if *entry == 2 {
                        duplicates.push(v);
                    }
                }
            }
            if !duplicates.is_empty() {
                ui.text_colored(
                    [1.0, 0.8, 0.0, 1.0],
                    format!("Duplicate values: {:?}", duplicates),
                );
            }

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

/// 从当前 ImGui 上下文读取系统剪贴板文本
fn read_clipboard_text() -> Option<String> {
    unsafe {
        let ptr = dear_imgui_rs::sys::igGetClipboardText();
        if ptr.is_null() {
            None
        } else {
            Some(std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned())
        }
    }
}

/// Parse clipboard text like `0 "Off" 1 "On"` into (value, description) pairs.
fn parse_val_desc_text(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let num_end = rest
            .find(|c: char| !matches!(c, '0'..='9' | '-' | '+'))
            .unwrap_or(rest.len());
        if num_end == 0 {
            let ch_len = rest.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            rest = rest[ch_len..].trim_start();
            continue;
        }
        let num = &rest[..num_end];
        rest = rest[num_end..].trim_start();

        let Some(stripped) = rest.strip_prefix('"') else {
            continue;
        };
        let desc = match stripped.find('"') {
            Some(end) => {
                let d = stripped[..end].to_string();
                rest = stripped[end + 1..].trim_start();
                d
            }
            None => {
                let d = stripped.trim_end().to_string();
                rest = "";
                d
            }
        };

        if num.parse::<i64>().is_ok() {
            out.push((num.to_string(), desc));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::parse_val_desc_text;

    #[test]
    fn parses_basic_pairs() {
        assert_eq!(
            parse_val_desc_text("0 \"Off\" 1 \"On\""),
            vec![
                ("0".to_string(), "Off".to_string()),
                ("1".to_string(), "On".to_string())
            ]
        );
    }

    #[test]
    fn parses_negative_and_multiline() {
        assert_eq!(
            parse_val_desc_text("-1 \"Reverse\"\n2 \"High speed\""),
            vec![
                ("-1".to_string(), "Reverse".to_string()),
                ("2".to_string(), "High speed".to_string())
            ]
        );
    }

    #[test]
    fn skips_garbage() {
        assert_eq!(
            parse_val_desc_text("VAL_ 100 Sig 3 \"X\" ;"),
            vec![("3".to_string(), "X".to_string())]
        );
    }
}
