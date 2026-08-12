//! 菜单栏渲染模块

use crate::ui::dbc_window::DbcWindow;
use crate::ui::state::UiState;
use imgui::Ui;

/// 渲染主菜单栏
pub fn render_main_menu_bar(ui: &Ui, ui_state: &mut UiState) {
    ui.main_menu_bar(|| {
        render_file_menu(ui, ui_state);
        render_edit_menu(ui, ui_state);
        render_view_menu(ui, ui_state);
        render_help_menu(ui, ui_state);
    });
}

/// 渲染文件菜单
fn render_file_menu(ui: &Ui, ui_state: &mut UiState) {
    ui.menu("File", || {
        if ui.menu_item("Load DBC File") {
            handle_load_dbc_file(ui_state);
        }
        if ui.menu_item("Close DBC") {
            handle_close_dbc(ui_state);
        }
        ui.separator();
        if ui.menu_item("Exit") {
            std::process::exit(0);
        }
    });
}

/// 渲染编辑菜单
fn render_edit_menu(ui: &Ui, ui_state: &mut UiState) {
    ui.menu("Edit", || {
        if let Some(idx) = ui_state.last_focused_dbc_index {
            if let Some(win) = ui_state.dbc_windows.get_mut(idx) {
                let can_undo = win.dbc.can_undo();

                if ui
                    .menu_item_config("Undo\tCtrl+Z")
                    .enabled(can_undo)
                    .build()
                {
                    if let Err(e) = win.dbc.undo() {
                        eprintln!("Undo failed: {}", e);
                    }
                }

                if ui.menu_item_config("Redo\tCtrl+Y").enabled(false).build() {}
            } else {
                ui.text_disabled("No active DBC window");
            }
        } else {
            ui.text_disabled("No active DBC window");
        }
    });
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

/// 处理关闭 DBC
fn handle_close_dbc(ui_state: &mut UiState) {
    if let Some(idx) = ui_state.last_focused_dbc_index {
        if let Some(window) = ui_state.dbc_windows.get_mut(idx) {
            window.is_open = false;
        }
        ui_state.dbc_windows.remove(idx);
        ui_state.last_focused_dbc_index = None;
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
    match DbcWindow::from_path(&path) {
        Ok(dbc_window) => {
            ui_state.dbc_windows.push(dbc_window);
            ui_state.last_focused_dbc_index = Some(ui_state.dbc_windows.len() - 1);
        }
        Err(e) => {
            println!("{}", e.as_str())
        }
    }
}
