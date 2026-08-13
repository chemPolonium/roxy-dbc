use can_dbc::ByteOrder;
use imgui::{TableFlags, Ui};

use crate::editable_dbc::EditableDbc;

pub enum MessageWindowEvent {
    None,
    EditSignal(String),
    CopySignal(String),
    CutSignal(String),
    DeleteSignal(String),
    PasteSignal,
}

/// Message 详细窗口状态（包含 Signal 表格）
#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct MessageWindow {
    pub message_id: u32,
    pub is_open: bool,
    pub parent_dbc_id: usize,
    pub selected_signal_name: Option<String>,
    pub focus_requested: bool,
}

impl MessageWindow {
    pub fn new(message_id: u32, parent_dbc_id: usize) -> Self {
        Self {
            message_id,
            is_open: true,
            parent_dbc_id,
            selected_signal_name: None,
            focus_requested: false,
        }
    }

    pub fn render(&mut self, ui: &Ui, dbc: &EditableDbc, has_clipboard: bool) -> MessageWindowEvent {
        let mut event = MessageWindowEvent::None;

        let Some(message) = dbc.get_message(self.message_id) else {
            return event;
        };

        let title = format!(
            "{} (0x{:03X})",
            message.message_name(),
            message.message_id()
        );
        let mut is_open = self.is_open;

        if self.focus_requested {
            unsafe { imgui::sys::igSetNextWindowFocus() };
            self.focus_requested = false;
        }

        ui.window(&title)
            .size([700.0, 400.0], imgui::Condition::FirstUseEver)
            .opened(&mut is_open)
            .build(|| {
                ui.text(format!(
                    "ID: 0x{:03X}  |  Size: {} bytes  |  Transmitter: {}",
                    message.message_id(),
                    message.message_size(),
                    message.transmitter()
                ));

                let comment = message.comment();
                if !comment.is_empty() {
                    ui.text(format!("Comment: {}", comment));
                }

                ui.separator();

                let signals = message.signals();
                if signals.is_empty() {
                    ui.text("No signals in this message");
                    return;
                }

                // Keyboard navigation
                if ui.is_window_focused() {
                    let current_pos = self
                        .selected_signal_name
                        .as_ref()
                        .and_then(|name| signals.iter().position(|s| s.name() == name.as_str()));

                    if ui.is_key_pressed(imgui::Key::DownArrow) {
                        let next_pos = match current_pos {
                            Some(p) => (p + 1).min(signals.len() - 1),
                            None => 0,
                        };
                        self.selected_signal_name = Some(signals[next_pos].name().to_string());
                    }
                    if ui.is_key_pressed(imgui::Key::UpArrow) {
                        let prev_pos = match current_pos {
                            Some(0) => 0,
                            Some(p) => p - 1,
                            None => signals.len() - 1,
                        };
                        self.selected_signal_name = Some(signals[prev_pos].name().to_string());
                    }
                    if ui.is_key_pressed(imgui::Key::Enter) {
                        if let Some(ref sig_name) = self.selected_signal_name {
                            event = MessageWindowEvent::EditSignal(sig_name.clone());
                        }
                    }
                }

                if let Some(_table) = ui.begin_table_with_flags(
                    "signals_table",
                    8,
                    TableFlags::RESIZABLE
                        | TableFlags::BORDERS
                        | TableFlags::SCROLL_Y
                        | TableFlags::SIZING_FIXED_FIT,
                ) {
                    ui.table_setup_column("Name");
                    ui.table_setup_column("Start");
                    ui.table_setup_column("Length");
                    ui.table_setup_column("Order");
                    ui.table_setup_column("Type");
                    ui.table_setup_column("Factor");
                    ui.table_setup_column("Offset");
                    ui.table_setup_column("Unit");
                    ui.table_headers_row();

                    for signal in signals.iter() {
                        ui.table_next_row();

                        let sig_name = signal.name();
                        let is_selected = self.selected_signal_name.as_deref() == Some(sig_name);

                        ui.table_set_column_index(0);
                        if ui
                            .selectable_config(sig_name)
                            .selected(is_selected)
                            .span_all_columns(true)
                            .build()
                        {
                            self.selected_signal_name = Some(sig_name.to_string());
                        }
                        if ui.is_item_hovered()
                            && ui.is_mouse_double_clicked(imgui::MouseButton::Left)
                        {
                            event = MessageWindowEvent::EditSignal(sig_name.to_string());
                        }

                        if let Some(_popup) = ui.begin_popup_context_with_label(format!("sig_ctx_{}", sig_name)) {
                            self.selected_signal_name = Some(sig_name.to_string());
                            if ui.menu_item("Edit") {
                                event = MessageWindowEvent::EditSignal(sig_name.to_string());
                            }
                            if ui.menu_item("Copy") {
                                event = MessageWindowEvent::CopySignal(sig_name.to_string());
                            }
                            if ui.menu_item("Cut") {
                                event = MessageWindowEvent::CutSignal(sig_name.to_string());
                            }
                            if ui.menu_item_config("Paste").enabled(has_clipboard).build() {
                                event = MessageWindowEvent::PasteSignal;
                            }
                            ui.separator();
                            if ui.menu_item("Delete") {
                                event = MessageWindowEvent::DeleteSignal(sig_name.to_string());
                            }
                        }

                        ui.table_set_column_index(1);
                        ui.text(format!("{}", signal.start_bit()));

                        ui.table_set_column_index(2);
                        ui.text(format!("{}", signal.signal_size()));

                        ui.table_set_column_index(3);
                        ui.text(match signal.byte_order() {
                            ByteOrder::LittleEndian => "Intel",
                            ByteOrder::BigEndian => "Motorola",
                        });

                        ui.table_set_column_index(4);
                        ui.text(match signal.value_type() {
                            can_dbc::ValueType::Unsigned => "Unsigned",
                            can_dbc::ValueType::Signed => "Signed",
                        });

                        ui.table_set_column_index(5);
                        ui.text(format!("{}", signal.factor()));

                        ui.table_set_column_index(6);
                        ui.text(format!("{}", signal.offset()));

                        ui.table_set_column_index(7);
                        ui.text(signal.unit());
                    }
                }
            });

        self.is_open = is_open;
        event
    }
}
