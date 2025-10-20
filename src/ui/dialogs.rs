//! 对话框渲染模块

use crate::ui::state::{ErrorDialog, UiState};
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

    if ui_state.signal_edit_dialog.show {
        render_signal_edit_dialog(ui, ui_state);
    }

    if ui_state.confirm_delete_dialog.show {
        render_confirm_delete_dialog(ui, ui_state);
    }
}

/// 渲染 Signal 编辑对话框
pub fn render_signal_edit_dialog(ui: &Ui, ui_state: &mut crate::ui::state::UiState) {
    if !ui_state.signal_edit_dialog.show {
        return;
    }

    let mut should_apply = false;
    let mut should_cancel = false;
    let mut should_ok = false;

    ui.window("Edit Signal")
        .size([500.0, 420.0], imgui::Condition::FirstUseEver)
        .position([120.0, 120.0], imgui::Condition::FirstUseEver)
        .opened(&mut ui_state.signal_edit_dialog.show)
        .build(|| {
            ui.text("Signal Properties");
            ui.separator();

            ui.text("Name:");
            ui.set_next_item_width(-1.0);
            ui.input_text("##sig_name", &mut ui_state.signal_edit_dialog.name_buffer)
                .build();

            ui.spacing();
            ui.columns(2, "sig_layout", false);

            ui.text("Start Bit:");
            ui.next_column();
            ui.set_next_item_width(-1.0);
            ui.input_text(
                "##sig_start",
                &mut ui_state.signal_edit_dialog.start_bit_buffer,
            )
            .build();
            ui.next_column();

            ui.text("Length:");
            ui.next_column();
            ui.set_next_item_width(-1.0);
            ui.input_text("##sig_len", &mut ui_state.signal_edit_dialog.size_buffer)
                .build();
            ui.next_column();

            ui.text("Byte Order (Intel):");
            ui.next_column();
            ui.checkbox(
                "##sig_bo",
                &mut ui_state.signal_edit_dialog.byte_order_is_little,
            );
            ui.next_column();

            ui.text("Signed:");
            ui.next_column();
            ui.checkbox("##sig_signed", &mut ui_state.signal_edit_dialog.signed);
            ui.next_column();

            ui.text("Factor:");
            ui.next_column();
            ui.set_next_item_width(-1.0);
            ui.input_text(
                "##sig_factor",
                &mut ui_state.signal_edit_dialog.factor_buffer,
            )
            .build();
            ui.next_column();

            ui.text("Offset:");
            ui.next_column();
            ui.set_next_item_width(-1.0);
            ui.input_text(
                "##sig_offset",
                &mut ui_state.signal_edit_dialog.offset_buffer,
            )
            .build();
            ui.next_column();

            ui.text("Min:");
            ui.next_column();
            ui.set_next_item_width(-1.0);
            ui.input_text("##sig_min", &mut ui_state.signal_edit_dialog.min_buffer)
                .build();
            ui.next_column();

            ui.text("Max:");
            ui.next_column();
            ui.set_next_item_width(-1.0);
            ui.input_text("##sig_max", &mut ui_state.signal_edit_dialog.max_buffer)
                .build();
            ui.next_column();

            ui.text("Unit:");
            ui.next_column();
            ui.set_next_item_width(-1.0);
            ui.input_text("##sig_unit", &mut ui_state.signal_edit_dialog.unit_buffer)
                .build();
            ui.next_column();

            ui.columns(1, "sig_layout_end", false);

            ui.spacing();
            ui.text("Comment:");
            ui.set_next_item_width(-1.0);
            ui.input_text_multiline(
                "##sig_comment",
                &mut ui_state.signal_edit_dialog.comment_buffer,
                [0.0, 60.0],
            )
            .build();

            ui.separator();

            if ui.button_with_size("OK", [80.0, 0.0]) {
                should_ok = true;
            }
            ui.same_line();
            if ui.button_with_size("Cancel", [80.0, 0.0]) {
                should_cancel = true;
            }
            ui.same_line();
            if ui.button_with_size("Apply", [80.0, 0.0]) {
                should_apply = true;
            }
        });

    // 处理按钮点击
    if should_ok {
        apply_signal_changes(ui_state);
        ui_state.signal_edit_dialog.close();
    } else if should_cancel {
        ui_state.signal_edit_dialog.close();
    } else if should_apply {
        apply_signal_changes(ui_state);
    }
}

/// 渲染删除确认对话框
pub fn render_confirm_delete_dialog(ui: &Ui, ui_state: &mut UiState) {
    if !ui_state.confirm_delete_dialog.show {
        return;
    }

    // copy needed fields so we don't hold a mutable borrow across the UI closure
    let parent_dbc_id = ui_state.confirm_delete_dialog.parent_dbc_id;
    let message_id = ui_state.confirm_delete_dialog.message_id;
    let display_name = ui_state.confirm_delete_dialog.display_name.clone();

    ui.open_popup("Confirm Delete");
    ui.modal_popup_config("Confirm Delete")
        .resizable(false)
        .build(|| {
            ui.text("Confirm Delete");
            ui.separator();
            ui.text(format!(
                "Delete message '{}' (0x{:03X})? This action can be undone.",
                display_name, message_id
            ));
            ui.separator();

            let mut do_delete = false;
            if ui.button_with_size("Delete", [80.0, 0.0]) {
                do_delete = true;
            }
            ui.same_line();
            if ui.button_with_size("Cancel", [80.0, 0.0]) {
                // cancel
                ui_state.confirm_delete_dialog.show = false;
            }

            if do_delete {
                // perform delete on the specified parent window
                if let Some(idx) = ui_state
                    .dbc_windows
                    .iter()
                    .position(|w| w.id == parent_dbc_id)
                {
                    if ui_state
                        .ensure_message_not_in_open_signal_windows(message_id)
                        .is_ok()
                    {
                        if let Some(window) = ui_state.dbc_windows.get_mut(idx) {
                            // Attempt to build a full MessageOverride representing the deleted message
                            let mut simple = crate::dbc::MessageOverride {
                                message_id: message_id,
                                message_name: display_name.clone(),
                                comment: None,
                                message_size: 8,
                                transmitter: None,
                                signals: Vec::new(),
                            };

                            // Try to obtain richer info from the editable data
                            if let Some(mref) =
                                window.editable_data.get_message_ref_by_id(message_id)
                            {
                                // Use to_message_override for a canonical set of fields
                                let cm = mref.to_message_override();
                                simple.message_name = cm.message_name.clone();
                                simple.comment = cm.comment.clone();
                                simple.message_size = cm.message_size;
                                simple.transmitter = cm.transmitter.clone();
                            } else {
                                // Fallback: use overrides where available
                                simple.comment =
                                    Some(window.editable_data.get_message_comment(message_id));
                                simple.message_size =
                                    window.editable_data.get_message_size(message_id, 8u64);
                                simple.transmitter =
                                    Some(window.editable_data.get_message_transmitter(message_id));
                            }

                            let op =
                                crate::edit_history::Operation::DeleteMessage { message: simple };
                            if let Err(e) = window.history.apply_new(op, &mut window.editable_data)
                            {
                                ui_state.error_dialog.message =
                                    format!("Failed to apply delete operation: {}", e);
                                ui_state.error_dialog.show = true;
                            } else {
                                window.selected_message_id = None;
                            }
                        }
                    }
                }
                ui_state.confirm_delete_dialog.show = false;
            }
        });
}
fn apply_signal_changes(ui_state: &mut crate::ui::state::UiState) {
    // 解析并写回覆盖层或直接修改 MessageOverride
    let parent = ui_state.signal_edit_dialog.parent_dbc_id;
    let message_id = ui_state.signal_edit_dialog.message_id;

    if let Some(dbc_window) = ui_state.dbc_windows.iter_mut().find(|w| w.id == parent) {
        // 尝试找到对应 signal（按 name）并修改
        let sig_name = ui_state.signal_edit_dialog.name_buffer.trim().to_string();

        // parse numeric fields
        let start_bit_opt = ui_state
            .signal_edit_dialog
            .start_bit_buffer
            .trim()
            .parse::<u64>()
            .ok();
        let size_opt = ui_state
            .signal_edit_dialog
            .size_buffer
            .trim()
            .parse::<u64>()
            .ok();
        let factor = ui_state
            .signal_edit_dialog
            .factor_buffer
            .trim()
            .parse::<f64>()
            .ok()
            .unwrap_or(1.0);
        let offset = ui_state
            .signal_edit_dialog
            .offset_buffer
            .trim()
            .parse::<f64>()
            .ok()
            .unwrap_or(0.0);
        let minimum = ui_state
            .signal_edit_dialog
            .min_buffer
            .trim()
            .parse::<f64>()
            .ok()
            .unwrap_or(0.0);
        let maximum = ui_state
            .signal_edit_dialog
            .max_buffer
            .trim()
            .parse::<f64>()
            .ok()
            .unwrap_or(0.0);

        // Validate start_bit and size
        // Determine message size in bits (consider overrides)
        let msg_size_bytes = dbc_window.editable_data.get_message_size(message_id, 8u64);
        let msg_size_bits = (msg_size_bytes as u64).saturating_mul(8);

        if start_bit_opt.is_none() || size_opt.is_none() {
            ui_state.error_dialog.message =
                format!("Invalid start bit or length: must be integer values");
            ui_state.error_dialog.show = true;
            return;
        }

        let start_bit = start_bit_opt.unwrap();
        let size = size_opt.unwrap();

        if size == 0 || size > msg_size_bits {
            ui_state.error_dialog.message = format!(
                "Invalid signal length: {} bits (message has {} bits)",
                size, msg_size_bits
            );
            ui_state.error_dialog.show = true;
            return;
        }

        if start_bit >= msg_size_bits {
            ui_state.error_dialog.message = format!(
                "Start bit {} out of range (message has {} bits)",
                start_bit, msg_size_bits
            );
            ui_state.error_dialog.show = true;
            return;
        }

        // Store a SignalOverride in editable_data.signal_overrides for both original and custom messages
        let override_entry = crate::dbc::SignalOverride {
            name: sig_name.clone(),
            start_bit: start_bit,
            signal_size: size,
            byte_order: if ui_state.signal_edit_dialog.byte_order_is_little {
                crate::dbc::ByteOrder::LittleEndian
            } else {
                crate::dbc::ByteOrder::BigEndian
            },
            value_type: if ui_state.signal_edit_dialog.signed {
                crate::dbc::ValueType::Signed
            } else {
                crate::dbc::ValueType::Unsigned
            },
            factor,
            offset,
            minimum,
            maximum,
            unit: ui_state.signal_edit_dialog.unit_buffer.trim().to_string(),
            comment: ui_state
                .signal_edit_dialog
                .comment_buffer
                .trim()
                .to_string(),
        };

        // Prepare old override (if any)
        let old_override = dbc_window
            .editable_data
            .signal_overrides
            .get(&(message_id, sig_name.clone()))
            .cloned();

        // Use History operation to record this change
        use crate::edit_history::Operation;
        let op = Operation::ModifySignal {
            message_id,
            signal_name: sig_name.clone(),
            old: old_override,
            new: override_entry.clone(),
        };
        if let Err(e) = dbc_window
            .history
            .apply_new(op, &mut dbc_window.editable_data)
        {
            ui_state.error_dialog.message = format!("Failed to apply signal edit: {}", e);
            ui_state.error_dialog.show = true;
            return;
        }

        // Update any open Signal windows showing this message so they reflect the new overrides
        if let Some(message_ref) = dbc_window.editable_data.get_message_ref_by_id(message_id) {
            let new_view = crate::ui::view::MessageView::from_message_ref(
                &message_ref,
                &dbc_window.editable_data,
            );
            for sw in ui_state.signal_windows.iter_mut() {
                if sw.parent_dbc_id == parent && sw.message.message_id == message_id {
                    sw.message = new_view.clone();
                }
            }
        }
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
        // (old snapshot-based undo disabled) -- before snapshot omitted

        // Build Operation(s) for the changed fields
        use crate::edit_history::Operation;

        let mut ops: Vec<Operation> = Vec::new();

        if name_changed {
            ops.push(Operation::RenameMessage {
                message_id,
                old: old_name.clone(),
                new: new_name.clone(),
            });
        }

        if comment_changed {
            ops.push(Operation::ModifyMessageComment {
                message_id,
                old: old_comment.clone(),
                new: new_comment.clone(),
            });
        }

        if id_changed {
            ops.push(Operation::ModifyMessageId {
                original_message_id: message_id,
                old_id,
                new_id: new_id.unwrap(),
            });
        }

        if size_changed {
            ops.push(Operation::ModifyMessageSize {
                message_id,
                old: old_size
                    .try_into()
                    .expect("old message size out of u8 range"),
                new: new_size
                    .unwrap()
                    .try_into()
                    .expect("new message size out of u8 range"),
            });
        }

        if transmitter_changed {
            ops.push(Operation::ModifyMessageTransmitter {
                message_id,
                old: old_transmitter.clone(),
                new: new_transmitter.clone(),
            });
        }

        // Compose single or multiple ops
        let op_to_apply = if ops.len() == 1 {
            ops.into_iter().next().unwrap()
        } else {
            Operation::Composite(ops)
        };

        // Apply via per-window history
        if let Err(e) = dbc_window
            .history
            .apply_new(op_to_apply, &mut dbc_window.editable_data)
        {
            ui_state.error_dialog.message = format!("Failed to apply edit operation: {}", e);
            ui_state.error_dialog.show = true;
            return;
        }

        // (old snapshot-based undo push has been disabled; history should handle this)
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
    use crate::edit_history::Operation;

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

        // Create a full MessageOverride and apply via per-window history
        let cm = crate::dbc::MessageOverride {
            message_id: message_id,
            message_name: name.clone(),
            comment: if comment.is_empty() {
                None
            } else {
                Some(comment.clone())
            },
            message_size: size.try_into().expect("size out of u8 range"),
            transmitter: if transmitter.is_empty() {
                None
            } else {
                Some(transmitter.clone())
            },
            signals: Vec::new(),
        };

        let op = Operation::AddMessage { message: cm };

        if let Err(e) = dbc_window
            .history
            .apply_new(op, &mut dbc_window.editable_data)
        {
            ui_state.error_dialog.message = format!("Failed to apply operation: {}", e);
            ui_state.error_dialog.show = true;
            return;
        }

        // Old snapshot-based undo calls intentionally removed; History is primary.

        // Select the newly created message
        dbc_window.selected_message_id = Some(message_id);

        println!("Created new message: {} (0x{:03X})", name, message_id);
    }
}
