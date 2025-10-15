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

/// Signal详细窗口状态
#[derive(Clone)]
pub struct SignalWindowState {
    pub id: usize,
    pub message: crate::dbc::Message,
    pub is_open: bool,
}

/// UI 状态管理
pub struct UiState {
    pub show_performance_window: bool,
    pub show_about_dialog: bool,
    pub dbc_windows: Vec<DbcWindowState>,
    pub signal_windows: Vec<SignalWindowState>,
    pub next_dbc_id: usize,
    pub error_dialog: ErrorDialog,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            show_performance_window: false,
            show_about_dialog: false,
            dbc_windows: Vec::new(),
            signal_windows: Vec::new(),
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
            let (still_open, clicked_message) = render_dbc_window(ui, window);
            if !still_open {
                windows_to_remove.push(index);
            }

            // 如果检测到双击消息，创建新的信号窗口
            if let Some(message) = clicked_message {
                let signal_window_id = ui_state.signal_windows.len();
                let signal_window = SignalWindowState {
                    id: signal_window_id,
                    message,
                    is_open: true,
                };
                ui_state.signal_windows.push(signal_window);
            }
        }
    }
    // Remove closed windows in reverse order to maintain indices
    for &index in windows_to_remove.iter().rev() {
        ui_state.dbc_windows.remove(index);
    }

    // Render all Signal windows
    let mut signal_windows_to_remove = Vec::new();
    for (index, window) in ui_state.signal_windows.iter_mut().enumerate() {
        if window.is_open {
            if !render_signal_window(ui, window) {
                signal_windows_to_remove.push(index);
            }
        }
    }
    // Remove closed signal windows in reverse order to maintain indices
    for &index in signal_windows_to_remove.iter().rev() {
        ui_state.signal_windows.remove(index);
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
fn render_dbc_window(
    ui: &Ui,
    window_state: &mut DbcWindowState,
) -> (bool, Option<crate::dbc::Message>) {
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
    let mut clicked_message = None;

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
            clicked_message = render_dbc_window_content(ui, window_state);
        });
    }

    if !is_open {
        window_state.is_open = false;
    }

    (window_state.is_open, clicked_message)
}

/// 渲染单个 Signal 详细窗口
fn render_signal_window(ui: &Ui, window_state: &mut SignalWindowState) -> bool {
    let window_title = format!(
        "Signals - {} (0x{:03X})",
        window_state.message.message_name(),
        window_state.message.message_id().raw()
    );

    let mut is_open = window_state.is_open;

    if is_open {
        let window = ui
            .window(&window_title)
            .opened(&mut is_open)
            .size([900.0, 500.0], Condition::FirstUseEver)
            .position(
                [
                    100.0 + (window_state.id as f32 * 30.0),
                    100.0 + (window_state.id as f32 * 30.0),
                ],
                Condition::FirstUseEver,
            );

        window.build(|| {
            render_signal_window_content(ui, &window_state.message);
        });
    }

    if !is_open {
        window_state.is_open = false;
    }

    window_state.is_open
}

/// 渲染Signal窗口的内容
fn render_signal_window_content(ui: &Ui, message: &crate::dbc::Message) {
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
            ("Type", TableColumnFlags::default(), 60.0),
            ("Order", TableColumnFlags::default(), 60.0),
            ("Start", TableColumnFlags::default(), 50.0),
            ("Length", TableColumnFlags::default(), 60.0),
            ("Factor", TableColumnFlags::default(), 80.0),
            ("Offset", TableColumnFlags::default(), 80.0),
            ("Min", TableColumnFlags::default(), 80.0),
            ("Max", TableColumnFlags::default(), 80.0),
            ("Unit", TableColumnFlags::default(), 60.0),
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

/// 渲染DBC窗口的内容
fn render_dbc_window_content(
    ui: &Ui,
    window_state: &mut DbcWindowState,
) -> Option<crate::dbc::Message> {
    // 文件信息区域
    render_dbc_file_info(ui, window_state);

    // 搜索栏
    render_dbc_search_bar(ui, window_state);

    // 消息列表表格
    render_messages_table(ui, window_state)
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
fn render_messages_table(
    ui: &Ui,
    window_state: &mut DbcWindowState,
) -> Option<crate::dbc::Message> {
    let mut clicked_message = None;

    ui.child_window("messages_list").size([0.0, 0.0]).build(|| {
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

            clicked_message =
                render_messages_rows(ui, &mut window_state.selected_message_id, sorted_messages);
        }
    });

    clicked_message
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

/// 渲染消息表格的行，返回双击的消息（如果有的话）
fn render_messages_rows(
    ui: &Ui,
    current_selection: &mut Option<u32>,
    messages: Vec<&crate::dbc::Message>,
) -> Option<crate::dbc::Message> {
    let mut double_clicked_message = None;

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

        // 检测鼠标悬停在这一行上
        if ui.is_item_hovered() {
            render_signal_popup(ui, message);

            // 检测双击事件
            if ui.is_mouse_double_clicked(imgui::MouseButton::Left) {
                double_clicked_message = Some(message.clone());
            }
        }

        ui.table_set_column_index(1);
        ui.text(message.message_name());

        ui.table_set_column_index(2);
        ui.text(format!("{}", message.message_size()));

        ui.table_set_column_index(3);
        ui.text(format!("{}", message.signals().len()));
    }

    double_clicked_message
}

/// 渲染信号详情弹出窗口
fn render_signal_popup(ui: &Ui, message: &crate::dbc::Message) {
    ui.tooltip(|| {
        ui.text(format!(
            "Message: {} (0x{:03X})",
            message.message_name(),
            message.message_id().raw()
        ));
        ui.separator();

        if message.signals().is_empty() {
            ui.text("No signals in this message");
            return;
        }

        // 创建一个简化的信号表格
        if let Some(_table) = ui.begin_table_with_flags(
            "popup_signals_table",
            5, // 减少列数，只显示关键信息
            TableFlags::BORDERS | TableFlags::SIZING_FIXED_FIT,
        ) {
            // 设置表格列
            ui.table_setup_column("Signal");
            ui.table_setup_column("Start");
            ui.table_setup_column("Length");
            ui.table_setup_column("Factor");
            ui.table_setup_column("Unit");
            ui.table_headers_row();

            // 显示前几个信号（避免popup过大）
            for signal in message.signals().iter().take(10) {
                ui.table_next_row();

                ui.table_set_column_index(0);
                ui.text(signal.name());

                ui.table_set_column_index(1);
                ui.text(format!("{}", signal.start_bit()));

                ui.table_set_column_index(2);
                ui.text(format!("{}", signal.signal_size()));

                ui.table_set_column_index(3);
                ui.text(format!("{}", signal.factor()));

                ui.table_set_column_index(4);
                ui.text(signal.unit());
            }

            if message.signals().len() > 10 {
                ui.table_next_row();
                ui.table_set_column_index(0);
                ui.text(format!(
                    "... and {} more signals",
                    message.signals().len() - 10
                ));
            }
        }
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
        ui.text("Version: 0.4.0");
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
