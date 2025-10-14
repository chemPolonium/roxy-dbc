//! UI 渲染和界面逻辑
use crate::dbc::DbcData;
use imgui::{Condition, TableColumnFlags, TableColumnSetup, TableFlags, TableSortDirection, Ui};
use std::time::Duration;

/// DBC 窗口状态
#[derive(Clone)]
pub struct DbcWindowState {
    pub id: usize,
    pub dbc_data: DbcData,
    pub search_query: String,
    pub selected_message_id: Option<u32>,
    pub is_open: bool,
}

/// 错误对话框状态
pub struct ErrorDialog {
    pub show: bool,
    pub message: String,
}

/// UI 状态管理
pub struct UiState {
    pub show_performance_window: bool,
    pub show_about_dialog: bool,
    pub show_can_window: bool,
    pub show_chart_window: bool,
    pub dbc_windows: Vec<DbcWindowState>,
    pub next_dbc_id: usize,
    pub error_dialog: ErrorDialog,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            show_performance_window: false,
            show_about_dialog: false,
            show_can_window: false,
            show_chart_window: false,
            dbc_windows: Vec::new(),
            next_dbc_id: 1,
            error_dialog: ErrorDialog {
                show: false,
                message: String::new(),
            },
        }
    }
}

/// 设置主dockspace占满整个窗口
fn setup_main_dockspace(ui: &Ui, ui_state: &mut UiState) {
    // 主菜单栏
    render_main_menu_bar(ui, ui_state);

    // 使用dockspace_over_main_viewport API创建全屏dockspace
    ui.dockspace_over_main_viewport();
}

/// 渲染主界面  
pub fn render_ui(ui: &Ui, delta_s: Duration, target_frame_time: Duration, ui_state: &mut UiState) {
    // 创建全屏主dockspace
    setup_main_dockspace(ui, ui_state);

    // 根据菜单状态显示窗口
    if ui_state.show_performance_window {
        render_performance_window(ui, delta_s, target_frame_time);
    }

    if ui_state.show_can_window {
        render_can_window(ui);
    }

    if ui_state.show_chart_window {
        render_chart_window(ui);
    }

    // Render all DBC windows
    let mut windows_to_remove = Vec::new();
    for (index, window) in ui_state.dbc_windows.iter_mut().enumerate() {
        if window.is_open {
            if !render_dbc_window(ui, window) {
                windows_to_remove.push(index);
            }
        }
    }
    // Remove closed windows in reverse order to maintain indices
    for &index in windows_to_remove.iter().rev() {
        ui_state.dbc_windows.remove(index);
    }

    // Render error dialog
    if ui_state.error_dialog.show {
        render_error_dialog(ui, &mut ui_state.error_dialog);
    }

    if ui_state.show_about_dialog {
        render_about_dialog(ui, &mut ui_state.show_about_dialog);
    }
}

/// 渲染主菜单栏
fn render_main_menu_bar(ui: &Ui, ui_state: &mut UiState) {
    ui.main_menu_bar(|| {
        ui.menu("File", || {
            if ui.menu_item("Load DBC File") {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("DBC files", &["dbc"])
                    .pick_file()
                {
                    let mut dbc_data = DbcData::new();
                    match dbc_data.load_dbc_file(&path) {
                        Ok(_) => {
                            ui_state.dbc_windows.push(DbcWindowState {
                                id: ui_state.next_dbc_id,
                                is_open: true,
                                dbc_data,
                                search_query: String::new(),
                                selected_message_id: None,
                            });
                            ui_state.next_dbc_id += 1;
                        }
                        Err(e) => {
                            ui_state.error_dialog.message =
                                format!("Failed to load DBC file: {}", e);
                            ui_state.error_dialog.show = true;
                        }
                    }
                }
            }
            ui.separator();
            if ui.menu_item("Exit") {
                // 退出程序逻辑
                std::process::exit(0);
            }
        });

        ui.menu("View", || {
            ui.checkbox("Performance Window", &mut ui_state.show_performance_window);
        });

        ui.menu("Help", || {
            if ui.menu_item("About") {
                ui_state.show_about_dialog = true;
            }
        });
    });
}

/// 渲染性能信息窗口
fn render_performance_window(ui: &Ui, delta_s: Duration, target_frame_time: Duration) {
    let window = ui.window("Performance Information");
    window
        .size([300.0, 150.0], Condition::FirstUseEver)
        .position([400.0, 50.0], Condition::FirstUseEver)
        .build(|| {
            ui.text(format!("Frame Time: {delta_s:?}"));
            let fps = 1.0 / delta_s.as_secs_f32();
            ui.text(format!("FPS: {fps:.1}"));
            ui.text(format!(
                "Target FPS: {:.1}",
                1.0 / target_frame_time.as_secs_f32()
            ));
        });
}

/// 渲染 CAN 数据窗口
fn render_can_window(ui: &Ui) {
    let window = ui.window("CAN Data Display");
    window
        .size([500.0, 300.0], Condition::FirstUseEver)
        .position([100.0, 100.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("CAN Bus Monitoring");
            ui.separator();

            ui.text("Status: Disconnected");
            ui.same_line();
            if ui.button("Connect") {
                println!("CAN connect clicked");
            }

            ui.separator();
            ui.text("Message Log:");
            ui.child_window("can_log").size([0.0, 150.0]).build(|| {
                ui.text("0x123: 01 02 03 04 05 06 07 08");
                ui.text("0x456: AA BB CC DD EE FF 00 11");
                ui.text("0x789: DE AD BE EF CA FE BA BE");
            });
        });
}

/// 渲染图表窗口
fn render_chart_window(ui: &Ui) {
    let window = ui.window("Charts and Plotting");
    window
        .size([600.0, 400.0], Condition::FirstUseEver)
        .position([200.0, 150.0], Condition::FirstUseEver)
        .build(|| {
            ui.text("Data Visualization");
            ui.separator();

            ui.text("Chart Type:");
            ui.radio_button("Line Chart", &mut 0, 0);
            ui.same_line();
            ui.radio_button("Bar Chart", &mut 0, 1);
            ui.same_line();
            ui.radio_button("Scatter Plot", &mut 0, 2);

            ui.separator();
            ui.text("Placeholder for chart rendering");
            ui.text("Chart area would be implemented here with plotting library");

            if ui.button("Generate Sample Data") {
                println!("Generate sample data clicked");
            }
        });
}

/// 渲染单个 DBC 浏览器窗口
fn render_dbc_window(ui: &Ui, window_state: &mut DbcWindowState) -> bool {
    let window_title = format!(
        "DBC Browser {} - {}",
        window_state.id,
        if window_state.dbc_data.file_path.is_empty() {
            "No file"
        } else {
            &window_state.dbc_data.file_path
        }
    );

    let mut is_open = window_state.is_open;

    if is_open {
        let window = ui
            .window(&window_title)
            .opened(&mut is_open)
            .size([800.0, 600.0], Condition::FirstUseEver)
            .position(
                [
                    50.0 + (window_state.id as f32 * 30.0),
                    50.0 + (window_state.id as f32 * 30.0),
                ],
                Condition::FirstUseEver,
            );

        window.build(|| {
            render_dbc_window_content(ui, window_state);
        });
    }

    if !is_open {
        window_state.is_open = false;
    }

    window_state.is_open
}

/// 渲染DBC窗口的内容
fn render_dbc_window_content(ui: &Ui, window_state: &mut DbcWindowState) {
    // 文件信息区域
    render_dbc_file_info(ui, window_state);

    // 搜索栏
    render_dbc_search_bar(ui, window_state);

    // 计算表格高度
    let table_height = (ui.content_region_avail()[1] - 40.0) * 0.5;

    // 消息列表表格
    render_messages_table(ui, window_state, table_height);

    // 信号详细信息表格
    render_signals_table(ui, window_state, table_height);
}

/// 渲染DBC文件信息区域
fn render_dbc_file_info(ui: &Ui, window_state: &mut DbcWindowState) {
    if ui.button("Reload") {
        if !window_state.dbc_data.file_path.is_empty() {
            let path = std::path::PathBuf::from(&window_state.dbc_data.file_path);
            match window_state.dbc_data.load_dbc_file(&path) {
                Ok(_) => {
                    println!("DBC file reloaded successfully");
                }
                Err(e) => {
                    window_state.dbc_data.error_message = e;
                }
            }
        }
    }

    // 显示当前加载的文件
    if !window_state.dbc_data.file_path.is_empty() {
        ui.text(format!("Loaded: {}", window_state.dbc_data.file_path));
        ui.text(format!(
            "Messages: {}",
            window_state.dbc_data.message_count()
        ));
    } else {
        ui.text("No DBC file loaded");
    }

    // 显示错误信息
    if !window_state.dbc_data.error_message.is_empty() {
        ui.text_colored([1.0, 0.0, 0.0, 1.0], &window_state.dbc_data.error_message);
    }

    ui.separator();
}

/// 渲染搜索栏
fn render_dbc_search_bar(ui: &Ui, window_state: &mut DbcWindowState) {
    ui.text("Search Messages:");
    ui.input_text("##search", &mut window_state.search_query)
        .build();
    ui.separator();
}

/// 渲染消息列表表格
fn render_messages_table(ui: &Ui, window_state: &mut DbcWindowState, table_height: f32) {
    ui.child_window("messages_list")
        .size([0.0, table_height])
        .build(|| {
            let filtered_messages = window_state
                .dbc_data
                .search_messages(&window_state.search_query);

            if let Some(_table) = ui.begin_table_with_flags(
                "messages_table",
                4,
                TableFlags::RESIZABLE
                    | TableFlags::BORDERS_V
                    | TableFlags::SCROLL_Y
                    | TableFlags::SORTABLE,
            ) {
                setup_messages_table_columns(ui);
                ui.table_headers_row();

                let sorted_messages = sort_messages(ui, filtered_messages);
                render_messages_rows(ui, &mut window_state.selected_message_id, sorted_messages);
            }
        });

    ui.separator();
}

/// 设置消息表格的列
fn setup_messages_table_columns(ui: &Ui) {
    ui.table_setup_column_with(TableColumnSetup {
        name: "ID",
        flags: TableColumnFlags::DEFAULT_SORT | TableColumnFlags::WIDTH_FIXED,
        init_width_or_weight: 80.0,
        user_id: ui.new_id_str("id_col"),
    });
    ui.table_setup_column_with(TableColumnSetup {
        name: "Name",
        flags: TableColumnFlags::WIDTH_STRETCH,
        init_width_or_weight: 0.0,
        user_id: ui.new_id_str("name_col"),
    });
    ui.table_setup_column_with(TableColumnSetup {
        name: "Length",
        flags: TableColumnFlags::WIDTH_FIXED,
        init_width_or_weight: 60.0,
        user_id: ui.new_id_str("len_col"),
    });
    ui.table_setup_column_with(TableColumnSetup {
        name: "Signals",
        flags: TableColumnFlags::WIDTH_FIXED,
        init_width_or_weight: 60.0,
        user_id: ui.new_id_str("sig_col"),
    });
}

/// 排序消息列表
fn sort_messages<'a>(
    ui: &Ui,
    mut messages: Vec<&'a crate::dbc::Message>,
) -> Vec<&'a crate::dbc::Message> {
    if let Some(sort_specs) = ui.table_sort_specs_mut() {
        let specs = sort_specs.specs();
        for (i, spec) in specs.iter().enumerate() {
            if i == 0 {
                let ascending = spec.sort_direction() == Some(TableSortDirection::Ascending);
                messages.sort_by(|a, b| {
                    let ordering = match spec.column_idx() {
                        0 => a.message_id().raw().cmp(&b.message_id().raw()),
                        1 => a.message_name().cmp(b.message_name()),
                        2 => a.message_size().cmp(b.message_size()),
                        3 => a.signals().len().cmp(&b.signals().len()),
                        _ => std::cmp::Ordering::Equal,
                    };
                    if ascending {
                        ordering
                    } else {
                        ordering.reverse()
                    }
                });
                break;
            }
        }
    }
    messages
}

/// 渲染消息表格的行
fn render_messages_rows(
    ui: &Ui,
    current_selection: &mut Option<u32>,
    messages: Vec<&crate::dbc::Message>,
) {
    for message in messages {
        ui.table_next_row();

        let selected = *current_selection == Some(message.message_id().raw());

        if selected {
            ui.table_set_bg_color(imgui::TableBgTarget::ROW_BG0, [0.3, 0.3, 0.7, 0.65]);
        }

        ui.table_set_column_index(0);
        if ui
            .selectable_config(format!("0x{:03X}", message.message_id().raw()))
            .selected(selected)
            .span_all_columns(true)
            .build()
        {
            *current_selection = Some(message.message_id().raw());
        }

        ui.table_set_column_index(1);
        ui.text(message.message_name());

        ui.table_set_column_index(2);
        ui.text(format!("{}", message.message_size()));

        ui.table_set_column_index(3);
        ui.text(format!("{}", message.signals().len()));
    }
}

/// 渲染信号详细信息表格
fn render_signals_table(ui: &Ui, window_state: &mut DbcWindowState, table_height: f32) {
    if let Some(selected_id) = window_state.selected_message_id {
        if let Some(message) = window_state.dbc_data.get_message_by_id(selected_id) {
            ui.text(format!(
                "Message Details: {} (0x{:03X})",
                message.message_name(),
                message.message_id().raw()
            ));
            ui.separator();

            ui.child_window("signals_list")
                .size([0.0, table_height])
                .build(|| {
                    if let Some(_table) = ui.begin_table_with_flags(
                        "signals_table",
                        10,
                        TableFlags::RESIZABLE
                            | TableFlags::BORDERS_V
                            | TableFlags::SCROLL_Y
                            | TableFlags::SORTABLE,
                    ) {
                        setup_signals_table_columns(ui);
                        ui.table_headers_row();

                        let sorted_signals = sort_signals(ui, message);
                        render_signals_rows(ui, sorted_signals);
                    }
                });
        }
    }
}

/// 设置信号表格的列
fn setup_signals_table_columns(ui: &Ui) {
    let columns = [
        (
            "Signal",
            TableColumnFlags::DEFAULT_SORT | TableColumnFlags::WIDTH_STRETCH,
            0.0,
            "signal_name_col",
        ),
        (
            "Start",
            TableColumnFlags::WIDTH_FIXED,
            50.0,
            "start_bit_col",
        ),
        (
            "Length",
            TableColumnFlags::WIDTH_FIXED,
            50.0,
            "signal_len_col",
        ),
        ("Factor", TableColumnFlags::WIDTH_FIXED, 50.0, "factor_col"),
        ("Offset", TableColumnFlags::WIDTH_FIXED, 50.0, "offset_col"),
        ("Min", TableColumnFlags::WIDTH_FIXED, 70.0, "min_col"),
        ("Max", TableColumnFlags::WIDTH_FIXED, 70.0, "max_col"),
        ("Unit", TableColumnFlags::WIDTH_FIXED, 60.0, "unit_col"),
        ("Type", TableColumnFlags::WIDTH_FIXED, 60.0, "data_type_col"),
        (
            "Order",
            TableColumnFlags::WIDTH_FIXED,
            60.0,
            "byte_order_col",
        ),
    ];

    for (name, flags, width, id) in &columns {
        ui.table_setup_column_with(TableColumnSetup {
            name,
            flags: *flags,
            init_width_or_weight: *width,
            user_id: ui.new_id_str(id),
        });
    }
}

/// 排序信号列表
fn sort_signals<'a>(ui: &Ui, message: &'a crate::dbc::Message) -> Vec<&'a can_dbc::Signal> {
    let mut sorted_signals: Vec<_> = message.signals().iter().collect();

    if let Some(sort_specs) = ui.table_sort_specs_mut() {
        let specs = sort_specs.specs();
        for (i, spec) in specs.iter().enumerate() {
            if i == 0 {
                let ascending = spec.sort_direction() == Some(TableSortDirection::Ascending);
                sorted_signals.sort_by(|a, b| {
                    let ordering = match spec.column_idx() {
                        0 => a.name().cmp(b.name()),
                        1 => a.start_bit().cmp(b.start_bit()),
                        2 => a.signal_size().cmp(b.signal_size()),
                        3 => a
                            .factor()
                            .partial_cmp(b.factor())
                            .unwrap_or(std::cmp::Ordering::Equal),
                        4 => a
                            .offset()
                            .partial_cmp(b.offset())
                            .unwrap_or(std::cmp::Ordering::Equal),
                        5 => a
                            .min()
                            .partial_cmp(b.min())
                            .unwrap_or(std::cmp::Ordering::Equal),
                        6 => a
                            .max()
                            .partial_cmp(b.max())
                            .unwrap_or(std::cmp::Ordering::Equal),
                        7 => a.unit().cmp(b.unit()),
                        8 => format!("{:?}", a.value_type()).cmp(&format!("{:?}", b.value_type())),
                        9 => format!("{:?}", a.byte_order()).cmp(&format!("{:?}", b.byte_order())),
                        _ => std::cmp::Ordering::Equal,
                    };
                    if ascending {
                        ordering
                    } else {
                        ordering.reverse()
                    }
                });
                break;
            }
        }
    }

    sorted_signals
}

/// 计算浮点数的小数位数
fn get_decimal_places(value: f64) -> usize {
    if value == 0.0 || value.fract() == 0.0 {
        return 0;
    }

    // 处理非常小的数字（如1e-7）
    let abs_value = value.abs();
    if abs_value < 1e-10 {
        return 10; // 对于极小的数字，返回最大精度
    }

    // 使用更高精度的格式化来处理小数
    let value_str = format!("{:.15}", abs_value); // 最多显示15位小数

    if let Some(decimal_part) = value_str.split('.').nth(1) {
        // 从右边开始删除尾随的0
        let trimmed = decimal_part.trim_end_matches('0');
        trimmed.len().min(10) // 最多10位小数，足以处理1e-7
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decimal_places() {
        assert_eq!(get_decimal_places(1.0), 0);
        assert_eq!(get_decimal_places(1.5), 1);
        assert_eq!(get_decimal_places(0.01), 2);
        assert_eq!(get_decimal_places(0.001), 3);
        assert_eq!(get_decimal_places(0.125), 3);
        assert_eq!(get_decimal_places(1.23456), 5);
        assert_eq!(get_decimal_places(1e-7), 7); // 处理科学计数法
        assert_eq!(get_decimal_places(0.0000001), 7); // 1e-7的十进制表示
        assert_eq!(get_decimal_places(1e-6), 6); // 1e-6
        assert_eq!(get_decimal_places(1e-5), 5); // 1e-5
        assert_eq!(get_decimal_places(1.234567890123456), 10); // 限制为最多10位
        assert_eq!(get_decimal_places(0.0), 0); // 零值
    }

    #[test]
    fn test_format_with_precision() {
        // 测试实际的格式化效果
        let factor = 1e-7;
        let precision = get_decimal_places(factor);
        let formatted = format!("{:.prec$}", factor, prec = precision);
        println!("1e-7 with {} decimal places: {}", precision, formatted);

        let factor2 = 0.0000001;
        let precision2 = get_decimal_places(factor2);
        let formatted2 = format!("{:.prec$}", factor2, prec = precision2);
        println!(
            "0.0000001 with {} decimal places: {}",
            precision2, formatted2
        );

        assert_eq!(precision, 7);
        assert_eq!(precision2, 7);
    }
}

/// 渲染信号表格的行
fn render_signals_rows(ui: &Ui, signals: Vec<&can_dbc::Signal>) {
    for signal in signals {
        ui.table_next_row();

        // 根据factor的小数位数确定显示精度
        let factor_precision = get_decimal_places(*signal.factor());
        let offset_precision = get_decimal_places(*signal.offset());

        // 对于min/max，使用factor精度来确定小数位数
        let value_precision = factor_precision;

        ui.table_set_column_index(0);
        ui.text(signal.name());

        ui.table_set_column_index(1);
        ui.text(format!("{}", signal.start_bit()));

        ui.table_set_column_index(2);
        ui.text(format!("{}", signal.signal_size()));

        ui.table_set_column_index(3);
        // Factor: 如果是整数则显示整数，否则使用适当精度
        let factor_text = if signal.factor().fract() == 0.0 {
            format!("{}", *signal.factor() as i64)
        } else {
            format!("{:.prec$}", signal.factor(), prec = factor_precision)
        };
        ui.text(factor_text);

        ui.table_set_column_index(4);
        // Offset: 如果是整数则显示整数，否则使用适当精度
        let offset_text = if signal.offset().fract() == 0.0 {
            format!("{}", *signal.offset() as i64)
        } else {
            format!("{:.prec$}", signal.offset(), prec = offset_precision)
        };
        ui.text(offset_text);

        ui.table_set_column_index(5);
        // Min: 如果是整数则显示整数，否则使用与factor相同的精度
        let min_text = if signal.min().fract() == 0.0 {
            format!("{}", *signal.min() as i64)
        } else {
            format!("{:.prec$}", signal.min(), prec = value_precision)
        };
        ui.text(min_text);

        ui.table_set_column_index(6);
        // Max: 如果是整数则显示整数，否则使用与factor相同的精度
        let max_text = if signal.max().fract() == 0.0 {
            format!("{}", *signal.max() as i64)
        } else {
            format!("{:.prec$}", signal.max(), prec = value_precision)
        };
        ui.text(max_text);

        ui.table_set_column_index(7);
        ui.text(signal.unit());

        ui.table_set_column_index(8);
        let data_type = match signal.value_type() {
            can_dbc::ValueType::Signed => "signed",
            can_dbc::ValueType::Unsigned => "unsigned",
        };
        ui.text(data_type);

        ui.table_set_column_index(9);
        let byte_order = match signal.byte_order() {
            can_dbc::ByteOrder::LittleEndian => "Intel",
            can_dbc::ByteOrder::BigEndian => "Motorola",
        };
        ui.text(byte_order);
    }
}

/// 渲染错误对话框
fn render_error_dialog(ui: &Ui, error_dialog: &mut ErrorDialog) {
    if error_dialog.show {
        ui.open_popup("Error");
    }

    ui.modal_popup_config("Error")
        .resizable(true)
        .always_auto_resize(false)
        .build(|| {
            ui.text("Error");
            ui.separator();

            // 添加一些空间使对话框更大
            ui.dummy([400.0, 0.0]); // 设置最小宽度

            // 使用文本包装来处理长错误消息
            ui.text_wrapped(&error_dialog.message);

            // 添加一些垂直空间
            ui.dummy([0.0, 20.0]);
            ui.separator();

            // 居中显示OK按钮
            let button_width = 80.0;
            let avail_width = ui.content_region_avail()[0];
            let offset = (avail_width - button_width) * 0.5;
            if offset > 0.0 {
                ui.dummy([offset, 0.0]);
                ui.same_line();
            }

            if ui.button_with_size("OK", [button_width, 0.0]) {
                ui.close_current_popup();
                error_dialog.show = false;
            }
        });
}

/// 渲染关于对话框
fn render_about_dialog(ui: &Ui, show_about: &mut bool) {
    if *show_about {
        ui.open_popup("About");
    }

    ui.modal_popup_config("About").resizable(false).build(|| {
        ui.text("Roxy dbc viewer");
        ui.separator();
        ui.text("Version: 0.3.0");
        ui.text("Built with Rust and ImGui");
        ui.separator();
        ui.text("An application for viewing CAN DBC files.");

        ui.separator();
        if ui.button("Close") {
            ui.close_current_popup();
            *show_about = false;
        }
    });
}
