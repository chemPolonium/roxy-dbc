//! FIBEX 查看与编辑窗口

pub mod bit_layout;
pub mod cluster_window;
pub mod ecu_window;
pub mod fibex_window;
pub mod frame_edit_window;
pub mod frame_window;
pub mod pdu_edit_window;
pub mod pdu_window;
pub mod schedule_window;
pub mod signal_edit_window;

use dear_imgui_rs::Ui;

/// 不固定小数位数的 f64 输入框（%.15g：去除尾随零、不固定小数位，且不损失 double 精度）
pub fn input_f64_auto(ui: &Ui, label: &str, value: &mut f64) -> bool {
    ui.input_double_config(label)
        .display_format(dear_imgui_rs::NumericFormat::new("%.15g").expect("static format"))
        .build(value)
}
