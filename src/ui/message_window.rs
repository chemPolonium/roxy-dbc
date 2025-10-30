// //! Message 窗口渲染模块 (包含 Signals 表格)

use crate::{editable_dbc::EditableMessage, ui::state::UiState};
use can_dbc::{ByteOrder, ValueType};
use imgui::{
    Condition, TableBgTarget, TableColumnFlags, TableColumnSetup, TableFlags, Ui,
    WindowFocusedFlags,
};

/// Message 详细窗口状态（包含 Signal 表格）
#[derive(Clone)]
pub struct MessageWindowState {
    pub id: usize,
    pub message: EditableMessage,
    pub is_open: bool,
    pub parent_dbc_id: usize,
    // 临时信号编辑请求（仅在窗口内双击某个信号时设置，主循环会处理并打开编辑对话框）
    pub pending_signal_edit: Option<String>,
    // 选中信号的名称（用于在表格中高亮整行）
    pub selected_signal_name: Option<String>,
}

// 渲染所有 Message 窗口
// pub fn render_message_windows(ui: &Ui, ui_state: &mut UiState) {
//     let mut windows_to_remove = Vec::new();

//     // for (index, window) in ui_state.message_windows.iter_mut().enumerate() {
//     //     request_window_focus_if_needed(ui_state.message_window_focus_request, window.id);

//     //     if !window.is_open {
//     //         continue;
//     //     }

//     //     let (still_open, focused) = render_message_window(ui, window);

//     //     if focused {
//     //         ui_state.last_focused_message_window = Some(index);
//     //         ui_state.last_focused_dbc_index = None;
//     //     }

//     //     if !still_open {
//     //         windows_to_remove.push(index);
//     //     }
//     // }

//     // // 清空聚焦请求
//     // ui_state.dbc_window_focus_request = None;
//     // ui_state.message_window_focus_request = None;

//     // After rendering all message windows, process any pending signal edit requests produced by double-clicks
//     // let mut pending = Vec::new();
//     // for w in ui_state.message_windows.iter_mut() {
//     //     if let Some(sig_name) = w.pending_signal_edit.take() {
//     //         pending.push((w.parent_dbc_id, w.message.message_id, sig_name));
//     //     }
//     // }

//     // 清空聚焦请求
//     ui_state.dbc_window_focus_request = None;
//     ui_state.message_window_focus_request = None;
//     // cleanup_closed_message_windows(ui_state, windows_to_remove);
// }

// /// 渲染单个 Message 详细窗口
// fn render_message_window(ui: &Ui, window_state: &mut MessageWindowState) -> (bool, bool) {
//     let window_title = format!(
//         "Signals - {} (0x{:03X})",
//         window_state.message.message_name, window_state.message.message_id
//     );

//     let mut is_open = window_state.is_open;
//     let mut focused = false;

//     if is_open {
//         let window = ui
//             .window(&window_title)
//             .opened(&mut is_open)
//             .size([800.0, 600.0], Condition::FirstUseEver)
//             .position(
//                 [
//                     100.0 + (window_state.id as f32 * 30.0),
//                     100.0 + (window_state.id as f32 * 30.0),
//                 ],
//                 Condition::FirstUseEver,
//             );

//         window.build(|| {
//             // render content; content may set window_state.pending_signal_edit when user double-clicks a signal
//             render_message_window_content(ui, window_state);
//             if ui.is_window_focused_with_flags(WindowFocusedFlags::ROOT_AND_CHILD_WINDOWS) {
//                 focused = true;
//             }
//         });
//     }

//     if !is_open {
//         window_state.is_open = false;
//     }

//     (window_state.is_open, focused)
// }

// /// 渲染Message窗口的内容
// fn render_message_window_content(ui: &Ui, window_state: &mut MessageWindowState) {
//     let message = &window_state.message;
//     ui.text(format!(
//         "Message: {} (0x{:03X}) - {} signals",
//         message.message_name,
//         message.message_id,
//         message.signals.len()
//     ));
//     ui.separator();

//     // 创建完整的信号表格（使用每窗口唯一的表 ID，使排序等状态独立）
//     let table_id = format!("full_signals_table_{}", window_state.id);
//     if let Some(_table) = ui.begin_table_with_flags(
//         &table_id,
//         10,
//         TableFlags::RESIZABLE
//             | TableFlags::REORDERABLE
//             | TableFlags::HIDEABLE
//             | TableFlags::BORDERS
//             | TableFlags::SIZING_FIXED_FIT
//             | TableFlags::SCROLL_Y
//             | TableFlags::SORTABLE,
//     ) {
//         // 设置完整的表格列
//         let columns = [
//             (
//                 "Signal",
//                 TableColumnFlags::DEFAULT_SORT | TableColumnFlags::WIDTH_STRETCH,
//                 0.0,
//             ),
//             ("Type", TableColumnFlags::default(), 0.0),
//             ("Order", TableColumnFlags::default(), 0.0),
//             ("Start", TableColumnFlags::default(), 0.0),
//             ("Length", TableColumnFlags::default(), 0.0),
//             ("Factor", TableColumnFlags::default(), 0.0),
//             ("Offset", TableColumnFlags::default(), 0.0),
//             ("Min", TableColumnFlags::default(), 0.0),
//             ("Max", TableColumnFlags::default(), 0.0),
//             ("Unit", TableColumnFlags::default(), 0.0),
//         ];

//         for (name, flags, width) in &columns {
//             ui.table_setup_column_with(TableColumnSetup {
//                 name,
//                 flags: *flags,
//                 init_width_or_weight: *width,
//                 user_id: ui.new_id_str(&format!("full_{}", name.to_lowercase())),
//             });
//         }

//         ui.table_headers_row();

//         // Prepare signals for optional sorting
//         let mut signals: Vec<_> = message.signals.iter().cloned().collect();
//         // If the table has sort specs, apply sorting similar to messages
//         if let Some(sort_specs) = ui.table_sort_specs_mut() {
//             let specs = sort_specs.specs();
//             for (i, spec) in specs.iter().enumerate() {
//                 if i == 0 {
//                     let ascending =
//                         spec.sort_direction() == Some(imgui::TableSortDirection::Ascending);
//                     signals.sort_by(|a, b| {
//                         let ordering = match spec.column_idx() {
//                             0 => a.name.cmp(&b.name),
//                             1 => format!("{:?}", a.value_type).cmp(&format!("{:?}", b.value_type)),
//                             2 => format!("{:?}", a.byte_order).cmp(&format!("{:?}", b.byte_order)),
//                             3 => a.start_bit.cmp(&b.start_bit),
//                             4 => a.signal_size.cmp(&b.signal_size),
//                             5 => a
//                                 .factor
//                                 .partial_cmp(&b.factor)
//                                 .unwrap_or(std::cmp::Ordering::Equal),
//                             6 => a
//                                 .offset
//                                 .partial_cmp(&b.offset)
//                                 .unwrap_or(std::cmp::Ordering::Equal),
//                             7 => a
//                                 .minimum
//                                 .partial_cmp(&b.minimum)
//                                 .unwrap_or(std::cmp::Ordering::Equal),
//                             8 => a
//                                 .maximum
//                                 .partial_cmp(&b.maximum)
//                                 .unwrap_or(std::cmp::Ordering::Equal),
//                             9 => a.unit.cmp(&b.unit),
//                             _ => std::cmp::Ordering::Equal,
//                         };
//                         if ascending {
//                             ordering
//                         } else {
//                             ordering.reverse()
//                         }
//                     });
//                     break;
//                 }
//             }
//         }

//         // 显示所有信号
//         for signal in signals.iter() {
//             ui.table_next_row();

//             // Determine if this signal is selected
//             let selected = window_state
//                 .selected_signal_name
//                 .as_ref()
//                 .map(|n| n == &signal.name)
//                 .unwrap_or(false);

//             // 如果选中，设置整行背景色
//             if selected {
//                 ui.table_set_bg_color(TableBgTarget::ROW_BG0, [0.3, 0.3, 0.7, 0.65]);
//             }

//             ui.table_set_column_index(0);
//             // Make the signal row selectable across all columns
//             if ui
//                 .selectable_config(signal.name.clone())
//                 .selected(selected)
//                 .span_all_columns(true)
//                 .build()
//             {
//                 // select this signal
//                 window_state.selected_signal_name = Some(signal.name.clone());
//             }

//             // Context menu id for this signal
//             let popup_id = format!(
//                 "signal_context_{}_{}",
//                 window_state.message.message_id, signal.name
//             );
//             if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
//                 // select this signal when right-clicking (mirror message table behavior)
//                 window_state.selected_signal_name = Some(signal.name.clone());
//                 ui.open_popup(&popup_id);
//             }

//             ui.popup(&popup_id, || {
//                 if ui.menu_item("Edit...") {
//                     window_state.pending_signal_edit = Some(signal.name.clone());
//                 }
//             });

//             ui.table_set_column_index(1);
//             let data_type = match signal.value_type {
//                 ::ValueType::Signed => "signed",
//                 ::ValueType::Unsigned => "unsigned",
//             };
//             ui.text(data_type);

//             ui.table_set_column_index(2);
//             let byte_order = match signal.byte_order {
//                 ::ByteOrder::LittleEndian => "Intel",
//                 ::ByteOrder::BigEndian => "Motorola",
//             };
//             ui.text(byte_order);

//             ui.table_set_column_index(3);
//             ui.text(format!("{}", signal.start_bit));

//             ui.table_set_column_index(4);
//             ui.text(format!("{}", signal.signal_size));

//             ui.table_set_column_index(5);
//             ui.text(format!("{}", signal.factor));

//             ui.table_set_column_index(6);
//             ui.text(format!("{}", signal.offset));

//             ui.table_set_column_index(7);
//             ui.text(format!("{}", signal.minimum));

//             ui.table_set_column_index(8);
//             ui.text(format!("{}", signal.maximum));

//             ui.table_set_column_index(9);
//             ui.text(&signal.unit);
//             // 双击已禁用；仅通过右键菜单 Edit... 进入编辑
//         }
//     }
// }

// /// 清理已关闭的 Message 窗口
// fn cleanup_closed_message_windows(ui_state: &mut UiState, windows_to_remove: Vec<usize>) {
//     for &index in windows_to_remove.iter().rev() {
//         ui_state.message_windows.remove(index);

//         // 修正焦点索引
//         if let Some(focused_sig) = ui_state.last_focused_message_window {
//             if focused_sig == index {
//                 ui_state.last_focused_message_window = None;
//             } else if focused_sig > index {
//                 ui_state.last_focused_message_window = Some(focused_sig - 1);
//             }
//         }
//     }
// }

// /// 请求窗口聚焦（如果需要）
// fn request_window_focus_if_needed(focus_request: Option<usize>, window_id: usize) {
//     if focus_request == Some(window_id) {
//         unsafe {
//             imgui::sys::igSetNextWindowFocus();
//         }
//     }
// }
