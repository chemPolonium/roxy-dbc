// //! DBC 窗口渲染模块

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::usize;

use crate::editable_dbc::{EditableDbc, EditableMessage};
use crate::ui::message_window::MessageWindow;
use crate::ui::state::UiState;
use can_dbc::ByteOrder;
use imgui::{
    Condition, StyleColor, TableBgTarget, TableColumnFlags, TableColumnSetup, TableFlags,
    TableSortDirection, Ui,
};

/// DBC 窗口状态
#[derive(Clone, Default)]
pub struct DbcWindow {
    pub is_open: bool,

    // 一个文件永远只对应一个 DBC 窗口
    // 所以文件路径可以作为 DBC 窗口的唯一标识
    pub file_path: String,

    // dbc 使用 EditableDbc 存储，后续会增加编辑功能
    pub dbc: EditableDbc,

    search_bar: DbcSearchBar,
    message_table: MessageTable,
    is_dirty: bool,

    // 待关闭的 Message 窗口 ID
    // 因为不会一次关闭多个，所以用 Option<usize> 就够了
    message_window_to_close: Option<usize>,

    // Message 窗口必然依附于 DBC 窗口
    // 一旦关闭了 DBC 窗口，所有相关的 Message 窗口也会被关闭
    // 这个时候就不用管 message_windows_to_close 了
    // 直接干掉整个 Vec 就行
    pub message_windows: Vec<MessageWindow>,
}

impl DbcWindow {
    /// 创建新的 DBC 窗口状态
    pub fn new(file_path: &str, dbc: EditableDbc) -> Self {
        Self {
            is_open: true,
            file_path: file_path.to_string(),
            dbc,
            message_table: MessageTable::new(),
            search_bar: DbcSearchBar::default(),
            is_dirty: true,
            message_window_to_close: None,
            message_windows: Vec::new(),
        }
    }

    /// 从文件路径创建新的 DBC 窗口状态
    pub fn from_path(file_path: &Path) -> Result<Self, String> {
        let mut file = File::open(file_path).unwrap();
        let mut contents = Vec::new();
        if let Ok(_) = file.read_to_end(&mut contents) {
            let contents_str = String::from_utf8_lossy(&contents).to_string();
            if let Ok(original_dbc) = can_dbc::Dbc::try_from(contents_str.as_str()) {
                let editable_dbc = EditableDbc::from_dbc(&original_dbc);
                Ok(Self::new(file_path.to_str().unwrap(), editable_dbc))
            } else {
                Err(format!("Failed to parse DBC: {}", file_path.display()))
            }
        } else {
            Err(format!("Filed to open file: {}", file_path.display()))
        }
    }

    /// 渲染DBC文件信息区域
    fn render_file_info(&self, ui: &Ui) {
        if !self.file_path.is_empty() {
            ui.text(format!("Loaded: {}", self.file_path));
            ui.text(format!("Messages: {}", self.dbc.message_count()));
        } else {
            ui.text("No DBC file loaded");
        }
    }

    pub fn render(&mut self, ui: &Ui) {
        if self.is_open {
            self.render_file_info(ui);
        }

        ui.separator();

        let pending_filter = self.search_bar.render(ui);

        if pending_filter {
            self.message_table
                .update_filter(&self.search_bar.query(), self.dbc.messages());
        }

        let message_table_event = self.message_table.render(ui, self.dbc.messages());

        // 处理双击事件，打开消息窗口
        if let Some(idx) = message_table_event.double_clicked_idx {
            let message = self.dbc.messages()[idx].clone();
            let message_window = MessageWindow::new(message, idx);
            self.message_windows.push(message_window);
        }

        let message_table_menu_event =
            render_message_table_menu(ui, &self, &message_table_event.right_clicked_idx);

        handle_message_table_menu_event(message_table_menu_event, &self);

        for message_window in &mut self.message_windows {
            println!(
                "Rendering message window {}",
                message_window.message.message_name()
            );
            message_window.render(ui);
        }

        if let Some(idx) = self.message_window_to_close {
            self.message_windows.remove(idx);
            self.message_window_to_close = None;
        }
    }
}

/// 此处使用的索引全都是 DBC 数据结构中消息向量的索引
#[derive(Clone, Default)]
struct MessageTable {
    sorted_indicies: Vec<usize>,
    filtered_indicies: Vec<usize>,
    selected_indicies: Vec<usize>,
}

impl MessageTable {
    pub fn new() -> Self {
        Self {
            sorted_indicies: Vec::new(),
            filtered_indicies: Vec::new(),
            selected_indicies: Vec::new(),
        }
    }

    // 在调用此方法前应确保调用过 update_sort
    // 筛选的是 sorted_indicies
    pub fn update_filter(&mut self, query: &str, messages: &[EditableMessage]) {
        if query.is_empty() {
            // 如果查询为空，直接将筛选结果设置为排序结果
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

    // 排序 sorted_indicies 和 filtered_indicies
    // 排序两个列表的逻辑在 sort_by 中实现
    pub fn update_sort(
        &mut self,
        idx: usize,
        sort_direction: TableSortDirection,
        messages: &[EditableMessage],
    ) {
        self.sorted_indicies = (0..messages.len()).collect();

        match idx {
            0 => self.sort_by_id(sort_direction, messages),
            1 => self.sort_by_name(sort_direction, messages),
            2 => self.sort_by_length(sort_direction, messages),
            _ => {}
        }
    }

    pub fn init_sort_and_filter(&mut self, query: &str, messages: &[EditableMessage]) {
        // 初始化排序和筛选
        // 以后还要改，现在改个消息排序直接没了
        self.update_sort(0, TableSortDirection::Ascending, messages);
        self.update_filter(query, messages);
        self.selected_indicies.clear();
    }

    pub fn filtered_indicies(&self) -> &Vec<usize> {
        &self.filtered_indicies
    }

    fn sort_by_id(&mut self, sort_direction: TableSortDirection, messages: &[EditableMessage]) {
        match sort_direction {
            TableSortDirection::Ascending => {
                self.sorted_indicies
                    .sort_by(|&a, &b| messages[a].message_id().cmp(&messages[b].message_id()));
                self.filtered_indicies
                    .sort_by(|&a, &b| messages[a].message_id().cmp(&messages[b].message_id()));
            }
            TableSortDirection::Descending => {
                self.sorted_indicies
                    .sort_by(|&a, &b| messages[b].message_id().cmp(&messages[a].message_id()));
                self.filtered_indicies
                    .sort_by(|&a, &b| messages[b].message_id().cmp(&messages[a].message_id()));
            }
        }
    }

    fn sort_by_name(&mut self, sort_direction: TableSortDirection, messages: &[EditableMessage]) {
        match sort_direction {
            TableSortDirection::Ascending => {
                self.sorted_indicies
                    .sort_by(|&a, &b| messages[a].message_name().cmp(&messages[b].message_name()));
                self.filtered_indicies
                    .sort_by(|&a, &b| messages[a].message_name().cmp(&messages[b].message_name()));
            }
            TableSortDirection::Descending => {
                self.sorted_indicies
                    .sort_by(|&a, &b| messages[b].message_name().cmp(&messages[a].message_name()));
                self.filtered_indicies
                    .sort_by(|&a, &b| messages[b].message_name().cmp(&messages[a].message_name()));
            }
        }
    }

    fn sort_by_length(&mut self, sort_direction: TableSortDirection, messages: &[EditableMessage]) {
        match sort_direction {
            TableSortDirection::Ascending => {
                self.sorted_indicies
                    .sort_by(|&a, &b| messages[a].message_size().cmp(&messages[b].message_size()));
                self.filtered_indicies
                    .sort_by(|&a, &b| messages[a].message_size().cmp(&messages[b].message_size()));
            }
            TableSortDirection::Descending => {
                self.sorted_indicies
                    .sort_by(|&a, &b| messages[b].message_size().cmp(&messages[a].message_size()));
                self.filtered_indicies
                    .sort_by(|&a, &b| messages[b].message_size().cmp(&messages[a].message_size()));
            }
        }
    }

    // 仅选择一个，对应左键
    fn select_index(&mut self, index: usize) {
        // 始终清空
        self.selected_indicies.clear();
        self.selected_indicies.push(index);
    }

    // 对应 Ctrl + 左键
    fn ctrl_select_index(&mut self, index: usize) {
        if self.selected_indicies.contains(&index) {
            // 如果点击到了已经选择的，那么就取消选择
            self.selected_indicies.retain(|&x| x != index);
        } else {
            // 如果点击到了没有选择的，那么就选择
            self.selected_indicies.push(index);
        }
    }

    // 对应右键
    fn right_select_index(&mut self, index: usize) {
        // 如果点击到了没有选择的，那么就清空重新选
        if !self.selected_indicies.contains(&index) {
            self.selected_indicies.clear();
            self.selected_indicies.push(index);
        }
        // 点击到了已经选择的，不用改
    }

    // 对应双击
    fn double_click_index(&mut self, index: usize) {
        // 返回打开信号表的事件
        println!("double clicked on index {}", index);
    }

    fn render_table_rows(
        &self,
        ui: &Ui,
        messages: &[EditableMessage],
    ) -> Option<MessageTableRowsEvent> {
        let mut table_rows_event = None;
        for &idx in self.filtered_indicies() {
            let message = &messages[idx];
            ui.table_next_row();
            if self.selected_indicies.contains(&idx) {
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
                // 点击后 build 返回 true
                if ui.io().key_ctrl {
                    // Ctrl + 左键点击
                    println!("Ctrl selected 0x{:03X}", message.message_id());
                    table_rows_event = Some(MessageTableRowsEvent::CtrlLeftClick(idx));
                } else {
                    // 左键点击
                    println!("Selected 0x{:03X}", message.message_id());
                    table_rows_event = Some(MessageTableRowsEvent::LeftClick(idx));
                }
            }
            if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
                // 判断右键点击
                println!("Right selected 0x{:03X}", message.message_id());
                table_rows_event = Some(MessageTableRowsEvent::RightClick(idx));
            }
            if ui.is_item_hovered() && ui.is_mouse_double_clicked(imgui::MouseButton::Left) {
                // 判断双击
                println!("Double clicked 0x{:03X}", message.message_id());
                table_rows_event = Some(MessageTableRowsEvent::DoubleClick(idx));
            }
            // 悬停时渲染信号表
            if ui.is_item_hovered() {
                render_signals_table_tooltip(ui, message);
            }
            ui.table_set_column_index(1);
            ui.text(message.message_name());
            ui.table_set_column_index(2);
            ui.text(format!("{}", message.message_size()));
            ui.table_set_column_index(3);
            ui.text(format!("{}", message.signals_count()));
        }

        table_rows_event
    }

    fn render(&mut self, ui: &Ui, messages: &[EditableMessage]) -> MessageTableEvent {
        // 提前创建可变返回事件
        let mut table_event = MessageTableEvent::default();
        let _ = ui
            .child_window("messages_table")
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

                    // 如果排序列有变化，更新排序
                    if let Some(mut sort_specs) = ui.table_sort_specs_mut() {
                        let should_sort = sort_specs.should_sort();
                        // 获取排序规范中的第一列作为排序列
                        if should_sort {
                            let sort_spec = sort_specs.specs().iter().next().unwrap();
                            let column_idx = sort_spec.column_idx();
                            let sort_direction = sort_spec.sort_direction().unwrap();
                            self.update_sort(column_idx, sort_direction, messages);
                        }
                        sort_specs.set_sorted();
                    }

                    let rows_event = self.render_table_rows(ui, messages);
                    // 处理点击
                    if let Some(rows_event) = rows_event {
                        match rows_event {
                            MessageTableRowsEvent::LeftClick(idx) => {
                                self.select_index(idx);
                            }
                            MessageTableRowsEvent::CtrlLeftClick(idx) => {
                                self.ctrl_select_index(idx);
                            }
                            MessageTableRowsEvent::RightClick(idx) => {
                                // 更新右键点击后的选中
                                self.right_select_index(idx);
                                // 右键菜单需要向上传播以打开菜单
                                table_event.right_clicked_idx = Some(idx);
                            }
                            MessageTableRowsEvent::DoubleClick(idx) => {
                                // 更新双击后的选中
                                self.double_click_index(idx);
                                // 双击菜单需要向上传播以打开消息窗口
                                table_event.double_clicked_idx = Some(idx);
                            }
                        }
                    }
                } else {
                    return;
                }
            })
            .unwrap();
        // 如果 child_window 没有创建成功，说明程序出问题了，让它崩

        table_event
    }
}

enum MessageTableRowsEvent {
    LeftClick(usize),
    CtrlLeftClick(usize),
    RightClick(usize),
    DoubleClick(usize),
}

#[derive(Clone, Default)]
struct DbcSearchBar {
    query: String,
}

impl DbcSearchBar {
    pub fn render(&mut self, ui: &Ui) -> bool {
        ui.text("Search Messages:");
        let mut pending = false;
        if ui.input_text("##search", &mut self.query).build() {
            pending = true;
        }
        ui.separator();
        pending
    }

    pub fn query(&self) -> &str {
        &self.query
    }
}

#[derive(Default)]
struct MessageTableEvent {
    right_clicked_idx: Option<usize>,
    double_clicked_idx: Option<usize>,
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

enum MessageTableMenuAction {
    Edit,
    Copy,
    Paste,
    Delete,
}

struct MessageTableMenuEvent {
    action: Option<MessageTableMenuAction>,
}

fn render_message_table_menu(
    ui: &Ui,
    window_state: &DbcWindow,
    right_clicked_idx: &Option<usize>,
) -> MessageTableMenuEvent {
    let mut response: MessageTableMenuEvent = MessageTableMenuEvent { action: None };
    let popup_id = format!("message_context_menu");
    if let Some(_) = right_clicked_idx {
        ui.open_popup(&popup_id);
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
                response.action = Some(MessageTableMenuAction::Edit);
            }
        });
    }
    ui.popup(&popup_id, || {
        if ui.menu_item("Copy") {
            response.action = Some(MessageTableMenuAction::Copy);
        }

        if ui.menu_item("Paste") {
            response.action = Some(MessageTableMenuAction::Paste);
        }

        if ui.menu_item("Delete") {
            response.action = Some(MessageTableMenuAction::Delete);
        }
    });
    response
}

fn handle_message_table_menu_event(response: MessageTableMenuEvent, window_state: &DbcWindow) {
    match response.action {
        None => {}
        Some(MessageTableMenuAction::Edit) => {
            println!(
                "Handle edit for message : {:?}",
                window_state.message_table.selected_indicies
            );
        }
        Some(MessageTableMenuAction::Copy) => {
            println!(
                "Handle copy for message : {:?}",
                window_state.message_table.selected_indicies
            );
        }
        Some(MessageTableMenuAction::Paste) => {
            println!("Handle paste");
        }
        Some(MessageTableMenuAction::Delete) => {
            println!(
                "Handle delete for message : {:?}",
                window_state.message_table.selected_indicies
            );
        }
    }
}

/// 渲染带有信号表格的详细弹出窗口
fn render_signals_table_tooltip(ui: &Ui, message: &EditableMessage) {
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
}

/// 请求窗口聚焦（如果需要）
fn request_window_focus() {
    unsafe {
        imgui::sys::igSetNextWindowFocus();
    }
}

/// 渲染所有 DBC 窗口
pub fn render_dbc_windows(ui: &Ui, ui_state: &mut UiState) {
    for (window_idx, dbc_window) in &mut ui_state.dbc_windows.iter_mut().enumerate() {
        if let Some(request_focus_idx) = ui_state.dbc_window_focus_request {
            if request_focus_idx == window_idx {
                request_window_focus();
                ui_state.dbc_window_focus_request = None;
            }
        }

        let window_title = format!(
            "DBC - {}",
            std::path::Path::new(&dbc_window.file_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_else(|| panic!("DBC window has empty file path"))
        );

        // 现在是始终打开的，没有对 is_open 的修改
        let mut is_open = dbc_window.is_open;

        let window_ui = ui
            .window(&window_title)
            .size([800.0, 600.0], Condition::FirstUseEver)
            .opened(&mut is_open);

        if dbc_window.is_dirty {
            dbc_window
                .message_table
                .init_sort_and_filter(&dbc_window.search_bar.query(), dbc_window.dbc.messages());
        }

        window_ui.build(|| {
            dbc_window.render(ui);
        });

        dbc_window.is_dirty = false;
    }
}
