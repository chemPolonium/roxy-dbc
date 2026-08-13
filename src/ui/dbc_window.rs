use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::editable_dbc::{EditableDbc, EditableMessage, FrameFormat};
use crate::ui::message_edit_window::{MessageEditEvent, MessageEditWindowState};
use crate::ui::message_window::{MessageWindow, MessageWindowEvent};
use crate::ui::signal_edit_window::SignalEditDialog;
use crate::ui::state::{DeleteTarget, UiState};
use imgui::{Condition, TableFlags, Ui};

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

    selected_message_ids: Vec<u32>,
    selection_anchor: Option<u32>,
    selection_cursor: Option<u32>,
    search_query: String,
    message_table: MessageTableState,
    pub message_windows: Vec<MessageWindow>,
    edit_windows: Vec<MessageEditWindowState>,
    signal_edit_dialog: SignalEditDialog,
}

impl Default for DbcWindow {
    fn default() -> Self {
        Self {
            is_open: false,
            file_path: String::new(),
            dbc: EditableDbc::default(),
            is_dirty: false,
            selected_message_ids: Vec::new(),
            selection_anchor: None,
            selection_cursor: None,
            search_query: String::new(),
            message_table: MessageTableState::default(),
            message_windows: Vec::new(),
            edit_windows: Vec::new(),
            signal_edit_dialog: SignalEditDialog::default(),
        }
    }
}

impl DbcWindow {
    pub fn new(file_path: &str, dbc: EditableDbc) -> Self {
        Self {
            is_open: true,
            file_path: file_path.to_string(),
            dbc,
            is_dirty: false,
            selected_message_ids: Vec::new(),
            selection_anchor: None,
            selection_cursor: None,
            search_query: String::new(),
            message_table: MessageTableState::default(),
            message_windows: Vec::new(),
            edit_windows: Vec::new(),
            signal_edit_dialog: SignalEditDialog::default(),
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

    fn render_message_table(&mut self, ui: &Ui, has_clipboard: bool) -> MessageTableEvent {
        let mut event = MessageTableEvent::None;

        if ui.small_button("+ Add Message") {
            event = MessageTableEvent::AddMessage;
        }
        ui.same_line();
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
        if ui.is_window_focused() && !filtered.is_empty() {
            let shift = ui.io().key_shift;
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
            if ui.is_key_pressed(imgui::Key::DownArrow) {
                new_pos = Some(move_cursor(cursor_pos, true));
            }
            if ui.is_key_pressed(imgui::Key::UpArrow) {
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
            if ui.is_key_pressed(imgui::Key::Enter) {
                if let Some(msg_id) = self.selection_cursor.or(self.selected_message_ids.first().copied()) {
                    event = MessageTableEvent::OpenMessage(msg_id);
                }
            }
        }

        if let Some(_table) = ui.begin_table_with_flags(
            "msg_table",
            6,
            TableFlags::RESIZABLE
                | TableFlags::BORDERS
                | TableFlags::SCROLL_Y
                | TableFlags::SORTABLE
                | TableFlags::SIZING_FIXED_FIT,
        ) {
            ui.table_setup_column("ID");
            ui.table_setup_column("Name");
            ui.table_setup_column("DLC");
            ui.table_setup_column("Transmitter");
            ui.table_setup_column("Signals");
            ui.table_setup_column("Comment");
            ui.table_headers_row();

            if let Some(mut sort_specs) = ui.table_sort_specs_mut() {
                if sort_specs.should_sort() {
                    if let Some(spec) = sort_specs.specs().iter().next() {
                        self.message_table.sort_column_idx = spec.column_idx() as u32;
                        self.message_table.sort_ascending =
                            spec.sort_direction() == Some(imgui::TableSortDirection::Ascending);
                    }
                    sort_specs.set_sorted();
                }
            }

            for (row_pos, &idx) in filtered.iter().enumerate() {
                let msg = &messages[idx];
                let msg_id = msg.message_id();

                ui.table_next_row();

                let is_selected = self.selected_message_ids.contains(&msg_id);

                ui.table_set_column_index(0);
                if ui
                    .selectable_config(format!("0x{:03X}", msg_id))
                    .selected(is_selected)
                    .span_all_columns(true)
                    .build()
                {
                    let ctrl = ui.io().key_ctrl;
                    let shift = ui.io().key_shift;
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
                    && ui.is_mouse_double_clicked(imgui::MouseButton::Left)
                {
                    event = MessageTableEvent::OpenMessage(msg_id);
                }

                if let Some(_popup) =
                    ui.begin_popup_context_with_label(format!("msg_ctx_{}", msg_id))
                {
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
                    if ui.menu_item_config("Paste").enabled(has_clipboard).build() {
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
            let win_event = msg_win.render(ui, &self.dbc, has_clipboard);
            match win_event {
                MessageWindowEvent::EditSignal(sig_name) => {
                    if let Some(msg) = self.dbc.get_message(msg_win.message_id) {
                        if let Some(sig) = msg.signals().iter().find(|s| s.name() == sig_name) {
                            self.signal_edit_dialog
                                .open_from_signal(msg.message_id(), sig);
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
        self.message_windows.retain(|w| w.is_open);

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
                .push(MessageWindow::new(msg_id, 0));
        }
    }

    fn render_edit_windows(&mut self, ui: &Ui) {
        let mut to_remove = None;

        for (i, edit_win) in self.edit_windows.iter_mut().enumerate() {
            let event = edit_win.render(ui);
            match event {
                MessageEditEvent::Apply => {
                    edit_win.apply_edit(&mut self.dbc);
                    self.is_dirty = true;
                }
                MessageEditEvent::Ok => {
                    edit_win.apply_edit(&mut self.dbc);
                    self.is_dirty = true;
                    to_remove = Some(i);
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

fn request_window_focus() {
    unsafe {
        imgui::sys::igSetNextWindowFocus();
    }
}

pub fn render_dbc_windows(ui: &Ui, ui_state: &mut UiState) {
    for window_idx in 0..ui_state.dbc_windows.len() {
        if let Some(request_focus_idx) = ui_state.dbc_window_focus_request {
            if request_focus_idx == window_idx {
                request_window_focus();
                ui_state.dbc_window_focus_request = None;
            }
        }

        let dbc_window = &ui_state.dbc_windows[window_idx];
        let dirty_marker = if dbc_window.is_dirty { "* " } else { "" };
        let window_title = format!(
            "DBC - {}{}",
            dirty_marker,
            std::path::Path::new(&dbc_window.file_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_else(|| panic!("DBC window has empty file path"))
        );

        let mut is_open = ui_state.dbc_windows[window_idx].is_open;
        let was_open = is_open;
        let has_clipboard = !ui_state.clipboard.copied_messages.is_empty();

        let window_ui = ui
            .window(&window_title)
            .size([900.0, 600.0], Condition::FirstUseEver)
            .opened(&mut is_open);

        let table_event = window_ui.build(|| {
            let event = ui_state.dbc_windows[window_idx].render_message_table(ui, has_clipboard);

            let msg_count = ui_state.dbc_windows[window_idx].dbc.messages().len();
            let sig_count: usize = ui_state.dbc_windows[window_idx]
                .dbc
                .messages()
                .iter()
                .map(|m| m.signals().len())
                .sum();
            let file_path = &ui_state.dbc_windows[window_idx].file_path;

            ui.separator();
            ui.text_disabled(format!(
                "{}  |  {} message(s)  |  {} signal(s)",
                file_path, msg_count, sig_count
            ));

            event
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

        if let Some(table_event) = table_event {
            if !matches!(table_event, MessageTableEvent::None) {
                handle_message_table_event(ui_state, window_idx, table_event);
            }
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
            if let Some(msg) = ui_state.dbc_windows[window_idx].dbc.get_message(msg_id) {
                let edit_win = MessageEditWindowState::open(msg);
                ui_state.dbc_windows[window_idx].edit_windows.push(edit_win);
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
            let copied = ui_state.clipboard.copied_messages.clone();
            if !copied.is_empty() {
                let win = &mut ui_state.dbc_windows[window_idx];
                let mut next_id = win
                    .dbc
                    .messages()
                    .iter()
                    .map(|m| m.message_id())
                    .max()
                    .unwrap_or(0)
                    + 1;
                for mut new_msg in copied {
                    new_msg.set_message_id(next_id);
                    let new_name = format!("{}_copy", new_msg.message_name());
                    new_msg.set_message_name(&new_name);
                    win.dbc.add_message(&new_msg);
                    next_id += 1;
                }
                win.is_dirty = true;
            }
        }
        MessageTableEvent::AddMessage => {
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
        MessageTableEvent::None => {}
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

    if let Some(_popup) = ui.begin_modal_popup("Confirm Delete") {
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
        .always_auto_resize(true)
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

                if let Some(_table) = ui.begin_table_with_flags(
                    "validation_table",
                    2,
                    imgui::TableFlags::RESIZABLE
                        | imgui::TableFlags::BORDERS
                        | imgui::TableFlags::SCROLL_Y
                        | imgui::TableFlags::SIZING_FIXED_FIT,
                ) {
                    ui.table_setup_column("Severity");
                    ui.table_setup_column("Message");
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
    let save_path = ui_state.dbc_windows[idx].file_path.clone();
    let dbc_string = ui_state.dbc_windows[idx].dbc.to_dbc_string();
    match std::fs::write(&save_path, &dbc_string) {
        Ok(_) => {
            ui_state.dbc_windows[idx].is_dirty = false;
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
                        let mut new_sig = sig.clone();
                        let new_name = format!("{}_copy", new_sig.name());
                        new_sig.set_name(&new_name);
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
