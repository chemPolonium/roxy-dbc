use dear_imgui_rs::{Condition, TableFlags, Ui};

use crate::fibex::editable_fibex::{EditableFibex, EditableFrame, FrChannel};

/// Static Segment调度矩阵窗口：行 = 周期 (0..64)，列 = Slot (1..gNumberOfStaticSlots)
#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct ScheduleWindow {
    show_channel_a: bool,
    show_channel_b: bool,
}

impl ScheduleWindow {
    pub fn render(
        &mut self,
        ui: &Ui,
        fibex: &EditableFibex,
        fibex_id: usize,
        is_open: &mut bool,
    ) {
        if !*is_open {
            return;
        }

        let cluster = fibex.cluster();
        let title = format!("Schedule Table - {}###SCHEDULE_{}", cluster.name, fibex_id);

        if !self.show_channel_a && !self.show_channel_b {
            self.show_channel_a = true;
            self.show_channel_b = true;
        }

        let frames: Vec<EditableFrame> = fibex.frames().clone();
        let num_slots = cluster.params.number_of_static_slots.clamp(1, 62) as usize;
        let mut channel_a = self.show_channel_a;
        let mut channel_b = self.show_channel_b;

        ui.window(&title)
            .size([900.0, 480.0], Condition::FirstUseEver)
            .opened(is_open)
            .build(|| {
                ui.text(format!(
                    "Static segment: {} slots x {} ms cycle. Cell = frame occupying (slot, cycle).",
                    cluster.params.number_of_static_slots, cluster.params.cycle_time_ms
                ));
                ui.checkbox("Channel A", &mut channel_a);
                ui.same_line();
                ui.checkbox("Channel B", &mut channel_b);

                let any = channel_a || channel_b;
                let on_a = frames.iter().any(|f| f.triggering().channel.covers(FrChannel::A));
                let on_b = frames.iter().any(|f| f.triggering().channel.covers(FrChannel::B));

                if !any || frames.is_empty() {
                    ui.text_disabled("No schedulable frames");
                    return;
                }
                if (channel_a && !on_a) && (channel_b && !on_b) {
                    ui.text_disabled("No frames on the selected channel");
                    return;
                }

                // 每个可见通道的表格平分窗口剩余高度
                let avail = ui.content_region_avail();
                let visible = channel_a as i32 + channel_b as i32;
                let table_h = ((avail[1] - 24.0) / visible.max(1) as f32).max(140.0);

                for (enabled, ch) in [(channel_a, FrChannel::A), (channel_b, FrChannel::B)] {
                    if !enabled {
                        continue;
                    }
                    let ch_frames: Vec<&EditableFrame> = frames
                        .iter()
                        .filter(|f| f.triggering().channel.covers(ch))
                        .collect();

                    ui.separator_with_text(format!("Channel {}", ch.label()).as_str());

                    if ch_frames.is_empty() {
                        ui.text_disabled("No frames on this channel");
                        continue;
                    }

                    let table_id = format!("schedule_table_{}_{}", fibex_id, ch.label());
                    // 注意：TableBuilder.columns() 会替换整个列列表，
                    // 因此 Cycle 列也要放进同一个迭代器
                    let columns = std::iter::once(
                        dear_imgui_rs::TableColumnSetup::new("Cycle".to_string()).fixed_width(60.0),
                    )
                    .chain(
                        (1..=num_slots).map(|slot| {
                            dear_imgui_rs::TableColumnSetup::new(format!("{}", slot))
                                .fixed_width(90.0)
                        }),
                    );
                    ui.table(&table_id)
                        .flags(TableFlags::RESIZABLE | TableFlags::BORDERS | TableFlags::SCROLL_X | TableFlags::SCROLL_Y | TableFlags::ROW_BG | TableFlags::NO_SAVED_SETTINGS)
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
                                    let active: Vec<&EditableFrame> = ch_frames
                                        .iter()
                                        .copied()
                                        .filter(|f| {
                                            f.triggering().slot_id == slot
                                                && f.triggering().is_active_at_cycle(cycle)
                                        })
                                        .collect();
                                    if active.is_empty() {
                                        continue;
                                    }
                                    let names: Vec<&str> =
                                        active.iter().map(|f| f.name()).collect();
                                    let has_conflict = active.len() > 1;
                                    if has_conflict {
                                        ui.text_colored(
                                            [1.0, 0.35, 0.35, 1.0],
                                            names.join(" | "),
                                        );
                                    } else {
                                        ui.text(names.join(" | "));
                                    }
                                }
                            }
                        });
                }
            });

        self.show_channel_a = channel_a;
        self.show_channel_b = channel_b;
    }
}
