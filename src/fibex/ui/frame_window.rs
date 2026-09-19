use dear_imgui_rs::{Condition, TableFlags, TableSizingPolicy, Ui};

use crate::fibex::editable_fibex::EditableFibex;

#[derive(Clone, Debug)]
pub enum FrameWindowEvent {
    None,
    /// 双击 PDU 行 -> 打开编辑窗口
    EditPdu(String),
    /// 打开 PDU 详情窗口
    OpenPdu(String),
    /// 向帧中添加已有的 PDU
    AddExistingPdu(String),
    /// Remove from Frame PDU 映射
    RemovePdu(String),
    /// 修改 PDU 起始字节
    SetPduStart(String, u32),
    /// 新建 PDU 并加入帧
    NewPdu,
}

/// Frame 详细窗口状态（包含 PDU 映射表）
#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct FrameWindow {
    /// 所属文件窗口 ID，用于生成唯一窗口标题 ID
    window_id: usize,
    pub frame_name: String,
    pub is_open: bool,
    selected_pdu_names: Vec<String>,
    focus_requested: bool,
}

impl FrameWindow {
    pub fn new(window_id: usize, frame_name: &str) -> Self {
        Self {
            window_id,
            frame_name: frame_name.to_string(),
            is_open: true,
            selected_pdu_names: Vec::new(),
            focus_requested: false,
        }
    }

    pub fn render(&mut self, ui: &Ui, fibex: &EditableFibex) -> FrameWindowEvent {
        let mut event = FrameWindowEvent::None;

        let Some(frame) = fibex.get_frame(&self.frame_name) else {
            return event;
        };
        let frame = frame.clone();

        // 标题 ID 带上文件窗口 ID，避免不同文件的同名帧窗口冲突
        let title = format!(
            "Frames - {}###FRAMEWIN{}_{}",
            frame.name(),
            self.window_id,
            frame.name()
        );
        let mut is_open = self.is_open;

        let mut window = ui
            .window(&title)
            .size([680.0, 400.0], Condition::FirstUseEver)
            .opened(&mut is_open);
        if self.focus_requested {
            window = window.focused(true);
            self.focus_requested = false;
        }

        window.build(|| {
            let t = frame.triggering();
            ui.text(format!(
                "Slot: {}  |  Channel: {}  |  Cycle: {}+{}/{}  |  Length: {} bytes{}",
                t.slot_id,
                t.channel.label(),
                t.base_cycle,
                t.base_cycle,
                t.cycle_repetition,
                frame.length(),
                if t.startup { "  |  Startup" } else { "" }
            ));
            if !frame.comment().is_empty() {
                ui.text(format!("Comment: {}", frame.comment()));
            }

            ui.separator();

            // PDU 布局条
            render_frame_layout_bar(ui, fibex, &frame);
            ui.separator();

            if ui.small_button("+ Add PDU") {
                event = FrameWindowEvent::NewPdu;
            }
            ui.same_line();
            if ui.small_button("Add existing PDU") {
                ui.open_popup("add_existing_pdu_popup");
            }

            // 已有 PDU 的下拉选择弹窗
            if let Some(_popup) = ui.begin_popup("add_existing_pdu_popup") {
                let mapped: Vec<&str> = frame.pdus().iter().map(|m| m.pdu_name.as_str()).collect();
                let candidates: Vec<String> = fibex
                    .pdus()
                    .iter()
                    .filter(|p| !mapped.contains(&p.name()))
                    .map(|p| p.name().to_string())
                    .collect();
                if candidates.is_empty() {
                    ui.text_disabled("No unmapped PDUs");
                }
                for name in &candidates {
                    if ui.selectable(name) {
                        event = FrameWindowEvent::AddExistingPdu(name.clone());
                        ui.close_current_popup();
                    }
                }
            }

            let pdus = frame.pdus().clone();
            if pdus.is_empty() {
                ui.text_disabled("No PDUs mapped to this frame");
                return;
            }

            // 键盘导航
            if ui.is_window_focused() && !ui.io().want_capture_keyboard() && !pdus.is_empty() {
                let cursor_pos = self
                    .selected_pdu_names
                    .first()
                    .and_then(|n| pdus.iter().position(|m| m.pdu_name == *n));

                let mut new_pos: Option<usize> = None;
                if ui.is_key_pressed(dear_imgui_rs::Key::DownArrow) {
                    new_pos = Some(match cursor_pos {
                        Some(p) => (p + 1).min(pdus.len() - 1),
                        None => 0,
                    });
                }
                if ui.is_key_pressed(dear_imgui_rs::Key::UpArrow) {
                    new_pos = Some(match cursor_pos {
                        Some(0) => 0,
                        Some(p) => p - 1,
                        None => pdus.len() - 1,
                    });
                }
                if let Some(pos) = new_pos {
                    self.selected_pdu_names = vec![pdus[pos].pdu_name.clone()];
                }
                if ui.is_key_pressed(dear_imgui_rs::Key::Enter)
                    && let Some(name) = self.selected_pdu_names.first().cloned()
                {
                    event = FrameWindowEvent::OpenPdu(name);
                }
            }

            // PDU 表占满窗口剩余高度
            let avail = ui.content_region_avail();
            ui.table("frame_pdu_table")
                .flags(TableFlags::RESIZABLE | TableFlags::BORDERS | TableFlags::SCROLL_X | TableFlags::SCROLL_Y | TableFlags::ROW_BG)
                .sizing_policy(TableSizingPolicy::FixedFit)
                .freeze(0, 1)
                .outer_size([0.0, avail[1].max(120.0)])
                .column("Start").done()
                .column("PDU").done()
                .column("Length").done()
                .column("Type").done()
                .column("Signals").done()
                .headers(true)
                .build(|ui| {
                    for m in &pdus {
                        let pdu_name = m.pdu_name.clone();
                        let pdu_info = fibex.get_pdu(&pdu_name);

                        ui.table_next_row();
                        let is_selected = self.selected_pdu_names.iter().any(|n| n == &pdu_name);

                        // 第 0 列：起始字节直接可编辑
                        ui.table_set_column_index(0);
                        let mut start_val = m.start_position as i32;
                        ui.set_next_item_width(70.0);
                        if ui.input_int(format!("##start_{}", pdu_name).as_str(), &mut start_val) {
                            let clamped = start_val.clamp(0, 253);
                            if clamped >= 0 && (clamped as u32) != m.start_position {
                                event = FrameWindowEvent::SetPduStart(pdu_name.clone(), clamped as u32);
                            }
                        }

                        // 第 1 列：PDU 名（选中 / 双击 / 右键菜单）
                        ui.table_set_column_index(1);
                        if ui
                            .selectable_config(&pdu_name)
                            .selected(is_selected)
                            .span_all_columns(true)
                            .build()
                        {
                            self.selected_pdu_names = vec![pdu_name.clone()];
                        }
                        if ui.is_item_hovered()
                            && ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left)
                        {
                            event = FrameWindowEvent::OpenPdu(pdu_name.clone());
                        }

                        if let Some(_popup) = ui.begin_popup_context_item_with_label(Some(
                            &format!("frame_pdu_ctx_{}", pdu_name),
                        )) {
                            if !self.selected_pdu_names.iter().any(|n| n == &pdu_name) {
                                self.selected_pdu_names = vec![pdu_name.clone()];
                            }
                            if ui.menu_item("Open") {
                                event = FrameWindowEvent::OpenPdu(pdu_name.clone());
                            }
                            if ui.menu_item("Edit Properties") {
                                event = FrameWindowEvent::EditPdu(pdu_name.clone());
                            }
                            ui.separator();
                            if ui.menu_item("Remove from Frame") {
                                event = FrameWindowEvent::RemovePdu(pdu_name.clone());
                            }
                        }

                        ui.table_set_column_index(2);
                        ui.text(format!(
                            "{}",
                            pdu_info.map(|p| p.length()).unwrap_or(0)
                        ));

                        ui.table_set_column_index(3);
                        ui.text(pdu_info.map(|p| p.kind().label()).unwrap_or("?"));

                        ui.table_set_column_index(4);
                        ui.text(format!(
                            "{}",
                            pdu_info.map(|p| p.signals().len()).unwrap_or(0)
                        ));
                    }
                });
        });

        self.is_open = is_open;
        event
    }
}

/// 绘制帧内 PDU 的字节布局条
fn render_frame_layout_bar(ui: &Ui, fibex: &EditableFibex, frame: &crate::fibex::editable_fibex::EditableFrame) {
    let num_bytes = frame.length().max(1) as usize;
    const CELL_W: f32 = 26.0;
    const CELL_H: f32 = 40.0;
    const PALETTE: [[f32; 4]; 8] = [
        [0.25, 0.55, 0.95, 0.75],
        [0.95, 0.55, 0.25, 0.75],
        [0.35, 0.8, 0.45, 0.75],
        [0.85, 0.35, 0.75, 0.75],
        [0.95, 0.85, 0.3, 0.75],
        [0.4, 0.85, 0.85, 0.75],
        [0.65, 0.5, 0.95, 0.75],
        [0.9, 0.4, 0.4, 0.75],
    ];

    let origin = ui.cursor_screen_pos();
    let total_w = num_bytes as f32 * CELL_W;
    ui.dummy([total_w, CELL_H + 16.0]);

    let dl = ui.get_window_draw_list();

    // 背景网格
    for byte in 0..num_bytes {
        let x = origin[0] + byte as f32 * CELL_W;
        dl.add_rect([x, origin[1]], [x + CELL_W, origin[1] + CELL_H], [0.35, 0.35, 0.35, 1.0])
            .build();
        dl.add_text(
            [x + 4.0, origin[1] + CELL_H + 2.0],
            [0.6, 0.6, 0.6, 1.0],
            format!("{}", byte),
        );
    }

    // PDU 区域着色
    for (i, m) in frame.pdus().iter().enumerate() {
        let Some(pdu) = fibex.get_pdu(&m.pdu_name) else {
            continue;
        };
        let color = PALETTE[i % PALETTE.len()];
        let x0 = origin[0] + m.start_position as f32 * CELL_W;
        let x1 = origin[0] + (m.start_position + pdu.length()).min(num_bytes as u32) as f32 * CELL_W;
        if x1 <= x0 {
            continue;
        }
        dl.add_rect([x0 + 1.0, origin[1] + 1.0], [x1 - 1.0, origin[1] + CELL_H - 1.0], color)
            .filled(true)
            .build();
        dl.add_rect([x0 + 1.0, origin[1] + 1.0], [x1 - 1.0, origin[1] + CELL_H - 1.0], [color[0], color[1], color[2], 1.0])
            .build();
        dl.add_text(
            [x0 + 3.0, origin[1] + CELL_H / 2.0 - 6.0],
            [1.0, 1.0, 1.0, 1.0],
            m.pdu_name.as_str(),
        );
    }
}
