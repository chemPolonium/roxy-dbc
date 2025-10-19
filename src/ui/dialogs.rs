//! 对话框渲染模块

use crate::dbc::OverridesSnapshot;
use crate::ui::state::{ErrorDialog, UiState, UndoOperationKind};
use imgui::Ui;

/// 渲染所有对话框
pub fn render_dialogs(ui: &Ui, ui_state: &mut UiState) {
    if ui_state.error_dialog.show {
        render_error_dialog(ui, &mut ui_state.error_dialog);
    }

    if ui_state.show_about_dialog {
        render_about_dialog(ui, &mut ui_state.show_about_dialog);
    }

    if ui_state.message_edit_dialog.show {
        render_message_edit_dialog(ui, ui_state);
    }

    if ui_state.message_create_dialog.show {
        render_message_create_dialog(ui, ui_state);
    }
}

/// 渲染错误对话框
pub fn render_error_dialog(ui: &Ui, error_dialog: &mut ErrorDialog) {
    if error_dialog.show {
        ui.open_popup("Error");
    }

    ui.modal_popup_config("Error").resizable(false).build(|| {
        ui.text("Error");
        ui.separator();

        // 使用固定大小的滚动区域显示错误消息
        ui.child_window("error_message")
            .size([400.0, 100.0])
            .build(|| {
                ui.text_wrapped(&error_dialog.message);
            });

        ui.separator();

        if ui.button("OK") {
            ui.close_current_popup();
            error_dialog.show = false;
        }
    });
}

/// 渲染关于对话框
pub fn render_about_dialog(ui: &Ui, show_about: &mut bool) {
    if *show_about {
        ui.open_popup("About");
    }

    ui.modal_popup_config("About").resizable(false).build(|| {
        ui.text("Roxy DBC Viewer");
        ui.separator();
        ui.text("Version: 0.5.0");
        ui.text("Built with Rust and ImGui");
        ui.separator();
        ui.text("A modern application for viewing and editing CAN DBC files.");

        ui.separator();
        if ui.button("Close") {
            ui.close_current_popup();
            *show_about = false;
        }
    });
}

/// 渲染 Message 编辑对话框
pub fn render_message_edit_dialog(ui: &Ui, ui_state: &mut UiState) {
    if !ui_state.message_edit_dialog.show {
        return;
    }

    // 用于记录按钮点击
    let mut should_apply = false;
    let mut should_cancel = false;
    let mut should_ok = false;

    // 提取需要的值以避免借用冲突
    let message_id = ui_state.message_edit_dialog.message_id;
    let has_changes = ui_state.message_edit_dialog.has_changes();

    ui.window("Edit Message")
        .size([600.0, 550.0], imgui::Condition::FirstUseEver)
        .position([100.0, 100.0], imgui::Condition::FirstUseEver)
        .opened(&mut ui_state.message_edit_dialog.show)
        .build(|| {
            ui.text(format!("Editing Message 0x{:03X}", message_id));
            ui.separator();

            // Message ID
            ui.text("Message ID:");
            ui.set_next_item_width(-1.0);
            ui.input_text("##message_id", &mut ui_state.message_edit_dialog.id_buffer)
                .build();
            if ui.is_item_hovered() {
                ui.tooltip_text("Format: 0x123 or 123 (hex or decimal)");
            }

            ui.spacing();

            // Message Name
            ui.text("Message Name:");
            ui.set_next_item_width(-1.0);
            ui.input_text(
                "##message_name",
                &mut ui_state.message_edit_dialog.name_buffer,
            )
            .build();

            ui.spacing();

            // Message Size
            ui.text("Message Size (bytes):");
            ui.set_next_item_width(-1.0);
            ui.input_text(
                "##message_size",
                &mut ui_state.message_edit_dialog.size_buffer,
            )
            .build();
            if ui.is_item_hovered() {
                ui.tooltip_text("Valid range: 0-8 bytes");
            }

            ui.spacing();

            // Transmitter
            ui.text("Transmitter:");
            ui.set_next_item_width(-1.0);
            ui.input_text(
                "##message_transmitter",
                &mut ui_state.message_edit_dialog.transmitter_buffer,
            )
            .build();
            if ui.is_item_hovered() {
                ui.tooltip_text("ECU/Node name that sends this message");
            }

            ui.spacing();

            // Comment
            ui.text("Comment:");
            ui.set_next_item_width(-1.0);
            ui.input_text_multiline(
                "##message_comment",
                &mut ui_state.message_edit_dialog.comment_buffer,
                [0.0, 80.0],
            )
            .build();

            ui.spacing();
            ui.separator();

            // 提示信息
            if has_changes {
                ui.text_colored([1.0, 0.7, 0.0, 1.0], "You have unsaved changes");
            } else {
                ui.text_colored([0.6, 0.6, 0.6, 1.0], "No changes");
            }
            ui.text_colored([0.6, 0.6, 0.6, 1.0], "Use Ctrl+Z/Ctrl+Y to undo/redo");

            ui.spacing();

            // 按钮布局
            // OK 按钮（应用并关闭）
            if ui.button_with_size("OK", [80.0, 0.0]) {
                should_ok = true;
            }
            if ui.is_item_hovered() {
                ui.tooltip_text("Apply changes and close");
            }

            ui.same_line();

            // Cancel 按钮（取消并关闭）
            if ui.button_with_size("Cancel", [80.0, 0.0]) {
                should_cancel = true;
            }
            if ui.is_item_hovered() {
                ui.tooltip_text("Discard changes and close");
            }

            ui.same_line();

            // Apply 按钮（应用但不关闭）
            let apply_enabled = has_changes;
            if !apply_enabled {
                ui.disabled(true, || {
                    ui.button_with_size("Apply", [80.0, 0.0]);
                });
            } else if ui.button_with_size("Apply", [80.0, 0.0]) {
                should_apply = true;
            }
            if ui.is_item_hovered() {
                if apply_enabled {
                    ui.tooltip_text("Apply changes without closing");
                } else {
                    ui.tooltip_text("No changes to apply");
                }
            }
        });

    // 处理按钮点击
    if should_ok {
        // 应用修改并关闭
        apply_changes(ui_state);
        ui_state.message_edit_dialog.close();
    } else if should_cancel {
        // 取消修改并关闭
        ui_state.message_edit_dialog.reset_to_original();
        ui_state.message_edit_dialog.close();
    } else if should_apply {
        // 只应用修改，不关闭
        apply_changes(ui_state);
    }
}

/// 应用编辑对话框中的修改
fn apply_changes(ui_state: &mut UiState) {
    // 提前提取所有需要的值，避免借用冲突
    let parent_dbc_id = ui_state.message_edit_dialog.parent_dbc_id;
    let message_id = ui_state.message_edit_dialog.message_id;
    let new_name = ui_state.message_edit_dialog.name_buffer.trim().to_string();
    let old_name = ui_state.message_edit_dialog.original_name.clone();
    let new_comment = ui_state
        .message_edit_dialog
        .comment_buffer
        .trim()
        .to_string();
    let old_comment = ui_state.message_edit_dialog.original_comment.clone();

    let new_id_str = ui_state.message_edit_dialog.id_buffer.trim();
    let old_id = ui_state.message_edit_dialog.original_id;

    let new_size_str = ui_state.message_edit_dialog.size_buffer.trim();
    let old_size = ui_state.message_edit_dialog.original_size;

    let new_transmitter = ui_state
        .message_edit_dialog
        .transmitter_buffer
        .trim()
        .to_string();
    let old_transmitter = ui_state.message_edit_dialog.original_transmitter.clone();

    // 解析 ID（支持 0x123 或 123 格式）
    let new_id = parse_message_id(new_id_str);

    // 解析 Size
    let new_size = new_size_str.parse::<u64>().ok();

    // 检查是否有任何变化
    let name_changed = new_name != old_name && !new_name.is_empty();
    let comment_changed = new_comment != old_comment;
    let id_changed = new_id.is_some() && new_id != Some(old_id);
    let size_changed = new_size.is_some() && new_size != Some(old_size) && new_size.unwrap() <= 8;
    let transmitter_changed = new_transmitter != old_transmitter;

    if !name_changed && !comment_changed && !id_changed && !size_changed && !transmitter_changed {
        return;
    }

    // 查找对应的 DBC 窗口
    if let Some(dbc_window) = ui_state
        .dbc_windows
        .iter_mut()
        .find(|w| w.id == parent_dbc_id)
    {
        // 记录 undo（在修改之前创建快照）
        let before = OverridesSnapshot::from_editable(&dbc_window.editable_data);

        // 应用名称修改
        if name_changed {
            dbc_window
                .editable_data
                .set_message_name(message_id, new_name.clone());
        }

        // 应用注释修改
        if comment_changed {
            dbc_window
                .editable_data
                .set_message_comment(message_id, new_comment.clone());
        }

        // 应用 ID 修改
        if id_changed {
            let new_id_value = new_id.unwrap();
            dbc_window
                .editable_data
                .set_message_id(message_id, new_id_value);
        }

        // 应用 Size 修改
        if size_changed {
            let new_size_value = new_size.unwrap();
            dbc_window
                .editable_data
                .set_message_size(message_id, new_size_value);
        }

        // 应用 Transmitter 修改
        if transmitter_changed {
            dbc_window
                .editable_data
                .set_message_transmitter(message_id, new_transmitter.clone());
        }

        let after = OverridesSnapshot::from_editable(&dbc_window.editable_data);

        // 根据修改类型选择 Undo 操作类型（简化：使用第一个修改的类型）
        let undo_op = if name_changed {
            UndoOperationKind::RenameMessage {
                message_id,
                old_name: old_name.clone(),
                new_name: new_name.clone(),
            }
        } else if comment_changed {
            UndoOperationKind::ModifyMessageComment {
                message_id,
                old_comment: old_comment.clone(),
                new_comment: new_comment.clone(),
            }
        } else if id_changed {
            UndoOperationKind::ModifyMessageId {
                original_message_id: message_id,
                old_id,
                new_id: new_id.unwrap(),
            }
        } else if size_changed {
            UndoOperationKind::ModifyMessageSize {
                message_id,
                old_size,
                new_size: new_size.unwrap(),
            }
        } else {
            UndoOperationKind::ModifyMessageTransmitter {
                message_id,
                old_transmitter: old_transmitter.clone(),
                new_transmitter: new_transmitter.clone(),
            }
        };

        dbc_window.push_undo(undo_op, &before, &after);
    }

    // 更新对话框的原始值，以便继续编辑（在窗口查找之外）
    ui_state.message_edit_dialog.original_name = new_name;
    ui_state.message_edit_dialog.original_comment = new_comment;
    if let Some(id) = new_id {
        ui_state.message_edit_dialog.original_id = id;
        ui_state.message_edit_dialog.id_buffer = format!("0x{:X}", id);
    }
    if let Some(size) = new_size {
        if size <= 8 {
            ui_state.message_edit_dialog.original_size = size;
            ui_state.message_edit_dialog.size_buffer = size.to_string();
        }
    }
    ui_state.message_edit_dialog.original_transmitter = new_transmitter;
}

/// 解析 Message ID（支持 0x123 或 123 格式）
fn parse_message_id(s: &str) -> Option<u32> {
    let s = s.trim();

    if s.is_empty() {
        return None;
    }

    // 尝试解析十六进制（0x 或 0X 前缀）
    if s.starts_with("0x") || s.starts_with("0X") {
        if let Ok(id) = u32::from_str_radix(&s[2..], 16) {
            return Some(id);
        }
    }

    // 尝试解析十进制
    if let Ok(id) = s.parse::<u32>() {
        return Some(id);
    }

    // 尝试直接解析为十六进制（没有 0x 前缀）
    if let Ok(id) = u32::from_str_radix(s, 16) {
        return Some(id);
    }

    None
}

/// 渲染 Message 创建对话框
pub fn render_message_create_dialog(ui: &Ui, ui_state: &mut UiState) {
    if !ui_state.message_create_dialog.show {
        return;
    }

    // 用于记录按钮点击
    let mut should_create = false;
    let mut should_cancel = false;

    // 提前提取需要的值以避免借用冲突
    let parent_dbc_id = ui_state.message_create_dialog.parent_dbc_id;
    let mut show_dialog = ui_state.message_create_dialog.show;

    // 检查输入是否有效
    let is_valid = ui_state.message_create_dialog.is_valid();

    // 检查ID冲突
    let id_conflict = if let Some(id) = ui_state.message_create_dialog.parse_id() {
        ui_state
            .dbc_windows
            .iter()
            .find(|w| w.id == parent_dbc_id)
            .map(|window| {
                let all_messages = window.editable_data.get_all_messages();
                all_messages.iter().any(|m| m.message_id() == id)
            })
            .unwrap_or(false)
    } else {
        false
    };

    let name_empty = ui_state.message_create_dialog.name_buffer.trim().is_empty();
    let size_invalid = ui_state.message_create_dialog.parse_size().is_none();

    ui.window("Create New Message")
        .size([600.0, 550.0], imgui::Condition::FirstUseEver)
        .position([150.0, 150.0], imgui::Condition::FirstUseEver)
        .opened(&mut show_dialog)
        .build(|| {
            ui.text("Create a new message in the DBC file");
            ui.separator();

            // Message ID
            ui.text("Message ID:");
            ui.set_next_item_width(-1.0);
            ui.input_text(
                "##new_message_id",
                &mut ui_state.message_create_dialog.id_buffer,
            )
            .build();
            if ui.is_item_hovered() {
                ui.tooltip_text("Format: 0x123 or 123 (hex or decimal)");
            }

            // 显示ID冲突警告
            if id_conflict {
                ui.text_colored([1.0, 0.0, 0.0, 1.0], "Warning: Message ID already exists!");
            }

            ui.spacing();

            // Message Name
            ui.text("Message Name:*");
            ui.set_next_item_width(-1.0);
            ui.input_text(
                "##new_message_name",
                &mut ui_state.message_create_dialog.name_buffer,
            )
            .build();
            if name_empty {
                ui.text_colored([1.0, 0.0, 0.0, 1.0], "Name is required");
            }

            ui.spacing();

            // Message Size
            ui.text("Message Size (bytes):*");
            ui.set_next_item_width(-1.0);
            ui.input_text(
                "##new_message_size",
                &mut ui_state.message_create_dialog.size_buffer,
            )
            .build();
            if ui.is_item_hovered() {
                ui.tooltip_text("Valid range: 0-8 bytes");
            }
            if size_invalid {
                ui.text_colored([1.0, 0.0, 0.0, 1.0], "Invalid size (must be 0-8)");
            }

            ui.spacing();

            // Transmitter
            ui.text("Transmitter:");
            ui.set_next_item_width(-1.0);
            ui.input_text(
                "##new_message_transmitter",
                &mut ui_state.message_create_dialog.transmitter_buffer,
            )
            .build();
            if ui.is_item_hovered() {
                ui.tooltip_text("ECU/Node name that sends this message (optional)");
            }

            ui.spacing();

            // Comment
            ui.text("Comment:");
            ui.set_next_item_width(-1.0);
            ui.input_text_multiline(
                "##new_message_comment",
                &mut ui_state.message_create_dialog.comment_buffer,
                [0.0, 80.0],
            )
            .build();

            ui.spacing();
            ui.separator();

            // 提示信息
            ui.text_colored([0.6, 0.6, 0.6, 1.0], "* Required fields");
            ui.spacing();

            // 按钮布局
            // Create 按钮
            if !is_valid {
                ui.disabled(true, || {
                    ui.button_with_size("Create", [80.0, 0.0]);
                });
            } else if ui.button_with_size("Create", [80.0, 0.0]) {
                should_create = true;
            }
            if ui.is_item_hovered() {
                if is_valid {
                    ui.tooltip_text("Create the new message");
                } else {
                    ui.tooltip_text("Fill in all required fields");
                }
            }

            ui.same_line();

            // Cancel 按钮
            if ui.button_with_size("Cancel", [80.0, 0.0]) {
                should_cancel = true;
            }
            if ui.is_item_hovered() {
                ui.tooltip_text("Cancel and close");
            }
        });

    // 同步对话框显示状态
    ui_state.message_create_dialog.show = show_dialog;

    // 处理按钮点击
    if should_create {
        handle_create_message(ui_state);
        ui_state.message_create_dialog.close();
    } else if should_cancel {
        ui_state.message_create_dialog.close();
    }
}

/// 处理创建新消息
fn handle_create_message(ui_state: &mut UiState) {
    use crate::dbc::CustomMessage;

    let parent_dbc_id = ui_state.message_create_dialog.parent_dbc_id;

    // 解析输入
    let Some(message_id) = ui_state.message_create_dialog.parse_id() else {
        ui_state.error_dialog.message = "Invalid message ID".to_string();
        ui_state.error_dialog.show = true;
        return;
    };

    let Some(size) = ui_state.message_create_dialog.parse_size() else {
        ui_state.error_dialog.message = "Invalid message size (must be 0-8 bytes)".to_string();
        ui_state.error_dialog.show = true;
        return;
    };

    let name = ui_state
        .message_create_dialog
        .name_buffer
        .trim()
        .to_string();
    if name.is_empty() {
        ui_state.error_dialog.message = "Message name cannot be empty".to_string();
        ui_state.error_dialog.show = true;
        return;
    }

    let transmitter = ui_state
        .message_create_dialog
        .transmitter_buffer
        .trim()
        .to_string();
    let comment = ui_state
        .message_create_dialog
        .comment_buffer
        .trim()
        .to_string();

    // 查找对应的 DBC 窗口
    if let Some(dbc_window) = ui_state
        .dbc_windows
        .iter_mut()
        .find(|w| w.id == parent_dbc_id)
    {
        // 检查 ID 是否已存在
        let all_messages = dbc_window.editable_data.get_all_messages();
        if all_messages.iter().any(|m| m.message_id() == message_id) {
            ui_state.error_dialog.message = format!(
                "Message ID 0x{:03X} already exists. Please choose a different ID.",
                message_id
            );
            ui_state.error_dialog.show = true;
            return;
        }

        // 创建操作前快照
        let before_snapshot = OverridesSnapshot::from_editable(&dbc_window.editable_data);

        // 创建新消息
        let new_message = CustomMessage {
            message_id,
            message_name: name.clone(),
            message_size: size,
            transmitter,
            comment,
            signals: Vec::new(),
        };

        // 添加消息
        dbc_window.editable_data.add_message(new_message);

        // 创建操作后快照
        let after_snapshot = OverridesSnapshot::from_editable(&dbc_window.editable_data);

        // 记录撤销操作
        dbc_window.push_undo(
            UndoOperationKind::AddMessage { message_id },
            &before_snapshot,
            &after_snapshot,
        );

        // 选中新创建的消息
        dbc_window.selected_message_id = Some(message_id);

        println!("Created new message: {} (0x{:03X})", name, message_id);
    }
}
