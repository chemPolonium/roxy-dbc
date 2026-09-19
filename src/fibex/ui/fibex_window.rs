use std::path::Path;

use crate::fibex::editable_fibex::{ByteOrder, EditableFrame, EditableFibex, ValueType};
use crate::fibex::ui::frame_edit_window::{FrameEditEvent, FrameEditWindowState};
use crate::fibex::ui::frame_window::{FrameWindow, FrameWindowEvent};
use crate::fibex::ui::pdu_edit_window::{PduEditEvent, PduEditWindowState};
use crate::fibex::ui::pdu_window::{PduWindow, PduWindowEvent};
use crate::fibex::ui::schedule_window::ScheduleWindow;
use crate::fibex::ui::signal_edit_window::{SignalEditDialog, SignalEditEvent};
use crate::fibex::state::{DeleteTarget, FibexUiState};
use dear_imgui_rs::{Condition, SortDirection, TableFlags, TableSizingPolicy, Ui};

#[derive(Clone)]
pub struct FibexWindow {
    pub fibex_id: usize,
    pub is_open: bool,
    pub file_path: String,
    pub fibex: EditableFibex,
    pub is_dirty: bool,
    /// 文本编码：打开时检测（UTF-8 / GBK），Save时按原编码写出
    pub text_encoding: &'static encoding_rs::Encoding,
    pub had_bom: bool,

    pub selected_frame_names: Vec<String>,
    selection_anchor: Option<String>,
    selection_cursor: Option<String>,
    search_query: String,
    sort_column_idx: usize,
    sort_ascending: bool,
    pdu_sort_column_idx: usize,
    pdu_sort_ascending: bool,
    pdu_search_query: String,
    signal_search_query: String,
    pub selected_pdu_names: Vec<String>,
    pdu_selection_anchor: Option<String>,
    /// 信号列表页的选中行（pdu 名, 信号名），支持 Ctrl / Shift 多选
    pub selected_signal_rows: Vec<(String, String)>,
    signal_selection_anchor: Option<(String, String)>,
    signal_selection_cursor: Option<(String, String)>,

    pub frame_windows: Vec<FrameWindow>,
    pub pdu_windows: Vec<PduWindow>,
    edit_frame_windows: Vec<FrameEditWindowState>,
    edit_pdu_windows: Vec<PduEditWindowState>,
    signal_edit_dialog: SignalEditDialog,
    pub schedule_window: ScheduleWindow,
    pub show_schedule: bool,
    /// 待Confirm Delete的信号，按 PDU 分组：(PDU 名, 信号名列表)
    pending_signal_delete: Option<Vec<(String, Vec<String>)>>,
    pending_pdu_delete: Option<Vec<String>>,
}

impl FibexWindow {
    pub fn new(fibex_id: usize, file_path: &str, fibex: EditableFibex) -> Self {
        Self {
            fibex_id,
            is_open: true,
            file_path: file_path.to_string(),
            fibex,
            is_dirty: false,
            text_encoding: encoding_rs::UTF_8,
            had_bom: false,
            selected_frame_names: Vec::new(),
            selection_anchor: None,
            selection_cursor: None,
            search_query: String::new(),
            sort_column_idx: 0,
            sort_ascending: true,
            pdu_sort_column_idx: 0,
            pdu_sort_ascending: true,
            pdu_search_query: String::new(),
            signal_search_query: String::new(),
            selected_pdu_names: Vec::new(),
            pdu_selection_anchor: None,
            selected_signal_rows: Vec::new(),
            signal_selection_anchor: None,
            signal_selection_cursor: None,
            frame_windows: Vec::new(),
            pdu_windows: Vec::new(),
            edit_frame_windows: Vec::new(),
            edit_pdu_windows: Vec::new(),
            signal_edit_dialog: SignalEditDialog::default(),
            schedule_window: ScheduleWindow::default(),
            show_schedule: false,
            pending_signal_delete: None,
            pending_pdu_delete: None,
        }
    }

    pub fn from_path(file_path: &Path) -> Result<Self, String> {
        let (fibex, encoding, had_bom) = crate::fibex::import::import_file(file_path)
            .map_err(|e| format!("Failed to parse {}: {}", file_path.display(), e))?;
        let mut window = Self::new(0, &file_path.to_string_lossy(), fibex);
        window.text_encoding = encoding;
        window.had_bom = had_bom;
        Ok(window)
    }

    pub fn selected_frame_names(&self) -> Vec<String> {
        self.selected_frame_names.clone()
    }

    pub fn set_selected_frame_name(&mut self, name: Option<&str>) {
        match name {
            Some(n) => {
                self.selected_frame_names = vec![n.to_string()];
                self.selection_anchor = Some(n.to_string());
                self.selection_cursor = Some(n.to_string());
            }
            None => self.clear_selection(),
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_frame_names.clear();
        self.selection_anchor = None;
        self.selection_cursor = None;
    }

    fn open_frame_window(&mut self, frame_name: &str) {
        if self.frame_windows.iter().any(|w| w.frame_name == frame_name) {
            return;
        }
        if self.fibex.get_frame(frame_name).is_some() {
            let win = FrameWindow::new(self.fibex_id, frame_name);
            self.frame_windows.push(win);
        }
    }

    pub fn open_pdu_window(&mut self, pdu_name: &str) {
        if self.pdu_windows.iter().any(|w| w.pdu_name == pdu_name) {
            return;
        }
        if self.fibex.get_pdu(pdu_name).is_some() {
            let win = PduWindow::new(self.fibex_id, pdu_name);
            self.pdu_windows.push(win);
        }
    }

    // ------------------------------------------------------------------
    // 帧总表
    // ------------------------------------------------------------------

    fn render_frame_table(&mut self, ui: &Ui, has_clipboard: bool) -> FrameTableEvent {
        let mut event = FrameTableEvent::None;

        if ui.button("+ Add Frame") {
            event = FrameTableEvent::AddFrame;
        }
        ui.same_line();
        ui.input_text("##frame_search", &mut self.search_query)
            .hint("Filter frames...")
            .build();
        ui.same_line();
        ui.text_disabled(format!("{} frames", self.fibex.frames().len()));

        // 闭包内需要可变访问 self，这里克隆一份帧数据避免借用冲突
        let frames: Vec<EditableFrame> = self.fibex.frames().clone();
        let query_lower = self.search_query.to_lowercase();

        let mut filtered: Vec<usize> = frames
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                query_lower.is_empty() || f.name().to_lowercase().contains(&query_lower)
            })
            .map(|(i, _)| i)
            .collect();

        let col = self.sort_column_idx;
        let asc = self.sort_ascending;
        filtered.sort_by(|&a, &b| {
            let fa = &frames[a];
            let fb = &frames[b];
            let cmp = match col {
                0 => fa.name().cmp(fb.name()),
                1 => fa.length().cmp(&fb.length()),
                2 => format!("{:?}", fa.triggering().channel).cmp(&format!("{:?}", fb.triggering().channel)),
                3 => fa.triggering().slot_id.cmp(&fb.triggering().slot_id),
                4 => fa.triggering().base_cycle.cmp(&fb.triggering().base_cycle),
                5 => fa.triggering().cycle_repetition.cmp(&fb.triggering().cycle_repetition),
                6 => fa.triggering().startup.cmp(&fb.triggering().startup),
                7 => fa.pdus().len().cmp(&fb.pdus().len()),
                8 => fa.comment().cmp(fb.comment()),
                _ => std::cmp::Ordering::Equal,
            };
            if asc { cmp } else { cmp.reverse() }
        });

        // 键盘导航
        if ui.is_window_focused() && !ui.io().want_capture_keyboard() && !filtered.is_empty() {
            let shift = ui.io().key_shift();
            let cursor_pos = self
                .selection_cursor
                .as_ref()
                .and_then(|n| filtered.iter().position(|&idx| frames[idx].name() == n.as_str()));

            let move_cursor = |pos: Option<usize>, down: bool| -> usize {
                match pos {
                    Some(p) if down => (p + 1).min(filtered.len() - 1),
                    Some(0) => 0,
                    Some(p) => p - 1,
                    None if down => 0,
                    None => filtered.len() - 1,
                }
            };

            let mut new_pos: Option<usize> = None;
            if ui.is_key_pressed(dear_imgui_rs::Key::DownArrow) {
                new_pos = Some(move_cursor(cursor_pos, true));
            }
            if ui.is_key_pressed(dear_imgui_rs::Key::UpArrow) {
                new_pos = Some(move_cursor(cursor_pos, false));
            }

            if let Some(pos) = new_pos {
                let new_name = frames[filtered[pos]].name().to_string();
                self.selection_cursor = Some(new_name.clone());
                if shift {
                    let anchor = self
                        .selection_anchor
                        .clone()
                        .unwrap_or_else(|| new_name.clone());
                    let anchor_pos = filtered
                        .iter()
                        .position(|&idx| frames[idx].name() == anchor)
                        .unwrap_or(pos);
                    let (lo, hi) = if anchor_pos <= pos { (anchor_pos, pos) } else { (pos, anchor_pos) };
                    self.selected_frame_names = filtered[lo..=hi]
                        .iter()
                        .map(|&idx| frames[idx].name().to_string())
                        .collect();
                } else {
                    self.selected_frame_names = vec![new_name.clone()];
                    self.selection_anchor = Some(new_name);
                }
            }
            if ui.is_key_pressed(dear_imgui_rs::Key::Enter) {
                let target = self
                    .selection_cursor
                    .clone()
                    .or_else(|| self.selected_frame_names.first().cloned());
                if let Some(name) = target {
                    event = FrameTableEvent::OpenFrame(name);
                }
            }
        }

        // 工具栏行结束；实测剩余高度让表格正好填满
        let avail = ui.content_region_avail();

        ui.table("frames_table")
            .flags(TableFlags::RESIZABLE | TableFlags::BORDERS | TableFlags::SCROLL_X | TableFlags::SCROLL_Y | TableFlags::SORTABLE | TableFlags::ROW_BG)
            .sizing_policy(TableSizingPolicy::FixedFit)
            .freeze(0, 1)
            .outer_size([0.0, avail[1].max(120.0)])
            .column("Name").done()
            .column("Length").done()
            .column("Channel").done()
            .column("Slot").done()
            .column("Base Cycle").done()
            .column("Repetition").done()
            .column("Startup").done()
            .column("PDUs").done()
            .column("Comment").weight(1.0).done()
            .headers(true)
            .build(|ui| {
                if let Some(mut specs) = ui.table_get_sort_specs()
                    && specs.is_dirty()
                {
                    if let Some(spec) = specs.iter().next() {
                        self.sort_column_idx = spec.column_index.get();
                        self.sort_ascending = spec.sort_direction == SortDirection::Ascending;
                    }
                    specs.clear_dirty(ui);
                }

                for (row_pos, frame) in filtered.iter().map(|&idx| &frames[idx]).enumerate() {
                    let frame_name = frame.name().to_string();
                    let t = frame.triggering();

                    ui.table_next_row();

                    let is_selected = self.selected_frame_names.iter().any(|n| n == &frame_name);

                    ui.table_set_column_index(0);
                    if ui
                        .selectable_config(&frame_name)
                        .selected(is_selected)
                        .span_all_columns(true)
                        .build()
                    {
                        let ctrl = ui.io().key_ctrl();
                        let shift = ui.io().key_shift();
                        if ctrl {
                            if let Some(pos) = self
                                .selected_frame_names
                                .iter()
                                .position(|n| n == &frame_name)
                            {
                                self.selected_frame_names.remove(pos);
                            } else {
                                self.selected_frame_names.push(frame_name.clone());
                            }
                            self.selection_anchor = Some(frame_name.clone());
                            self.selection_cursor = Some(frame_name.clone());
                        } else if shift {
                            let anchor = self
                                .selection_anchor
                                .clone()
                                .unwrap_or_else(|| frame_name.clone());
                            let anchor_pos = filtered
                                .iter()
                                .position(|&i| frames[i].name() == anchor)
                                .unwrap_or(row_pos);
                            let (lo, hi) = if anchor_pos <= row_pos {
                                (anchor_pos, row_pos)
                            } else {
                                (row_pos, anchor_pos)
                            };
                            self.selected_frame_names = filtered[lo..=hi]
                                .iter()
                                .map(|&i| frames[i].name().to_string())
                                .collect();
                            self.selection_cursor = Some(frame_name.clone());
                        } else {
                            self.selected_frame_names = vec![frame_name.clone()];
                            self.selection_anchor = Some(frame_name.clone());
                            self.selection_cursor = Some(frame_name.clone());
                        }
                    }
                    if ui.is_item_hovered()
                        && ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left)
                    {
                        event = FrameTableEvent::OpenFrame(frame_name.clone());
                    }

                    if let Some(_popup) = ui.begin_popup_context_item_with_label(Some(&format!(
                        "frame_ctx_{}_{}",
                        self.fibex_id, frame_name
                    ))) {
                        if !self.selected_frame_names.iter().any(|n| n == &frame_name) {
                            self.selected_frame_names = vec![frame_name.clone()];
                            self.selection_anchor = Some(frame_name.clone());
                            self.selection_cursor = Some(frame_name.clone());
                        }
                        let selected_names = self.selected_frame_names.clone();
                        if ui.menu_item("Open") {
                            event = FrameTableEvent::OpenFrame(frame_name.clone());
                        }
                        if ui.menu_item("Edit") {
                            event = FrameTableEvent::EditFrame(frame_name.clone());
                        }
                        if ui.menu_item("Copy") {
                            event = FrameTableEvent::CopyFrame(selected_names.clone());
                        }
                        if ui.menu_item("Cut") {
                            event = FrameTableEvent::CutFrame(selected_names.clone());
                        }
                        ui.separator();
                        if ui
                            .menu_item_enabled_selected_no_shortcut("Paste", false, has_clipboard)
                        {
                            event = FrameTableEvent::PasteFrame;
                        }
                        ui.separator();
                        if ui.menu_item("Delete") {
                            event = FrameTableEvent::DeleteFrame(selected_names);
                        }
                    }

                    ui.table_set_column_index(1);
                    ui.text(format!("{}", frame.length()));

                    ui.table_set_column_index(2);
                    ui.text(t.channel.label());

                    ui.table_set_column_index(3);
                    ui.text(format!("{}", t.slot_id));

                    ui.table_set_column_index(4);
                    ui.text(format!("{}", t.base_cycle));

                    ui.table_set_column_index(5);
                    ui.text(format!("{}", t.cycle_repetition));

                    ui.table_set_column_index(6);
                    ui.text(if t.startup { "Yes" } else { "-" });

                    ui.table_set_column_index(7);
                    ui.text(format!("{}", frame.pdus().len()));

                    ui.table_set_column_index(8);
                    ui.text(frame.comment());
                }
            });

        event
    }

    // ------------------------------------------------------------------
    // 浮动子窗口（Frame / PDU / 编辑窗口 / 对话框）
    // ------------------------------------------------------------------

    pub fn render_floating_windows(&mut self, ui: &Ui, clipboard: &mut crate::fibex::state::ClipboardState) {
        // Frame 窗口
        let mut to_open_pdu: Option<String> = None;
        for i in 0..self.frame_windows.len() {
            let win_event = {
                let win = &mut self.frame_windows[i];
                win.render(ui, &self.fibex)
            };
            match win_event {
                FrameWindowEvent::EditPdu(pdu_name) => {
                    if let Some(pdu) = self.fibex.get_pdu(&pdu_name) {
                        let edit_win = PduEditWindowState::open(&pdu_name, pdu);
                        self.edit_pdu_windows.push(edit_win);
                    }
                }
                FrameWindowEvent::OpenPdu(pdu_name) => {
                    to_open_pdu = Some(pdu_name);
                }
                FrameWindowEvent::AddExistingPdu(pdu_name) => {
                    let frame_name = self.frame_windows[i].frame_name.clone();
                    if self.fibex.get_pdu(&pdu_name).is_some() {
                        let mapping = crate::fibex::editable_fibex::FramePduMapping::new(
                            &pdu_name,
                            next_free_start(&self.fibex, &frame_name),
                        );
                        self.fibex.add_frame_pdu(&frame_name, mapping);
                        self.is_dirty = true;
                    }
                }
                FrameWindowEvent::RemovePdu(pdu_name) => {
                    let frame_name = self.frame_windows[i].frame_name.clone();
                    self.fibex.delete_frame_pdu(&frame_name, &pdu_name);
                    self.is_dirty = true;
                }
                FrameWindowEvent::SetPduStart(pdu_name, start) => {
                    let frame_name = self.frame_windows[i].frame_name.clone();
                    self.fibex.set_frame_pdu_start(&frame_name, &pdu_name, start);
                    self.is_dirty = true;
                }
                FrameWindowEvent::NewPdu => {
                    let frame_name = self.frame_windows[i].frame_name.clone();
                    let before = self.fibex.pdu_count();
                    self.fibex.new_pdu();
                    self.is_dirty = true;
                    if self.fibex.pdu_count() > before {
                        let new_pdu = self.fibex.pdus().last().unwrap().name().to_string();
                        let mapping = crate::fibex::editable_fibex::FramePduMapping::new(
                            &new_pdu,
                            next_free_start(&self.fibex, &frame_name),
                        );
                        self.fibex.add_frame_pdu(&frame_name, mapping);
                        to_open_pdu = Some(new_pdu);
                    }
                }
                FrameWindowEvent::None => {}
            }
        }
        // 删除已关闭的 Frame 窗口
        self.frame_windows.retain(|w| w.is_open);

        if let Some(pdu_name) = to_open_pdu {
            self.open_pdu_window(&pdu_name);
        }

        // PDU 窗口
        let mut signal_events: Vec<(String, PduWindowEvent)> = Vec::new();
        for i in 0..self.pdu_windows.len() {
            let pdu_name = self.pdu_windows[i].pdu_name.clone();
            let has_sig_clipboard = !clipboard.copied_signals.is_empty();
            let win_event = self.pdu_windows[i].render(ui, &self.fibex, has_sig_clipboard);
            if !matches!(win_event, PduWindowEvent::None) {
                signal_events.push((pdu_name, win_event));
            }
        }
        self.pdu_windows.retain(|w| w.is_open);

        for (pdu_name, event) in signal_events {
            match event {
                PduWindowEvent::EditSignal(sig_name) => {
                    if let Some(pdu) = self.fibex.get_pdu(&pdu_name)
                        && let Some(sig) = pdu.signals().iter().find(|s| s.name() == sig_name)
                    {
                        self.signal_edit_dialog.open_from_signal(&pdu_name, sig);
                    }
                }
                PduWindowEvent::CopySignal(names) => {
                    if let Some(pdu) = self.fibex.get_pdu(&pdu_name) {
                        let sigs: Vec<crate::fibex::editable_fibex::EditableSignal> = names
                            .iter()
                            .filter_map(|n| {
                                pdu.signals()
                                    .iter()
                                    .find(|s| s.name() == n.as_str())
                                    .cloned()
                            })
                            .collect();
                        if !sigs.is_empty() {
                            clipboard.copied_signals = sigs;
                        }
                    }
                }
                PduWindowEvent::CutSignal(names) => {
                    if let Some(pdu) = self.fibex.get_pdu(&pdu_name) {
                        let sigs: Vec<crate::fibex::editable_fibex::EditableSignal> = names
                            .iter()
                            .filter_map(|n| {
                                pdu.signals()
                                    .iter()
                                    .find(|s| s.name() == n.as_str())
                                    .cloned()
                            })
                            .collect();
                        if !sigs.is_empty() {
                            clipboard.copied_signals = sigs;
                        }
                    }
                    self.pending_signal_delete = Some(vec![(pdu_name, names)]);
                }
                PduWindowEvent::DeleteSignal(names) => {
                    self.pending_signal_delete = Some(vec![(pdu_name, names)]);
                }
                PduWindowEvent::PasteSignal => {
                    let copied = clipboard.copied_signals.clone();
                    if copied.is_empty() {
                        return;
                    }
                    for sig in copied {
                        // 粘贴重名信号时自动追加序号，避免静默丢弃
                        let mut suffix = 1;
                        let mut new_name = format!("{}_copy", sig.name());
                        while self
                            .fibex
                            .find_signal_index(&pdu_name, &new_name)
                            .is_some()
                        {
                            suffix += 1;
                            new_name = format!("{}_copy{}", sig.name(), suffix);
                        }
                        let mut new_sig = sig;
                        new_sig.set_name(&new_name);
                        self.fibex.add_signal(&pdu_name, &new_sig);
                    }
                    self.is_dirty = true;
                }
                PduWindowEvent::AddSignal => {
                    self.fibex.new_signal(&pdu_name);
                    self.is_dirty = true;
                }
                PduWindowEvent::None => {}
            }
        }

        // 调度矩阵窗口
        self.schedule_window
            .render(ui, &self.fibex, self.fibex_id, &mut self.show_schedule);

        // Frame 编辑窗口
        let mut to_remove: Option<usize> = None;
        for (i, edit_win) in self.edit_frame_windows.iter_mut().enumerate() {
            let event = edit_win.render(ui);
            match event {
                FrameEditEvent::Apply => {
                    edit_win.apply_edit(&mut self.fibex);
                    self.is_dirty = true;
                }
                FrameEditEvent::Ok => {
                    edit_win.apply_edit(&mut self.fibex);
                    self.is_dirty = true;
                    to_remove = Some(i);
                }
                FrameEditEvent::Cancel => {
                    to_remove = Some(i);
                }
                FrameEditEvent::None => {}
            }
        }
        if let Some(idx) = to_remove {
            self.edit_frame_windows.remove(idx);
        }

        // PDU 编辑窗口
        let mut to_remove: Option<usize> = None;
        for (i, edit_win) in self.edit_pdu_windows.iter_mut().enumerate() {
            let event = edit_win.render(ui);
            match event {
                PduEditEvent::Apply => {
                    edit_win.apply_edit(&mut self.fibex);
                    self.is_dirty = true;
                }
                PduEditEvent::Ok => {
                    edit_win.apply_edit(&mut self.fibex);
                    self.is_dirty = true;
                    to_remove = Some(i);
                }
                PduEditEvent::Cancel => {
                    to_remove = Some(i);
                }
                PduEditEvent::None => {}
            }
        }
        if let Some(idx) = to_remove {
            self.edit_pdu_windows.remove(idx);
        }

        // 信号编辑对话框
        let dialog_was_shown = self.signal_edit_dialog.show;
        self.render_signal_edit_dialog(ui);
        if dialog_was_shown && !self.signal_edit_dialog.show {
            let pdu = self.signal_edit_dialog.pdu_name.clone();
            if let Some(pdu_win) = self.pdu_windows.iter_mut().find(|w| w.pdu_name == pdu) {
                pdu_win.focus_requested = true;
            }
        }
    }

    fn render_pdu_master_content(&mut self, ui: &Ui) {
        if ui.button("+ Add PDU") {
            self.handle_pdu_master_event(PduMasterEvent::AddPdu);
        }
        ui.same_line();
        ui.input_text("##pdu_search", &mut self.pdu_search_query)
            .hint("Filter PDUs...")
            .build();
        ui.same_line();
        let signal_total: usize = self.fibex.pdus().iter().map(|p| p.signals().len()).sum();
        ui.text_disabled(format!(
            "{} PDUs  |  {} signals",
            self.fibex.pdus().len(),
            signal_total
        ));
        // 工具栏行结束；实测剩余高度让表格正好填满
        let avail = ui.content_region_avail();

        // 表格闭包需要可变访问 self，这里克隆一份 PDU 数据避免借用冲突
        let mut pdus: Vec<crate::fibex::editable_fibex::EditablePdu> = self.fibex.pdus().clone();
        // 筛选
        let query = self.pdu_search_query.to_lowercase();
        if !query.is_empty() {
            pdus.retain(|p| p.name().to_lowercase().contains(&query));
        }
        // 按上次选择的列排序
        let col = self.pdu_sort_column_idx;
        let asc = self.pdu_sort_ascending;
        pdus.sort_by(|a, b| {
            let cmp = match col {
                0 => a.name().cmp(b.name()),
                1 => a.length().cmp(&b.length()),
                2 => a.kind().label().cmp(b.kind().label()),
                3 => a.signals().len().cmp(&b.signals().len()),
                4 => a.senders().join(",").cmp(&b.senders().join(",")),
                5 => a.receivers().join(",").cmp(&b.receivers().join(",")),
                6 => a.comment().cmp(b.comment()),
                _ => std::cmp::Ordering::Equal,
            };
            if asc { cmp } else { cmp.reverse() }
        });
        let mut event = PduMasterEvent::None;

        ui.table("pdus_master_table")
            .flags(TableFlags::RESIZABLE | TableFlags::BORDERS | TableFlags::SCROLL_X | TableFlags::SCROLL_Y | TableFlags::SORTABLE | TableFlags::ROW_BG)
            .sizing_policy(TableSizingPolicy::FixedFit)
            .freeze(0, 1)
            .outer_size([0.0, avail[1].max(120.0)])
            .column("Name").done()
            .column("Length").done()
            .column("Type").done()
            .column("Signals").done()
            .column("Tx").done()
            .column("Rx").done()
            .column("Comment").weight(1.0).done()
            .headers(true)
            .build(|ui| {
                if let Some(mut specs) = ui.table_get_sort_specs()
                    && specs.is_dirty()
                {
                    if let Some(spec) = specs.iter().next() {
                        self.pdu_sort_column_idx = spec.column_index.get();
                        self.pdu_sort_ascending =
                            spec.sort_direction == SortDirection::Ascending;
                    }
                    specs.clear_dirty(ui);
                }
                for (row_pos, pdu) in pdus.iter().enumerate() {
                    let pdu_name = pdu.name().to_string();
                    ui.table_next_row();
                    let is_selected =
                        self.selected_pdu_names.iter().any(|n| n == &pdu_name);

                    ui.table_set_column_index(0);
                    if ui
                        .selectable_config(&pdu_name)
                        .selected(is_selected)
                        .span_all_columns(true)
                        .build()
                    {
                        let ctrl = ui.io().key_ctrl();
                        let shift = ui.io().key_shift();
                        if ctrl {
                            if let Some(pos) =
                                self.selected_pdu_names.iter().position(|n| n == &pdu_name)
                            {
                                self.selected_pdu_names.remove(pos);
                            } else {
                                self.selected_pdu_names.push(pdu_name.clone());
                            }
                            self.pdu_selection_anchor = Some(pdu_name.clone());
                        } else if shift {
                            let anchor = self
                                .pdu_selection_anchor
                                .clone()
                                .unwrap_or_else(|| pdu_name.clone());
                            let anchor_pos = pdus
                                .iter()
                                .position(|p| p.name() == anchor.as_str())
                                .unwrap_or(row_pos);
                            let (lo, hi) = if anchor_pos <= row_pos {
                                (anchor_pos, row_pos)
                            } else {
                                (row_pos, anchor_pos)
                            };
                            self.selected_pdu_names = pdus[lo..=hi]
                                .iter()
                                .map(|p| p.name().to_string())
                                .collect();
                        } else {
                            self.selected_pdu_names = vec![pdu_name.clone()];
                            self.pdu_selection_anchor = Some(pdu_name.clone());
                        }
                    }
                    if ui.is_item_hovered()
                        && ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left)
                    {
                        event = PduMasterEvent::OpenPdu(pdu_name.clone());
                    }
                    if let Some(_popup) = ui.begin_popup_context_item_with_label(Some(
                        &format!("pdu_master_ctx_{}_{}", self.fibex_id, pdu_name),
                    )) {
                        if !self.selected_pdu_names.iter().any(|n| n == &pdu_name) {
                            self.selected_pdu_names = vec![pdu_name.clone()];
                            self.pdu_selection_anchor = Some(pdu_name.clone());
                        }
                        let selected = self.selected_pdu_names.clone();
                        if ui.menu_item("Open") {
                            event = PduMasterEvent::OpenPdu(pdu_name.clone());
                        }
                        if ui.menu_item("Edit") {
                            event = PduMasterEvent::EditPdu(pdu_name.clone());
                        }
                        ui.separator();
                        if ui.menu_item("Delete") {
                            event = PduMasterEvent::DeletePdu(selected);
                        }
                    }

                    ui.table_set_column_index(1);
                    ui.text(format!("{}", pdu.length()));

                    ui.table_set_column_index(2);
                    ui.text(pdu.kind().label());

                    ui.table_set_column_index(3);
                    ui.text(format!("{}", pdu.signals().len()));

                    ui.table_set_column_index(4);
                    let senders = pdu.senders().join(",");
                    if senders.is_empty() {
                        ui.text_disabled("-");
                    } else {
                        ui.text(&senders);
                    }

                    ui.table_set_column_index(5);
                    let receivers = pdu.receivers().join(",");
                    if receivers.is_empty() {
                        ui.text_disabled("-");
                    } else {
                        ui.text(&receivers);
                    }

                    ui.table_set_column_index(6);
                    ui.text(pdu.comment());
                }
            });

        self.handle_pdu_master_event(event);
    }

    /// 全文件信号列表（信号列表标签页）：跨 PDU 汇总所有信号，支持筛选与双击定位
    fn render_signal_list_content(&mut self, ui: &Ui) {
        // 汇总 (PDU 名, 信号) 并按筛选条件过滤
        let query = self.signal_search_query.to_lowercase();
        let mut rows: Vec<(String, crate::fibex::editable_fibex::EditableSignal)> = Vec::new();
        for pdu in self.fibex.pdus() {
            for sig in pdu.signals() {
                if !query.is_empty()
                    && !sig.name().to_lowercase().contains(&query)
                    && !pdu.name().to_lowercase().contains(&query)
                {
                    continue;
                }
                rows.push((pdu.name().to_string(), sig.clone()));
            }
        }

        ui.input_text("##signal_search", &mut self.signal_search_query)
            .hint("Filter signals / PDUs...")
            .build();
        ui.same_line();
        ui.text_disabled(format!("{} signals", rows.len()));
        // 工具栏行结束；实测剩余高度让表格正好填满
        let avail = ui.content_region_avail();

        let table_height = avail[1].max(120.0);
        ui.table("all_signals_table")
            .flags(TableFlags::RESIZABLE | TableFlags::BORDERS | TableFlags::SCROLL_X | TableFlags::SCROLL_Y | TableFlags::ROW_BG)
            .sizing_policy(TableSizingPolicy::FixedFit)
            .freeze(0, 1)
            .outer_size([0.0, table_height])
            .column("PDU").done()
            .column("Name").done()
            .column("Start").done()
            .column("Bits").done()
            .column("Byte Order").done()
            .column("Type").done()
            .column("Factor").done()
            .column("Offset").done()
            .column("Unit").done()
            .column("Tx").done()
            .column("Rx").done()
            .headers(true)
            .build(|ui| {
                // 选择行为与帧 / PDU 列表一致：单击选中、Ctrl 切换、Shift 范围选择
                if let Some(mut specs) = ui.table_get_sort_specs()
                    && specs.is_dirty()
                {
                    specs.clear_dirty(ui);
                }
                for (row_pos, (pdu_name, sig)) in rows.iter().enumerate() {
                    let sig_name = sig.name().to_string();
                    let row_key = (pdu_name.clone(), sig_name.clone());
                    let is_selected =
                        self.selected_signal_rows.iter().any(|k| k == &row_key);

                    ui.table_next_row();

                    ui.table_set_column_index(0);
                    // ## 后缀保证同名 PDU 的行 ID 唯一
                    if ui
                        .selectable_config(format!("{}##all_sig_row{}", pdu_name, row_pos))
                        .selected(is_selected)
                        .span_all_columns(true)
                        .build()
                    {
                        let ctrl = ui.io().key_ctrl();
                        let shift = ui.io().key_shift();
                        if ctrl {
                            if let Some(pos) = self
                                .selected_signal_rows
                                .iter()
                                .position(|k| k == &row_key)
                            {
                                self.selected_signal_rows.remove(pos);
                            } else {
                                self.selected_signal_rows.push(row_key.clone());
                            }
                            self.signal_selection_anchor = Some(row_key.clone());
                            self.signal_selection_cursor = Some(row_key.clone());
                        } else if shift {
                            let anchor = self
                                .signal_selection_anchor
                                .clone()
                                .unwrap_or_else(|| row_key.clone());
                            let anchor_pos = rows
                                .iter()
                                .position(|(p, s)| (p, s.name()) == (&anchor.0, anchor.1.as_str()))
                                .unwrap_or(row_pos);
                            let (lo, hi) = if anchor_pos <= row_pos {
                                (anchor_pos, row_pos)
                            } else {
                                (row_pos, anchor_pos)
                            };
                            self.selected_signal_rows = rows[lo..=hi]
                                .iter()
                                .map(|(p, s)| (p.clone(), s.name().to_string()))
                                .collect();
                            self.signal_selection_cursor = Some(row_key.clone());
                        } else {
                            self.selected_signal_rows = vec![row_key.clone()];
                            self.signal_selection_anchor = Some(row_key.clone());
                            self.signal_selection_cursor = Some(row_key.clone());
                        }
                    }
                    if ui.is_item_hovered()
                        && ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left)
                    {
                        self.open_pdu_window(pdu_name);
                    }

                    if let Some(_popup) = ui.begin_popup_context_item_with_label(Some(
                        &format!("sig_list_ctx_{}_{}", pdu_name, sig_name),
                    )) {
                        if self.selected_signal_rows.is_empty() {
                            self.selected_signal_rows = vec![row_key.clone()];
                        }
                        if ui.menu_item("Open PDU") {
                            self.open_pdu_window(pdu_name);
                        }
                        ui.separator();
                        let selected = self.selected_signal_rows.clone();
                        if ui.menu_item(format!(
                            "Delete selected signals ({})",
                            selected.len()
                        )) {
                            // 按 PDU 分组
                            let mut groups: Vec<(String, Vec<String>)> = Vec::new();
                            for (p, s) in selected {
                                match groups.iter_mut().find(|(gp, _)| *gp == p) {
                                    Some((_, names)) => names.push(s),
                                    None => groups.push((p, vec![s])),
                                }
                            }
                            self.pending_signal_delete = Some(groups);
                        }
                    }

                    ui.table_set_column_index(1);
                    ui.text(sig.name());
                    ui.table_set_column_index(2);
                    ui.text(format!("{}", sig.start_bit()));
                    ui.table_set_column_index(3);
                    ui.text(format!("{}", sig.length_bits()));
                    ui.table_set_column_index(4);
                    ui.text(match sig.byte_order() {
                        ByteOrder::BigEndian => "Motorola (BE)",
                        ByteOrder::LittleEndian => "Intel (LE)",
                    });
                    ui.table_set_column_index(5);
                    ui.text(match sig.value_type() {
                        ValueType::Unsigned => "Unsigned",
                        ValueType::Signed => "Signed",
                    });
                    ui.table_set_column_index(6);
                    ui.text(format!("{}", sig.factor()));
                    ui.table_set_column_index(7);
                    ui.text(format!("{}", sig.offset()));
                    ui.table_set_column_index(8);
                    ui.text(sig.unit());
                    ui.table_set_column_index(9);
                    let senders = sig.senders().join(",");
                    if senders.is_empty() {
                        ui.text_disabled("-");
                    } else {
                        ui.text(&senders);
                    }
                    ui.table_set_column_index(10);
                    let receivers = sig.receivers().join(",");
                    if receivers.is_empty() {
                        ui.text_disabled("-");
                    } else {
                        ui.text(&receivers);
                    }
                }
            });
    }

    fn handle_pdu_master_event(&mut self, event: PduMasterEvent) {
        match event {
            PduMasterEvent::OpenPdu(name) => self.open_pdu_window(&name),
            PduMasterEvent::EditPdu(name) => {
                if let Some(pdu) = self.fibex.get_pdu(&name) {
                    let edit_win = PduEditWindowState::open(&name, pdu);
                    self.edit_pdu_windows.push(edit_win);
                }
            }
            PduMasterEvent::DeletePdu(names) => {
                self.pending_pdu_delete = Some(names);
            }
            PduMasterEvent::AddPdu => {
                self.fibex.new_pdu();
                self.is_dirty = true;
            }
            PduMasterEvent::None => {}
        }
    }

    fn render_signal_edit_dialog(&mut self, ui: &Ui) {
        if !self.signal_edit_dialog.show {
            return;
        }

        let event = self.signal_edit_dialog.render(ui);
        match event {
            SignalEditEvent::Apply => {
                self.signal_edit_dialog.apply_edit(&mut self.fibex);
                self.is_dirty = true;
                self.signal_edit_dialog.focus_requested = true;
            }
            SignalEditEvent::Ok => {
                self.signal_edit_dialog.apply_edit(&mut self.fibex);
                self.is_dirty = true;
                self.signal_edit_dialog.show = false;
            }
            SignalEditEvent::Cancel => {
                self.signal_edit_dialog.show = false;
            }
            SignalEditEvent::None => {}
        }
    }

    /// 待确认的删除目标（PDU / 信号），由外层（render_fibex_windows）弹出确认对话框
    pub fn take_pending_signal_delete(&mut self) -> Option<Vec<(String, Vec<String>)>> {
        self.pending_signal_delete.take()
    }

    pub fn take_pending_pdu_delete(&mut self) -> Option<Vec<String>> {
        self.pending_pdu_delete.take()
    }
}

#[allow(dead_code)]
enum FrameTableEvent {
    None,
    OpenFrame(String),
    EditFrame(String),
    CopyFrame(Vec<String>),
    CutFrame(Vec<String>),
    DeleteFrame(Vec<String>),
    PasteFrame,
    AddFrame,
}

#[allow(dead_code)]
enum PduMasterEvent {
    None,
    OpenPdu(String),
    EditPdu(String),
    DeletePdu(Vec<String>),
    AddPdu,
}

/// 计算帧内下一个空闲字节偏移
fn next_free_start(fibex: &EditableFibex, frame_name: &str) -> u32 {
    fibex
        .get_frame(frame_name)
        .map(|f| {
            let mut end = 0u32;
            for m in f.pdus() {
                if let Some(pdu) = fibex.get_pdu(&m.pdu_name) {
                    end = end.max(m.start_position + pdu.length());
                }
            }
            end
        })
        .unwrap_or(0)
}

// =====================================================================
// 窗口循环与全局对话框
// =====================================================================

pub fn render_fibex_windows(ui: &Ui, ui_state: &mut FibexUiState) {
    for window_idx in 0..ui_state.fibex_windows.len() {
        let fibex_window = &ui_state.fibex_windows[window_idx];
        let dirty_marker = if fibex_window.is_dirty { "* " } else { "" };
        // 标题栏展示完整路径（含所在文件夹），### 固定窗口 ID 避免 dirty 切换时窗口重置
        let window_title = format!(
            "FIBEX - {}{}###FIBEX_{}",
            dirty_marker, fibex_window.file_path, fibex_window.fibex_id
        );

        if let Some(request_focus_idx) = ui_state.fibex_window_focus_request
            && request_focus_idx == window_idx
        {
            ui.set_window_focus(Some(window_title.as_str()));
            ui_state.fibex_window_focus_request = None;
        }

        let mut is_open = ui_state.fibex_windows[window_idx].is_open;
        let was_open = is_open;
        let has_clipboard = ui_state.has_clipboard_frame();

        let window_ui = ui
            .window(&window_title)
            .size([880.0, 560.0], Condition::FirstUseEver)
            .opened(&mut is_open);

        let table_event = window_ui.build(|| {
            // 主窗口内标签页：帧 / PDU 列表 / 信号列表 / ECU 列表 / 集群参数
            // 各表格用 content_region_avail() 实测剩余高度，正好填满
            let mut table_event = FrameTableEvent::None;

            if let Some(_tab_bar) = ui.tab_bar("main_tabs") {
                if ui.tab_item("Frames").is_some() {
                    table_event =
                        ui_state.fibex_windows[window_idx].render_frame_table(ui, has_clipboard);
                }
                if ui.tab_item("PDU List").is_some() {
                    ui_state.fibex_windows[window_idx].render_pdu_master_content(ui);
                }
                if ui.tab_item("Signal List").is_some() {
                    ui_state.fibex_windows[window_idx].render_signal_list_content(ui);
                }
                if ui.tab_item("ECU List").is_some() {
                    let (win, ecu) =
                        (&mut ui_state.fibex_windows[window_idx], &mut ui_state.ecu_window);
                    ecu.render_content(ui, &mut win.fibex);
                }
                if ui.tab_item("Cluster Parameters").is_some() {
                    ui.child_window("cluster_scroll")
                        .size([0.0, 0.0])
                        .build(ui, || {
                            crate::fibex::ui::cluster_window::render_cluster_content(
                                ui,
                                &mut ui_state.fibex_windows[window_idx].fibex,
                            );
                        });
                }
            }

            table_event
        });

        if ui.is_window_focused() {
            ui_state.last_focused_fibex_index = Some(window_idx);
            ui_state.focus_claimed = true;
        }

        if !is_open && was_open && ui_state.fibex_windows[window_idx].is_dirty {
            is_open = true;
            ui_state.close_confirm_dialog.show = true;
            ui_state.close_confirm_dialog.fibex_window_index = Some(window_idx);
        }

        ui_state.fibex_windows[window_idx].is_open = is_open;

        if let Some(table_event) = table_event
            && !matches!(table_event, FrameTableEvent::None)
        {
            handle_frame_table_event(ui_state, window_idx, table_event);
        }

        // 浮动子窗口
        ui_state.fibex_windows[window_idx].render_floating_windows(ui, &mut ui_state.clipboard);

        // 待Confirm Delete（信号 / PDU）
        if let Some(groups) = ui_state.fibex_windows[window_idx].take_pending_signal_delete() {
            let total: usize = groups.iter().map(|(_, names)| names.len()).sum();
            ui_state.confirm_delete_dialog.display_name = format!("{} signals", total);
            ui_state.confirm_delete_dialog.target = Some(DeleteTarget::Signals(groups));
            ui_state.confirm_delete_dialog.show = true;
        }
        if let Some(names) = ui_state.fibex_windows[window_idx].take_pending_pdu_delete() {
            ui_state.confirm_delete_dialog.display_name = format!("{} PDUs", names.len());
            ui_state.confirm_delete_dialog.target = Some(DeleteTarget::Pdus(names.clone()));
            ui_state.confirm_delete_dialog.show = true;
        }
    }

    // 移除已关闭的窗口
    let mut closed_indices: Vec<usize> = ui_state
        .fibex_windows
        .iter()
        .enumerate()
        .filter(|(_, w)| !w.is_open)
        .map(|(i, _)| i)
        .collect();
    closed_indices.sort_by(|a, b| b.cmp(a));
    for idx in closed_indices {
        ui_state.fibex_windows.remove(idx);
        if let Some(focused) = ui_state.last_focused_fibex_index {
            if focused == idx {
                ui_state.last_focused_fibex_index = None;
            } else if focused > idx {
                ui_state.last_focused_fibex_index = Some(focused - 1);
            }
        }
    }

    render_confirm_delete_dialog(ui, ui_state);
    render_close_confirm_dialog(ui, ui_state);
    render_validation_dialog(ui, ui_state);
}

fn handle_frame_table_event(
    ui_state: &mut FibexUiState,
    window_idx: usize,
    event: FrameTableEvent,
) {
    match event {
        FrameTableEvent::OpenFrame(frame_name) => {
            ui_state.fibex_windows[window_idx].open_frame_window(&frame_name);
        }
        FrameTableEvent::EditFrame(frame_name) => {
            if let Some(frame) = ui_state.fibex_windows[window_idx].fibex.get_frame(&frame_name) {
                let edit_win = FrameEditWindowState::open(&frame_name, frame);
                ui_state.fibex_windows[window_idx].edit_frame_windows.push(edit_win);
            }
        }
        FrameTableEvent::CopyFrame(names) => {
            let frames: Vec<EditableFrame> = names
                .iter()
                .filter_map(|n| {
                    ui_state.fibex_windows[window_idx]
                        .fibex
                        .get_frame(n)
                        .cloned()
                })
                .collect();
            if !frames.is_empty() {
                ui_state.clipboard.copied_frames = frames;
            }
        }
        FrameTableEvent::CutFrame(names) => {
            let frames: Vec<EditableFrame> = names
                .iter()
                .filter_map(|n| {
                    ui_state.fibex_windows[window_idx]
                        .fibex
                        .get_frame(n)
                        .cloned()
                })
                .collect();
            if !frames.is_empty() {
                ui_state.clipboard.copied_frames = frames;
            }
            ui_state.confirm_delete_dialog.target = Some(DeleteTarget::Frames(names.clone()));
            ui_state.confirm_delete_dialog.display_name = frames_display_name(
                &ui_state.fibex_windows[window_idx].fibex,
                &names,
            );
            ui_state.confirm_delete_dialog.show = true;
        }
        FrameTableEvent::DeleteFrame(names) => {
            ui_state.confirm_delete_dialog.target = Some(DeleteTarget::Frames(names.clone()));
            ui_state.confirm_delete_dialog.display_name = frames_display_name(
                &ui_state.fibex_windows[window_idx].fibex,
                &names,
            );
            ui_state.confirm_delete_dialog.show = true;
        }
        FrameTableEvent::PasteFrame => {
            let copied = ui_state.clipboard.copied_frames.clone();
            if !copied.is_empty() {
                let win = &mut ui_state.fibex_windows[window_idx];
                for mut new_frame in copied {
                    let mut new_name = format!("{}_copy", new_frame.name());
                    while win.fibex.get_frame(&new_name).is_some() {
                        new_name = format!("{}_copy{}", new_name, 2);
                    }
                    new_frame.set_name(&new_name);
                    win.fibex.add_frame(&new_frame);
                }
                win.is_dirty = true;
            }
        }
        FrameTableEvent::AddFrame => {
            let win = &mut ui_state.fibex_windows[window_idx];
            let new_name = win.fibex.new_frame();
            win.set_selected_frame_name(Some(&new_name));
            win.is_dirty = true;
        }
        FrameTableEvent::None => {}
    }
}

fn frames_display_name(fibex: &EditableFibex, names: &[String]) -> String {
    if names.len() == 1 {
        let name = fibex
            .get_frame(&names[0])
            .map(|f| f.name().to_string())
            .unwrap_or_default();
        format!("frame '{}'", name)
    } else {
        format!("{} frames", names.len())
    }
}

fn render_confirm_delete_dialog(ui: &Ui, ui_state: &mut FibexUiState) {
    if ui_state.confirm_delete_dialog.show {
        ui.open_popup("Confirm Delete");
        ui_state.confirm_delete_dialog.show = false;
    }

    let popup = ui.begin_modal_popup("Confirm Delete");
    let is_open_now = popup.is_some();
    if let Some(_popup) = popup {
        ui.text(format!(
            "Are you sure you want to delete {}?",
            ui_state.confirm_delete_dialog.display_name
        ));
        ui.separator();
        if ui.button("OK") {
            execute_delete(ui_state);
            ui.close_current_popup();
        }
        ui.same_line();
        if ui.button("Cancel") {
            ui_state.confirm_delete_dialog.target = None;
            ui.close_current_popup();
        }
    }

    // 模态框被按钮以外的方式关闭（如 Esc）时清理残留 target，避免阻塞后续删除Actions
    if ui_state.confirm_delete_dialog.was_open
        && !is_open_now
        && ui_state.confirm_delete_dialog.target.is_some()
    {
        ui_state.confirm_delete_dialog.target = None;
    }
    ui_state.confirm_delete_dialog.was_open = is_open_now;
}

fn execute_delete(ui_state: &mut FibexUiState) {
    let target = ui_state.confirm_delete_dialog.target.take();
    let focused = ui_state.last_focused_fibex_index;
    match target {
        Some(DeleteTarget::Frames(names)) => {
            if let Some(idx) = focused
                && let Some(win) = ui_state.fibex_windows.get_mut(idx)
            {
                for name in &names {
                    win.fibex.delete_frame(name);
                    win.frame_windows.retain(|w| w.frame_name != *name);
                }
                if names.len() > 1 {
                    win.fibex.merge_last_compounds(names.len());
                }
                win.selected_frame_names.retain(|n| !names.contains(n));
                win.is_dirty = true;
            }
        }
        Some(DeleteTarget::Pdus(names)) => {
            if let Some(idx) = focused
                && let Some(win) = ui_state.fibex_windows.get_mut(idx)
            {
                for name in &names {
                    win.fibex.delete_pdu(name);
                    win.pdu_windows.retain(|w| w.pdu_name != *name);
                }
                if names.len() > 1 {
                    win.fibex.merge_last_compounds(names.len());
                }
                win.selected_pdu_names.retain(|n| !names.contains(n));
                win.is_dirty = true;
            }
        }
        Some(DeleteTarget::Signals(groups)) => {
            if let Some(idx) = focused
                && let Some(win) = ui_state.fibex_windows.get_mut(idx)
            {
                let mut total = 0;
                for (pdu_name, names) in &groups {
                    for name in names {
                        win.fibex.delete_signal(pdu_name, name);
                        total += 1;
                    }
                }
                if total > 1 {
                    win.fibex.merge_last_compounds(total);
                }
                win.selected_signal_rows.retain(|(p, _)| {
                    !groups.iter().any(|(gp, _)| gp == p)
                });
                win.is_dirty = true;
            }
        }
        None => {}
    }
}

fn render_close_confirm_dialog(ui: &Ui, ui_state: &mut FibexUiState) {
    if ui_state.close_confirm_dialog.show {
        ui.open_popup("Save Changes");
        ui_state.close_confirm_dialog.show = false;
    }

    if let Some(_popup) = ui.begin_modal_popup("Save Changes") {
        let idx = ui_state.close_confirm_dialog.fibex_window_index.unwrap_or(0);
        let file_name = ui_state
            .fibex_windows
            .get(idx)
            .map(|w| {
                std::path::Path::new(&w.file_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&w.file_path)
                    .to_string()
            })
            .unwrap_or_default();

        ui.text(format!("Save changes to '{}'?", file_name));
        ui.separator();
        if ui.button("Save") {
            if let Some(idx) = ui_state.close_confirm_dialog.fibex_window_index {
                save_fibex_window(ui_state, idx);
                if let Some(win) = ui_state.fibex_windows.get_mut(idx) {
                    win.is_open = false;
                }
            }
            ui_state.close_confirm_dialog.fibex_window_index = None;
            ui.close_current_popup();
        }
        ui.same_line();
        if ui.button("Don't Save") {
            if let Some(idx) = ui_state.close_confirm_dialog.fibex_window_index
                && let Some(win) = ui_state.fibex_windows.get_mut(idx)
            {
                win.is_open = false;
            }
            ui_state.close_confirm_dialog.fibex_window_index = None;
            ui.close_current_popup();
        }
        ui.same_line();
        if ui.button("Cancel") {
            ui_state.close_confirm_dialog.fibex_window_index = None;
            ui.close_current_popup();
        }
    }
}

fn render_validation_dialog(ui: &Ui, ui_state: &mut FibexUiState) {
    if !ui_state.validation_dialog.show {
        return;
    }

    let mut is_open = true;
    ui.window("Validation Results")
        .opened(&mut is_open)
        .flags(dear_imgui_rs::WindowFlags::ALWAYS_AUTO_RESIZE)
        .build(|| {
            let issues = &ui_state.validation_dialog.issues;
            let error_count = issues
                .iter()
                .filter(|i| matches!(i.severity, crate::fibex::editable_fibex::Severity::Error))
                .count();
            let warning_count = issues
                .iter()
                .filter(|i| matches!(i.severity, crate::fibex::editable_fibex::Severity::Warning))
                .count();

            if issues.is_empty() {
                ui.text_colored([0.0, 0.8, 0.0, 1.0], "No issues found. The database is valid.");
            } else {
                ui.text(format!(
                    "Found {} error(s), {} warning(s):",
                    error_count, warning_count
                ));
                ui.separator();

                if let Some(_table) = ui.begin_table_with_flags(
                    "validation_table",
                    2,
                    imgui_table_flags(),
                ) {
                    // 冻结标题行
                    ui.table_setup_scroll_freeze(0, 1);
                    ui.table_setup_column("Severity", dear_imgui_rs::TableColumnFlags::NONE, None);
                    ui.table_setup_column("Message", dear_imgui_rs::TableColumnFlags::NONE, None);
                    ui.table_headers_row();

                    for issue in issues {
                        ui.table_next_row();
                        ui.table_set_column_index(0);
                        match issue.severity {
                            crate::fibex::editable_fibex::Severity::Error => {
                                ui.text_colored([1.0, 0.3, 0.3, 1.0], "Error");
                            }
                            crate::fibex::editable_fibex::Severity::Warning => {
                                ui.text_colored([1.0, 0.8, 0.0, 1.0], "Warning");
                            }
                        }
                        ui.table_set_column_index(1);
                        ui.text(&issue.message);
                    }
                }
            }
        });

    if !is_open {
        ui_state.validation_dialog.show = false;
    }
}

fn imgui_table_flags() -> dear_imgui_rs::TableFlags {
    dear_imgui_rs::TableFlags::RESIZABLE
        | dear_imgui_rs::TableFlags::BORDERS
        
        | dear_imgui_rs::TableFlags::SCROLL_Y
        | dear_imgui_rs::TableFlags::ROW_BG
}

pub fn save_fibex_window(ui_state: &mut FibexUiState, idx: usize) {
    let win = &mut ui_state.fibex_windows[idx];
    let save_path = win.file_path.clone();
    let xml = match save_format(&save_path) {
        SaveFormat::Arxml => crate::fibex::export::arxml::export_arxml(&win.fibex),
        SaveFormat::Fibex => crate::fibex::export::fibex::export_fibex(&win.fibex),
    };
    // 按打开时检测到的编码写出（GBK 文件Save后仍是 GBK）
    let bytes = crate::file_encoding::encode_to_bytes(&xml, win.text_encoding, win.had_bom);
    match std::fs::write(&save_path, &bytes) {
        Ok(_) => {
            win.is_dirty = false;
        }
        Err(e) => {
            ui_state.error_dialog.message = format!("Failed to save: {}", e);
            ui_state.error_dialog.show = true;
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum SaveFormat {
    Fibex,
    Arxml,
}

pub fn save_format(path: &str) -> SaveFormat {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    if ext == "arxml" {
        SaveFormat::Arxml
    } else {
        SaveFormat::Fibex
    }
}
