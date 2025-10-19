//! 菜单栏渲染模块

use crate::dbc::{DbcData, EditableDbcData, OverridesSnapshot};
use crate::ui::state::{DbcWindowState, UiState, UndoOperationKind};
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
                return Some(format!("{}/{}", file_part, &sw.message.message_name));
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

/// 渲染消息编辑相关菜单项
fn render_message_edit_menu_items(ui: &Ui, ui_state: &mut UiState) {
    // Add Message 菜单项
    if ui.menu_item("Add Message...") {
        if let Some(window) = ui_state.get_focused_dbc_window() {
            let parent_dbc_id = window.id;
            // 生成建议的消息ID
            let suggested_id = generate_suggested_message_id(window);
            ui_state
                .message_create_dialog
                .open(parent_dbc_id, suggested_id);
        }
    }
    if ui.is_item_hovered() {
        ui.tooltip_text("Create a new message");
    }

    ui.separator();

    let has_selected_message = ui_state
        .get_focused_dbc_window()
        .and_then(|w| w.selected_message_id)
        .is_some();

    // Copy Message 菜单项
    if ui
        .menu_item_config("Copy Message\tCtrl+C")
        .enabled(has_selected_message)
        .build()
    {
        if let Some(window) = ui_state.get_focused_dbc_window() {
            if let Some(message_id) = window.selected_message_id {
                handle_copy_message(ui_state, message_id);
            }
        }
    }
    if ui.is_item_hovered() {
        if has_selected_message {
            ui.tooltip_text("Copy the selected message to clipboard");
        } else {
            ui.tooltip_text("Select a message to copy it");
        }
    }

    // Paste Message 菜单项
    let has_clipboard = ui_state.has_clipboard_message();
    if ui
        .menu_item_config("Paste Message\tCtrl+V")
        .enabled(has_clipboard)
        .build()
    {
        handle_paste_message(ui_state);
    }
    if ui.is_item_hovered() {
        if has_clipboard {
            ui.tooltip_text("Paste message from clipboard");
        } else {
            ui.tooltip_text("No message in clipboard");
        }
    }

    ui.separator();

    if ui
        .menu_item_config("Delete Message")
        .enabled(has_selected_message)
        .build()
    {
        if let Some(window) = ui_state.get_focused_dbc_window() {
            if let Some(message_id) = window.selected_message_id {
                handle_delete_message(ui_state, message_id);
            }
        }
    }

    if ui.is_item_hovered() {
        if has_selected_message {
            ui.tooltip_text("Delete the selected message");
        } else {
            ui.tooltip_text("Select a message to delete it");
        }
    }
}

/// 处理删除消息请求
pub(crate) fn handle_delete_message(ui_state: &mut UiState, message_id: u32) {
    // 检查是否有打开的 Signal 窗口引用这个消息
    if ui_state
        .ensure_message_not_in_open_signal_windows(message_id)
        .is_err()
    {
        return;
    }

    if let Some(window) = ui_state.get_focused_dbc_window() {
        // 查找消息以获取其名称用于确认对话框
        let all_messages = window.editable_data.get_all_messages();
        let message_name = all_messages
            .iter()
            .find(|m| m.message_id() == message_id)
            .map(|m| {
                window
                    .editable_data
                    .get_message_name(message_id, m.message_name())
            })
            .unwrap_or_else(|| format!("Message 0x{:03X}", message_id));

        // 创建快照用于撤销
        let before_snapshot = OverridesSnapshot::from_editable(&window.editable_data);

        // 执行删除
        window.editable_data.delete_message(message_id);

        // 创建删除后快照
        let after_snapshot = OverridesSnapshot::from_editable(&window.editable_data);

        // 记录撤销操作
        window.push_undo(
            UndoOperationKind::DeleteMessage { message_id },
            &before_snapshot,
            &after_snapshot,
        );

        // 清除选择
        window.selected_message_id = None;

        println!("Deleted message: {} (0x{:03X})", message_name, message_id);
    }
}

/// 生成建议的消息ID（找到第一个未使用的ID）
fn generate_suggested_message_id(window: &DbcWindowState) -> u32 {
    let all_messages = window.editable_data.get_all_messages();
    let used_ids: std::collections::HashSet<u32> =
        all_messages.iter().map(|m| m.message_id()).collect();

    // 从0x100开始查找第一个未使用的ID
    let mut suggested_id = 0x100;
    while used_ids.contains(&suggested_id) {
        suggested_id += 1;
        if suggested_id > 0x7FF {
            // 如果超过标准CAN ID范围，从0开始重新查找
            suggested_id = 0;
            break;
        }
    }

    // 如果0x100-0x7FF都被占用，从0开始找
    if suggested_id == 0 {
        suggested_id = 1;
        while used_ids.contains(&suggested_id) {
            suggested_id += 1;
            if suggested_id > 0x1FFFFFFF {
                // 如果连扩展ID范围也满了，返回0（让用户手动输入）
                return 0;
            }
        }
    }

    suggested_id
}

/// 处理复制消息
fn handle_copy_message(ui_state: &mut UiState, message_id: u32) {
    // 先获取消息信息
    let message_to_copy = if let Some(window) = ui_state.get_focused_dbc_window() {
        let all_messages = window.editable_data.get_all_messages();
        all_messages
            .iter()
            .find(|m| m.message_id() == message_id)
            .map(|m| (m.to_custom_message(), m.message_name().to_string()))
    } else {
        None
    };

    // 再执行复制操作
    if let Some((custom_msg, name)) = message_to_copy {
        ui_state.clipboard.copied_message = Some(custom_msg);
        println!("Copied message: {} (0x{:03X})", name, message_id);
    }
}

/// 处理粘贴消息
fn handle_paste_message(ui_state: &mut UiState) {
    use crate::dbc::OverridesSnapshot;

    let Some(clipboard_message) = ui_state.clipboard.copied_message.clone() else {
        return;
    };

    let Some(window) = ui_state.get_focused_dbc_window() else {
        return;
    };

    // 生成新的ID（避免冲突）
    let new_id = generate_suggested_message_id(window);

    // 创建操作前快照
    let before_snapshot = OverridesSnapshot::from_editable(&window.editable_data);

    // 创建新消息（使用剪贴板的内容但ID不同）
    let mut new_message = clipboard_message.clone();
    new_message.message_id = new_id;
    new_message.message_name = format!("{}_Copy", new_message.message_name);

    // 添加消息
    window.editable_data.add_message(new_message.clone());

    // 创建操作后快照
    let after_snapshot = OverridesSnapshot::from_editable(&window.editable_data);

    // 记录撤销操作
    window.push_undo(
        UndoOperationKind::AddMessage { message_id: new_id },
        &before_snapshot,
        &after_snapshot,
    );

    // 选中新粘贴的消息
    window.selected_message_id = Some(new_id);

    println!(
        "Pasted message: {} (0x{:03X})",
        new_message.message_name, new_id
    );
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
                ui.separator();
                render_message_edit_menu_items(ui, ui_state);
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

    let Some(idx) = ui_state.last_focused_dbc_index else {
        return;
    };

    // 处理 Delete 键删除选中的消息
    if ui.is_key_pressed(Key::Delete) {
        if let Some(window) = ui_state.dbc_windows.get(idx) {
            if let Some(message_id) = window.selected_message_id {
                handle_delete_message(ui_state, message_id);
                return;
            }
        }
    }

    // 处理 Ctrl 组合键
    if !io.key_ctrl {
        return;
    }

    // Ctrl+C: 复制选中的消息
    if ui.is_key_pressed(Key::C) {
        if let Some(window) = ui_state.dbc_windows.get(idx) {
            if let Some(message_id) = window.selected_message_id {
                handle_copy_message(ui_state, message_id);
                return;
            }
        }
    }

    // Ctrl+V: 粘贴消息
    if ui.is_key_pressed(Key::V) {
        if ui_state.has_clipboard_message() {
            handle_paste_message(ui_state);
            return;
        }
    }

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
