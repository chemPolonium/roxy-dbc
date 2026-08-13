//! 菜单栏渲染模块

use crate::editable_dbc::{EditableDbc, EditableMessage, FrameFormat};
use crate::ui::dbc_window::DbcWindow;
use crate::ui::state::{DeleteTarget, UiState};
use imgui::Ui;

/// 渲染主菜单栏
pub fn render_main_menu_bar(ui: &Ui, ui_state: &mut UiState) {
    ui.main_menu_bar(|| {
        render_file_menu(ui, ui_state);
        render_edit_menu(ui, ui_state);
        render_view_menu(ui, ui_state);
        render_tools_menu(ui, ui_state);
        render_help_menu(ui, ui_state);
    });
}

/// 渲染文件菜单
fn render_file_menu(ui: &Ui, ui_state: &mut UiState) {
    let ctrl = ui.io().key_ctrl;
    let shift = ui.io().key_shift;

    ui.menu("File", || {
        if ui
            .menu_item_config("New DBC")
            .shortcut("Ctrl+N")
            .build()
        {
            handle_new_dbc(ui_state);
        }
        ui.separator();
        if ui
            .menu_item_config("Load DBC File")
            .shortcut("Ctrl+O")
            .build()
        {
            handle_load_dbc_file(ui_state);
        }
        if ui.menu_item("Import...") {
            handle_import_file(ui_state);
        }
        if ui
            .menu_item_config("Export ARXML...")
            .enabled(ui_state.last_focused_dbc_index.is_some())
            .build()
        {
            handle_export_arxml(ui_state);
        }
        if !ui_state.recent_files.is_empty() {
            ui.menu("Recent Files", || {
                let recent: Vec<String> = ui_state.recent_files.clone();
                for path in &recent {
                    let file_name = std::path::Path::new(path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(path);
                    if ui.menu_item(file_name) {
                        handle_open_recent(ui_state, path);
                    }
                }
                ui.separator();
                if ui.menu_item("Clear Recent") {
                    ui_state.recent_files.clear();
                    ui_state.save_recent_files();
                }
            });
        }
        ui.separator();
        let has_dbc = !ui_state.dbc_windows.is_empty();
        if ui.menu_item_config("Save").shortcut("Ctrl+S").enabled(has_dbc).build() {
            handle_save_dbc(ui_state, false);
        }
        if ui.menu_item_config("Save As").shortcut("Ctrl+Shift+S").enabled(has_dbc).build() {
            handle_save_dbc(ui_state, true);
        }
        ui.separator();
        if ui.menu_item("Close DBC") {
            handle_close_dbc(ui_state);
        }
        ui.separator();
        if ui.menu_item("Exit") {
            std::process::exit(0);
        }
    });

    if ctrl && !ui_state.dbc_windows.is_empty() {
        if shift && ui.is_key_pressed_no_repeat(imgui::Key::S) {
            handle_save_dbc(ui_state, true);
        } else if ui.is_key_pressed_no_repeat(imgui::Key::S) {
            handle_save_dbc(ui_state, false);
        }
    }

    if ctrl && ui.is_key_pressed_no_repeat(imgui::Key::O) {
        handle_load_dbc_file(ui_state);
    }

    if ctrl && ui.is_key_pressed_no_repeat(imgui::Key::N) {
        handle_new_dbc(ui_state);
    }
}

/// 渲染编辑菜单
fn render_edit_menu(ui: &Ui, ui_state: &mut UiState) {
    let ctrl = ui.io().key_ctrl;

    ui.menu("Edit", || {
        if let Some(idx) = ui_state.last_focused_dbc_index {
            if let Some(win) = ui_state.dbc_windows.get_mut(idx) {
                let can_undo = win.dbc.can_undo();
                let can_redo = win.dbc.can_redo();

                if ui
                    .menu_item_config("Undo")
                    .shortcut("Ctrl+Z")
                    .enabled(can_undo)
                    .build()
                {
                    if let Err(e) = win.dbc.undo() {
                        eprintln!("Undo failed: {}", e);
                    }
                }

                if ui
                    .menu_item_config("Redo")
                    .shortcut("Ctrl+Y")
                    .enabled(can_redo)
                    .build()
                {
                    if let Err(e) = win.dbc.redo() {
                        eprintln!("Redo failed: {}", e);
                    }
                }

                ui.separator();

                let has_selection = !win.selected_message_ids().is_empty();
                let has_clipboard = ui_state.has_clipboard_message();

                if ui
                    .menu_item_config("Copy")
                    .shortcut("Ctrl+C")
                    .enabled(has_selection)
                    .build()
                {
                    edit_copy_message(ui_state, idx);
                }
                if ui
                    .menu_item_config("Cut")
                    .shortcut("Ctrl+X")
                    .enabled(has_selection)
                    .build()
                {
                    edit_cut_message(ui_state, idx);
                }
                if ui
                    .menu_item_config("Paste")
                    .shortcut("Ctrl+V")
                    .enabled(has_clipboard)
                    .build()
                {
                    edit_paste_message(ui_state, idx);
                }
                ui.separator();
                if ui
                    .menu_item_config("Delete")
                    .shortcut("Del")
                    .enabled(has_selection)
                    .build()
                {
                    edit_delete_message(ui_state, idx);
                }
                ui.separator();
                if ui.menu_item("Add Message") {
                    edit_add_message(ui_state, idx);
                }
            } else {
                ui.text_disabled("No active DBC window");
            }
        } else {
            ui.text_disabled("No active DBC window");
        }
    });

    if ctrl {
        if let Some(idx) = ui_state.last_focused_dbc_index {
            if ui_state.dbc_windows.get(idx).is_some() {
                if ui.is_key_pressed_no_repeat(imgui::Key::Z) {
                    if let Some(win) = ui_state.dbc_windows.get_mut(idx) {
                        if let Err(e) = win.dbc.undo() {
                            eprintln!("Undo failed: {}", e);
                        }
                    }
                }
                if ui.is_key_pressed_no_repeat(imgui::Key::Y) {
                    if let Some(win) = ui_state.dbc_windows.get_mut(idx) {
                        if let Err(e) = win.dbc.redo() {
                            eprintln!("Redo failed: {}", e);
                        }
                    }
                }
                if ui.is_key_pressed_no_repeat(imgui::Key::C) {
                    edit_copy_message(ui_state, idx);
                }
                if ui.is_key_pressed_no_repeat(imgui::Key::X) {
                    edit_cut_message(ui_state, idx);
                }
                if ui.is_key_pressed_no_repeat(imgui::Key::V) {
                    edit_paste_message(ui_state, idx);
                }
            }
        }
    }
}

fn edit_copy_message(ui_state: &mut UiState, idx: usize) {
    let ids = ui_state.dbc_windows[idx].selected_message_ids();
    let msgs: Vec<EditableMessage> = ids
        .iter()
        .filter_map(|id| ui_state.dbc_windows[idx].dbc.get_message(*id).cloned())
        .collect();
    if !msgs.is_empty() {
        ui_state.clipboard.copied_messages = msgs;
    }
}

fn edit_cut_message(ui_state: &mut UiState, idx: usize) {
    let ids = ui_state.dbc_windows[idx].selected_message_ids();
    if ids.is_empty() {
        return;
    }
    let msgs: Vec<EditableMessage> = ids
        .iter()
        .filter_map(|id| ui_state.dbc_windows[idx].dbc.get_message(*id).cloned())
        .collect();
    if !msgs.is_empty() {
        ui_state.clipboard.copied_messages = msgs;
    }
    ui_state.confirm_delete_dialog.target = Some(DeleteTarget::Messages(ids.clone()));
    ui_state.confirm_delete_dialog.display_name = if ids.len() == 1 {
        let name = ui_state.dbc_windows[idx]
            .dbc
            .get_message(ids[0])
            .map(|m| m.message_name().to_string())
            .unwrap_or_default();
        format!("message '{}'", name)
    } else {
        format!("{} messages", ids.len())
    };
    ui_state.confirm_delete_dialog.show = true;
}

fn edit_paste_message(ui_state: &mut UiState, idx: usize) {
    let copied = ui_state.clipboard.copied_messages.clone();
    if copied.is_empty() {
        return;
    }
    let win = &mut ui_state.dbc_windows[idx];
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

fn edit_delete_message(ui_state: &mut UiState, idx: usize) {
    let ids = ui_state.dbc_windows[idx].selected_message_ids();
    if ids.is_empty() {
        return;
    }
    ui_state.confirm_delete_dialog.target = Some(DeleteTarget::Messages(ids.clone()));
    ui_state.confirm_delete_dialog.display_name = if ids.len() == 1 {
        let name = ui_state.dbc_windows[idx]
            .dbc
            .get_message(ids[0])
            .map(|m| m.message_name().to_string())
            .unwrap_or_default();
        format!("message '{}'", name)
    } else {
        format!("{} messages", ids.len())
    };
    ui_state.confirm_delete_dialog.show = true;
}

fn edit_add_message(ui_state: &mut UiState, idx: usize) {
    let next_id = ui_state.generate_next_message_id(idx);
    let msg_count = if let Some(win) = ui_state.dbc_windows.get(idx) {
        win.dbc.messages().len()
    } else {
        return;
    };
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
    if let Some(win) = ui_state.dbc_windows.get_mut(idx) {
        win.dbc.add_message(&msg);
        win.set_selected_message_id(Some(next_id));
        win.is_dirty = true;
    }
}

/// 渲染视图菜单
fn render_view_menu(ui: &Ui, ui_state: &mut UiState) {
    ui.menu("View", || {
        ui.checkbox("Performance Window", &mut ui_state.show_performance_window);
    });
}

/// 渲染工具菜单
fn render_tools_menu(ui: &Ui, ui_state: &mut UiState) {
    let has_dbc = !ui_state.dbc_windows.is_empty();

    ui.menu("Tools", || {
        if ui.menu_item_config("Validate").enabled(has_dbc).build() {
            if let Some(idx) = ui_state.last_focused_dbc_index {
                if let Some(win) = ui_state.dbc_windows.get(idx) {
                    ui_state.validation_dialog.issues = win.dbc.validate();
                    ui_state.validation_dialog.show = true;
                }
            }
        }
        if ui
            .menu_item_config("Nodes")
            .enabled(ui_state.last_focused_dbc_index.is_some())
            .build()
        {
            ui_state.node_dialog.show = true;
        }
    });
}

/// 渲染帮助菜单
fn render_help_menu(ui: &Ui, ui_state: &mut UiState) {
    ui.menu("Help", || {
        if ui.menu_item("About") {
            ui_state.show_about_dialog = true;
        }
    });
}

/// 处理新建 DBC 文件
fn handle_new_dbc(ui_state: &mut UiState) {
    let editable_dbc = EditableDbc::new();
    let dbc_window = DbcWindow::new("Untitled.dbc", editable_dbc);
    ui_state.dbc_windows.push(dbc_window);
    ui_state.last_focused_dbc_index = Some(ui_state.dbc_windows.len() - 1);
}

/// 处理加载 DBC 文件
fn handle_load_dbc_file(ui_state: &mut UiState) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("DBC files", &["dbc"])
        .pick_file()
    else {
        return;
    };

    let path_str = path.to_string_lossy().to_string();

    if let Some(existing_idx) = ui_state
        .dbc_windows
        .iter()
        .position(|w| w.file_path == path_str)
    {
        focus_existing_dbc_window(ui_state, existing_idx);
    } else {
        load_new_dbc_file(ui_state, &path);
    }
}

/// 处理导入 ARXML/KCD 文件
fn handle_import_file(ui_state: &mut UiState) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("ARXML/KCD files", &["arxml", "kcd"])
        .pick_file()
    else {
        return;
    };

    let path_str = path.to_string_lossy().to_string();
    match crate::import::import_file(&path) {
        Ok(editable_dbc) => {
            let dbc_window = DbcWindow::new(&path_str, editable_dbc);
            ui_state.add_recent_file(&path_str);
            ui_state.dbc_windows.push(dbc_window);
            ui_state.last_focused_dbc_index = Some(ui_state.dbc_windows.len() - 1);
        }
        Err(e) => {
            ui_state.error_dialog.message = format!("Import failed: {}", e);
            ui_state.error_dialog.show = true;
        }
    }
}

fn handle_export_arxml(ui_state: &mut UiState) {
    let idx = match ui_state.last_focused_dbc_index {
        Some(i) => i,
        None => return,
    };

    let Some(path) = rfd::FileDialog::new()
        .add_filter("ARXML files", &["arxml"])
        .set_file_name("export.arxml")
        .save_file()
    else {
        return;
    };

    let xml = crate::export::arxml::export_arxml(&ui_state.dbc_windows[idx].dbc);
    if let Err(e) = std::fs::write(&path, xml) {
        ui_state.error_dialog.message = format!("Export failed: {}", e);
        ui_state.error_dialog.show = true;
    }
}

/// 处理关闭 DBC
fn handle_close_dbc(ui_state: &mut UiState) {
    if let Some(idx) = ui_state.last_focused_dbc_index {
        if ui_state.dbc_windows[idx].is_dirty {
            ui_state.close_confirm_dialog.show = true;
            ui_state.close_confirm_dialog.dbc_window_index = Some(idx);
        } else {
            ui_state.dbc_windows.remove(idx);
            ui_state.last_focused_dbc_index = None;
        }
    }
}

/// 聚焦已存在的 DBC 窗口
fn focus_existing_dbc_window(ui_state: &mut UiState, window_index: usize) {
    if let Some(window) = ui_state.dbc_windows.get_mut(window_index) {
        window.is_open = true;
        ui_state.dbc_window_focus_request = Some(window_index);
    }
    ui_state.last_focused_dbc_index = Some(window_index);
    ui_state.last_focused_message_window = None;
}

/// 加载新的 DBC 文件
fn load_new_dbc_file(ui_state: &mut UiState, path: &std::path::Path) {
    let path_str = path.to_string_lossy().to_string();
    match DbcWindow::from_path(&path) {
        Ok(dbc_window) => {
            ui_state.add_recent_file(&path_str);
            ui_state.dbc_windows.push(dbc_window);
            ui_state.last_focused_dbc_index = Some(ui_state.dbc_windows.len() - 1);
        }
        Err(e) => {
            println!("{}", e.as_str())
        }
    }
}

fn handle_open_recent(ui_state: &mut UiState, path: &str) {
    let path_buf = std::path::PathBuf::from(path);
    if !path_buf.exists() {
        ui_state.error_dialog.message = format!("File not found: {}", path);
        ui_state.error_dialog.show = true;
        return;
    }

    if let Some(existing_idx) = ui_state
        .dbc_windows
        .iter()
        .position(|w| w.file_path == path)
    {
        focus_existing_dbc_window(ui_state, existing_idx);
    } else {
        load_new_dbc_file(ui_state, &path_buf);
    }
}

fn handle_save_dbc(ui_state: &mut UiState, save_as: bool) {
    let idx = match ui_state.last_focused_dbc_index {
        Some(i) => i,
        None => return,
    };

    let file_path = &ui_state.dbc_windows[idx].file_path;
    let needs_dialog = save_as || !std::path::Path::new(file_path).exists();

    let save_path = if needs_dialog {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("DBC files", &["dbc"])
            .set_file_name("output.dbc")
            .save_file()
        else {
            return;
        };
        path.to_string_lossy().to_string()
    } else {
        ui_state.dbc_windows[idx].file_path.clone()
    };

    let dbc_string = ui_state.dbc_windows[idx].dbc.to_dbc_string();
    match std::fs::write(&save_path, &dbc_string) {
        Ok(_) => {
            ui_state.dbc_windows[idx].file_path = save_path;
            ui_state.dbc_windows[idx].is_dirty = false;
        }
        Err(e) => {
            ui_state.error_dialog.message = format!("Failed to save: {}", e);
            ui_state.error_dialog.show = true;
        }
    }
}
