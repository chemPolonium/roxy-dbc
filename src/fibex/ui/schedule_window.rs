use dear_imgui_rs::{StyleColor, TableColumnSetup, TableFlags, Ui};

use crate::fibex::editable_fibex::{EditableFibex, EditableFrame, FrChannel, FrameTriggering};

/// 时隙列宽：格子点击区也按这个宽度画
const SLOT_COL_W: f32 = 120.0;

/// Static Segment 调度矩阵：行 = 周期 (0..64)，列 = Slot (1..gNumberOfStaticSlots)。
/// 选中一个帧（Frames 页里选，或在本网格里点已排程的格子）后，点空格子即可改排程。
#[derive(Clone)]
pub struct ScheduleWindow {
    show_channel_a: bool,
    show_channel_b: bool,
    /// 一次性操作反馈：(是否为错误, 文本)
    message: Option<(bool, String)>,
}

impl Default for ScheduleWindow {
    fn default() -> Self {
        Self {
            show_channel_a: true,
            show_channel_b: true,
            message: None,
        }
    }
}

/// 网格里的一次交互结果
pub enum ScheduleEvent {
    None,
    /// 点了已排程的格子，要求把该帧设为选中
    SelectFrame(String),
    /// 排程已被修改，调用方需标记为已改动并保持该帧选中
    Modified(String),
}

impl ScheduleWindow {
    pub fn render_content(
        &mut self,
        ui: &Ui,
        fibex: &mut EditableFibex,
        fibex_id: usize,
        picked: Option<String>,
    ) -> ScheduleEvent {
        let mut event = ScheduleEvent::None;
        let frames: Vec<EditableFrame> = fibex.frames().clone();
        // 列数取集群声明的静态时隙数，但不小于实际用到的最大时隙号，
        // 否则超出的帧在矩阵里根本无处显示
        let declared_slots = fibex.cluster().params.number_of_static_slots.max(1);
        let cycle_ms = fibex.cluster().params.cycle_time_ms;
        let used_slots = frames
            .iter()
            .flat_map(|f| f.triggerings().iter().map(|t| t.slot_id))
            .max()
            .unwrap_or(0);
        let num_slots = declared_slots.max(used_slots).clamp(1, 2048) as usize;
        let mut channel_a = self.show_channel_a;
        let mut channel_b = self.show_channel_b;

        ui.checkbox("Channel A", &mut channel_a);
        ui.same_line();
        ui.checkbox("Channel B", &mut channel_b);
        ui.same_line();
        match &picked {
            Some(name) => {
                if ui.button("Unschedule") {
                    fibex.unschedule_frame(name);
                    self.message = Some((false, format!("'{name}' unscheduled: slot cleared")));
                    event = ScheduleEvent::Modified(name.clone());
                }
                ui.same_line();
                ui.text_disabled(format!("Scheduling '{name}' - click an empty cell"));
            }
            None => ui.text_disabled("Select a frame first (Frames tab, or a scheduled cell)"),
        }
        if let Some((is_error, message)) = &self.message {
            let color = if *is_error {
                [1.0, 0.4, 0.4, 1.0]
            } else {
                [0.5, 0.9, 0.5, 1.0]
            };
            ui.text_colored(color, message.clone());
        } else {
            ui.text_disabled(format!(
                "Static segment: {} slots x {} ms cycle. Red cell = two frames share this slot and cycle.",
                declared_slots, cycle_ms
            ));
        }

        let any = channel_a || channel_b;
        let on_a = frames
            .iter()
            .any(|f| f.channel_triggering(FrChannel::A).is_some());
        let on_b = frames
            .iter()
            .any(|f| f.channel_triggering(FrChannel::B).is_some());

        if !any || frames.is_empty() {
            ui.text_disabled("No schedulable frames");
            return event;
        }
        if (channel_a && !on_a) && (channel_b && !on_b) {
            ui.text_disabled("No frames on the selected channel");
            return event;
        }

        // 每张表格按"还剩几张没画"均分此刻剩余的高度：最后一张正好吃掉剩下的全部，
        // 外层窗口就不会再出现滚动条
        let mut tables_left = (channel_a as i32 + channel_b as i32).max(1);

        for (enabled, ch) in [(channel_a, FrChannel::A), (channel_b, FrChannel::B)] {
            if !enabled {
                continue;
            }
            let ch_frames: Vec<(&EditableFrame, FrameTriggering)> = frames
                .iter()
                .filter_map(|f| f.channel_triggering(ch).map(|t| (f, t)))
                .collect();

            ui.separator_with_text(format!("Channel {}", ch.label()).as_str());

            if ch_frames.is_empty() {
                ui.text_disabled("No frames on this channel");
                tables_left -= 1;
                continue;
            }

            let table_h = (ui.content_region_avail()[1] / tables_left.max(1) as f32).max(120.0);
            tables_left -= 1;

            let table_id = format!("schedule_table_{}_{}", fibex_id, ch.label());
            // 注意：TableBuilder.columns() 会替换整个列列表，
            // 因此 Cycle 列也要放进同一个迭代器
            let columns =
                std::iter::once(TableColumnSetup::new("Cycle".to_string()).fixed_width(60.0))
                    .chain((1..=num_slots).map(|slot| {
                        TableColumnSetup::new(format!("{}", slot)).fixed_width(SLOT_COL_W)
                    }));
            let picked_cell = picked.clone();
            ui.table(&table_id)
                .flags(
                    // 不给 RESIZABLE：格子点击区按 SLOT_COL_W 画，改列宽会让两者错位
                    TableFlags::BORDERS
                        | TableFlags::SCROLL_X
                        | TableFlags::SCROLL_Y
                        | TableFlags::ROW_BG
                        | TableFlags::NO_SAVED_SETTINGS,
                )
                .freeze(1, 1)
                .outer_size([0.0, table_h])
                .columns(columns)
                .headers(true)
                .build(|ui| {
                    for cycle in 0..64u32 {
                        ui.table_next_row();
                        ui.table_set_column_index(0);
                        ui.text(format!("{}", cycle));

                        for slot in 1..=num_slots as u32 {
                            ui.table_set_column_index(slot as usize);
                            let active: Vec<&str> = ch_frames
                                .iter()
                                .filter(|(_, t)| {
                                    t.slot_id == slot && t.is_active_at_cycle(cycle)
                                })
                                .map(|(f, _)| f.name())
                                .collect();
                            let label = if active.is_empty() {
                                " ".to_string()
                            } else {
                                active.join(" | ")
                            };
                            let conflict = active.len() > 1;
                            // ColorStackToken 在 drop 时恢复颜色，故显式限定其作用域
                            let color_token = conflict
                                .then(|| ui.push_style_color(StyleColor::Text, [1.0, 0.35, 0.35, 1.0]));
                            let cell =
                                format!("{}##sched_{}_{}_{}", label, ch.label(), slot, cycle);
                            // 不用 span_all_columns：那会让每格向右覆盖整行，
                            // 命中判定落到别致的格子上；按列宽定尺寸即可
                            let clicked = ui
                                .selectable_config(cell.as_str())
                                .size([SLOT_COL_W, 0.0])
                                .build();
                            drop(color_token);
                            if !clicked {
                                continue;
                            }
                            match active.first() {
                                Some(name) => {
                                    // 重新选中帧时清掉上一条提示
                                    self.message = None;
                                    event = ScheduleEvent::SelectFrame(name.to_string());
                                }
                                None => match &picked_cell {
                                    Some(name) => match fibex.schedule_frame(name, ch, slot, cycle)
                                    {
                                        Ok(()) => {
                                            let rep = fibex
                                                .get_frame(name)
                                                .and_then(|f| f.channel_triggering(ch))
                                                .map(|t| t.cycle_repetition)
                                                .unwrap_or(1)
                                                .max(1);
                                            self.message = Some((
                                                false,
                                                if rep <= 1 {
                                                    format!(
                                                        "'{name}' -> slot {slot} on channel {}, every cycle",
                                                        ch.label()
                                                    )
                                                } else {
                                                    format!(
                                                        "'{name}' -> slot {slot} on channel {}, every {rep} cycles from cycle {cycle}",
                                                        ch.label()
                                                    )
                                                },
                                            ));
                                            event = ScheduleEvent::Modified(name.clone());
                                        }
                                        Err(reason) => self.message = Some((true, reason)),
                                    },
                                    None => {
                                        self.message = Some((
                                            true,
                                            "Select a frame before scheduling an empty cell"
                                                .to_string(),
                                        ));
                                    }
                                },
                            }
                        }
                    }
                });
        }

        self.show_channel_a = channel_a;
        self.show_channel_b = channel_b;
        event
    }
}
