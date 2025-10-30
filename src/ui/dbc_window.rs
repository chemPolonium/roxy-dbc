// //! DBC 窗口渲染模块

use std::fs::File;
use std::io::Read;
use std::os::windows;
use std::string;

use crate::editable_dbc::{EditableDbc, EditableMessage, EditableSignal};
use crate::ui::message_edit_window::MessageEditWindowState;
use crate::ui::message_window::MessageWindowState;
use crate::ui::state::UiState;
use can_dbc::{ByteOrder, ValueType};
use imgui::{
    Condition, Selectable, SelectableFlags, StyleColor, TableBgTarget, TableColumnFlags,
    TableColumnSetup, TableFlags, TableSortDirection, Ui, WindowFocusedFlags,
};

/// DBC 窗口状态
#[derive(Clone, Default)]
pub struct DbcWindowState {
    is_open: bool,

    // 一个文件永远只对应一个 DBC 窗口
    // 所以文件路径可以作为 DBC 窗口的唯一标识
    pub file_path: String,

    // dbc 使用 EditableDbc 存储，后续会增加编辑功能
    pub dbc: EditableDbc,

    // 这里是 Message Table 相关的状态
    message_table: MessageTableState,
    pending_filter: bool,
    pending_sort: bool,
    is_dirty: bool,
    search_query: String,
    // 右键会产生 Message 的编辑请求
    right_clicked_index: Option<usize>,

    // 待关闭的 Message 窗口 ID
    // 因为不会一次关闭多个，所以用 Option<usize> 就够了
    message_windows_to_close: Option<usize>,
    message_edit_windows_to_close: Option<usize>,

    // Message 窗口必然依附于 DBC 窗口
    // 一旦关闭了 DBC 窗口，所有相关的 Message 窗口也会被关闭
    // 这个时候就不用管 message_windows_to_close 了
    // 直接干掉整个 Vec 就行
    pub message_windows: Vec<MessageWindowState>,
    pub message_edit_windows: Vec<MessageEditWindowState>,
}

impl DbcWindowState {
    const MAX_UNDO_ENTRIES: usize = 100;

    /// 创建新的 DBC 窗口状态
    pub fn new(file_path: String, dbc: EditableDbc) -> Self {
        Self {
            is_open: true,
            file_path,
            dbc,
            message_table: MessageTableState::new(),
            pending_filter: true,
            pending_sort: true,
            is_dirty: false,
            search_query: String::new(),
            right_clicked_index: None,
            message_windows_to_close: None,
            message_windows: Vec::new(),
            message_edit_windows_to_close: None,
            message_edit_windows: Vec::new(),
        }
    }

    pub fn from_file(file_path: &str) -> Self {
        let mut file = File::open(file_path).unwrap();
        let mut contents = Vec::new();
        if let Ok(_) = file.read_to_end(&mut contents) {
            if let Ok(original_dbc) = can_dbc::DBC::from_slice(&contents) {
                let editable_dbc = EditableDbc::from_dbc(&original_dbc);
                println!("{}", original_dbc.version().0);
                return Self::new(file_path.to_string(), editable_dbc);
            } else {
                println!("Failed to parse DBC file: {}", file_path);
                Self::new(file_path.to_string(), EditableDbc::default())
            }
        } else {
            println!("Failed to parse DBC file: {}", file_path);
            Self::new(file_path.to_string(), EditableDbc::default())
        }
    }

    // pub fn mark_dirty(&mut self) {
    //     self.is_dirty = true;
    // }

    // pub fn sorted_indicies(&mut self) -> &[usize] {
    //     if self.is_dirty {
    //         self.message_table.update_sort(self.dbc.messages());
    //         self.is_dirty = false;
    //     }
    //     &self.message_table.sorted_indicies()
    // }

    pub fn set_sort(&mut self, column: usize, ascending: bool) {
        self.message_table.set_sort(column, ascending);
        self.message_table.update_sort(self.dbc.messages());
        self.is_dirty = true;
    }

    // pub fn selected_original_indicies(&self) -> Option<usize> {
    //     let sorted_indicies = &self.message_table.sorted_indicies();
    //     self.message_table.selected_indicies().
    // }

    // pub fn select_display_index(&mut self, display_index: usize) {
    //     self.selected_index = Some(display_index);
    // }

    // pub fn clear_selection(&mut self) {
    //     self.selected_index = None;
    // }

    // pub fn selected_message(&self) -> Option<&EditableMessage> {
    //     let original_index = self.selected_original_index()?;
    //     self.dbc.messages().get(original_index)
    // }
}

/// 渲染所有 DBC 窗口
pub fn render_dbc_windows(ui: &Ui, ui_state: &mut UiState) {
    for window in &mut ui_state.dbc_windows {
        let window_title = format!(
            "DBC - {}",
            std::path::Path::new(&window.file_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_else(|| panic!("DBC window has empty file path"))
        );

        let window_ui = ui
            .window(&window_title)
            .size([800.0, 600.0], Condition::FirstUseEver);

        window_ui.build(|| {
            render_dbc_window_content(ui, window);
        });
    }
}

#[derive(Clone, Default)]
struct MessageTableState {
    sorted_indicies: Vec<usize>,
    filtered_indicies: Vec<usize>,
    sort_column: Option<usize>,
    sort_ascending: bool,
    selected_indicies: Vec<usize>,
}

impl MessageTableState {
    pub fn new() -> Self {
        Self {
            sorted_indicies: Vec::new(),
            filtered_indicies: Vec::new(),
            sort_column: None,
            sort_ascending: true,
            selected_indicies: Vec::new(),
        }
    }

    // 在调用此方法前应确保调用过 update_sort
    // 筛选的是 sorted_indicies
    pub fn update_filter(&mut self, query: &str, messages: &[EditableMessage]) {
        if query.is_empty() {
            self.filtered_indicies = self.sorted_indicies.clone();
        } else {
            let query_lower = query.to_lowercase();
            self.filtered_indicies = self
                .sorted_indicies
                .iter()
                .filter_map(|idx| {
                    messages[*idx]
                        .message_name()
                        .to_lowercase()
                        .contains(&query_lower)
                        .then_some(*idx)
                })
                .collect();
        }
    }

    // 调用这个并不会更新 filtered_indicies
    pub fn update_sort(&mut self, messages: &[EditableMessage]) {
        self.sorted_indicies = (0..messages.len()).collect();

        if let Some(column) = self.sort_column {
            match column {
                0 => self.sort_by_id(messages),
                _ => {}
            }
        }
    }

    pub fn set_sort(&mut self, column: usize, ascending: bool) {
        self.sort_column = Some(column);
        self.sort_ascending = ascending;
    }

    pub fn sorted_indicies(&self) -> &Vec<usize> {
        &self.sorted_indicies
    }

    pub fn filtered_indicies(&self) -> &Vec<usize> {
        &self.filtered_indicies
    }

    fn sort_by_id(&mut self, messages: &[EditableMessage]) {
        if self.sort_ascending {
            self.sorted_indicies
                .sort_by(|&a, &b| messages[a].message_id().cmp(&messages[b].message_id()));
        } else {
            self.sorted_indicies
                .sort_by(|&a, &b| messages[b].message_id().cmp(&messages[a].message_id()));
        }
    }
}

#[derive(Default)]
struct DbcWindowEvent {
    double_clicked_message_id: Option<u32>,
}

/// 渲染 DBC 窗口的内容
/// 这个 render 比较特殊，负责处理整体的逻辑
/// 所以使用 DbcWindowState 的可变引用
fn render_dbc_window_content(ui: &Ui, window_state: &mut DbcWindowState) -> DbcWindowEvent {
    // 文件信息区域
    render_dbc_file_info(ui, window_state);

    // 搜索栏
    render_dbc_search_bar(ui, window_state);

    // 处理排序请求
    if window_state.pending_sort {
        window_state
            .message_table
            .update_sort(window_state.dbc.messages());
        // 如果经历了一次排序，也要经历一次筛选
        window_state
            .message_table
            .update_filter(&window_state.search_query, window_state.dbc.messages());
        window_state.pending_sort = false;
    }

    // 处理筛选请求
    if window_state.pending_filter || window_state.pending_sort {
        window_state
            .message_table
            .update_filter(&window_state.search_query, window_state.dbc.messages());
        window_state.pending_filter = false;
    }

    // 消息列表表格（返回双击、编辑、复制、粘贴请求）
    let messages_table_response = render_messages_table(ui, window_state);
    handle_messages_table_event(messages_table_response, window_state);

    // 渲染右键菜单
    let message_menu_response = render_message_table_menu(ui, window_state);
    handle_message_table_menu_event(message_menu_response, window_state);

    DbcWindowEvent::default()
}

/// 渲染DBC文件信息区域
fn render_dbc_file_info(ui: &Ui, window_state: &DbcWindowState) {
    // 显示当前加载的文件
    if !window_state.file_path.is_empty() {
        ui.text(format!("Loaded: {}", window_state.file_path));

        let message_count = window_state.dbc.message_count();
        ui.text(format!("Messages: {}", message_count));
    } else {
        ui.text("No DBC file loaded");
    }

    ui.separator();
}

/// 渲染搜索栏
fn render_dbc_search_bar(ui: &Ui, window_state: &mut DbcWindowState) {
    ui.text("Search Messages & Signals:");
    // 直接写回 window_state.search_query
    if ui
        .input_text("##search", &mut window_state.search_query)
        .build()
    {
        // 触发搜索
        window_state.pending_filter = true;
    }
    ui.separator();
}

#[derive(Default)]
struct MessageTableEvent {
    sort_column: Option<u32>,
    selected_idx: Option<usize>,
    multiple_selection: bool,
    right_clicked_idx: Option<usize>,
}

/// 渲染消息列表表格
/// 接受的全是引用，只负责绘制，不会修改状态
/// 返回各种请求
fn render_messages_table(ui: &Ui, window_state: &DbcWindowState) -> MessageTableEvent {
    ui.child_window("messages_table")
        .size([0.0, 0.0])
        .build(|| {
            if let Some(_) = ui.begin_table_with_flags(
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

                return render_messages_rows(ui, window_state);
            } else {
                return MessageTableEvent::default();
            }
        })
        .unwrap()
    // 如果没创建出来大概率是寄了
    // 直接让程序崩
}

fn handle_messages_table_event(response: MessageTableEvent, window_state: &mut DbcWindowState) {
    if let Some(column) = response.sort_column {
        let ascending = true; // TODO: 从 TableSortDirection 获取
        window_state.set_sort(column as usize, ascending);
        window_state.message_table.selected_indicies.clear();
    } else if let Some(selected_idx) = response.selected_idx {
        if response.multiple_selection {
            // 多选模式
            if let Some(pos) = window_state
                .message_table
                .selected_indicies
                .iter()
                .position(|&x| x == selected_idx)
            {
                // 已经选中，取消选中
                window_state.message_table.selected_indicies.remove(pos);
            } else {
                // 未选中，添加选中
                window_state
                    .message_table
                    .selected_indicies
                    .push(selected_idx);
                println!(
                    "Now the selections are {:?}",
                    window_state.message_table.selected_indicies
                );
            }
        } else {
            // 单选模式
            window_state.message_table.selected_indicies.clear();
            window_state
                .message_table
                .selected_indicies
                .push(selected_idx);
        }
    } else if let Some(right_clicked_idx) = response.right_clicked_idx {
        // 处理右键点击事件
        // 如果右键点击到了未选中的项，则先选中它
        // 如果右键点击到了已选中的项，则保持不变
        // 在具体的右键菜单中处理选中了单个项还是多个项
        if !window_state
            .message_table
            .selected_indicies
            .contains(&right_clicked_idx)
        {
            window_state.message_table.selected_indicies.clear();
            window_state
                .message_table
                .selected_indicies
                .push(right_clicked_idx);
        }
        window_state.right_clicked_index = Some(right_clicked_idx);
    }
}

/// 设置消息表格的列
fn setup_messages_table_columns(ui: &Ui) {
    ui.table_setup_scroll_freeze(0, 1);
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

fn render_messages_rows(ui: &Ui, window_state: &DbcWindowState) -> MessageTableEvent {
    let mut selected_idx = None;
    let mut multiple_selection = false;
    let mut right_clicked_idx = None;
    for &idx in window_state.message_table.filtered_indicies() {
        let message = &window_state.dbc.messages()[idx];
        ui.table_next_row();
        if window_state.message_table.selected_indicies.contains(&idx) {
            ui.table_set_bg_color(
                TableBgTarget::ROW_BG0,
                ui.style_color(StyleColor::TextSelectedBg),
            );
        }
        ui.table_set_column_index(0);
        if ui
            .selectable_config(format!("0x{:03X}", message.message_id()))
            .span_all_columns(true)
            .build()
        {
            selected_idx = Some(idx);
            if ui.io().key_ctrl {
                println!(
                    "Toggled selection for message ID: 0x{:03X}",
                    message.message_id()
                );
                multiple_selection = true;
            } else {
                println!("Selected message ID: 0x{:03X}", message.message_id());
            }
        }
        if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
            println!("Right-clicked message ID: 0x{:03X}", message.message_id());
            right_clicked_idx = Some(idx);
        }
        if ui.is_item_hovered() {
            let _ = render_signals_table_tooltip(ui, message);
        }
        ui.table_set_column_index(1);
        ui.text(message.message_name());
        ui.table_set_column_index(2);
        ui.text(format!("{}", message.message_size()));
        ui.table_set_column_index(3);
        ui.text(format!("{}", message.signals_count()));
    }
    MessageTableEvent {
        sort_column: None,
        selected_idx,
        multiple_selection,
        right_clicked_idx,
    }
}

enum RightClickSelection {
    OnSelectedSingle,
    OnSelectedMultiple,
    OnUnselected,
}

enum MessageTablePopupAction {
    Edit,
    Copy,
    Paste,
    Delete,
}

struct MessageTablePopupEvent {
    menu_shown: bool,
    action: Option<MessageTablePopupAction>,
}

fn render_message_table_menu(ui: &Ui, window_state: &DbcWindowState) -> MessageTablePopupEvent {
    let mut response = MessageTablePopupEvent {
        action: None,
        menu_shown: false,
    };
    let popup_id = format!("message_context_menu");
    if let Some(_) = window_state.right_clicked_index {
        ui.open_popup(&popup_id);
        response.menu_shown = true;
    }
    // 如果选择了单个项，允许编辑
    // 如果选择了多个项，只允许复制/粘贴/删除
    ui.popup(&popup_id, || {
        ui.text(format!(
            "[{}]",
            window_state
                .message_table
                .selected_indicies
                .iter()
                .map(|i| { window_state.dbc.messages()[*i].message_name() })
                .collect::<Vec<&str>>()
                .join("]\n[")
        ));
        ui.separator();
    });
    if window_state.message_table.selected_indicies.len() == 1 {
        ui.popup(&popup_id, || {
            if ui.menu_item("Edit") {
                response.action = Some(MessageTablePopupAction::Edit);
            }
        });
    }
    ui.popup(&popup_id, || {
        if ui.menu_item("Copy") {
            response.action = Some(MessageTablePopupAction::Copy);
        }

        if ui.menu_item("Paste") {
            response.action = Some(MessageTablePopupAction::Paste);
        }

        if ui.menu_item("Delete") {
            response.action = Some(MessageTablePopupAction::Delete);
        }
    });
    response
}

fn handle_message_table_menu_event(
    response: MessageTablePopupEvent,
    window_state: &mut DbcWindowState,
) {
    match response.action {
        None => {}
        Some(MessageTablePopupAction::Edit) => {
            println!(
                "Handle edit for message : {:?}",
                window_state.message_table.selected_indicies
            );
        }
        Some(MessageTablePopupAction::Copy) => {
            println!(
                "Handle copy for message : {:?}",
                window_state.message_table.selected_indicies
            );
        }
        Some(MessageTablePopupAction::Paste) => {
            println!("Handle paste");
        }
        Some(MessageTablePopupAction::Delete) => {
            println!(
                "Handle delete for message : {:?}",
                window_state.message_table.selected_indicies
            );
        }
    }
    window_state.right_clicked_index = None;
}

/// 渲染带有信号表格的详细弹出窗口
fn render_signals_table_tooltip(ui: &Ui, message: &EditableMessage) -> Option<String> {
    let requested: Option<String> = None;
    let message_name = message.message_name();
    let message_id = message.message_id();
    let signals = message.signals();
    ui.tooltip(|| {
        ui.text(format!("Message: {} (0x{:03X})", message_name, message_id));
        ui.separator();

        if signals.is_empty() {
            ui.text("No signals in this message");
            ui.text("Right-click to Edit");
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
                ui.text(&signal.name());

                ui.table_set_column_index(1);
                ui.text(format!("{}", signal.start_bit()));

                ui.table_set_column_index(2);
                ui.text(format!("{}", signal.signal_size()));

                ui.table_set_column_index(3);
                let byte_order = match signal.byte_order() {
                    ByteOrder::LittleEndian => "LE",
                    ByteOrder::BigEndian => "BE",
                };
                ui.text(byte_order);

                ui.table_set_column_index(4);
                ui.text(format!("{:.2}", signal.factor()));

                ui.table_set_column_index(5);
                ui.text(&signal.unit());
            }

            if signals.len() > 10 {
                ui.table_next_row();
                ui.table_set_column_index(0);
                ui.text(format!("... and {} more signals", signals.len() - 10));
            }
        }

        ui.separator();
        ui.text("Double-click to open the window");
        ui.text("Right-click to Edit");
    });
    requested
}

// /// 清理已关闭的 DBC 窗口及其关联的 Signal 窗口
// fn cleanup_closed_dbc_windows(ui_state: &mut UiState, windows_to_remove: Vec<usize>) {
//     if windows_to_remove.is_empty() {
//         return;
//     }

//     // 收集被关闭窗口的 ID
//     let closed_parent_ids: std::collections::HashSet<_> = windows_to_remove
//         .iter()
//         .filter_map(|&idx| ui_state.dbc_windows.get(idx).map(|w| w.id))
//         .collect();

//     // 移除关联的 Message 窗口
//     if !closed_parent_ids.is_empty() {
//         ui_state
//             .message_windows
//             .retain(|sw| !closed_parent_ids.contains(&sw.parent_dbc_id));

//         // 修正焦点索引
//         if let Some(sig_idx) = ui_state.last_focused_message_window {
//             if sig_idx >= ui_state.message_windows.len() {
//                 ui_state.last_focused_message_window = None;
//             }
//         }
//     }

//     // 逆序移除 DBC 窗口
//     for &idx in windows_to_remove.iter().rev() {
//         ui_state.dbc_windows.remove(idx);
//     }

//     // 修正 DBC 焦点索引
//     if let Some(focused_idx) = ui_state.last_focused_dbc_index {
//         if focused_idx >= ui_state.dbc_windows.len() {
//             ui_state.last_focused_dbc_index = None;
//         }
//     }
// }

// /// 请求窗口聚焦（如果需要）
// fn request_window_focus_if_needed(focus_request: Option<usize>, window_id: usize) {
//     if focus_request == Some(window_id) {
//         unsafe {
//             imgui::sys::igSetNextWindowFocus();
//         }
//     }
// }
