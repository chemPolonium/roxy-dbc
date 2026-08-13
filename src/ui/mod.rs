//! UI 模块 - 用户界面渲染和交互逻辑
//!
//! 这个模块被重构为多个子模块以提高可维护性：
//! - `state`: UI 状态结构和 Undo/Redo 系统
//! - `dbc_window`: DBC 浏览器窗口渲染
//! - `message_window`: Message 详细窗口渲染（包含 Signal 表格）
//! - `dialogs`: 各种对话框（错误、关于、编辑等）
//! - `menu`: 菜单栏和文件操作

pub mod bit_layout;
pub mod dbc_window;
mod menu;
mod message_create_window;
mod message_edit_window;
mod message_window;
pub mod node_window;
mod signal_edit_window;
pub mod state;

use imgui::Ui;
use std::time::Duration;

pub use state::UiState;

/// 主 UI 渲染函数
pub fn render_ui(ui: &Ui, delta_s: Duration, target_frame_time: Duration, ui_state: &mut UiState) {
    setup_main_dockspace(ui);

    if ui_state.show_performance_window {
        render_performance_window(ui, delta_s, target_frame_time);
    }

    menu::render_main_menu_bar(ui, ui_state);
    dbc_window::render_dbc_windows(ui, ui_state);

    if ui_state.node_dialog.show {
        match ui_state.last_focused_dbc_index {
            Some(idx) => ui_state.node_dialog.render(ui, &mut ui_state.dbc_windows[idx].dbc),
            None => ui_state.node_dialog.show = false,
        }
    }

    render_about_dialog(ui, ui_state);
    render_error_dialog(ui, ui_state);

    if ui_state.file_hovering {
        render_drop_zone_overlay(ui);
    }
}

fn render_about_dialog(ui: &Ui, ui_state: &mut UiState) {
    if !ui_state.show_about_dialog {
        return;
    }

    let mut is_open = true;
    ui.window("About roxy-dbc")
        .always_auto_resize(true)
        .opened(&mut is_open)
        .build(|| {
            ui.text(format!("roxy-dbc v{}", env!("CARGO_PKG_VERSION")));
            ui.text("A CAN database (DBC/ARXML/KCD) editor");
            ui.separator();
            if ui.button("OK") {
                ui_state.show_about_dialog = false;
            }
        });

    if !is_open {
        ui_state.show_about_dialog = false;
    }
}

fn render_error_dialog(ui: &Ui, ui_state: &mut UiState) {
    if !ui_state.error_dialog.show {
        return;
    }

    let mut is_open = true;
    ui.window("Error")
        .always_auto_resize(true)
        .opened(&mut is_open)
        .build(|| {
            ui.text(&ui_state.error_dialog.message);
            ui.separator();
            if ui.button("OK") {
                ui_state.error_dialog.show = false;
            }
        });

    if !is_open {
        ui_state.error_dialog.show = false;
    }
}

/// 设置主dockspace占满整个窗口
fn setup_main_dockspace(ui: &Ui) {
    // 使用dockspace_over_main_viewport API创建全屏dockspace
    ui.dockspace_over_main_viewport();
}

/// 渲染性能信息窗口
fn render_performance_window(ui: &Ui, delta_s: Duration, target_frame_time: Duration) {
    let window = ui.window("Performance Information");
    window
        .size([300.0, 150.0], imgui::Condition::FirstUseEver)
        .position([400.0, 50.0], imgui::Condition::FirstUseEver)
        .build(|| {
            ui.text(format!("Frame Time: {delta_s:?}"));
            let fps = 1.0 / delta_s.as_secs_f32();
            ui.text(format!("FPS: {fps:.1}"));
            ui.text(format!(
                "Target FPS: {:.1}",
                1.0 / target_frame_time.as_secs_f32()
            ));
        });
}

fn render_drop_zone_overlay(ui: &Ui) {
    let draw_list = ui.get_foreground_draw_list();
    let display = ui.io().display_size;
    let inset = 24.0;

    draw_list
        .add_rect([0.0, 0.0], display, [0.0, 0.0, 0.0, 0.4])
        .filled(true)
        .build();

    draw_list
        .add_rect(
            [inset, inset],
            [display[0] - inset, display[1] - inset],
            [0.3, 0.6, 1.0, 0.9],
        )
        .thickness(3.0)
        .rounding(12.0)
        .build();

    let text = "Drop file to open";
    let text_size = ui.calc_text_size(text);
    let text_pos = [
        (display[0] - text_size[0]) / 2.0,
        (display[1] - text_size[1]) / 2.0,
    ];
    draw_list.add_text(text_pos, [1.0, 1.0, 1.0, 0.9], text);
}
