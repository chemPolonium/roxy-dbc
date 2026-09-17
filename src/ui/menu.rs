//! 菜单栏渲染模块

use crate::editable_dbc::{EditableDbc, EditableMessage};
use crate::ui::dbc_window::DbcWindow;
use crate::ui::state::{DeleteTarget, UiState};
use dear_imgui_rs::{Key, Ui};

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
    let ctrl = ui.io().key_ctrl();
    let shift = ui.io().key_shift();

    ui.menu("File", || {
        if ui.menu_item_with_shortcut("New DBC", "Ctrl+N") {
            handle_new_dbc(ui_state);
        }
        ui.separator();
        if ui.menu_item_with_shortcut("Load DBC File", "Ctrl+O") {
            handle_load_dbc_file(ui_state);
        }
        if ui.menu_item("Import...") {
            handle_import_file(ui_state);
        }
        if ui.menu_item_enabled_selected_no_shortcut(
            "Export ARXML...",
            false,
            ui_state.last_focused_dbc_index.is_some(),
        ) {
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
                    // label 里带完整路径做 ID：不同目录的同名文件不冲突
                    if ui.menu_item(format!("{}##{}", file_name, path)) {
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
        if ui.menu_item_enabled_selected_with_shortcut("Save", "Ctrl+S", false, has_dbc) {
            handle_save_dbc(ui_state, false);
        }
        if ui.menu_item_enabled_selected_with_shortcut("Save As", "Ctrl+Shift+S", false, has_dbc) {
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

    // 文本输入框激活时跳过全局快捷键
    if ctrl && !ui.io().want_capture_keyboard() {
        if !ui_state.dbc_windows.is_empty() {
            if shift && ui.is_key_pressed_with_repeat(Key::S, false) {
                handle_save_dbc(ui_state, true);
            } else if ui.is_key_pressed_with_repeat(Key::S, false) {
                handle_save_dbc(ui_state, false);
            }
        }

        if ui.is_key_pressed_with_repeat(Key::O, false) {
            handle_load_dbc_file(ui_state);
        }

        if ui.is_key_pressed_with_repeat(Key::N, false) {
            handle_new_dbc(ui_state);
        }
    }
}

/// 渲染编辑菜单
fn render_edit_menu(ui: &Ui, ui_state: &mut UiState) {
    let ctrl = ui.io().key_ctrl();

    ui.menu("Edit", || {
        if let Some(idx) = ui_state.last_focused_dbc_index {
            if let Some(win) = ui_state.dbc_windows.get_mut(idx) {
                let can_undo = win.dbc.can_undo();
                let can_redo = win.dbc.can_redo();

                if ui.menu_item_enabled_selected_with_shortcut("Undo", "Ctrl+Z", false, can_undo) {
                    if let Err(e) = win.dbc.undo() {
                        eprintln!("Undo failed: {}", e);
                    }
                }

                if ui.menu_item_enabled_selected_with_shortcut("Redo", "Ctrl+Y", false, can_redo) {
                    if let Err(e) = win.dbc.redo() {
                        eprintln!("Redo failed: {}", e);
                    }
                }

                ui.separator();

                let has_selection = !win.selected_message_ids().is_empty();
                let has_clipboard = ui_state.has_clipboard_message();

                if ui.menu_item_enabled_selected_with_shortcut("Copy", "Ctrl+C", false, has_selection) {
                    edit_copy_message(ui_state, idx);
                }
                if ui.menu_item_enabled_selected_with_shortcut("Cut", "Ctrl+X", false, has_selection) {
                    edit_cut_message(ui_state, idx);
                }
                if ui.menu_item_enabled_selected_with_shortcut("Paste", "Ctrl+V", false, has_clipboard) {
                    edit_paste_message(ui_state, idx);
                }
                ui.separator();
                if ui.menu_item_enabled_selected_with_shortcut("Delete", "Del", false, has_selection) {
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

    // 文本输入框激活时跳过全局快捷键，避免编辑属性时误触发消息级操作
    if ctrl && !ui.io().want_capture_keyboard() {
        if let Some(idx) = ui_state.last_focused_dbc_index {
            if ui_state.dbc_windows.get(idx).is_some() {
                if ui.is_key_pressed_with_repeat(Key::Z, false) {
                    if let Some(win) = ui_state.dbc_windows.get_mut(idx) {
                        if let Err(e) = win.dbc.undo() {
                            eprintln!("Undo failed: {}", e);
                        }
                    }
                }
                if ui.is_key_pressed_with_repeat(Key::Y, false) {
                    if let Some(win) = ui_state.dbc_windows.get_mut(idx) {
                        if let Err(e) = win.dbc.redo() {
                            eprintln!("Redo failed: {}", e);
                        }
                    }
                }
                if ui.is_key_pressed_with_repeat(Key::C, false) {
                    edit_copy_message(ui_state, idx);
                }
                if ui.is_key_pressed_with_repeat(Key::X, false) {
                    edit_cut_message(ui_state, idx);
                }
                if ui.is_key_pressed_with_repeat(Key::V, false) {
                    edit_paste_message(ui_state, idx);
                }
            }
        }
    }

    // Del 删除选中的消息（无需 Ctrl）；跳过文本输入焦点和待确认的删除对话框
    if !ui.io().want_capture_keyboard() && ui_state.confirm_delete_dialog.target.is_none() {
        if let Some(idx) = ui_state.last_focused_dbc_index {
            if ui_state.dbc_windows.get(idx).is_some()
                && ui.is_key_pressed_with_repeat(Key::Delete, false)
            {
                edit_delete_message(ui_state, idx);
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
    crate::ui::dbc_window::paste_messages(ui_state, idx);
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
    crate::ui::dbc_window::add_new_message(ui_state, idx);
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
        if ui.menu_item_enabled_selected_no_shortcut("Validate", false, has_dbc) {
            if let Some(idx) = ui_state.last_focused_dbc_index {
                if let Some(win) = ui_state.dbc_windows.get(idx) {
                    ui_state.validation_dialog.issues = win.dbc.validate();
                    ui_state.validation_dialog.show = true;
                }
            }
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

    let path_str = crate::ui::state::normalize_path(&path.to_string_lossy());

    if let Some(existing_idx) = ui_state
        .dbc_windows
        .iter()
        .position(|w| w.file_path == path_str)
    {
        focus_existing_dbc_window(ui_state, existing_idx);
    } else {
        load_new_dbc_file(ui_state, std::path::Path::new(&path_str));
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

    let path_str = crate::ui::state::normalize_path(&path.to_string_lossy());
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
}

/// 加载新的 DBC 文件
fn load_new_dbc_file(ui_state: &mut UiState, path: &std::path::Path) {
    let path_str = path.to_string_lossy().to_string();
    match DbcWindow::from_path(path) {
        Ok(dbc_window) => {
            ui_state.add_recent_file(&path_str);
            ui_state.dbc_windows.push(dbc_window);
            ui_state.last_focused_dbc_index = Some(ui_state.dbc_windows.len() - 1);
        }
        Err(e) => {
            // 打开失败时用错误对话框提示（release 构建无控制台）
            ui_state.error_dialog.message = e;
            ui_state.error_dialog.show = true;
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
    // 新路径（另存为）默认 UTF-8；原路径按原编码写出
    let win = &mut ui_state.dbc_windows[idx];
    if save_path != win.file_path {
        win.text_encoding = encoding_rs::UTF_8;
        win.had_bom = false;
    }
    let bytes = crate::file_encoding::encode_to_bytes(&dbc_string, win.text_encoding, win.had_bom);
    match std::fs::write(&save_path, &bytes) {
        Ok(_) => {
            win.file_path = crate::ui::state::normalize_path(&save_path);
            win.is_dirty = false;
        }
        Err(e) => {
            ui_state.error_dialog.message = format!("Failed to save: {}", e);
            ui_state.error_dialog.show = true;
        }
    }
}
