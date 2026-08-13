use can_dbc::ByteOrder;
use imgui::{TableFlags, Ui};

use crate::editable_dbc::EditableDbc;

pub enum MessageWindowEvent {
    None,
    EditSignal(String),
    CopySignal(Vec<String>),
    CutSignal(Vec<String>),
    DeleteSignal(Vec<String>),
    PasteSignal,
    AddSignal,
}

/// Message 详细窗口状态（包含 Signal 表格）
#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct MessageWindow {
    pub message_id: u32,
    pub is_open: bool,
    pub parent_dbc_id: usize,
    pub selected_signal_names: Vec<String>,
    pub signal_anchor: Option<String>,
    pub signal_cursor: Option<String>,
    pub focus_requested: bool,
}

impl MessageWindow {
    pub fn new(message_id: u32, parent_dbc_id: usize) -> Self {
        Self {
            message_id,
            is_open: true,
            parent_dbc_id,
            selected_signal_names: Vec::new(),
            signal_anchor: None,
            signal_cursor: None,
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

                if ui.small_button("+ Add Signal") {
                    event = MessageWindowEvent::AddSignal;
                }

                let signals = message.signals();
                if signals.is_empty() {
                    ui.same_line();
                    ui.text("No signals in this message");
                    return;
                }

                let selected = self.selected_signal_names.clone();
                crate::ui::bit_layout::render_bit_layout(ui, message, &selected);
                ui.separator();

                // Keyboard navigation
                if ui.is_window_focused() {
                    let shift = ui.io().key_shift;
                    let cursor_pos = self
                        .signal_cursor
                        .as_ref()
                        .and_then(|name| signals.iter().position(|s| s.name() == name.as_str()));

                    let mut new_pos: Option<usize> = None;
                    if ui.is_key_pressed(imgui::Key::DownArrow) {
                        new_pos = Some(match cursor_pos {
                            Some(p) => (p + 1).min(signals.len() - 1),
                            None => 0,
                        });
                    }
                    if ui.is_key_pressed(imgui::Key::UpArrow) {
                        new_pos = Some(match cursor_pos {
                            Some(0) => 0,
                            Some(p) => p - 1,
                            None => signals.len() - 1,
                        });
                    }

                    if let Some(pos) = new_pos {
                        let new_name = signals[pos].name().to_string();
                        self.signal_cursor = Some(new_name.clone());
                        if shift {
                            let anchor = self
                                .signal_anchor
                                .clone()
                                .unwrap_or_else(|| new_name.clone());
                            let anchor_pos = signals
                                .iter()
                                .position(|s| s.name() == anchor.as_str())
                                .unwrap_or(pos);
                            let (lo, hi) = if anchor_pos <= pos {
                                (anchor_pos, pos)
                            } else {
                                (pos, anchor_pos)
                            };
                            self.selected_signal_names = signals[lo..=hi]
                                .iter()
                                .map(|s| s.name().to_string())
                                .collect();
                        } else {
                            self.selected_signal_names = vec![new_name.clone()];
                            self.signal_anchor = Some(new_name);
                        }
                    }
                    if ui.is_key_pressed(imgui::Key::Enter) {
                        let target = self
                            .signal_cursor
                            .clone()
                            .or_else(|| self.selected_signal_names.first().cloned());
                        if let Some(sig_name) = target {
                            event = MessageWindowEvent::EditSignal(sig_name);
                        }
                    }
                }

                if let Some(_table) = ui.begin_table_with_flags(
                    "signals_table",
                    9,
                    TableFlags::RESIZABLE
                        | TableFlags::BORDERS
                        | TableFlags::NO_BORDERS_IN_BODY
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
                    ui.table_setup_column("Comment");
                    ui.table_headers_row();

                    for (row_pos, signal) in signals.iter().enumerate() {
                        ui.table_next_row();

                        let sig_name = signal.name();
                        let is_selected = self
                            .selected_signal_names
                            .iter()
                            .any(|n| n == sig_name);

                        ui.table_set_column_index(0);
                        if ui
                            .selectable_config(sig_name)
                            .selected(is_selected)
                            .span_all_columns(true)
                            .build()
                        {
                            let ctrl = ui.io().key_ctrl;
                            let shift = ui.io().key_shift;
                            let name_owned = sig_name.to_string();
                            if ctrl {
                                if let Some(pos) = self
                                    .selected_signal_names
                                    .iter()
                                    .position(|n| n == &name_owned)
                                {
                                    self.selected_signal_names.remove(pos);
                                } else {
                                    self.selected_signal_names.push(name_owned.clone());
                                }
                                self.signal_anchor = Some(name_owned.clone());
                                self.signal_cursor = Some(name_owned);
                            } else if shift {
                                let anchor = self
                                    .signal_anchor
                                    .clone()
                                    .unwrap_or_else(|| name_owned.clone());
                                let anchor_pos = signals
                                    .iter()
                                    .position(|s| s.name() == anchor.as_str())
                                    .unwrap_or(row_pos);
                                let (lo, hi) = if anchor_pos <= row_pos {
                                    (anchor_pos, row_pos)
                                } else {
                                    (row_pos, anchor_pos)
                                };
                                self.selected_signal_names = signals[lo..=hi]
                                    .iter()
                                    .map(|s| s.name().to_string())
                                    .collect();
                                self.signal_cursor = Some(name_owned);
                            } else {
                                self.selected_signal_names = vec![name_owned.clone()];
                                self.signal_anchor = Some(name_owned.clone());
                                self.signal_cursor = Some(name_owned);
                            }
                        }
                        if ui.is_item_hovered()
                            && ui.is_mouse_double_clicked(imgui::MouseButton::Left)
                        {
                            event = MessageWindowEvent::EditSignal(sig_name.to_string());
                        }

                        if let Some(_popup) = ui.begin_popup_context_with_label(format!("sig_ctx_{}", sig_name)) {
                            if !self.selected_signal_names.iter().any(|n| n == sig_name) {
                                let name_owned = sig_name.to_string();
                                self.selected_signal_names = vec![name_owned.clone()];
                                self.signal_anchor = Some(name_owned.clone());
                                self.signal_cursor = Some(name_owned);
                            }
                            let selected_names = self.selected_signal_names.clone();
                            if ui.menu_item("Edit") {
                                event = MessageWindowEvent::EditSignal(sig_name.to_string());
                            }
                            if ui.menu_item("Copy") {
                                event = MessageWindowEvent::CopySignal(selected_names.clone());
                            }
                            if ui.menu_item("Cut") {
                                event = MessageWindowEvent::CutSignal(selected_names.clone());
                            }
                            if ui.menu_item_config("Paste").enabled(has_clipboard).build() {
                                event = MessageWindowEvent::PasteSignal;
                            }
                            ui.separator();
                            if ui.menu_item("Delete") {
                                event = MessageWindowEvent::DeleteSignal(selected_names);
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

                        ui.table_set_column_index(8);
                        ui.text(signal.comment());
                    }
                }
            });

        self.is_open = is_open;
        event
    }
}
