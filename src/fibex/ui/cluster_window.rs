use dear_imgui_rs::Ui;

use crate::fibex::editable_fibex::{ClusterParam, ClusterValue, EditableFibex};

/// Cluster 参数内容（渲染在文件主窗口的标签页内）
/// 直接编辑协议参数，每次修改都会记录撤销历史
pub fn render_cluster_content(ui: &Ui, fibex: &mut EditableFibex) {
    let mut name = fibex.cluster().name.clone();
    let mut params = fibex.cluster().params;

    let label_w = 230.0;

    ui.align_text_to_frame_padding();
    ui.text("Cluster Name");
    ui.same_line_with_pos(label_w);
    if ui.input_text("##cluster_name", &mut name).build() {
        fibex.set_cluster_param(
            ClusterParam::Name,
            ClusterValue::Text(name.trim().to_string()),
        );
    }

    ui.separator_with_text("Timing");

    f64_field(ui, label_w, "Cycle Time (ms)", &mut params.cycle_time_ms, ClusterParam::CycleTimeMs, fibex);
    f64_field(ui, label_w, "Macrotick Duration (µs)", &mut params.macrotick_duration_us, ClusterParam::MacrotickDurationUs, fibex);
    u32_field(ui, label_w, "Coldstart Attempts (g)", &mut params.coldstart_attempts, ClusterParam::ColdstartAttempts, fibex);
    u32_field(ui, label_w, "Action Point Offset (gd)", &mut params.action_point_offset, ClusterParam::ActionPointOffset, fibex);
    u32_field(ui, label_w, "Offset Correction Start (g)", &mut params.offset_correction_start, ClusterParam::OffsetCorrectionStart, fibex);
    u32_field(ui, label_w, "Minor Version (gd)", &mut params.minor_version, ClusterParam::MinorVersion, fibex);

    ui.separator_with_text("Static Segment");

    u32_field(ui, label_w, "Number of Static Slots (g)", &mut params.number_of_static_slots, ClusterParam::NumberOfStaticSlots, fibex);
    u32_field(ui, label_w, "Static Slot Duration (gd)", &mut params.static_slot_duration, ClusterParam::StaticSlotDuration, fibex);

    ui.separator_with_text("Dynamic Segment");

    u32_field(ui, label_w, "Number of Minislots (g)", &mut params.number_of_minislots, ClusterParam::NumberOfMinislots, fibex);
    u32_field(ui, label_w, "Minislot Duration (gd)", &mut params.minislot_duration, ClusterParam::MinislotDuration, fibex);
    u32_field(ui, label_w, "Minislot Action Point Offset (gd)", &mut params.minislot_action_point_offset, ClusterParam::MinislotActionPointOffset, fibex);
    u32_field(ui, label_w, "Dynamic Slot Idle Phase (gd)", &mut params.dynamic_slot_idle_phase, ClusterParam::DynamicSlotIdlePhase, fibex);

    ui.separator_with_text("Symbol Window / NIT");

    u32_field(ui, label_w, "Symbol Window (gd)", &mut params.symbol_window, ClusterParam::SymbolWindow, fibex);
    u32_field(ui, label_w, "Symbol Window Idle Phase (gd)", &mut params.symbol_window_idle_phase, ClusterParam::SymbolWindowIdlePhase, fibex);
    u32_field(ui, label_w, "Network Idle Time (gd NIT)", &mut params.network_idle_time, ClusterParam::NetworkIdleTime, fibex);

    ui.separator();
    u32_field(ui, label_w, "Speed (kbit/s)", &mut params.speed_kbps, ClusterParam::SpeedKbps, fibex);
}

fn u32_field(
    ui: &Ui,
    label_w: f32,
    label: &str,
    value: &mut u32,
    param: ClusterParam,
    fibex: &mut EditableFibex,
) {
    ui.align_text_to_frame_padding();
    ui.text(label);
    ui.same_line_with_pos(label_w);
    let mut v = *value as i32;
    ui.set_next_item_width(140.0);
    if ui.input_int(format!("##{}", label).as_str(), &mut v) && v >= 0 {
        *value = v as u32;
        fibex.set_cluster_param(param, ClusterValue::U32(v as u32));
    }
}

fn f64_field(
    ui: &Ui,
    label_w: f32,
    label: &str,
    value: &mut f64,
    param: ClusterParam,
    fibex: &mut EditableFibex,
) {
    ui.align_text_to_frame_padding();
    ui.text(label);
    ui.same_line_with_pos(label_w);
    let mut v = *value;
    ui.set_next_item_width(140.0);
    if crate::fibex::ui::input_f64_auto(ui, format!("##{}", label).as_str(), &mut v) && v >= 0.0 {
        *value = v;
        fibex.set_cluster_param(param, ClusterValue::F64(v));
    }
}
