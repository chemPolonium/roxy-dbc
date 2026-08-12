use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::editable_dbc::EditableDbc;
use crate::ui::message_edit_window::{MessageEditEvent, MessageEditWindowState};
use crate::ui::message_window::{MessageWindow, MessageWindowEvent};
use crate::ui::signal_edit_window::SignalEditDialog;
use crate::ui::state::UiState;
use crate::ui::tree_view::{TreeEvent, TreeState};
use imgui::{Condition, Ui};

/// DBC 窗口状态
#[derive(Clone, Default)]
pub struct DbcWindow {
    pub is_open: bool,
    pub file_path: String,
    pub dbc: EditableDbc,
    is_dirty: bool,

    tree_state: TreeState,
    pub message_windows: Vec<MessageWindow>,
    edit_windows: Vec<MessageEditWindowState>,
    signal_edit_dialog: SignalEditDialog,
}

impl DbcWindow {
    pub fn new(file_path: &str, dbc: EditableDbc) -> Self {
        Self {
            is_open: true,
            file_path: file_path.to_string(),
            dbc,
            is_dirty: false,
            tree_state: TreeState::default(),
            message_windows: Vec::new(),
            edit_windows: Vec::new(),
            signal_edit_dialog: SignalEditDialog::default(),
        }
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

    pub fn render_tree_pane(&mut self, ui: &Ui) {
        let file_name = Path::new(&self.file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.file_path)
            .to_string();

        let mut tree_event = TreeEvent::None;
        ui.child_window("tree_pane")
            .size([250.0, 0.0])
            .build(|| {
                tree_event = self.tree_state.render(ui, &self.dbc, &file_name);
            });

        ui.same_line();

        ui.child_window("mdi_area")
            .size([0.0, 0.0])
            .build(|| {});

        self.handle_tree_event(tree_event);
    }

    pub fn render_floating_windows(&mut self, ui: &Ui) {
        for msg_win in &mut self.message_windows {
            let win_event = msg_win.render(ui, &self.dbc);
            if let MessageWindowEvent::EditSignal(sig_name) = win_event {
                if let Some(msg) = self.dbc.get_message(msg_win.message_id) {
                    if let Some(sig) = msg.signals().iter().find(|s| s.name() == sig_name) {
                        self.signal_edit_dialog
                            .open_from_signal(msg.message_id(), sig);
                    }
                }
            }
        }
        self.message_windows.retain(|w| w.is_open);

        self.render_edit_windows(ui);
        self.render_signal_edit_dialog(ui);
    }

    fn handle_tree_event(&mut self, event: TreeEvent) {
        match event {
            TreeEvent::OpenMessage(msg_id) => {
                self.open_message_window(msg_id);
            }
            TreeEvent::EditSignal(msg_id, sig_name) => {
                if let Some(msg) = self.dbc.get_message(msg_id) {
                    if let Some(sig) = msg.signals().iter().find(|s| s.name() == sig_name) {
                        self.signal_edit_dialog.open_from_signal(msg_id, sig);
                    }
                }
            }
            _ => {}
        }
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

        let mut is_open = dbc_window.is_open;

        let window_ui = ui
            .window(&window_title)
            .size([900.0, 600.0], Condition::FirstUseEver)
            .opened(&mut is_open);

        window_ui.build(|| {
            dbc_window.render_tree_pane(ui);
        });

        if ui.is_window_focused() {
            ui_state.last_focused_dbc_index = Some(window_idx);
        }

        dbc_window.is_open = is_open;
    }

    for dbc_window in &mut ui_state.dbc_windows.iter_mut() {
        dbc_window.render_floating_windows(ui);
    }
}
