//! Signal 窗口渲染模块

use crate::dbc::Message;
use crate::ui::state::{SignalWindowState, UiState};
use imgui::{Condition, TableColumnFlags, TableColumnSetup, TableFlags, Ui, WindowFocusedFlags};

/// 渲染所有 Signal 窗口
pub fn render_signal_windows(ui: &Ui, ui_state: &mut UiState) {
    let mut windows_to_remove = Vec::new();

    for (index, window) in ui_state.signal_windows.iter_mut().enumerate() {
        request_window_focus_if_needed(ui_state.signal_window_focus_request, window.id);

        if !window.is_open {
            continue;
        }

        let (still_open, focused) = render_signal_window(ui, window);

        if focused {
            ui_state.last_focused_signal_window = Some(index);
            ui_state.last_focused_dbc_index = None;
        }

        if !still_open {
            windows_to_remove.push(index);
        }
    }

    // 清空聚焦请求
    ui_state.dbc_window_focus_request = None;
    ui_state.signal_window_focus_request = None;

    cleanup_closed_signal_windows(ui_state, windows_to_remove);
}

/// 渲染单个 Signal 详细窗口
fn render_signal_window(ui: &Ui, window_state: &mut SignalWindowState) -> (bool, bool) {
    let window_title = format!(
        "Signals - {} (0x{:03X})",
        window_state.message.message_name(),
        window_state.message.message_id().raw()
    );

    let mut is_open = window_state.is_open;
    let mut focused = false;

    if is_open {
        let window = ui
            .window(&window_title)
            .opened(&mut is_open)
            .size([800.0, 600.0], Condition::FirstUseEver)
            .position(
                [
                    100.0 + (window_state.id as f32 * 30.0),
                    100.0 + (window_state.id as f32 * 30.0),
                ],
                Condition::FirstUseEver,
            );

        window.build(|| {
            render_signal_window_content(ui, &window_state.message);
            if ui.is_window_focused_with_flags(WindowFocusedFlags::ROOT_AND_CHILD_WINDOWS) {
                focused = true;
            }
        });
    }

    if !is_open {
        window_state.is_open = false;
    }

    (window_state.is_open, focused)
}

/// 渲染Signal窗口的内容
fn render_signal_window_content(ui: &Ui, message: &Message) {
    ui.text(format!(
        "Message: {} (0x{:03X}) - {} signals",
        message.message_name(),
        message.message_id().raw(),
        message.signals().len()
    ));
    ui.separator();

    // 创建完整的信号表格
    if let Some(_table) = ui.begin_table_with_flags(
        "full_signals_table",
        10,
        TableFlags::RESIZABLE
            | TableFlags::REORDERABLE
            | TableFlags::HIDEABLE
            | TableFlags::BORDERS
            | TableFlags::SIZING_FIXED_FIT
            | TableFlags::SCROLL_Y
            | TableFlags::SORTABLE,
    ) {
        // 设置完整的表格列
        let columns = [
            (
                "Signal",
                TableColumnFlags::DEFAULT_SORT | TableColumnFlags::WIDTH_STRETCH,
                0.0,
            ),
            ("Type", TableColumnFlags::default(), 0.0),
            ("Order", TableColumnFlags::default(), 0.0),
            ("Start", TableColumnFlags::default(), 0.0),
            ("Length", TableColumnFlags::default(), 0.0),
            ("Factor", TableColumnFlags::default(), 0.0),
            ("Offset", TableColumnFlags::default(), 0.0),
            ("Min", TableColumnFlags::default(), 0.0),
            ("Max", TableColumnFlags::default(), 0.0),
            ("Unit", TableColumnFlags::default(), 0.0),
        ];

        for (name, flags, width) in &columns {
            ui.table_setup_column_with(TableColumnSetup {
                name,
                flags: *flags,
                init_width_or_weight: *width,
                user_id: ui.new_id_str(&format!("full_{}", name.to_lowercase())),
            });
        }

        ui.table_headers_row();

        // 显示所有信号
        for signal in message.signals().iter() {
            ui.table_next_row();

            ui.table_set_column_index(0);
            ui.text(signal.name());

            ui.table_set_column_index(1);
            let data_type = match signal.value_type() {
                can_dbc::ValueType::Signed => "signed",
                can_dbc::ValueType::Unsigned => "unsigned",
            };
            ui.text(data_type);

            ui.table_set_column_index(2);
            let byte_order = match signal.byte_order() {
                can_dbc::ByteOrder::LittleEndian => "Intel",
                can_dbc::ByteOrder::BigEndian => "Motorola",
            };
            ui.text(byte_order);

            ui.table_set_column_index(3);
            ui.text(format!("{}", signal.start_bit()));

            ui.table_set_column_index(4);
            ui.text(format!("{}", signal.signal_size()));

            ui.table_set_column_index(5);
            ui.text(format!("{}", signal.factor()));

            ui.table_set_column_index(6);
            ui.text(format!("{}", signal.offset()));

            ui.table_set_column_index(7);
            ui.text(format!("{}", signal.min()));

            ui.table_set_column_index(8);
            ui.text(format!("{}", signal.max()));

            ui.table_set_column_index(9);
            ui.text(signal.unit());
        }
    }
}

/// 清理已关闭的 Signal 窗口
fn cleanup_closed_signal_windows(ui_state: &mut UiState, windows_to_remove: Vec<usize>) {
    for &index in windows_to_remove.iter().rev() {
        ui_state.signal_windows.remove(index);

        // 修正焦点索引
        if let Some(focused_sig) = ui_state.last_focused_signal_window {
            if focused_sig == index {
                ui_state.last_focused_signal_window = None;
            } else if focused_sig > index {
                ui_state.last_focused_signal_window = Some(focused_sig - 1);
            }
        }
    }
}

/// 请求窗口聚焦（如果需要）
fn request_window_focus_if_needed(focus_request: Option<usize>, window_id: usize) {
    if focus_request == Some(window_id) {
        unsafe {
            imgui::sys::igSetNextWindowFocus();
        }
    }
}
