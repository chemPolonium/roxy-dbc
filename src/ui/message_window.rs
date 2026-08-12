use can_dbc::ByteOrder;
use imgui::{TableFlags, Ui};

use crate::editable_dbc::EditableMessage;

/// Message 详细窗口状态（包含 Signal 表格）
#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct MessageWindow {
    pub message: EditableMessage,
    pub is_open: bool,
    pub parent_dbc_id: usize,
    pub pending_signal_edit: Option<String>,
    pub selected_signal_name: Option<String>,
}

impl MessageWindow {
    pub fn new(message: EditableMessage, parent_dbc_id: usize) -> Self {
        Self {
            message,
            is_open: true,
            parent_dbc_id,
            pending_signal_edit: None,
            selected_signal_name: None,
        }
    }

    pub fn render(&mut self, ui: &Ui) {
        let title = format!(
            "Signals - {} (0x{:03X})",
            self.message.message_name(),
            self.message.message_id()
        );
        let mut is_open = self.is_open;

        ui.window(&title)
            .size([600.0, 400.0], imgui::Condition::FirstUseEver)
            .opened(&mut is_open)
            .build(|| {
                ui.text(format!("Size: {} bytes", self.message.message_size()));
                ui.text(format!("Transmitter: {}", self.message.transmitter()));

                let comment = self.message.comment();
                if !comment.is_empty() {
                    ui.text(format!("Comment: {}", comment));
                }

                ui.separator();

                let signals = self.message.signals();
                if signals.is_empty() {
                    ui.text("No signals in this message");
                    return;
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

                        ui.table_set_column_index(0);
                        ui.text(signal.name());

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
                        ui.text(format!("{:?}", signal.value_type()));

                        ui.table_set_column_index(5);
                        ui.text(format!("{:.4}", signal.factor()));

                        ui.table_set_column_index(6);
                        ui.text(format!("{:.4}", signal.offset()));

                        ui.table_set_column_index(7);
                        ui.text(signal.unit());
                    }
                }
            });

        self.is_open = is_open;
    }
}
