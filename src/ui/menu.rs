//! 菜单栏渲染模块

use crate::dbc::{DbcData, EditableDbcData};
use crate::ui::state::{DbcWindowState, UiState};
use imgui::{Key, Ui};

/// 渲染主菜单栏
pub fn render_main_menu_bar(ui: &Ui, ui_state: &mut UiState) {
    ui.main_menu_bar(|| {
        render_context_label(ui, ui_state);
        ui.same_line();

        render_file_menu(ui, ui_state);
        render_edit_menu(ui, ui_state);
        render_view_menu(ui, ui_state);
        render_help_menu(ui, ui_state);
    });
}

/// 渲染菜单栏左侧的上下文标签
fn render_context_label(ui: &Ui, ui_state: &UiState) {
    if let Some(label) = get_context_label(ui_state) {
        ui.text(label);
    } else {
        ui.text("(No file)");
    }
}

/// 获取当前上下文标签（显示当前聚焦的文件/消息）
fn get_context_label(ui_state: &UiState) -> Option<String> {
    // 优先显示 Signal 窗口上下文
    if let Some(sig_idx) = ui_state.last_focused_signal_window {
        if let Some(sw) = ui_state.signal_windows.get(sig_idx) {
            if let Some(parent) = ui_state
                .dbc_windows
                .iter()
                .find(|w| w.id == sw.parent_dbc_id)
            {
                let file_part =
                    format_file_label(&parent.editable_data.base.file_path, "(message)");
                return Some(format!("{}/{}", file_part, sw.message.message_name()));
            }
        }
    }

    // 否则显示 DBC 窗口上下文
    if let Some(idx) = ui_state.last_focused_dbc_index {
        if let Some(win) = ui_state.dbc_windows.get(idx) {
            return Some(format_file_label(
                &win.editable_data.base.file_path,
                "(dbc)",
            ));
        }
    }

    None
}

/// 格式化文件标签
fn format_file_label(file_path: &str, prefix: &str) -> String {
    if file_path.is_empty() {
        "(No file)".to_string()
    } else {
        let path = std::path::Path::new(file_path);
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| format!("{} {}", prefix, s))
            .unwrap_or_else(|| format!("{} Unknown", prefix))
    }
}

/// 渲染文件菜单
fn render_file_menu(ui: &Ui, ui_state: &mut UiState) {
    ui.menu("File", || {
        if ui.menu_item("Load DBC File") {
            handle_load_dbc_file(ui_state);
        }
        ui.separator();
        if ui.menu_item("Exit") {
            std::process::exit(0);
        }
    });
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

    // 检查文件是否已经打开
    if let Some(existing_idx) = ui_state
        .dbc_windows
        .iter()
        .position(|w| w.editable_data.base.file_path == path_str)
    {
        focus_existing_dbc_window(ui_state, existing_idx);
    } else {
        load_new_dbc_file(ui_state, &path);
    }
}

/// 聚焦已存在的 DBC 窗口
fn focus_existing_dbc_window(ui_state: &mut UiState, window_index: usize) {
    if let Some(window) = ui_state.dbc_windows.get_mut(window_index) {
        window.is_open = true;
        ui_state.dbc_window_focus_request = Some(window.id);
    }
    ui_state.last_focused_dbc_index = Some(window_index);
    ui_state.last_focused_signal_window = None;
}

/// 加载新的 DBC 文件
fn load_new_dbc_file(ui_state: &mut UiState, path: &std::path::Path) {
    let mut dbc_data = DbcData::new();
    match dbc_data.load_dbc_file(path) {
        Ok(_) => {
            let editable_data = EditableDbcData::from_dbc_data(dbc_data);
            ui_state
                .dbc_windows
                .push(DbcWindowState::new(ui_state.next_dbc_id, editable_data));
            ui_state.next_dbc_id += 1;
        }
        Err(e) => {
            ui_state.error_dialog.message = format!("Failed to load DBC file: {}", e);
            ui_state.error_dialog.show = true;
        }
    }
}

/// 渲染编辑菜单
fn render_edit_menu(ui: &Ui, ui_state: &mut UiState) {
    ui.menu("Edit", || {
        if let Some(idx) = ui_state.last_focused_dbc_index {
            if let Some(win) = ui_state.dbc_windows.get_mut(idx) {
                render_undo_redo_menu_items(ui, win);
            } else {
                ui.text_disabled("No active DBC window");
            }
        } else {
            ui.text_disabled("No active DBC window");
        }
    });
}

/// 渲染撤销/重做菜单项
fn render_undo_redo_menu_items(ui: &Ui, window: &mut DbcWindowState) {
    let undo_label = if let Some(desc) = window.last_undo_description() {
        format!("Undo {}\tCtrl+Z", desc)
    } else {
        "Undo\tCtrl+Z".to_string()
    };

    let redo_label = if let Some(desc) = window.last_redo_description() {
        format!("Redo {}\tCtrl+Y", desc)
    } else {
        "Redo\tCtrl+Y".to_string()
    };

    if ui
        .menu_item_config(&undo_label)
        .enabled(window.can_undo())
        .build()
    {
        window.undo();
    }

    if ui
        .menu_item_config(&redo_label)
        .enabled(window.can_redo())
        .build()
    {
        window.redo();
    }
}

/// 渲染视图菜单
fn render_view_menu(ui: &Ui, ui_state: &mut UiState) {
    ui.menu("View", || {
        ui.checkbox("Performance Window", &mut ui_state.show_performance_window);
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

/// 处理全局快捷键（依赖 last_focused_dbc_index）
pub fn handle_global_shortcuts(ui: &Ui, ui_state: &mut UiState) {
    let io = ui.io();
    if !io.key_ctrl {
        return;
    }

    let Some(idx) = ui_state.last_focused_dbc_index else {
        return;
    };

    if let Some(win) = ui_state.dbc_windows.get_mut(idx) {
        let shift = io.key_shift;

        // 优先 Undo: Ctrl+Z
        if ui.is_key_pressed(Key::Z) && !shift {
            win.undo();
            return;
        }

        // Redo: Ctrl+Shift+Z 或 Ctrl+Y
        if (ui.is_key_pressed(Key::Z) && shift) || ui.is_key_pressed(Key::Y) {
            win.redo();
        }
    }
}
