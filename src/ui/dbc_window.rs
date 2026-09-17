use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::editable_dbc::{EditableDbc, EditableMessage, FrameFormat};
use crate::file_encoding::{decode_file_bytes, encode_to_bytes};
use crate::ui::all_signals_window::{self, AllSignalsEvent, AllSignalsWindow};
use crate::ui::comm_matrix::render_comm_matrix;
use crate::ui::message_edit_window::{MessageEditEvent, MessageEditWindowState};
use crate::ui::message_window::{MessageWindow, MessageWindowEvent};
use crate::ui::node_window::{render_node_list, NodeListState};
use crate::ui::signal_edit_window::SignalEditDialog;
use crate::ui::state::{DeleteTarget, UiState};
use dear_imgui_rs::{Condition, MouseButton, SortDirection, TabItemFlags, TableFlags, Ui};
use encoding_rs::Encoding;

/// DBC 窗口内的标签页（CANdb++ 风格）
#[derive(Clone, Copy, PartialEq)]
enum DbcTab {
    Messages,
    AllSignals,
    NodeList,
    CommMatrix,
}

/// DBC 窗口标签页向上抛的事件
enum DbcWindowEvent {
    None,
    Msg(MessageTableEvent),
    AllSignals(AllSignalsEvent),
}

#[allow(dead_code)]
enum MessageTableEvent {
    None,
    OpenMessage(u32),
    EditMessage(u32),
    CopyMessage(Vec<u32>),
    CutMessage(Vec<u32>),
    DeleteMessage(Vec<u32>),
    PasteMessage,
    AddMessage,
    OpenAllSignals,
}

#[derive(Clone)]
struct MessageTableState {
    sort_column_idx: u32,
    sort_ascending: bool,
}

impl Default for MessageTableState {
    fn default() -> Self {
        Self {
            sort_column_idx: 0,
            sort_ascending: true,
        }
    }
}

#[allow(dead_code)]
pub enum SignalWindowEvent {
    CopySignal { msg_id: u32, sig_names: Vec<String> },
    CutSignal { msg_id: u32, sig_names: Vec<String> },
    DeleteSignal { msg_id: u32, sig_names: Vec<String> },
    PasteSignal { msg_id: u32 },
}

#[derive(Clone)]
pub struct DbcWindow {
    pub is_open: bool,
    pub file_path: String,
    pub dbc: EditableDbc,
    pub is_dirty: bool,
    /// 文本编码：打开时检测（UTF-8 / GBK），保存时按原编码写出
    pub text_encoding: &'static Encoding,
    /// 打开的文件是否带 UTF-8 BOM（保存时还原）
    pub had_bom: bool,

    selected_message_ids: Vec<u32>,
    selection_anchor: Option<u32>,
    selection_cursor: Option<u32>,
    search_query: String,
    message_table: MessageTableState,
    pub message_windows: Vec<MessageWindow>,
    edit_windows: Vec<MessageEditWindowState>,
    signal_edit_dialog: SignalEditDialog,
    /// All Signals 标签页状态
    all_signals_window: AllSignalsWindow,
    /// 当前选中的标签页
    /// 请求切换到的标签页（下一次渲染生效）
    pending_tab: Option<DbcTab>,
    /// Node List 标签页状态
    node_list: NodeListState,
}

impl Default for DbcWindow {
    fn default() -> Self {
        Self {
            is_open: false,
            file_path: String::new(),
            dbc: EditableDbc::default(),
            is_dirty: false,
            text_encoding: encoding_rs::UTF_8,
            had_bom: false,
            selected_message_ids: Vec::new(),
            selection_anchor: None,
            selection_cursor: None,
            search_query: String::new(),
            message_table: MessageTableState::default(),
            message_windows: Vec::new(),
            edit_windows: Vec::new(),
            signal_edit_dialog: SignalEditDialog::default(),
            all_signals_window: AllSignalsWindow::default(),
            pending_tab: None,
            node_list: NodeListState::default(),
        }
    }
}

impl DbcWindow {
    pub fn new(file_path: &str, dbc: EditableDbc) -> Self {
        Self {
            is_open: true,
            // 统一为规范化绝对路径，供"已打开则聚焦"与最近文件去重比较
            file_path: crate::ui::state::normalize_path(file_path),
            dbc,
            is_dirty: false,
            text_encoding: encoding_rs::UTF_8,
            had_bom: false,
            selected_message_ids: Vec::new(),
            selection_anchor: None,
            selection_cursor: None,
            search_query: String::new(),
            message_table: MessageTableState::default(),
            message_windows: Vec::new(),
            edit_windows: Vec::new(),
            signal_edit_dialog: SignalEditDialog::default(),
            all_signals_window: AllSignalsWindow::default(),
            pending_tab: None,
            node_list: NodeListState::default(),
        }
    }

    pub fn selected_message_ids(&self) -> Vec<u32> {
        self.selected_message_ids.clone()
    }

    pub fn set_selected_message_id(&mut self, id: Option<u32>) {
        match id {
            Some(id) => {
                self.selected_message_ids = vec![id];
                self.selection_anchor = Some(id);
                self.selection_cursor = Some(id);
            }
            None => self.clear_selection(),
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_message_ids.clear();
        self.selection_anchor = None;
        self.selection_cursor = None;
    }

    pub fn from_path(file_path: &Path) -> Result<Self, String> {
        let mut file = File::open(file_path)
            .map_err(|e| format!("Failed to open file {}: {}", file_path.display(), e))?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)
            .map_err(|e| format!("Failed to read file {}: {}", file_path.display(), e))?;
        // 嗅探编码（UTF-8 BOM / 严格 UTF-8 / GBK），GBK 是国内工具链的 ANSI 惯例
        let decoded = decode_file_bytes(&contents);
        match can_dbc::Dbc::try_from(decoded.text.as_str()) {
            Ok(original_dbc) => {
                let editable_dbc = EditableDbc::from_dbc(&original_dbc);
                let path_str = file_path
                    .to_str()
                    .ok_or_else(|| format!("Invalid path: {}", file_path.display()))?
                    .to_string();
                let mut window = Self::new(&path_str, editable_dbc);
                window.text_encoding = decoded.encoding;
                window.had_bom = decoded.had_bom;
                Ok(window)
            }
            Err(e) => Err(format!("Failed to parse DBC {}: {:?}", file_path.display(), e)),
        }
    }

    fn render_message_table(&mut self, ui: &Ui, has_clipboard: bool) -> MessageTableEvent {
        let mut event = MessageTableEvent::None;

        if ui.button("+ Add Message") {
            event = MessageTableEvent::AddMessage;
        }
        ui.same_line();
        if ui.button("All Signals") {
            event = MessageTableEvent::OpenAllSignals;
        }
        ui.same_line();
        ui.set_next_item_width(260.0);
        ui.input_text("##msg_search", &mut self.search_query)
            .hint("Filter messages...")
            .build();

        let messages = self.dbc.messages();
        let query_lower = self.search_query.to_lowercase();

        let mut filtered: Vec<usize> = messages
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                query_lower.is_empty() || m.message_name().to_lowercase().contains(&query_lower)
            })
            .map(|(i, _)| i)
            .collect();
        ui.same_line();
        ui.text_disabled(format!("{} message(s)", filtered.len()));

        let col = self.message_table.sort_column_idx;
        let asc = self.message_table.sort_ascending;
        filtered.sort_by(|&a, &b| {
            let ma = &messages[a];
            let mb = &messages[b];
            let cmp = match col {
                0 => ma.message_id().cmp(&mb.message_id()),
                1 => ma.message_name().cmp(mb.message_name()),
                2 => ma.message_size().cmp(&mb.message_size()),
                3 => ma.transmitter().cmp(mb.transmitter()),
                4 => ma.signals().len().cmp(&mb.signals().len()),
                5 => ma.comment().cmp(mb.comment()),
                _ => std::cmp::Ordering::Equal,
            };
            if asc { cmp } else { cmp.reverse() }
        });

        // Keyboard navigation
        if ui.is_window_focused() && !ui.io().want_capture_keyboard() && !filtered.is_empty() {
            let shift = ui.io().key_shift();
            let cursor_pos = self
                .selection_cursor
                .and_then(|id| filtered.iter().position(|&idx| messages[idx].message_id() == id));

            let move_cursor = |pos: Option<usize>, down: bool| -> usize {
                match pos {
                    Some(p) if down => (p + 1).min(filtered.len() - 1),
                    Some(0) => 0,
                    Some(p) => p - 1,
                    None if down => 0,
                    None => filtered.len() - 1,
                }
            };

            let mut new_pos: Option<usize> = None;
            if ui.is_key_pressed(dear_imgui_rs::Key::DownArrow) {
                new_pos = Some(move_cursor(cursor_pos, true));
            }
            if ui.is_key_pressed(dear_imgui_rs::Key::UpArrow) {
                new_pos = Some(move_cursor(cursor_pos, false));
            }

            if let Some(pos) = new_pos {
                let new_id = messages[filtered[pos]].message_id();
                self.selection_cursor = Some(new_id);
                if shift {
                    let anchor = self.selection_anchor.unwrap_or(new_id);
                    let anchor_pos = filtered
                        .iter()
                        .position(|&idx| messages[idx].message_id() == anchor)
                        .unwrap_or(pos);
                    let (lo, hi) = if anchor_pos <= pos { (anchor_pos, pos) } else { (pos, anchor_pos) };
                    self.selected_message_ids = filtered[lo..=hi]
                        .iter()
                        .map(|&idx| messages[idx].message_id())
                        .collect();
                } else {
                    self.selected_message_ids = vec![new_id];
                    self.selection_anchor = Some(new_id);
                }
            }
            if ui.is_key_pressed(dear_imgui_rs::Key::Enter) {
                if let Some(msg_id) = self.selection_cursor.or(self.selected_message_ids.first().copied()) {
                    event = MessageTableEvent::OpenMessage(msg_id);
                }
            }
        }

        // 底部状态栏预留 20px
        // 表格填满标签页剩余空间（底部已无状态栏），窗口不出现滚动条
        let avail_h = ui.content_region_avail()[1];
        if let Some(_table) = ui.begin_table_with_sizing(
            "msg_table",
            6,
            dear_imgui_rs::TableOptions::new()
                .flags(
                    TableFlags::RESIZABLE
                        | TableFlags::BORDERS
                        | TableFlags::ROW_BG
                        | TableFlags::SCROLL_Y
                        | TableFlags::SORTABLE,
                )
                .sizing_policy(dear_imgui_rs::TableSizingPolicy::FixedFit),
            [0.0, avail_h],
            0.0,
        ) {
            ui.table_setup_column("ID", dear_imgui_rs::TableColumnFlags::NONE, None);
            ui.table_setup_column("Name", dear_imgui_rs::TableColumnFlags::NONE, None);
            ui.table_setup_column("DLC", dear_imgui_rs::TableColumnFlags::NONE, None);
            ui.table_setup_column("Transmitter", dear_imgui_rs::TableColumnFlags::NONE, None);
            ui.table_setup_column("Signals", dear_imgui_rs::TableColumnFlags::NONE, None);
            ui.table_setup_column("Comment", dear_imgui_rs::TableColumnFlags::NONE, None);
            // 冻结标题行，滚动时表头保持可见
            ui.table_setup_scroll_freeze(0, 1);
            ui.table_headers_row();

            if let Some(mut sort_specs) = ui.table_get_sort_specs() {
                if sort_specs.is_dirty() {
                    if let Some(spec) = sort_specs.iter().next() {
                        self.message_table.sort_column_idx = usize::from(spec.column_index) as u32;
                        self.message_table.sort_ascending =
                            spec.sort_direction == SortDirection::Ascending;
                    }
                    sort_specs.clear_dirty(ui);
                }
            }

            for (row_pos, &idx) in filtered.iter().enumerate() {
                let msg = &messages[idx];
                let msg_id = msg.message_id();

                ui.table_next_row();

                let is_selected = self.selected_message_ids.contains(&msg_id);

                ui.table_set_column_index(0);
                let id_label = format!("0x{:03X}{}", msg_id, if msg.is_extended() { "x" } else { "" });
                if ui
                    .selectable_config(id_label)
                    .selected(is_selected)
                    .span_all_columns(true)
                    .build()
                {
                    let ctrl = ui.io().key_ctrl();
                    let shift = ui.io().key_shift();
                    if ctrl {
                        if let Some(pos) =
                            self.selected_message_ids.iter().position(|&id| id == msg_id)
                        {
                            self.selected_message_ids.remove(pos);
                        } else {
                            self.selected_message_ids.push(msg_id);
                        }
                        self.selection_anchor = Some(msg_id);
                        self.selection_cursor = Some(msg_id);
                    } else if shift {
                        let anchor = self.selection_anchor.unwrap_or(msg_id);
                        let anchor_pos = filtered
                            .iter()
                            .position(|&i| messages[i].message_id() == anchor)
                            .unwrap_or(row_pos);
                        let (lo, hi) = if anchor_pos <= row_pos {
                            (anchor_pos, row_pos)
                        } else {
                            (row_pos, anchor_pos)
                        };
                        self.selected_message_ids = filtered[lo..=hi]
                            .iter()
                            .map(|&i| messages[i].message_id())
                            .collect();
                        self.selection_cursor = Some(msg_id);
                    } else {
                        self.selected_message_ids = vec![msg_id];
                        self.selection_anchor = Some(msg_id);
                        self.selection_cursor = Some(msg_id);
                    }
                }
                if ui.is_item_hovered()
                    && ui.is_mouse_double_clicked(MouseButton::Left)
                {
                    event = MessageTableEvent::OpenMessage(msg_id);
                }

                if let Some(_popup) = ui.begin_popup_context_item_with_label(Some(
                    &format!("msg_ctx_{}", msg_id),
                )) {
                    if !self.selected_message_ids.contains(&msg_id) {
                        self.selected_message_ids = vec![msg_id];
                        self.selection_anchor = Some(msg_id);
                        self.selection_cursor = Some(msg_id);
                    }
                    let selected_ids = self.selected_message_ids.clone();
                    if ui.menu_item("Edit") {
                        event = MessageTableEvent::EditMessage(msg_id);
                    }
                    if ui.menu_item("Copy") {
                        event = MessageTableEvent::CopyMessage(selected_ids.clone());
                    }
                    if ui.menu_item("Cut") {
                        event = MessageTableEvent::CutMessage(selected_ids.clone());
                    }
                    ui.separator();
                    if ui.menu_item_enabled_selected_no_shortcut("Paste", false, has_clipboard) {
                        event = MessageTableEvent::PasteMessage;
                    }
                    ui.separator();
                    if ui.menu_item("Delete") {
                        event = MessageTableEvent::DeleteMessage(selected_ids);
                    }
                }

                ui.table_set_column_index(1);
                ui.text(msg.message_name());

                ui.table_set_column_index(2);
                ui.text(format!("{}", msg.message_size()));

                ui.table_set_column_index(3);
                ui.text(msg.transmitter());

                ui.table_set_column_index(4);
                ui.text(format!("{}", msg.signals().len()));

                ui.table_set_column_index(5);
                ui.text(msg.comment());
            }
        }

        event
    }

    pub fn render_floating_windows(&mut self, ui: &Ui, has_clipboard: bool) -> Vec<SignalWindowEvent> {
        let mut signal_events = Vec::new();

        for msg_win in &mut self.message_windows {
            let win_event = msg_win.render(ui, &self.dbc, has_clipboard, &self.file_path);
            match win_event {
                MessageWindowEvent::EditSignal(sig_name) => {
                    if let Some(msg) = self.dbc.get_message(msg_win.message_id) {
                        if let Some(sig) = msg.signals().iter().find(|s| s.name() == sig_name) {
                            self.signal_edit_dialog
                                .open_from_signal(msg.message_id(), sig, &self.file_path);
                        }
                    }
                }
                MessageWindowEvent::CopySignal(sig_names) => {
                    signal_events.push(SignalWindowEvent::CopySignal {
                        msg_id: msg_win.message_id,
                        sig_names,
                    });
                }
                MessageWindowEvent::CutSignal(sig_names) => {
                    signal_events.push(SignalWindowEvent::CutSignal {
                        msg_id: msg_win.message_id,
                        sig_names,
                    });
                }
                MessageWindowEvent::DeleteSignal(sig_names) => {
                    signal_events.push(SignalWindowEvent::DeleteSignal {
                        msg_id: msg_win.message_id,
                        sig_names,
                    });
                }
                MessageWindowEvent::PasteSignal => {
                    signal_events.push(SignalWindowEvent::PasteSignal {
                        msg_id: msg_win.message_id,
                    });
                }
                MessageWindowEvent::AddSignal => {
                    self.dbc.new_signal(msg_win.message_id);
                    self.is_dirty = true;
                }
                MessageWindowEvent::None => {}
            }
        }
        self.message_windows.retain(|w| {
            w.is_open && self.dbc.get_message(w.message_id).is_some()
        });

        self.render_edit_windows(ui);

        let dialog_was_shown = self.signal_edit_dialog.show;
        let dialog_msg_id = self.signal_edit_dialog.message_id;
        self.render_signal_edit_dialog(ui);

        if dialog_was_shown && !self.signal_edit_dialog.show {
            if let Some(msg_win) = self
                .message_windows
                .iter_mut()
                .find(|w| w.message_id == dialog_msg_id)
            {
                msg_win.focus_requested = true;
            }
        }

        signal_events
    }

    fn open_message_window(&mut self, msg_id: u32) {
        if self.message_windows.iter().any(|w| w.message_id == msg_id) {
            return;
        }
        if self.dbc.get_message(msg_id).is_some() {
            self.message_windows
                .push(MessageWindow::new(msg_id));
        }
    }

    /// 关闭与被删消息关联的 Message 窗口、编辑窗口和信号编辑对话框
    pub fn close_windows_for_messages(&mut self, ids: &[u32]) {
        self.message_windows.retain(|w| !ids.contains(&w.message_id));
        self.edit_windows.retain(|w| !ids.contains(&w.current_id));
        if ids.contains(&self.signal_edit_dialog.message_id) {
            self.signal_edit_dialog.show = false;
        }
    }

    /// 渲染标签页内容（CANdb++ 风格：Messages & Signals / All Signals / Node List / Communication Matrix）
    fn render_tabs(&mut self, ui: &Ui, has_clipboard: bool) -> DbcWindowEvent {
        let mut event = DbcWindowEvent::None;

        if let Some(_bar) = ui.tab_bar("##dbc_tabs") {
            let pending = self.pending_tab.take();
            let flags_for = |tab: DbcTab| {
                if pending == Some(tab) {
                    TabItemFlags::SET_SELECTED
                } else {
                    TabItemFlags::NONE
                }
            };

            if let Some(_tab) = ui.tab_item_with_flags("Messages & Signals", None, flags_for(DbcTab::Messages)) {
                match self.render_message_table(ui, has_clipboard) {
                    MessageTableEvent::OpenAllSignals => {
                        self.pending_tab = Some(DbcTab::AllSignals);
                    }
                    MessageTableEvent::None => {}
                    ev => event = DbcWindowEvent::Msg(ev),
                }
            }
            if let Some(_tab) = ui.tab_item_with_flags("All Signals", None, flags_for(DbcTab::AllSignals)) {
                let ctx = all_signals_window::MatrixContext {
                    dbc: &mut self.dbc,
                    signal_edit_dialog: &mut self.signal_edit_dialog,
                    edit_windows: &mut self.edit_windows,
                    is_dirty: &mut self.is_dirty,
                    file_path: &self.file_path,
                };
                match all_signals_window::render(&mut self.all_signals_window, ctx, ui) {
                    AllSignalsEvent::None => {}
                    ev => event = DbcWindowEvent::AllSignals(ev),
                }
            }
            if let Some(_tab) = ui.tab_item_with_flags("Node List", None, flags_for(DbcTab::NodeList)) {
                if render_node_list(ui, &mut self.dbc, &mut self.node_list) {
                    self.is_dirty = true;
                }
            }
            if let Some(_tab) = ui.tab_item_with_flags("Communication Matrix", None, flags_for(DbcTab::CommMatrix)) {
                render_comm_matrix(ui, &mut self.dbc, &mut self.is_dirty);
            }
        }

        event
    }

    fn render_edit_windows(&mut self, ui: &Ui) {
        let mut to_remove = None;

        for (i, edit_win) in self.edit_windows.iter_mut().enumerate() {
            let event = edit_win.render(ui);
            match event {
                MessageEditEvent::Apply => {
                    let old_id = edit_win.current_id;
                    edit_win.apply_edit(&mut self.dbc);
                    if edit_win.error_message.is_empty() {
                        self.is_dirty = true;
                        let new_id = edit_win.current_id;
                        if old_id != new_id {
                            for win in &mut self.message_windows {
                                if win.message_id == old_id {
                                    win.message_id = new_id;
                                }
                            }
                            if self.signal_edit_dialog.message_id == old_id {
                                self.signal_edit_dialog.message_id = new_id;
                            }
                        }
                    }
                }
                MessageEditEvent::Ok => {
                    let old_id = edit_win.current_id;
                    edit_win.apply_edit(&mut self.dbc);
                    if edit_win.error_message.is_empty() {
                        self.is_dirty = true;
                        let new_id = edit_win.current_id;
                        if old_id != new_id {
                            for win in &mut self.message_windows {
                                if win.message_id == old_id {
                                    win.message_id = new_id;
                                }
                            }
                            if self.signal_edit_dialog.message_id == old_id {
                                self.signal_edit_dialog.message_id = new_id;
                            }
                        }
                        to_remove = Some(i);
                    }
                }
                MessageEditEvent::Cancel => {
                    to_remove = Some(i);
                }
                MessageEditEvent::None => {}
            }
        }

        if let Some(idx) = to_remove {
            self.edit_windows.remove(idx);
        }

        // 消息已被删除（或撤销导致）时关闭对应的编辑窗口
        let message_ids: Vec<u32> = self.dbc.messages().iter().map(|m| m.message_id()).collect();
        self.edit_windows
            .retain(|w| message_ids.contains(&w.current_id));
    }

    fn render_signal_edit_dialog(&mut self, ui: &Ui) {
        if !self.signal_edit_dialog.show {
            return;
        }

        let event = self.signal_edit_dialog.render(ui);
        match event {
            crate::ui::signal_edit_window::SignalEditEvent::Apply => {
                self.signal_edit_dialog.apply_edit(&mut self.dbc);
                self.is_dirty = true;
                self.signal_edit_dialog.focus_requested = true;
            }
            crate::ui::signal_edit_window::SignalEditEvent::Ok => {
                self.signal_edit_dialog.apply_edit(&mut self.dbc);
                self.is_dirty = true;
                self.signal_edit_dialog.show = false;
            }
            crate::ui::signal_edit_window::SignalEditEvent::Cancel => {
                self.signal_edit_dialog.show = false;
            }
            crate::ui::signal_edit_window::SignalEditEvent::None => {}
        }
    }
}

pub fn render_dbc_windows(ui: &Ui, ui_state: &mut UiState) {
    for window_idx in 0..ui_state.dbc_windows.len() {
        let dbc_window = &ui_state.dbc_windows[window_idx];

        // 标题显示完整路径（含所在文件夹）；ID 取 ## 后的稳定路径，
        // 脏标记变化不重置布局，同名文件（不同路径）也不互相覆盖
        let dirty_marker = if dbc_window.is_dirty { "* " } else { "" };
        let title = format!("DBC - {}{}##dbc_{}", dirty_marker, dbc_window.file_path, dbc_window.file_path);

        if let Some(request_focus_idx) = ui_state.dbc_window_focus_request {
            if request_focus_idx == window_idx {
                ui.set_window_focus(Some(&title));
                ui_state.dbc_window_focus_request = None;
            }
        }

        let mut is_open = ui_state.dbc_windows[window_idx].is_open;
        let was_open = is_open;
        let has_clipboard = !ui_state.clipboard.copied_messages.is_empty();

        let window_ui = ui
            .window(&title)
            .size([900.0, 600.0], Condition::FirstUseEver)
            .opened(&mut is_open);

        let window_event = window_ui.build(|| {
            ui_state.dbc_windows[window_idx].render_tabs(ui, has_clipboard)
        });

        if ui.is_window_focused() {
            ui_state.last_focused_dbc_index = Some(window_idx);
        }

        if !is_open && was_open && ui_state.dbc_windows[window_idx].is_dirty {
            is_open = true;
            ui_state.close_confirm_dialog.show = true;
            ui_state.close_confirm_dialog.dbc_window_index = Some(window_idx);
        }

        ui_state.dbc_windows[window_idx].is_open = is_open;

        match window_event {
            Some(DbcWindowEvent::Msg(table_event)) => {
                if !matches!(table_event, MessageTableEvent::None) {
                    handle_message_table_event(ui_state, window_idx, table_event);
                }
            }
            Some(DbcWindowEvent::AllSignals(AllSignalsEvent::DeleteSignal { msg_id, sig_name })) => {
                ui_state.confirm_delete_dialog.target =
                    Some(DeleteTarget::Signals(msg_id, vec![sig_name.clone()]));
                ui_state.confirm_delete_dialog.display_name = format!("signal '{}'", sig_name);
                ui_state.confirm_delete_dialog.show = true;
            }
            Some(DbcWindowEvent::AllSignals(AllSignalsEvent::Error(message))) => {
                ui_state.error_dialog.message = message;
                ui_state.error_dialog.show = true;
            }
            Some(DbcWindowEvent::AllSignals(AllSignalsEvent::None))
            | Some(DbcWindowEvent::None)
            | None => {}
        }
    }

    // Remove closed windows
    let mut closed_indices: Vec<usize> = ui_state
        .dbc_windows
        .iter()
        .enumerate()
        .filter(|(_, w)| !w.is_open)
        .map(|(i, _)| i)
        .collect();
    closed_indices.sort_by(|a, b| b.cmp(a));
    for idx in closed_indices {
        ui_state.dbc_windows.remove(idx);
        if let Some(focused) = ui_state.last_focused_dbc_index {
            if focused == idx {
                ui_state.last_focused_dbc_index = None;
            } else if focused > idx {
                ui_state.last_focused_dbc_index = Some(focused - 1);
            }
        }
    }

    render_confirm_delete_dialog(ui, ui_state);
    render_close_confirm_dialog(ui, ui_state);
    render_validation_dialog(ui, ui_state);

    for i in 0..ui_state.dbc_windows.len() {
        let has_clipboard = !ui_state.clipboard.copied_signals.is_empty();
        let signal_events = ui_state.dbc_windows[i].render_floating_windows(ui, has_clipboard);
        for evt in signal_events {
            handle_signal_window_event(ui_state, evt);
        }
    }
}

fn handle_message_table_event(
    ui_state: &mut UiState,
    window_idx: usize,
    event: MessageTableEvent,
) {
    match event {
        MessageTableEvent::OpenMessage(msg_id) => {
            ui_state.dbc_windows[window_idx].open_message_window(msg_id);
        }
        MessageTableEvent::EditMessage(msg_id) => {
            if let Some(win) = ui_state.dbc_windows.get_mut(window_idx) {
                if let Some(msg) = win.dbc.get_message(msg_id) {
                    let edit_win = MessageEditWindowState::open(msg, &win.file_path);
                    win.edit_windows.push(edit_win);
                }
            }
        }
        MessageTableEvent::CopyMessage(ids) => {
            let msgs: Vec<EditableMessage> = ids
                .iter()
                .filter_map(|id| ui_state.dbc_windows[window_idx].dbc.get_message(*id).cloned())
                .collect();
            if !msgs.is_empty() {
                ui_state.clipboard.copied_messages = msgs;
            }
        }
        MessageTableEvent::CutMessage(ids) => {
            let msgs: Vec<EditableMessage> = ids
                .iter()
                .filter_map(|id| ui_state.dbc_windows[window_idx].dbc.get_message(*id).cloned())
                .collect();
            if !msgs.is_empty() {
                ui_state.clipboard.copied_messages = msgs;
            }
            ui_state.confirm_delete_dialog.target = Some(DeleteTarget::Messages(ids.clone()));
            ui_state.confirm_delete_dialog.display_name = messages_display_name(
                &ui_state.dbc_windows[window_idx].dbc,
                &ids,
            );
            ui_state.confirm_delete_dialog.show = true;
        }
        MessageTableEvent::DeleteMessage(ids) => {
            ui_state.confirm_delete_dialog.target = Some(DeleteTarget::Messages(ids.clone()));
            ui_state.confirm_delete_dialog.display_name = messages_display_name(
                &ui_state.dbc_windows[window_idx].dbc,
                &ids,
            );
            ui_state.confirm_delete_dialog.show = true;
        }
        MessageTableEvent::PasteMessage => {
            paste_messages(ui_state, window_idx);
        }
        MessageTableEvent::AddMessage => {
            add_new_message(ui_state, window_idx);
        }
        // 正常情况下该事件在 render_tabs 内被拦截；此分支仅作兜底
        MessageTableEvent::OpenAllSignals => {
            ui_state.dbc_windows[window_idx].pending_tab = Some(DbcTab::AllSignals);
        }
        MessageTableEvent::None => {}
    }
}

/// 在指定 DBC 窗口新建一条默认消息（ID / 帧格式自动分配）
pub(crate) fn add_new_message(ui_state: &mut UiState, window_idx: usize) {
    let next_id = ui_state.generate_next_message_id(window_idx);
    let msg_count = ui_state.dbc_windows[window_idx].dbc.messages().len();
    let frame_format = if next_id > 0x7FF {
        FrameFormat::Extended
    } else {
        FrameFormat::Standard
    };
    let msg = EditableMessage::build(
        next_id,
        frame_format,
        format!("Message_{}", msg_count),
        8,
        "Vector__XXX".to_string(),
        Vec::new(),
        String::new(),
    );
    ui_state.dbc_windows[window_idx].dbc.add_message(&msg);
    ui_state.dbc_windows[window_idx].set_selected_message_id(Some(next_id));
    ui_state.dbc_windows[window_idx].is_dirty = true;
}

/// 粘贴剪贴板中的消息：ID 从第一个空闲 ID 起分配，名称自动去重（_copy / _copy2 ...）
pub(crate) fn paste_messages(ui_state: &mut UiState, window_idx: usize) {
    let copied = ui_state.clipboard.copied_messages.clone();
    if copied.is_empty() {
        return;
    }
    let win = &mut ui_state.dbc_windows[window_idx];
    let max_id = win
        .dbc
        .messages()
        .iter()
        .map(|m| m.message_id())
        .max()
        .unwrap_or(0);
    let mut id = crate::editable_dbc::next_free_message_id(&win.dbc, max_id + 1);
    for mut msg in copied {
        let base = msg.message_name().to_string();
        let mut name = format!("{}_copy", base);
        let mut n = 1;
        while win.dbc.get_message_by_name(&name).is_some() {
            n += 1;
            name = format!("{}_copy{}", base, n);
        }
        msg.set_message_id(id);
        msg.set_message_name(&name);
        win.dbc.add_message(&msg);
        id = crate::editable_dbc::next_free_message_id(&win.dbc, id + 1);
        win.is_dirty = true;
    }
}

fn messages_display_name(dbc: &EditableDbc, ids: &[u32]) -> String {
    if ids.len() == 1 {
        let name = dbc
            .get_message(ids[0])
            .map(|m| m.message_name().to_string())
            .unwrap_or_default();
        format!("message '{}'", name)
    } else {
        format!("{} messages", ids.len())
    }
}

fn render_confirm_delete_dialog(ui: &Ui, ui_state: &mut UiState) {
    if ui_state.confirm_delete_dialog.show {
        ui.open_popup("Confirm Delete");
        ui_state.confirm_delete_dialog.show = false;
    }

    let popup = ui.begin_modal_popup("Confirm Delete");
    let is_open_now = popup.is_some();
    if let Some(_popup) = popup {
        ui.text(format!(
            "Are you sure you want to delete {}?",
            ui_state.confirm_delete_dialog.display_name
        ));
        ui.separator();
        if ui.button("OK") {
            execute_delete(ui_state);
            ui.close_current_popup();
        }
        ui.same_line();
        if ui.button("Cancel") {
            ui_state.confirm_delete_dialog.target = None;
            ui.close_current_popup();
        }
    }

    // 模态框被按钮以外的方式关闭（如 Esc）时清理残留 target，避免阻塞后续删除操作
    if ui_state.confirm_delete_dialog.was_open
        && !is_open_now
        && ui_state.confirm_delete_dialog.target.is_some()
    {
        ui_state.confirm_delete_dialog.target = None;
    }
    ui_state.confirm_delete_dialog.was_open = is_open_now;
}

fn render_close_confirm_dialog(ui: &Ui, ui_state: &mut UiState) {
    if ui_state.close_confirm_dialog.show {
        ui.open_popup("Save Changes");
        ui_state.close_confirm_dialog.show = false;
    }

    if let Some(_popup) = ui.begin_modal_popup("Save Changes") {
        let idx = ui_state.close_confirm_dialog.dbc_window_index.unwrap_or(0);
        let file_name = ui_state
            .dbc_windows
            .get(idx)
            .map(|w| {
                std::path::Path::new(&w.file_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&w.file_path)
                    .to_string()
            })
            .unwrap_or_default();

        ui.text(format!("Save changes to '{}'?", file_name));
        ui.separator();
        if ui.button("Save") {
            if let Some(idx) = ui_state.close_confirm_dialog.dbc_window_index {
                save_dbc_window(ui_state, idx);
                if let Some(win) = ui_state.dbc_windows.get_mut(idx) {
                    win.is_open = false;
                }
            }
            ui_state.close_confirm_dialog.dbc_window_index = None;
            ui.close_current_popup();
        }
        ui.same_line();
        if ui.button("Don't Save") {
            if let Some(idx) = ui_state.close_confirm_dialog.dbc_window_index {
                if let Some(win) = ui_state.dbc_windows.get_mut(idx) {
                    win.is_open = false;
                }
            }
            ui_state.close_confirm_dialog.dbc_window_index = None;
            ui.close_current_popup();
        }
        ui.same_line();
        if ui.button("Cancel") {
            ui_state.close_confirm_dialog.dbc_window_index = None;
            ui.close_current_popup();
        }
    }
}

fn render_validation_dialog(ui: &Ui, ui_state: &mut UiState) {
    if !ui_state.validation_dialog.show {
        return;
    }

    let mut is_open = true;
    ui.window("Validation Results")
        .opened(&mut is_open)
        .size([640.0, 420.0], dear_imgui_rs::Condition::FirstUseEver)
        .build(|| {
            let issues = &ui_state.validation_dialog.issues;
            let error_count = issues
                .iter()
                .filter(|i| matches!(i.severity, crate::editable_dbc::Severity::Error))
                .count();
            let warning_count = issues
                .iter()
                .filter(|i| matches!(i.severity, crate::editable_dbc::Severity::Warning))
                .count();

            if issues.is_empty() {
                ui.text_colored([0.0, 0.8, 0.0, 1.0], "No issues found. DBC is valid.");
            } else {
                ui.text(format!(
                    "Found {} error(s), {} warning(s):",
                    error_count, warning_count
                ));
                ui.separator();

                let avail_h = ui.content_region_avail()[1];
                if let Some(_table) = ui.begin_table_with_sizing(
                    "validation_table",
                    2,
                    dear_imgui_rs::TableOptions::new()
                        .flags(
                            dear_imgui_rs::TableFlags::RESIZABLE
                                | dear_imgui_rs::TableFlags::BORDERS
                                | dear_imgui_rs::TableFlags::ROW_BG
                                | dear_imgui_rs::TableFlags::SCROLL_Y,
                        )
                        .sizing_policy(dear_imgui_rs::TableSizingPolicy::FixedFit),
                    [0.0, avail_h],
                    0.0,
                ) {
                    ui.table_setup_column("Severity", dear_imgui_rs::TableColumnFlags::NONE, None);
                    ui.table_setup_column("Message", dear_imgui_rs::TableColumnFlags::NONE, None);
                    ui.table_setup_scroll_freeze(0, 1);
                    ui.table_headers_row();

                    for issue in issues {
                        ui.table_next_row();
                        ui.table_set_column_index(0);
                        match issue.severity {
                            crate::editable_dbc::Severity::Error => {
                                ui.text_colored([1.0, 0.3, 0.3, 1.0], "ERROR");
                            }
                            crate::editable_dbc::Severity::Warning => {
                                ui.text_colored([1.0, 0.8, 0.0, 1.0], "WARN");
                            }
                        }
                        ui.table_set_column_index(1);
                        ui.text(&issue.message);
                    }
                }
            }
        });

    if !is_open {
        ui_state.validation_dialog.show = false;
    }
}

pub fn save_dbc_window(ui_state: &mut UiState, idx: usize) {
    let win = &mut ui_state.dbc_windows[idx];
    let save_path = win.file_path.clone();
    let dbc_string = win.dbc.to_dbc_string();
    // 按打开时的编码写出（GBK 文件保存后仍是 GBK），保持与其它工具的互换性
    let bytes = encode_to_bytes(&dbc_string, win.text_encoding, win.had_bom);
    match std::fs::write(&save_path, &bytes) {
        Ok(_) => {
            win.is_dirty = false;
        }
        Err(e) => {
            ui_state.error_dialog.message = format!("Failed to save: {}", e);
            ui_state.error_dialog.show = true;
        }
    }
}

fn handle_signal_window_event(ui_state: &mut UiState, event: SignalWindowEvent) {
    match event {
        SignalWindowEvent::CopySignal { msg_id, sig_names } => {
            if let Some(idx) = ui_state.last_focused_dbc_index {
                if let Some(win) = ui_state.dbc_windows.get(idx) {
                    if let Some(msg) = win.dbc.get_message(msg_id) {
                        let sigs: Vec<crate::editable_dbc::EditableSignal> = sig_names
                            .iter()
                            .filter_map(|name| {
                                msg.signals()
                                    .iter()
                                    .find(|s| s.name() == name.as_str())
                                    .cloned()
                            })
                            .collect();
                        if !sigs.is_empty() {
                            ui_state.clipboard.copied_signals = sigs;
                        }
                    }
                }
            }
        }
        SignalWindowEvent::CutSignal { msg_id, sig_names } => {
            if let Some(idx) = ui_state.last_focused_dbc_index {
                if let Some(win) = ui_state.dbc_windows.get(idx) {
                    if let Some(msg) = win.dbc.get_message(msg_id) {
                        let sigs: Vec<crate::editable_dbc::EditableSignal> = sig_names
                            .iter()
                            .filter_map(|name| {
                                msg.signals()
                                    .iter()
                                    .find(|s| s.name() == name.as_str())
                                    .cloned()
                            })
                            .collect();
                        if !sigs.is_empty() {
                            ui_state.clipboard.copied_signals = sigs;
                        }
                    }
                }
            }
            ui_state.confirm_delete_dialog.target =
                Some(DeleteTarget::Signals(msg_id, sig_names.clone()));
            ui_state.confirm_delete_dialog.display_name = signals_display_name(&sig_names);
            ui_state.confirm_delete_dialog.show = true;
        }
        SignalWindowEvent::DeleteSignal { msg_id, sig_names } => {
            ui_state.confirm_delete_dialog.target =
                Some(DeleteTarget::Signals(msg_id, sig_names.clone()));
            ui_state.confirm_delete_dialog.display_name = signals_display_name(&sig_names);
            ui_state.confirm_delete_dialog.show = true;
        }
        SignalWindowEvent::PasteSignal { msg_id } => {
            let copied = ui_state.clipboard.copied_signals.clone();
            if copied.is_empty() {
                return;
            }
            if let Some(idx) = ui_state.last_focused_dbc_index {
                if let Some(win) = ui_state.dbc_windows.get_mut(idx) {
                    for sig in copied {
                        let mut new_sig = sig;
                        // 名称自动去重：_copy / _copy2 ...
                        let base = new_sig.name().to_string();
                        let mut name = format!("{}_copy", base);
                        let mut n = 1;
                        while win
                            .dbc
                            .get_message(msg_id)
                            .is_some_and(|m| m.signals().iter().any(|s| s.name() == name))
                        {
                            n += 1;
                            name = format!("{}_copy{}", base, n);
                        }
                        new_sig.set_name(&name);
                        win.dbc.add_signal(msg_id, &new_sig);
                    }
                    win.is_dirty = true;
                }
            }
        }
    }
}

fn signals_display_name(sig_names: &[String]) -> String {
    if sig_names.len() == 1 {
        format!("signal '{}'", sig_names[0])
    } else {
        format!("{} signals", sig_names.len())
    }
}

fn execute_delete(ui_state: &mut UiState) {
    let target = ui_state.confirm_delete_dialog.target.take();
    match target {
        Some(DeleteTarget::Messages(ids)) => {
            if let Some(idx) = ui_state.last_focused_dbc_index {
                if let Some(win) = ui_state.dbc_windows.get_mut(idx) {
                    for id in &ids {
                        win.dbc.delete_message(*id);
                    }
                    if ids.len() > 1 {
                        win.dbc.merge_last_compounds(ids.len());
                    }
                    win.selected_message_ids.retain(|id| !ids.contains(id));
                    // 关闭被删消息的所有窗口，避免悬空
                    win.close_windows_for_messages(&ids);
                    win.is_dirty = true;
                }
            }
        }
        Some(DeleteTarget::Signals(msg_id, sig_names)) => {
            if let Some(idx) = ui_state.last_focused_dbc_index {
                if let Some(win) = ui_state.dbc_windows.get_mut(idx) {
                    for name in &sig_names {
                        win.dbc.delete_signal(msg_id, name);
                    }
                    if sig_names.len() > 1 {
                        win.dbc.merge_last_compounds(sig_names.len());
                    }
                    win.is_dirty = true;
                }
            }
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_encoding::encode_to_bytes;

    /// GBK 编码的 DBC 文件打开后中文注释完整，且保存时保持 GBK
    #[test]
    fn gbk_file_round_trips_through_from_path() {
        let text = "VERSION \"\"\n\nBO_ 1 TestMsg: 8 Vector__XXX\n\nCM_ BO_ 1 \"中文注释\";\n";
        let (bytes, _, _) = encoding_rs::GBK.encode(text);

        let dir = std::env::temp_dir().join(format!("roxy-dbc-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("gbk_sample.dbc");
        std::fs::write(&path, &bytes).unwrap();

        let window = DbcWindow::from_path(&path).expect("GBK file must load");
        assert_eq!(window.text_encoding, encoding_rs::GBK);
        assert!(!window.had_bom);
        let msg = window.dbc.get_message(1).unwrap();
        assert_eq!(msg.comment(), "中文注释");

        // 保存：按检测到的 GBK 编码写出，重新打开仍能读到中文
        let out = encode_to_bytes(&window.dbc.to_dbc_string(), window.text_encoding, window.had_bom);
        std::fs::write(&path, &out).unwrap();
        let reopened = DbcWindow::from_path(&path).expect("saved GBK file must load");
        assert_eq!(reopened.dbc.get_message(1).unwrap().comment(), "中文注释");

        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }
}
