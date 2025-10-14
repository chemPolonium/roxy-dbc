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
    pub dbc_windows: Vec<DbcWindowState>,
    pub next_dbc_id: usize,
    pub error_dialog: ErrorDialog,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            show_performance_window: false,
            show_about_dialog: false,
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

/// 渲染单个 DBC 浏览器窗口
fn render_dbc_window(ui: &Ui, window_state: &mut DbcWindowState) -> bool {
    let window_title = format!(
        "DBC Browser {} - {}",
        window_state.id,
        if window_state.dbc_data.file_path.is_empty() {
            "No file"
        } else {
            // 只显示文件名，不显示完整路径
            std::path::Path::new(&window_state.dbc_data.file_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Unknown file")
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
    ui.text("Search Messages & Signals:");
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
                    | TableFlags::REORDERABLE
                    | TableFlags::SIZING_FIXED_FIT
                    | TableFlags::BORDERS
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
        flags: TableColumnFlags::DEFAULT_SORT,
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
        flags: TableColumnFlags::default(),
        init_width_or_weight: 60.0,
        user_id: ui.new_id_str("len_col"),
    });
    ui.table_setup_column_with(TableColumnSetup {
        name: "Signals",
        flags: TableColumnFlags::default(),
        init_width_or_weight: 60.0,
        user_id: ui.new_id_str("sig_col"),
    });
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
                            | TableFlags::REORDERABLE
                            | TableFlags::HIDEABLE
                            | TableFlags::BORDERS
                            | TableFlags::SIZING_FIXED_FIT
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
        ("Type", TableColumnFlags::default(), 50.0, "data_type_col"),
        ("Order", TableColumnFlags::default(), 50.0, "byte_order_col"),
        ("Start", TableColumnFlags::default(), 45.0, "start_bit_col"),
        (
            "Length",
            TableColumnFlags::default(),
            45.0,
            "signal_len_col",
        ),
        ("Factor", TableColumnFlags::default(), 55.0, "factor_col"),
        ("Offset", TableColumnFlags::default(), 45.0, "offset_col"),
        ("Min", TableColumnFlags::default(), 70.0, "min_col"),
        ("Max", TableColumnFlags::default(), 70.0, "max_col"),
        ("Unit", TableColumnFlags::default(), 60.0, "unit_col"),
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
                        1 => format!("{:?}", a.value_type()).cmp(&format!("{:?}", b.value_type())),
                        2 => format!("{:?}", a.byte_order()).cmp(&format!("{:?}", b.byte_order())),
                        3 => a.start_bit().cmp(b.start_bit()),
                        4 => a.signal_size().cmp(b.signal_size()),
                        5 => a
                            .factor()
                            .partial_cmp(b.factor())
                            .unwrap_or(std::cmp::Ordering::Equal),
                        6 => a
                            .offset()
                            .partial_cmp(b.offset())
                            .unwrap_or(std::cmp::Ordering::Equal),
                        7 => a
                            .min()
                            .partial_cmp(b.min())
                            .unwrap_or(std::cmp::Ordering::Equal),
                        8 => a
                            .max()
                            .partial_cmp(b.max())
                            .unwrap_or(std::cmp::Ordering::Equal),
                        9 => a.unit().cmp(b.unit()),
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

/// 渲染信号表格的行
fn render_signals_rows(ui: &Ui, signals: Vec<&can_dbc::Signal>) {
    for signal in signals {
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

/// 渲染错误对话框
fn render_error_dialog(ui: &Ui, error_dialog: &mut ErrorDialog) {
    if error_dialog.show {
        ui.open_popup("Error");
    }

    ui.modal_popup_config("Error").resizable(false).build(|| {
        ui.text("Error");
        ui.separator();

        // 使用固定大小的滚动区域显示错误消息
        ui.child_window("error_message")
            .size([400.0, 100.0])
            .build(|| {
                ui.text_wrapped(&error_dialog.message);
            });

        ui.separator();

        if ui.button("OK") {
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
