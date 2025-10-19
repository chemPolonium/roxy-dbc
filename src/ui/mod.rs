//! UI 模块 - 用户界面渲染和交互逻辑
//!
//! 这个模块被重构为多个子模块以提高可维护性：
//! - `state`: UI 状态结构和 Undo/Redo 系统
//! - `dbc_window`: DBC 浏览器窗口渲染
//! - `signal_window`: Signal 详细窗口渲染
//! - `dialogs`: 各种对话框（错误、关于、编辑等）
//! - `menu`: 菜单栏和文件操作

mod dbc_window;
mod dialogs;
mod menu;
mod signal_window;
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
    menu::handle_global_shortcuts(ui, ui_state);
    signal_window::render_signal_windows(ui, ui_state);
    dialogs::render_dialogs(ui, ui_state);
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
