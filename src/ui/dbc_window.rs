//! DBC 窗口渲染模块

use crate::ui::state::{DbcWindowState, UiState};
use imgui::{
    Condition, TableBgTarget, TableColumnFlags, TableColumnSetup, TableFlags, TableSortDirection,
    Ui, WindowFocusedFlags,
};

/// 渲染所有 DBC 窗口并处理相关逻辑
pub fn render_dbc_windows(ui: &Ui, ui_state: &mut UiState) {
    let mut windows_to_remove = Vec::new();
    let mut clicked_message_ids = Vec::new(); // 收集双击事件，稍后处理
    let mut edit_request_ids = Vec::new(); // 收集编辑请求，稍后处理
    let mut copy_request_ids = Vec::new(); // 收集复制请求，稍后处理
    let mut paste_requests = Vec::new(); // 收集粘贴请求，稍后处理
    let mut delete_request_ids = Vec::new(); // 收集删除请求，稍后处理
    let mut signal_edit_requests: Vec<(u32, String, usize)> = Vec::new(); // (message_id, signal_name, parent_dbc_id)

    for (index, window) in ui_state.dbc_windows.iter_mut().enumerate() {
        request_window_focus_if_needed(ui_state.dbc_window_focus_request, window.id);

        if !window.is_open {
            continue;
        }

        let (
            still_open,
            double_clicked_id,
            edit_requested_id,
            copy_requested_id,
            paste_requested,
            focused,
        ) = render_dbc_window(ui, window);

        if !still_open {
            windows_to_remove.push(index);
        }

        if focused {
            ui_state.last_focused_dbc_index = Some(index);
            ui_state.last_focused_signal_window = None;
        }

        if let Some(msg_id) = double_clicked_id {
            clicked_message_ids.push((msg_id, window.id));
        }

        if let Some(msg_id) = edit_requested_id {
            edit_request_ids.push((msg_id, window.id));
        }

        if let Some(msg_id) = copy_requested_id {
            copy_request_ids.push((msg_id, window.id));
        }

        // collect delete request if window set it during rendering
        if let Some(msg_id) = window.pending_delete_message.take() {
            delete_request_ids.push((msg_id, window.id));
        }

        if paste_requested {
            paste_requests.push(window.id);
        }

        // check if this window reported a pending signal edit (set during tooltip handling)
        if let Some((msg_id, sig_name)) = window.pending_signal_edit.take() {
            signal_edit_requests.push((msg_id, sig_name, window.id));
        }
    }

    // 处理所有双击事件
    for (message_id, parent_dbc_id) in clicked_message_ids {
        // call handler with ids; the handler will perform mutable lookup inside to avoid borrow conflicts
        handle_message_double_click(ui_state, message_id, parent_dbc_id);
    }

    // 处理所有信号编辑请求（来自 tooltip 双击信号名）
    for (message_id, signal_name, parent_dbc_id) in signal_edit_requests {
        // 找到对应的 DBC 窗口
        if let Some(dbc_window) = ui_state
            .dbc_windows
            .iter_mut()
            .find(|w| w.id == parent_dbc_id)
        {
            // 使用 MessageRef 来支持原始和新建的 Message
            if let Some(message_ref) = dbc_window.editable_data.get_message_ref_by_id(message_id) {
                // 打开并初始化 SignalEditDialog
                ui_state.signal_edit_dialog.open(parent_dbc_id, message_id);

                // Try to populate fields from overrides first
                if let Some(ov) = dbc_window
                    .editable_data
                    .signal_overrides
                    .get(&(message_id, signal_name.clone()))
                {
                    ui_state.signal_edit_dialog.name_buffer = ov.name.clone();
                    ui_state.signal_edit_dialog.original_name = ov.name.clone();
                    ui_state.signal_edit_dialog.start_bit_buffer = ov.start_bit.to_string();
                    ui_state.signal_edit_dialog.size_buffer = ov.signal_size.to_string();
                    ui_state.signal_edit_dialog.byte_order_is_little =
                        matches!(ov.byte_order, crate::dbc::ByteOrder::LittleEndian);
                    ui_state.signal_edit_dialog.signed =
                        matches!(ov.value_type, crate::dbc::ValueType::Signed);
                    ui_state.signal_edit_dialog.factor_buffer = ov.factor.to_string();
                    ui_state.signal_edit_dialog.offset_buffer = ov.offset.to_string();
                    ui_state.signal_edit_dialog.min_buffer = ov.minimum.to_string();
                    ui_state.signal_edit_dialog.max_buffer = ov.maximum.to_string();
                    ui_state.signal_edit_dialog.unit_buffer = ov.unit.clone();
                    ui_state.signal_edit_dialog.comment_buffer = ov.comment.clone();
                } else {
                    // Build a MessageView and find the SignalView by name (avoid using can_dbc::Signal in UI path)
                    let view = crate::ui::view::MessageView::from_message_ref(
                        &message_ref,
                        &dbc_window.editable_data,
                    );
                    if let Some(sig_view) = view.signals.iter().find(|s| s.name == signal_name) {
                        ui_state.signal_edit_dialog.name_buffer = sig_view.name.clone();
                        ui_state.signal_edit_dialog.original_name = sig_view.name.clone();
                        ui_state.signal_edit_dialog.start_bit_buffer =
                            sig_view.start_bit.to_string();
                        ui_state.signal_edit_dialog.size_buffer = sig_view.signal_size.to_string();
                        ui_state.signal_edit_dialog.byte_order_is_little =
                            matches!(sig_view.byte_order, crate::dbc::ByteOrder::LittleEndian);
                        ui_state.signal_edit_dialog.signed =
                            matches!(sig_view.value_type, crate::dbc::ValueType::Signed);
                        ui_state.signal_edit_dialog.factor_buffer = sig_view.factor.to_string();
                        ui_state.signal_edit_dialog.offset_buffer = sig_view.offset.to_string();
                        ui_state.signal_edit_dialog.min_buffer = sig_view.minimum.to_string();
                        ui_state.signal_edit_dialog.max_buffer = sig_view.maximum.to_string();
                        ui_state.signal_edit_dialog.unit_buffer = sig_view.unit.clone();
                        ui_state.signal_edit_dialog.comment_buffer = sig_view.comment.clone();
                    }
                }
            }
        }
    }

    // 处理所有编辑请求
    for (message_id, parent_dbc_id) in edit_request_ids {
        if let Some(dbc_window) = ui_state.dbc_windows.iter().find(|w| w.id == parent_dbc_id) {
            // 使用 MessageRef 来支持原始和新建的 Message
            if let Some(message_ref) = dbc_window.editable_data.get_message_ref_by_id(message_id) {
                ui_state.message_edit_dialog.open_with_ref(
                    parent_dbc_id,
                    &message_ref,
                    &dbc_window.editable_data,
                );
            }
        }
    }

    // 处理所有复制请求
    for (message_id, parent_dbc_id) in copy_request_ids {
        // 先查找窗口索引
        if let Some(dbc_index) = ui_state
            .dbc_windows
            .iter()
            .position(|w| w.id == parent_dbc_id)
        {
            // 然后获取 MessageRef
            if let Some(window) = ui_state.dbc_windows.get(dbc_index) {
                if let Some(message_ref) = window.editable_data.get_message_ref_by_id(message_id) {
                    // 转换为 CustomMessage 并保存到剪贴板
                    ui_state.clipboard.copied_message = Some(message_ref.to_custom_message());
                }
            }
        }
    }

    // 处理所有粘贴请求
    for parent_dbc_id in paste_requests {
        if let Some(dbc_index) = ui_state
            .dbc_windows
            .iter()
            .position(|w| w.id == parent_dbc_id)
        {
            handle_paste_message(ui_state, dbc_index);
        }
    }

    // If any delete requests were collected during rendering, open the confirmation dialog for the first one
    if let Some((message_id, parent_dbc_id)) = delete_request_ids.into_iter().next() {
        if let Some(idx) = ui_state
            .dbc_windows
            .iter()
            .position(|w| w.id == parent_dbc_id)
        {
            ui_state.confirm_delete_dialog.show = true;
            ui_state.confirm_delete_dialog.parent_dbc_id = parent_dbc_id;
            ui_state.confirm_delete_dialog.message_id = message_id;
            ui_state.confirm_delete_dialog.display_name = ui_state.dbc_windows[idx]
                .pending_confirm_delete_display_name
                .take()
                .unwrap_or_else(|| format!("Message 0x{:03X}", message_id));
        }
    }

    cleanup_closed_dbc_windows(ui_state, windows_to_remove);
}

/// 渲染单个 DBC 浏览器窗口
/// 返回：(is_open, double_clicked_message_id, edit_requested_message_id, copy_requested_message_id, paste_requested, focused)
fn render_dbc_window(
    ui: &Ui,
    window_state: &mut DbcWindowState,
) -> (bool, Option<u32>, Option<u32>, Option<u32>, bool, bool) {
    let window_title = format!(
        "DBC Browser {} - {}",
        window_state.id,
        if window_state.editable_data.base.file_path.is_empty() {
            "No file"
        } else {
            // 只显示文件名，不显示完整路径
            std::path::Path::new(&window_state.editable_data.base.file_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Unknown file")
        }
    );

    let mut is_open = window_state.is_open;
    let mut double_clicked_message = None;
    let mut edit_requested_message = None;
    let mut copy_requested_message = None;
    let mut paste_requested = false;
    let mut focused = false;

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
            let (double_click, edit_request, copy_request, paste_request) =
                render_dbc_window_content(ui, window_state);
            double_clicked_message = double_click;
            edit_requested_message = edit_request;
            copy_requested_message = copy_request;
            paste_requested = paste_request;
            if ui.is_window_focused_with_flags(WindowFocusedFlags::ROOT_AND_CHILD_WINDOWS) {
                focused = true;
            }
        });
    }

    if !is_open {
        window_state.is_open = false;
    }

    (
        window_state.is_open,
        double_clicked_message,
        edit_requested_message,
        copy_requested_message,
        paste_requested,
        focused,
    )
}

/// 渲染DBC窗口的内容
fn render_dbc_window_content(
    ui: &Ui,
    window_state: &mut DbcWindowState,
) -> (Option<u32>, Option<u32>, Option<u32>, bool) {
    // 文件信息区域
    render_dbc_file_info(ui, window_state);

    // 搜索栏
    render_dbc_search_bar(ui, window_state);

    // 消息列表表格（返回双击、编辑、复制、粘贴请求）
    render_messages_table(ui, window_state)
}

/// 渲染DBC文件信息区域
fn render_dbc_file_info(ui: &Ui, window_state: &DbcWindowState) {
    // 显示当前加载的文件
    if !window_state.editable_data.base.file_path.is_empty() {
        ui.text(format!(
            "Loaded: {}",
            window_state.editable_data.base.file_path
        ));

        let message_count = window_state
            .editable_data
            .base
            .dbc
            .as_ref()
            .map_or(0, |dbc| dbc.messages().len());
        ui.text(format!("Messages: {}", message_count));

        // 显示修改状态
        if window_state.editable_data.has_modifications() {
            ui.same_line();
            ui.text_colored(
                [1.0, 0.7, 0.0, 1.0],
                format!(
                    "(Modified: {})",
                    window_state.editable_data.modification_count()
                ),
            );
        }
    } else {
        ui.text("No DBC file loaded");
    }

    // 显示错误信息
    if !window_state.editable_data.base.error_message.is_empty() {
        ui.text_colored(
            [1.0, 0.0, 0.0, 1.0],
            &window_state.editable_data.base.error_message,
        );
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

/// Message and all display attributes packaged for UI
#[derive(Clone)]
struct MessageWithDisplayData {
    message_id: u32,
    message_name: String,
    display_name: String,
    display_id: u32,
    display_size: u64,
    signals_count: usize,
    view: crate::ui::view::MessageView,
}

/// 渲染消息列表表格
fn render_messages_table(
    ui: &Ui,
    window_state: &mut DbcWindowState,
) -> (Option<u32>, Option<u32>, Option<u32>, bool) {
    // 提前提取所有需要的数据，避免在闭包中借用冲突
    let search_query = window_state.search_query.clone();
    let filtered_messages = window_state.editable_data.search_messages(&search_query);

    // 提前收集消息列表和所有显示属性（打包在一起）
    let messages_with_data: Vec<MessageWithDisplayData> = filtered_messages
        .iter()
        .map(|m| {
            let original_id = m.message_id();
            let display_name = window_state
                .editable_data
                .get_message_name(original_id, m.message_name());
            let display_id = window_state.editable_data.get_message_id(original_id);
            let display_size = window_state
                .editable_data
                .get_message_size(original_id, m.message_size());
            let view =
                crate::ui::view::MessageView::from_message_ref(m, &window_state.editable_data);
            MessageWithDisplayData {
                message_id: original_id,
                message_name: m.message_name().to_string(),
                display_name,
                display_id,
                display_size,
                signals_count: view.signals.len(),
                view,
            }
        })
        .collect();

    // use window_state.selected_message_id directly

    ui.child_window("messages_list")
        .size([0.0, 0.0])
        .build(|| {
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

                // 排序（Message 和所有显示数据一起排序）
                let sorted_messages_with_data = sort_messages_with_data(ui, messages_with_data);

                return render_messages_rows_with_data(ui, window_state, sorted_messages_with_data);
            }
            (None, None, None, false)
        })
        .unwrap_or((None, None, None, false))
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

/// 渲染消息表格的行（使用打包的消息和所有显示数据）
fn render_messages_rows_with_data(
    ui: &Ui,
    window_state: &mut DbcWindowState,
    messages_with_data: Vec<MessageWithDisplayData>,
) -> (Option<u32>, Option<u32>, Option<u32>, bool) {
    let mut double_clicked_message_id = None;
    let mut edit_requested_message_id = None;
    let mut copy_requested_message_id = None;
    let mut paste_requested = false;

    for item in messages_with_data.iter() {
        let message_id = item.message_id;
        let message_name = &item.message_name;
        let display_name = &item.display_name;
        let display_id = item.display_id;
        let display_size = item.display_size;
        let signals_count = item.signals_count;

        ui.table_next_row();

        let selected = window_state.selected_message_id == Some(message_id);

        if selected {
            ui.table_set_bg_color(TableBgTarget::ROW_BG0, [0.3, 0.3, 0.7, 0.65]);
        }

        ui.table_set_column_index(0);
        if ui
            .selectable_config(format!("0x{:03X}", display_id))
            .selected(selected)
            .span_all_columns(true)
            .build()
        {
            window_state.selected_message_id = Some(message_id);
        }

        // 检测鼠标悬停在这一行上
        if ui.is_item_hovered() {
            // render signal tooltip; if a signal name was double-clicked in tooltip, request signal edit
            if let Some(sig_name) =
                render_signal_popup_with_table(ui, message_name, message_id, &item.view.signals)
            {
                window_state.pending_signal_edit = Some((message_id, sig_name));
            }

            // 检测双击事件
            if ui.is_mouse_double_clicked(imgui::MouseButton::Left) {
                double_clicked_message_id = Some(message_id);
            }
        }

        // 右键菜单
        let popup_id = format!("message_context_menu_{}", message_id);
        if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
            // 右键点击时也要选中该行
            window_state.selected_message_id = Some(message_id);
            ui.open_popup(&popup_id);
        }

        ui.popup(&popup_id, || {
            ui.text(format!("Message: {} (0x{:03X})", message_name, message_id));
            ui.separator();

            if ui.menu_item("Edit...") {
                edit_requested_message_id = Some(message_id);
            }

            if ui.menu_item("Copy") {
                copy_requested_message_id = Some(message_id);
            }

            if ui.menu_item("Paste") {
                paste_requested = true;
            }

            if ui.menu_item("Delete") {
                // mark pending delete on the window and store a display name for confirmation
                window_state.pending_delete_message = Some(message_id);
                window_state.pending_confirm_delete_display_name = Some(
                    window_state
                        .editable_data
                        .get_message_name(message_id, message_name.as_str()),
                );
            }

            // 未来可以添加更多选项
            // if ui.menu_item("Delete") { ... }
            // if ui.menu_item("Duplicate") { ... }
        });

        ui.table_set_column_index(1);
        ui.text(display_name);

        ui.table_set_column_index(2);
        ui.text(format!("{}", display_size));

        ui.table_set_column_index(3);
        ui.text(format!("{}", signals_count));
    }

    // 检测键盘快捷键（Ctrl+C 复制，Ctrl+V 粘贴）
    if ui.is_window_focused() {
        let io = ui.io();

        // Ctrl+C: 复制选中的消息
        if io.key_ctrl && ui.is_key_pressed(imgui::Key::C) {
            if let Some(msg_id) = window_state.selected_message_id {
                copy_requested_message_id = Some(msg_id);
            }
        }

        // Ctrl+V: 粘贴
        if io.key_ctrl && ui.is_key_pressed(imgui::Key::V) {
            paste_requested = true;
        }
    }

    (
        double_clicked_message_id,
        edit_requested_message_id,
        copy_requested_message_id,
        paste_requested,
    )
}

/// 渲染带有信号表格的详细弹出窗口
fn render_signal_popup_with_table(
    ui: &Ui,
    message_name: &str,
    message_id: u32,
    signals: &[crate::ui::view::SignalView],
) -> Option<String> {
    let requested: Option<String> = None;
    ui.tooltip(|| {
        ui.text(format!("Message: {} (0x{:03X})", message_name, message_id));
        ui.separator();

        if signals.is_empty() {
            ui.text("No signals in this message");
            ui.text("Double-click to edit message");
            return;
        }

        ui.text(format!("Signals: {}", signals.len()));
        ui.separator();

        // 创建一个非交互的信号表格（tooltip 本身不支持可靠交互，因此不捕获双击）
        if let Some(_table) = ui.begin_table_with_flags(
            "popup_signals_table",
            6, // 显示更多列：Name, Start Bit, Length, Byte Order, Factor, Unit
            TableFlags::BORDERS | TableFlags::SIZING_FIXED_FIT | TableFlags::ROW_BG,
        ) {
            // 设置表格列
            ui.table_setup_column("Signal");
            ui.table_setup_column("Start");
            ui.table_setup_column("Length");
            ui.table_setup_column("Order");
            ui.table_setup_column("Factor");
            ui.table_setup_column("Unit");
            ui.table_headers_row();

            // 显示前 10 个信号（避免 tooltip 过大）
            for signal in signals.iter().take(10) {
                ui.table_next_row();

                ui.table_set_column_index(0);
                // tooltip 内仅显示文本，避免交互
                ui.text(&signal.name);

                ui.table_set_column_index(1);
                ui.text(format!("{}", signal.start_bit));

                ui.table_set_column_index(2);
                ui.text(format!("{}", signal.signal_size));

                ui.table_set_column_index(3);
                let byte_order = match signal.byte_order {
                    crate::dbc::ByteOrder::LittleEndian => "LE",
                    crate::dbc::ByteOrder::BigEndian => "BE",
                };
                ui.text(byte_order);

                ui.table_set_column_index(4);
                ui.text(format!("{:.2}", signal.factor));

                ui.table_set_column_index(5);
                ui.text(&signal.unit);
            }

            if signals.len() > 10 {
                ui.table_next_row();
                ui.table_set_column_index(0);
                ui.text(format!("... and {} more signals", signals.len() - 10));
            }
        }

        ui.separator();
        ui.text_colored([0.7, 0.7, 0.7, 1.0], "Open the full Signal window (double-click a message) or right-click in the Signal window to Edit...");
    });
    requested
}

// Note: previous helper `render_signal_popup` removed — tooltip now handled by
// `render_signal_popup_with_table` which returns an edit request when a signal
// name is double-clicked.

/// 排序消息列表（使用打包的消息和所有显示数据）
fn sort_messages_with_data(
    ui: &Ui,
    mut messages_with_data: Vec<MessageWithDisplayData>,
) -> Vec<MessageWithDisplayData> {
    if let Some(sort_specs) = ui.table_sort_specs_mut() {
        let specs = sort_specs.specs();
        for (i, spec) in specs.iter().enumerate() {
            if i == 0 {
                let ascending = spec.sort_direction() == Some(TableSortDirection::Ascending);
                messages_with_data.sort_by(|a, b| {
                    let ordering = match spec.column_idx() {
                        0 => a.display_id.cmp(&b.display_id),
                        1 => a.display_name.cmp(&b.display_name),
                        2 => a.display_size.cmp(&b.display_size),
                        3 => a.signals_count.cmp(&b.signals_count),
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
    messages_with_data
}

/// 处理消息双击事件，打开或聚焦对应的 Signal 窗口
fn handle_message_double_click(ui_state: &mut UiState, message_id: u32, parent_dbc_id: usize) {
    // first check if a signal window for this message is already open
    if let Some(existing_idx) = ui_state
        .signal_windows
        .iter()
        .position(|w| w.message.message_id == message_id && w.parent_dbc_id == parent_dbc_id)
    {
        let existing_id = ui_state.signal_windows[existing_idx].id;
        ui_state.signal_window_focus_request = Some(existing_id);
        return;
    }

    // otherwise create a new Signal window by finding the corresponding DBC window mutably
    if let Some(dbc_window) = ui_state
        .dbc_windows
        .iter_mut()
        .find(|w| w.id == parent_dbc_id)
    {
        if let Some(message_ref) = dbc_window.editable_data.get_message_ref_by_id(message_id) {
            let view = crate::ui::view::MessageView::from_message_ref(
                &message_ref,
                &dbc_window.editable_data,
            );
            let new_id = ui_state.signal_windows.len();
            ui_state
                .signal_windows
                .push(crate::ui::state::SignalWindowState {
                    id: new_id,
                    message: view,
                    is_open: true,
                    parent_dbc_id,
                    pending_signal_edit: None,
                    selected_signal_name: None,
                });
            ui_state.signal_window_focus_request = Some(new_id);
        }
    }
}

/// 处理粘贴消息事件
fn handle_paste_message(ui_state: &mut UiState, dbc_window_index: usize) {
    // 检查剪贴板是否有内容
    if !ui_state.has_clipboard_message() {
        return;
    }

    // 获取剪贴板中的消息
    let copied_message = match &ui_state.clipboard.copied_message {
        Some(msg) => msg.clone(),
        None => return,
    };

    // 生成新的 Message ID
    let new_id = ui_state.generate_next_message_id(dbc_window_index);

    // 创建新消息（基于复制的消息）
    let mut new_message = copied_message.duplicate(new_id);

    // 如果名称已存在，添加后缀
    if let Some(window) = ui_state.dbc_windows.get_mut(dbc_window_index) {
        let all_messages = window.editable_data.get_all_messages();
        let existing_names: std::collections::HashSet<String> = all_messages
            .iter()
            .map(|m| m.message_name().to_string())
            .collect();

        let mut counter = 1;
        let base_name = new_message.message_name.clone();
        while existing_names.contains(&new_message.message_name) {
            new_message.message_name = format!("{}_{}", base_name, counter);
            counter += 1;
        }

        // 添加到 DBC 窗口
        window.editable_data.add_message(new_message);
    }
}

/// 清理已关闭的 DBC 窗口及其关联的 Signal 窗口
fn cleanup_closed_dbc_windows(ui_state: &mut UiState, windows_to_remove: Vec<usize>) {
    if windows_to_remove.is_empty() {
        return;
    }

    // 收集被关闭窗口的 ID
    let closed_parent_ids: std::collections::HashSet<_> = windows_to_remove
        .iter()
        .filter_map(|&idx| ui_state.dbc_windows.get(idx).map(|w| w.id))
        .collect();

    // 移除关联的 Signal 窗口
    if !closed_parent_ids.is_empty() {
        ui_state
            .signal_windows
            .retain(|sw| !closed_parent_ids.contains(&sw.parent_dbc_id));

        // 修正焦点索引
        if let Some(sig_idx) = ui_state.last_focused_signal_window {
            if sig_idx >= ui_state.signal_windows.len() {
                ui_state.last_focused_signal_window = None;
            }
        }
    }

    // 逆序移除 DBC 窗口
    for &idx in windows_to_remove.iter().rev() {
        ui_state.dbc_windows.remove(idx);
    }

    // 修正 DBC 焦点索引
    if let Some(focused_idx) = ui_state.last_focused_dbc_index {
        if focused_idx >= ui_state.dbc_windows.len() {
            ui_state.last_focused_dbc_index = None;
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
