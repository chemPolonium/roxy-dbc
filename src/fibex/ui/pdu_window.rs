use dear_imgui_rs::{Condition, TableFlags, TableSizingPolicy, Ui};

use crate::fibex::editable_fibex::{ByteOrder, EditableFibex, ValueType};
use crate::fibex::ui::bit_layout::render_bit_layout;

#[derive(Clone, Debug)]
pub enum PduWindowEvent {
    None,
    /// 双击信号行 -> 打开信号编辑对话框
    EditSignal(String),
    CopySignal(Vec<String>),
    CutSignal(Vec<String>),
    DeleteSignal(Vec<String>),
    PasteSignal,
    AddSignal,
}

/// PDU 详细窗口状态（包含信号表与位布局）
#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct PduWindow {
    /// 所属文件窗口 ID，用于生成唯一窗口标题 ID
    window_id: usize,
    pub pdu_name: String,
    pub is_open: bool,
    pub focus_requested: bool,
    selected_signal_names: Vec<String>,
    signal_anchor: Option<String>,
    signal_cursor: Option<String>,
    sort_column_idx: usize,
    sort_ascending: bool,
}

impl PduWindow {
    pub fn new(window_id: usize, pdu_name: &str) -> Self {
        Self {
            window_id,
            pdu_name: pdu_name.to_string(),
            is_open: true,
            focus_requested: false,
            selected_signal_names: Vec::new(),
            signal_anchor: None,
            signal_cursor: None,
            // 默认按起始位升序，保持导入文件的自然位序
            sort_column_idx: 1,
            sort_ascending: true,
        }
    }

    pub fn render(&mut self, ui: &Ui, fibex: &EditableFibex, has_clipboard: bool) -> PduWindowEvent {
        let mut event = PduWindowEvent::None;

        let Some(pdu) = fibex.get_pdu(&self.pdu_name) else {
            return event;
        };
        let pdu = pdu.clone();

        // 标题 ID 带上文件窗口 ID，避免不同文件的同名 PDU 窗口冲突
        let title = format!(
            "PDU - {}###PDUWIN{}_{}",
            pdu.name(),
            self.window_id,
            pdu.name()
        );
        let mut is_open = self.is_open;

        let mut window = ui
            .window(&title)
            .size([780.0, 460.0], Condition::FirstUseEver)
            .opened(&mut is_open);
        if self.focus_requested {
            window = window.focused(true);
            self.focus_requested = false;
        }

        window.build(|| {
            ui.text(format!(
                "Length: {} bytes  |  Type: {}",
                pdu.length(),
                pdu.kind().label()
            ));
            if !pdu.comment().is_empty() {
                ui.text(format!("Comment: {}", pdu.comment()));
            }

            ui.separator();

            if ui.small_button("+ Add Signal") {
                event = PduWindowEvent::AddSignal;
            }

            let mut signals = pdu.signals().clone();
            // 按上次选择的列排序
            let col = self.sort_column_idx;
            let asc = self.sort_ascending;
            signals.sort_by(|a, b| {
                let cmp = match col {
                    0 => a.name().cmp(b.name()),
                    1 => a.start_bit().cmp(&b.start_bit()),
                    2 => a.length_bits().cmp(&b.length_bits()),
                    3 => format!("{:?}", a.byte_order()).cmp(&format!("{:?}", b.byte_order())),
                    4 => format!("{:?}", a.value_type()).cmp(&format!("{:?}", b.value_type())),
                    5 => a.factor().partial_cmp(&b.factor()).unwrap_or(std::cmp::Ordering::Equal),
                    6 => a.offset().partial_cmp(&b.offset()).unwrap_or(std::cmp::Ordering::Equal),
                    7 => a.unit().cmp(b.unit()),
                    8 => a.receivers().join(",").cmp(&b.receivers().join(",")),
                    9 => a.comment().cmp(b.comment()),
                    _ => std::cmp::Ordering::Equal,
                };
                if asc { cmp } else { cmp.reverse() }
            });
            if signals.is_empty() {
                ui.same_line();
                ui.text("No signals in this PDU");
                return;
            }

            // 位布局放进限高、可滚动的子区域，长 PDU 不会把信号表挤出窗口
            const LAYOUT_MAX_H: f32 = 200.0;
            let selected = self.selected_signal_names.clone();
            ui.child_window("pdu_bit_layout_scroll")
                .size([0.0, LAYOUT_MAX_H])
                .build(ui, || {
                    render_bit_layout(ui, &signals, pdu.length() as usize, &selected);
                });
            ui.separator();

            // 键盘导航
            if ui.is_window_focused() && !ui.io().want_capture_keyboard() {
                let shift = ui.io().key_shift();
                let cursor_pos = self
                    .signal_cursor
                    .as_ref()
                    .and_then(|name| signals.iter().position(|s| s.name() == name.as_str()));

                let mut new_pos: Option<usize> = None;
                if ui.is_key_pressed(dear_imgui_rs::Key::DownArrow) {
                    new_pos = Some(match cursor_pos {
                        Some(p) => (p + 1).min(signals.len() - 1),
                        None => 0,
                    });
                }
                if ui.is_key_pressed(dear_imgui_rs::Key::UpArrow) {
                    new_pos = Some(match cursor_pos {
                        Some(0) => 0,
                        Some(p) => p - 1,
                        None => signals.len() - 1,
                    });
                }

                if let Some(pos) = new_pos {
                    let new_name = signals[pos].name().to_string();
                    self.signal_cursor = Some(new_name.clone());
                    if shift {
                        let anchor = self
                            .signal_anchor
                            .clone()
                            .unwrap_or_else(|| new_name.clone());
                        let anchor_pos = signals
                            .iter()
                            .position(|s| s.name() == anchor.as_str())
                            .unwrap_or(pos);
                        let (lo, hi) = if anchor_pos <= pos {
                            (anchor_pos, pos)
                        } else {
                            (pos, anchor_pos)
                        };
                        self.selected_signal_names = signals[lo..=hi]
                            .iter()
                            .map(|s| s.name().to_string())
                            .collect();
                    } else {
                        self.selected_signal_names = vec![new_name.clone()];
                        self.signal_anchor = Some(new_name);
                    }
                }
                if ui.is_key_pressed(dear_imgui_rs::Key::Enter) {
                    let target = self
                        .signal_cursor
                        .clone()
                        .or_else(|| self.selected_signal_names.first().cloned());
                    if let Some(sig_name) = target {
                        event = PduWindowEvent::EditSignal(sig_name);
                    }
                }
            }

            // 信号表占满窗口剩余高度
            let avail = ui.content_region_avail();
            ui.table("pdu_signal_table")
                .flags(TableFlags::RESIZABLE | TableFlags::BORDERS | TableFlags::SCROLL_X | TableFlags::SCROLL_Y | TableFlags::SORTABLE | TableFlags::ROW_BG)
                .sizing_policy(TableSizingPolicy::FixedFit)
                .freeze(0, 1)
                .outer_size([0.0, avail[1].max(120.0)])
                .column("Name").done()
                .column("Start").done()
                .column("Bits").done()
                .column("Byte Order").done()
                .column("Type").done()
                .column("Factor").done()
                .column("Offset").done()
                .column("Unit").done()
                .column("Receivers").done()
                .column("Comment").weight(1.0).done()
                .headers(true)
                .build(|ui| {
                    if let Some(mut specs) = ui.table_get_sort_specs()
                        && specs.is_dirty()
                    {
                        if let Some(spec) = specs.iter().next() {
                            self.sort_column_idx = spec.column_index.get();
                            self.sort_ascending =
                                spec.sort_direction == dear_imgui_rs::SortDirection::Ascending;
                        }
                        specs.clear_dirty(ui);
                    }
                    for (row_pos, signal) in signals.iter().enumerate() {
                        let sig_name = signal.name().to_string();

                        ui.table_next_row();

                        let is_selected = self
                            .selected_signal_names
                            .iter()
                            .any(|n| n == &sig_name);

                        ui.table_set_column_index(0);
                        if ui
                            .selectable_config(&sig_name)
                            .selected(is_selected)
                            .span_all_columns(true)
                            .build()
                        {
                            let ctrl = ui.io().key_ctrl();
                            let shift = ui.io().key_shift();
                            if ctrl {
                                if let Some(pos) = self
                                    .selected_signal_names
                                    .iter()
                                    .position(|n| n == &sig_name)
                                {
                                    self.selected_signal_names.remove(pos);
                                } else {
                                    self.selected_signal_names.push(sig_name.clone());
                                }
                                self.signal_anchor = Some(sig_name.clone());
                                self.signal_cursor = Some(sig_name.clone());
                            } else if shift {
                                let anchor = self
                                    .signal_anchor
                                    .clone()
                                    .unwrap_or_else(|| sig_name.clone());
                                let anchor_pos = signals
                                    .iter()
                                    .position(|s| s.name() == anchor.as_str())
                                    .unwrap_or(row_pos);
                                let (lo, hi) = if anchor_pos <= row_pos {
                                    (anchor_pos, row_pos)
                                } else {
                                    (row_pos, anchor_pos)
                                };
                                self.selected_signal_names = signals[lo..=hi]
                                    .iter()
                                    .map(|s| s.name().to_string())
                                    .collect();
                                self.signal_cursor = Some(sig_name.clone());
                            } else {
                                self.selected_signal_names = vec![sig_name.clone()];
                                self.signal_anchor = Some(sig_name.clone());
                                self.signal_cursor = Some(sig_name.clone());
                            }
                        }
                        if ui.is_item_hovered()
                            && ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left)
                        {
                            event = PduWindowEvent::EditSignal(sig_name.clone());
                        }

                        if let Some(_popup) = ui.begin_popup_context_item_with_label(Some(
                            &format!("pdu_sig_ctx_{}_{}", pdu.name(), sig_name),
                        )) {
                            if !self.selected_signal_names.iter().any(|n| n == &sig_name) {
                                let name_owned = sig_name.clone();
                                self.selected_signal_names = vec![name_owned.clone()];
                                self.signal_anchor = Some(name_owned.clone());
                                self.signal_cursor = Some(name_owned);
                            }
                            let selected_names = self.selected_signal_names.clone();
                            if ui.menu_item("Edit") {
                                event = PduWindowEvent::EditSignal(sig_name.clone());
                            }
                            if ui.menu_item("Copy") {
                                event = PduWindowEvent::CopySignal(selected_names.clone());
                            }
                            if ui.menu_item("Cut") {
                                event = PduWindowEvent::CutSignal(selected_names.clone());
                            }
                            ui.separator();
                            if ui
                                .menu_item_enabled_selected_no_shortcut("Paste", false, has_clipboard)
                            {
                                event = PduWindowEvent::PasteSignal;
                            }
                            ui.separator();
                            if ui.menu_item("Delete") {
                                event = PduWindowEvent::DeleteSignal(selected_names);
                            }
                        }

                        ui.table_set_column_index(1);
                        ui.text(format!("{}", signal.start_bit()));

                        ui.table_set_column_index(2);
                        ui.text(format!("{}", signal.length_bits()));

                        ui.table_set_column_index(3);
                        ui.text(match signal.byte_order() {
                            ByteOrder::BigEndian => "Motorola (BE)",
                            ByteOrder::LittleEndian => "Intel (LE)",
                        });

                        ui.table_set_column_index(4);
                        ui.text(match signal.value_type() {
                            ValueType::Unsigned => "Unsigned",
                            ValueType::Signed => "Signed",
                        });

                        ui.table_set_column_index(5);
                        ui.text(format!("{}", signal.factor()));

                        ui.table_set_column_index(6);
                        ui.text(format!("{}", signal.offset()));

                        ui.table_set_column_index(7);
                        ui.text(signal.unit());

                        ui.table_set_column_index(8);
                        ui.text(signal.receivers().join(", "));

                        ui.table_set_column_index(9);
                        ui.text(signal.comment());
                    }
                });
        });

        self.is_open = is_open;
        event
    }
}
