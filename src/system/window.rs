//! 系统窗口：一份文件里的 CAN 与 FlexRay 集群放在一起看
//!
//! 布局照 Vector AUTOSAR Explorer：左边 `SYSTEM > 集群 > ECU` 的树；中间是选中集群
//! 的实体表，页签随集群的总线种类变化——CAN 给 Frames / PDUs / Signals，FlexRay 额外
//! 给排程相关的 Schedule 与 Cluster Params；右边是选中帧的属性面板（Frame /
//! CAN Properties / PDU Mappings / Rx-Tx），独立滚动区。
//!
//! 这一版只读：能看、能把读到的内容原样写回 .arxml；改还不能改，编辑与撤销在下一步接上。

use crate::system::source::ArxmlSource;
use crate::system::{Cluster, Ecu, Frame, Pdu, Signal, SysTab, SystemDoc};
use dear_imgui_rs::{Condition, TableColumnFlags, TableFlags, TableOptions, TableSizingPolicy, Ui};

/// 属性面板的输入缓冲区：选中对象变化时从模型重新装填，Apply 才写进模型与补丁
#[derive(Default)]
pub struct EditBuffers {
    /// 缓冲区当前装的是哪个对象，形如 `frame/Console_1`
    pub key: String,
    pub name: String,
    pub comment: String,
    pub length: i32,
    pub identifier: i32,
    /// 0 = Standard，1 = Extended
    pub addressing: usize,
    /// 0 = CAN-20，1 = CAN-FD
    pub tx_behavior: usize,
    pub rx_behavior: usize,
    /// 周期毫秒的文本；空表示不改
    pub cycle: String,
    pub start: i32,
    pub bits: i32,
    /// 0 = Motorola，1 = Intel
    pub order: usize,
    pub factor: String,
    pub offset: String,
    pub min: String,
    pub max: String,
    pub unit: String,
    /// 值表编辑缓冲
    pub values: Vec<(i32, String)>,
}

/// 双击树里的 ECU / 集群时弹出的编辑框
#[derive(Clone, Default)]
pub struct Modal {
    /// "ecu" 或 "cluster"
    pub kind: String,
    pub target: String,
    pub name: String,
    pub comment: String,
    /// 集群的波特率文本
    pub extra: String,
}

/// 混合集群文件的浏览与编辑窗口
pub struct SystemWindow {
    pub is_open: bool,
    /// ImGui 窗口 ID 的后缀：与文件路径解绑，另存为换路径才不会重置布局
    pub sys_id: usize,
    pub file_path: String,
    pub doc: SystemDoc,
    /// 打开时的原文与各字段出处：保存只写改过的地方
    pub source: ArxmlSource,
    /// 撤销栈：每一步编辑前的 (模型, 补丁) 基线
    pub undo_stack: Vec<(SystemDoc, ArxmlSource)>,
    pub redo_stack: Vec<(SystemDoc, ArxmlSource)>,
    pub edit: EditBuffers,
    pub modal: Option<Modal>,
    /// PDUs 页的种类过滤：0 = 全部，其余对应 PduType 的顺序
    pub pdu_filter: usize,
    pub tab: SysTab,
    pub selected_cluster: Option<String>,
    pub selected_frame: Option<String>,
    pub selected_pdu: Option<String>,
    pub selected_signal: Option<String>,
    /// 上一次保存摊平/丢掉了什么，显示在工具条上
    pub save_note: Option<String>,
    /// 保存警告的全文，显示在窗口下方的可关闭区域
    pub save_report: Option<String>,
    /// 关窗口时有未保存改动：先问，答完才真关
    pub close_ask: bool,
}

static NEXT_SYS_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);

impl SystemWindow {
    pub fn new(doc: SystemDoc, file_path: &str, source: ArxmlSource) -> Self {
        let first = doc.clusters.first().map(|c| c.name.clone());
        let selected_frame = doc.frames.first().map(|f| f.name.clone());
        let selected_pdu = doc.pdus.first().map(|p| p.name.clone());
        Self {
            is_open: true,
            sys_id: NEXT_SYS_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            file_path: file_path.to_string(),
            tab: SysTab::default(),
            selected_cluster: first,
            selected_frame,
            selected_pdu,
            selected_signal: None,
            save_note: None,
            save_report: None,
            close_ask: false,
            edit: EditBuffers::default(),
            modal: None,
            pdu_filter: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            doc,
            source,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.source.is_dirty()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) {
        let Some(prev) = self.undo_stack.pop() else {
            return;
        };
        self.redo_stack
            .push((self.doc.clone(), self.source.clone()));
        self.doc = prev.0;
        self.source = prev.1;
        self.edit.key.clear();
    }

    pub fn redo(&mut self) {
        let Some(next) = self.redo_stack.pop() else {
            return;
        };
        self.undo_stack
            .push((self.doc.clone(), self.source.clone()));
        self.doc = next.0;
        self.source = next.1;
        self.edit.key.clear();
    }

    /// 跑一步编辑：先压撤销快照，失败则回滚并返回错误说明
    fn apply<F>(&mut self, op: F) -> Result<(), String>
    where
        F: FnOnce(&mut SystemDoc, &mut ArxmlSource) -> Result<(), String>,
    {
        let snapshot = (self.doc.clone(), self.source.clone());
        match op(&mut self.doc, &mut self.source) {
            Ok(()) => {
                self.undo_stack.push(snapshot);
                self.redo_stack.clear();
                self.edit.key.clear();
                Ok(())
            }
            Err(e) => {
                self.doc = snapshot.0;
                self.source = snapshot.1;
                Err(e)
            }
        }
    }

    /// 把当前文档写回 .arxml：原文上只替换改过的那一段，其余字节不动
    pub fn save_as(&mut self) {
        let default_name = std::path::Path::new(&self.file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("system.arxml")
            .to_string();
        let Some(path) = rfd::FileDialog::new()
            .add_filter("AUTOSAR ARXML", &["arxml"])
            .set_directory(crate::paths::save_dir_for(&self.file_path))
            .set_file_name(default_name)
            .save_file()
        else {
            return;
        };
        let xml = self.source.render();
        match std::fs::write(&path, xml) {
            Ok(_) => {
                self.file_path = crate::ui::state::normalize_path(&path.to_string_lossy());
                self.source.clear_patches();
                self.save_note = None;
                self.save_report = None;
            }
            Err(e) => self.save_note = Some(format!("写入失败：{e}")),
        }
    }
}

/// 渲染全部系统窗口
pub fn render_system_windows(ui: &Ui, ui_state: &mut crate::ui::UiState) {
    let mut focused: Option<usize> = None;
    for (i, w) in ui_state.sys_windows.iter_mut().enumerate() {
        if !w.is_open {
            continue;
        }
        let title = format!(
            "SYSTEM - {}{}###sys_{}",
            w.file_path,
            if w.is_dirty() { " *" } else { "" },
            w.sys_id
        );
        let mut is_open = w.is_open;
        let focused_slot = &mut focused;
        ui.window(&title)
            .opened(&mut is_open)
            .size([1120.0, 720.0], Condition::FirstUseEver)
            .build(|| {
                // 焦点必须在 Begin/End 之间查询：build 闭包返回后当前窗口已出栈
                if ui.is_window_focused() {
                    *focused_slot = Some(i);
                }
                render_body(ui, w);
            });
        // 有未保存改动时不让窗口直接消失：先留下问一句
        if w.is_open && !is_open && w.is_dirty() {
            w.is_open = true;
            w.close_ask = true;
        } else {
            w.is_open = is_open;
        }
    }
    for w in ui_state.sys_windows.iter_mut() {
        if !w.close_ask {
            continue;
        }
        let mut open = true;
        ui.window(&format!("Save changes?###sysclose_{}", w.sys_id))
            .opened(&mut open)
            .flags(
                dear_imgui_rs::WindowFlags::NO_COLLAPSE
                    | dear_imgui_rs::WindowFlags::ALWAYS_AUTO_RESIZE,
            )
            .build(|| {
                ui.text(format!("{} 有未保存的改动。", w.file_path));
                if ui.button("Save") {
                    w.save_as();
                    if !w.is_dirty() {
                        w.close_ask = false;
                        w.is_open = false;
                    }
                }
                ui.same_line();
                if ui.button("Discard changes") {
                    w.close_ask = false;
                    w.is_open = false;
                }
                ui.same_line();
                if ui.button("Keep editing") {
                    w.close_ask = false;
                }
            });
        if !open {
            w.close_ask = false;
        }
    }
    if let Some(i) = focused {
        ui_state.last_focused_sys_index = Some(i);
        ui_state.focus = crate::ui::state::FocusTarget::System;
    }
}

/// 一次渲染期间从窗口取出的选择状态：画表格时不能再借用窗口本身
struct View {
    tab: SysTab,
    cluster: Option<String>,
    frame: Option<String>,
    pdu: Option<String>,
    signal: Option<String>,
    pdu_filter: usize,
    report: Option<String>,
}

fn render_body(ui: &Ui, w: &mut SystemWindow) {
    w.sync_buffers();
    if ui.button("Save ARXML...") {
        w.save_as();
    }
    ui.same_line();
    if ui.button("Apply") {
        let res = w.commit_selected();
        w.note(res);
    }
    ui.same_line();
    if ui.button("Delete") {
        let res = w.delete_selected();
        w.note(res);
    }
    ui.same_line();
    if ui.button("Add Frame...") {
        let cluster = w.selected_cluster.clone().unwrap_or_default();
        let next_id = w
            .doc
            .frames
            .iter()
            .filter_map(|f| f.can.map(|c| c.identifier))
            .max()
            .map(|m| m + 1)
            .unwrap_or(0x100);
        let name = format!("Frame_{}", w.doc.frames.len() + 1);
        if let Err(e) = w
            .apply(|doc, src| crate::system::edit::add_frame(doc, src, &cluster, &name, next_id, 8))
        {
            w.save_note = Some(e);
        }
    }
    ui.same_line();
    if w.tab == SysTab::Pdus && w.selected_pdu.is_some() {
        if ui.button("Add Signal") {
            let pdu = w.selected_pdu.clone().unwrap_or_default();
            let count = w
                .doc
                .pdus
                .iter()
                .find(|p| p.name == pdu)
                .map(|p| p.signals.len())
                .unwrap_or(0);
            let res = w.apply(move |doc, src| {
                crate::system::edit::add_signal(doc, src, &pdu, &format!("Sig_{}", count + 1), 0, 8)
            });
            w.note(res);
        }
    }
    ui.same_line();
    if button_enabled(ui, "Undo", w.can_undo()) {
        w.undo();
    }
    ui.same_line();
    if button_enabled(ui, "Redo", w.can_redo()) {
        w.redo();
    }
    if let Some(note) = w.save_note.clone() {
        ui.same_line();
        ui.text_colored([0.95, 0.75, 0.25, 1.0], note);
    }
    ui.separator();

    // 先取快照再画：表格要边显示边改选中项，借用不重叠
    let clusters = w.doc.clusters.clone();
    let ecus = w.doc.ecus.clone();
    let all_frames = w.doc.frames.clone();
    let all_pdus = w.doc.pdus.clone();
    let mut view = View {
        tab: w.tab,
        cluster: w.selected_cluster.clone(),
        frame: w.selected_frame.clone(),
        pdu: w.selected_pdu.clone(),
        signal: w.selected_signal.clone(),
        pdu_filter: w.pdu_filter,
        report: w.save_report.clone(),
    };
    if view.cluster.is_none() {
        view.cluster = clusters.first().map(|c| c.name.clone());
    }

    let [avail_w, avail_h] = ui.content_region_avail();
    let left_w = 260.0f32.min(avail_w * 0.3);
    let body_h = (avail_h - 6.0).max(80.0);

    ui.child_window("sys_tree")
        .size([left_w, body_h])
        .build(ui, || {
            tree(ui, &clusters, &ecus, &mut view, &mut w.modal);
        });
    ui.same_line();
    ui.child_window("sys_main")
        .size([avail_w - left_w - 12.0, body_h])
        .build(ui, || {
            let Some(cluster_name) = view.cluster.clone() else {
                ui.text("No cluster in this file.");
                return;
            };
            let Some(cluster) = clusters.iter().find(|c| c.name == cluster_name).cloned() else {
                ui.text("Selected cluster is missing.");
                return;
            };
            let frames: Vec<Frame> = all_frames
                .iter()
                .filter(|f| f.cluster == cluster.name)
                .cloned()
                .collect();
            let pdus: Vec<Pdu> = all_pdus
                .iter()
                .filter(|p| p.cluster == cluster.name)
                .cloned()
                .collect();
            let signals: Vec<(String, Signal)> = pdus
                .iter()
                .flat_map(|p| p.signals.iter().cloned().map(move |s| (p.name.clone(), s)))
                .collect();
            let ecus_on_cluster: Vec<Ecu> = ecus
                .iter()
                .filter(|e| e.clusters.iter().any(|n| *n == cluster.name))
                .cloned()
                .collect();
            let groups: Vec<crate::system::SignalGroup> = w
                .doc
                .groups
                .iter()
                .filter(|g| g.cluster == cluster.name)
                .cloned()
                .collect();
            w.main_pane(
                ui,
                &mut view,
                &cluster,
                &frames,
                &pdus,
                &signals,
                &ecus_on_cluster,
                &groups,
            );
        });

    let report = view.report.clone();
    if let Some(report) = report.as_deref() {
        ui.separator();
        let dismiss = ui.button("Dismiss report");
        let lines = report.lines().count().min(6);
        ui.child_window("sys_report")
            .size([0.0, (lines as f32 + 0.6) * ui.text_line_height()])
            .build(ui, || {
                for line in report.lines() {
                    ui.text(line);
                }
            });
        if dismiss {
            view.report = None;
        }
    }

    // 双击树里的 ECU / 集群弹出的编辑框
    if let Some(m) = w.modal.clone() {
        let mut open = true;
        let is_ecu = m.kind == "ecu";
        let title = format!(
            "Edit {} - {}###sysmodal_{}",
            if is_ecu { "ECU" } else { "Cluster" },
            m.target,
            w.sys_id
        );
        ui.window(&title)
            .opened(&mut open)
            .size([400.0, 200.0], Condition::Always)
            .flags(dear_imgui_rs::WindowFlags::NO_COLLAPSE)
            .build(|| {
                let mut m = m.clone();
                ui.align_text_to_frame_padding();
                ui.text("Short Name");
                ui.same_line_with_pos(130.0);
                if is_ecu {
                    ui.input_text("##mname", &mut m.name).build();
                } else {
                    ui.text(&m.name);
                }
                ui.align_text_to_frame_padding();
                ui.text("Description");
                ui.same_line_with_pos(130.0);
                ui.input_text("##mcomment", &mut m.comment).build();
                if !is_ecu {
                    ui.align_text_to_frame_padding();
                    ui.text("Baudrate [bit/s]");
                    ui.same_line_with_pos(130.0);
                    ui.input_text("##mbaud", &mut m.extra).build();
                } else if let Some(e) = w.doc.ecus.iter().find(|e| e.name == m.target) {
                    for c in &e.controllers {
                        ui.text(format!(
                            "Controller {}: quanta {}..{}, sample point {}..{}",
                            c.name,
                            c.min_quanta.map(fmt_f).unwrap_or_else(|| "-".to_string()),
                            c.max_quanta.map(fmt_f).unwrap_or_else(|| "-".to_string()),
                            c.min_sample.map(fmt_f).unwrap_or_else(|| "-".to_string()),
                            c.max_sample.map(fmt_f).unwrap_or_else(|| "-".to_string()),
                        ));
                    }
                }
                let mut done = false;
                if ui.button("Apply") {
                    let kind = m.kind.clone();
                    let target = m.target.clone();
                    let name = m.name.clone();
                    let comment = m.comment.clone();
                    let baud = parse_f(&m.extra);
                    let res = w.apply(move |doc, src| {
                        use crate::system::edit as e;
                        if kind == "ecu" {
                            let cur = doc
                                .ecus
                                .iter()
                                .find(|x| x.name == target)
                                .map(|x| x.comment.clone())
                                .unwrap_or_default();
                            e::rename_ecu(doc, src, &target, &name)?;
                            if comment != cur {
                                e::set_ecu_comment(doc, src, &name, &comment)?;
                            }
                        } else {
                            let cur = doc
                                .clusters
                                .iter()
                                .find(|x| x.name == target)
                                .map(|x| (x.comment.clone(), x.baudrate))
                                .unwrap_or_default();
                            if let Some(b) = baud {
                                if Some(b) != cur.1 {
                                    e::set_cluster_baudrate(doc, src, &target, b)?;
                                }
                            }
                            if comment != cur.0 {
                                e::set_cluster_comment(doc, src, &target, &comment)?;
                            }
                        }
                        Ok(())
                    });
                    w.note(res);
                    done = true;
                }
                ui.same_line();
                if ui.button("Cancel") {
                    done = true;
                }
                if done {
                    w.modal = None;
                } else {
                    w.modal = Some(m);
                }
            });
        if !open {
            w.modal = None;
        }
    }

    w.tab = view.tab;
    w.selected_cluster = view.cluster;
    w.selected_frame = view.frame;
    w.selected_pdu = view.pdu;
    w.selected_signal = view.signal;
    w.pdu_filter = view.pdu_filter;
    w.save_report = view.report;
}

fn tree(ui: &Ui, clusters: &[Cluster], ecus: &[Ecu], view: &mut View, modal: &mut Option<Modal>) {
    if let Some(_root) = ui.tree_node_config("SYSTEM").default_open(true).push() {
        for c in clusters {
            let on_bus: Vec<&Ecu> = ecus
                .iter()
                .filter(|e| e.clusters.iter().any(|n| n == &c.name))
                .collect();
            let label = format!("{} [{}] — {} ECU", c.name, c.kind.label(), on_bus.len());
            let node = ui.tree_node_config(&label).default_open(true).push();
            let clicked = ui.is_item_clicked();
            if clicked {
                view.cluster = Some(c.name.clone());
            }
            if clicked && ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left) {
                *modal = Some(Modal {
                    kind: "cluster".to_string(),
                    name: c.name.clone(),
                    comment: c.comment.clone(),
                    extra: c.baudrate.map(fmt_f).unwrap_or_default(),
                    target: c.name.clone(),
                });
            }
            if node.is_some() {
                for e in on_bus {
                    ui.text(&e.name);
                    if ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left)
                        && ui.is_item_hovered()
                    {
                        *modal = Some(Modal {
                            kind: "ecu".to_string(),
                            name: e.name.clone(),
                            comment: e.comment.clone(),
                            extra: String::new(),
                            target: e.name.clone(),
                        });
                    }
                }
            }
        }
    }
}

impl SystemWindow {
    fn main_pane(
        &mut self,
        ui: &Ui,
        view: &mut View,
        cluster: &Cluster,
        frames: &[Frame],
        pdus: &[Pdu],
        signals: &[(String, Signal)],
        ecus_on_cluster: &[Ecu],
        groups: &[crate::system::SignalGroup],
    ) {
        // FlexRay 才有排程与集群参数；CAN 才有通信矩阵与信号组
        let flexray = cluster.kind == crate::system::BusKind::FlexRay;
        if !flexray && matches!(view.tab, SysTab::Schedule | SysTab::ClusterParams) {
            view.tab = SysTab::Frames;
        }
        if flexray && matches!(view.tab, SysTab::CommMatrix | SysTab::SignalGroups) {
            view.tab = SysTab::Frames;
        }
        let total_h = ui.content_region_avail()[1];
        let with_detail = view.tab == SysTab::Frames;
        let table_h = (total_h * if with_detail { 0.42 } else { 0.52 }).max(140.0);

        if let Some(_bar) = ui.tab_bar("sys_tabs") {
            if ui.tab_item("Frames").is_some() {
                view.tab = SysTab::Frames;
            }
            if ui.tab_item("PDUs").is_some() {
                view.tab = SysTab::Pdus;
            }
            if ui.tab_item("Signals").is_some() {
                view.tab = SysTab::Signals;
            }
            if flexray {
                if ui.tab_item("Schedule").is_some() {
                    view.tab = SysTab::Schedule;
                }
                if ui.tab_item("Cluster Params").is_some() {
                    view.tab = SysTab::ClusterParams;
                }
            } else {
                if ui.tab_item("Comm Matrix").is_some() {
                    view.tab = SysTab::CommMatrix;
                }
                if ui.tab_item("Signal Groups").is_some() {
                    view.tab = SysTab::SignalGroups;
                }
            }
            let shown = view.tab;
            match shown {
                SysTab::Frames => {
                    header(ui, "Frames", &cluster.name, frames.len());
                    frames_table(ui, frames, &mut view.frame, table_h);
                }
                SysTab::Pdus => {
                    ui.text(format!("PDUs of {} ({})", cluster.name, pdus.len()));
                    ui.same_line();
                    ui.text("   PDU Type Filter:");
                    ui.same_line();
                    let labels = pdu_filter_labels();
                    let mut idx = view.pdu_filter.min(labels.len() - 1);
                    if ui.combo_simple_string("##pdufilter", &mut idx, &labels) {
                        view.pdu_filter = idx;
                    }
                    let shown_pdus: Vec<Pdu> = match filter_type(view.pdu_filter) {
                        Some(t) => pdus.iter().filter(|p| p.pdu_type == t).cloned().collect(),
                        None => pdus.to_vec(),
                    };
                    pdus_table(ui, &shown_pdus, &mut view.pdu, table_h, flexray);
                }
                SysTab::Signals => {
                    header(ui, "Signals", &cluster.name, signals.len());
                    signals_table(ui, signals, &mut view.signal, table_h);
                }
                SysTab::CommMatrix => {
                    header(ui, "Communication Matrix", &cluster.name, frames.len());
                    comm_matrix(ui, ecus_on_cluster, frames, table_h);
                }
                SysTab::SignalGroups => {
                    header(ui, "Signal Groups", &cluster.name, groups.len());
                    groups_table(ui, groups, table_h);
                }
                SysTab::Schedule => {
                    header(
                        ui,
                        "Schedule",
                        &cluster.name,
                        frames.iter().map(|f| f.triggerings.len()).sum(),
                    );
                    schedule_table(ui, frames, table_h);
                }
                SysTab::ClusterParams => {
                    ui.text(format!("Cluster parameters of {}", cluster.name));
                    params_table(ui, cluster, table_h);
                }
            }
        }

        // 下方明细：选中帧的各 PDU 与信号，点信号跳去编辑（参考界面表格下方那栏）
        if with_detail {
            if let Some(f) = view
                .frame
                .clone()
                .and_then(|n| frames.iter().find(|f| f.name == n).cloned())
            {
                let detail_h = (total_h * 0.24).max(90.0);
                let pdus_owned = pdus.to_vec();
                ui.child_window("sys_detail")
                    .size([0.0, detail_h])
                    .build(ui, || {
                        detail_pane(ui, &f, &pdus_owned, &mut view.signal);
                    });
                if view.signal.is_some() && view.tab == SysTab::Frames {
                    // 点明细里的信号只是选中，不跳页；属性面板在 Signals 页编辑
                }
            }
        }

        let rest_h = ui.content_region_avail()[1].max(80.0);
        let tab = view.tab;
        let frame = view
            .frame
            .clone()
            .and_then(|n| frames.iter().find(|f| f.name == n).cloned());
        let pdu = view
            .pdu
            .clone()
            .and_then(|n| pdus.iter().find(|p| p.name == n).cloned());
        let signal = view.signal.clone().and_then(|n| {
            signals
                .iter()
                .find(|(_, s)| s.name == n)
                .map(|(p, s)| (p.clone(), s.clone()))
        });
        ui.child_window("sys_props")
            .size([0.0, rest_h])
            .build(ui, || match tab {
                SysTab::Pdus => match pdu {
                    Some(p) => self.pdu_panel(ui, &p),
                    None => ui.text("Select a PDU."),
                },
                SysTab::Signals => match signal {
                    Some((p, s)) => self.signal_panel(ui, &p, &s),
                    None => ui.text("Select a signal."),
                },
                SysTab::Frames => match frame {
                    Some(f) => self.frame_panel(ui, &f),
                    None => ui.text("Select a frame."),
                },
                _ => ui.text("This view has no property panel."),
            });
    }

    /// 选中对象变化时把模型里的值装进输入缓冲区
    fn load(&mut self, key: &str, fill: impl FnOnce(&mut EditBuffers)) {
        if self.edit.key != key {
            self.edit.key = key.to_string();
            fill(&mut self.edit);
        }
    }

    /// 选中对象变化时把模型里的值装进输入缓冲区
    fn sync_buffers(&mut self) {
        match self.tab {
            SysTab::Pdus => {
                let sel = self.selected_pdu.clone();
                if let Some(p) =
                    sel.and_then(|n| self.doc.pdus.iter().find(|p| p.name == n).cloned())
                {
                    let key = format!("pdu/{}", p.name);
                    let (name, comment, len, cycle) = (
                        p.name.clone(),
                        p.comment.clone(),
                        p.length_bytes,
                        p.cycle_ms,
                    );
                    self.load(&key, move |b| {
                        b.name = name;
                        b.comment = comment;
                        b.length = len as i32;
                        b.cycle = cycle.map(fmt_f).unwrap_or_default();
                    });
                }
            }
            SysTab::Signals => {
                let sel = self.selected_signal.clone();
                if let Some(cur) = sel.and_then(|n| {
                    self.doc
                        .pdus
                        .iter()
                        .find_map(|p| p.signals.iter().find(|x| x.name == n).cloned())
                }) {
                    let key = format!("signal/{}", cur.name);
                    self.load(&key, move |b| {
                        b.name = cur.name.clone();
                        b.comment = cur.comment.clone();
                        b.start = cur.start_bit as i32;
                        b.bits = cur.length_bits as i32;
                        b.order =
                            usize::from(cur.byte_order == crate::system::ByteOrder::LittleEndian);
                        b.factor = fmt_f(cur.factor);
                        b.offset = fmt_f(cur.offset);
                        b.min = fmt_f(cur.min);
                        b.max = fmt_f(cur.max);
                        b.unit = cur.unit.clone();
                        b.values = cur
                            .value_descriptions
                            .iter()
                            .map(|(v, d)| (*v as i32, d.clone()))
                            .collect();
                    });
                }
            }
            _ => {
                let sel = self.selected_frame.clone();
                if let Some(f) =
                    sel.and_then(|n| self.doc.frames.iter().find(|f| f.name == n).cloned())
                {
                    let key = format!("frame/{}", f.name);
                    let can = f.can.unwrap_or_default();
                    let (name, comment, len, id, ext, tx, rx) = (
                        f.name.clone(),
                        f.comment.clone(),
                        f.length_bytes,
                        can.identifier,
                        can.extended,
                        can.tx_behavior,
                        can.rx_behavior,
                    );
                    self.load(&key, move |b| {
                        b.name = name;
                        b.comment = comment;
                        b.length = len as i32;
                        b.identifier = id as i32;
                        b.addressing = usize::from(ext);
                        b.tx_behavior = usize::from(tx == crate::system::CanBehavior::CanFd);
                        b.rx_behavior = usize::from(rx == crate::system::CanBehavior::CanFd);
                    });
                }
            }
        }
    }

    /// 工具条 Apply：按当前页签与选中对象提交输入缓冲区
    fn commit_selected(&mut self) -> Result<(), String> {
        match self.tab {
            SysTab::Pdus => match self.selected_pdu.clone() {
                Some(n) => self.commit_pdu(&n),
                None => Err("先选一个 PDU".to_string()),
            },
            SysTab::Signals => match self.selected_signal.clone() {
                Some(s) => {
                    let pdu = self
                        .doc
                        .pdus
                        .iter()
                        .find(|p| p.signals.iter().any(|x| x.name == s))
                        .map(|p| p.name.clone())
                        .ok_or_else(|| format!("没有信号 {s}"))?;
                    self.commit_signal(&pdu, &s)
                }
                None => Err("先选一条信号".to_string()),
            },
            _ => match self.selected_frame.clone() {
                Some(n) => self.commit_frame(&n),
                None => Err("先选一帧".to_string()),
            },
        }
    }

    /// 工具条 Delete：按当前页签删除选中对象
    fn delete_selected(&mut self) -> Result<(), String> {
        match self.tab {
            SysTab::Pdus => match self.selected_pdu.clone() {
                Some(n) => {
                    self.apply(move |doc, src| crate::system::edit::delete_pdu(doc, src, &n))
                }
                None => Err("先选一个 PDU".to_string()),
            },
            SysTab::Signals => match self.selected_signal.clone() {
                Some(s) => {
                    let pdu = self
                        .doc
                        .pdus
                        .iter()
                        .find(|p| p.signals.iter().any(|x| x.name == s))
                        .map(|p| p.name.clone())
                        .ok_or_else(|| format!("没有信号 {s}"))?;
                    self.apply(move |doc, src| {
                        crate::system::edit::delete_signal(doc, src, &pdu, &s)
                    })
                }
                None => Err("先选一条信号".to_string()),
            },
            _ => match self.selected_frame.clone() {
                Some(n) => {
                    self.apply(move |doc, src| crate::system::edit::delete_frame(doc, src, &n))
                }
                None => Err("先选一帧".to_string()),
            },
        }
    }

    fn commit_frame(&mut self, old: &str) -> Result<(), String> {
        let cur = self
            .doc
            .frames
            .iter()
            .find(|f| f.name == old)
            .cloned()
            .ok_or_else(|| format!("没有帧 {old}"))?;
        let can = cur.can.unwrap_or_default();
        let (cur_id, cur_ext, cur_tx, cur_rx, cur_len, cur_comment) = (
            can.identifier,
            can.extended,
            can.tx_behavior,
            can.rx_behavior,
            cur.length_bytes,
            cur.comment.clone(),
        );
        let new_name = self.edit.name.clone();
        let comment = self.edit.comment.clone();
        let id = self.edit.identifier.max(0) as u32;
        let len = self.edit.length.max(1) as u32;
        let ext = self.edit.addressing == 1;
        let tx = if self.edit.tx_behavior == 1 {
            crate::system::CanBehavior::CanFd
        } else {
            crate::system::CanBehavior::Can20
        };
        let rx = if self.edit.rx_behavior == 1 {
            crate::system::CanBehavior::CanFd
        } else {
            crate::system::CanBehavior::Can20
        };
        let apply_name = old.to_string();
        self.apply(move |doc, src| {
            use crate::system::edit as e;
            e::rename_frame(doc, src, &apply_name, &new_name)?;
            if id != cur_id {
                e::set_frame_identifier(doc, src, &new_name, id)?;
            }
            if len != cur_len {
                e::set_frame_length(doc, src, &new_name, len)?;
            }
            if ext != cur_ext {
                e::set_frame_addressing(doc, src, &new_name, ext)?;
            }
            if tx != cur_tx {
                e::set_frame_tx_behavior(doc, src, &new_name, tx)?;
            }
            if rx != cur_rx {
                e::set_frame_rx_behavior(doc, src, &new_name, rx)?;
            }
            if comment != cur_comment {
                e::set_frame_comment(doc, src, &new_name, &comment)?;
            }
            Ok(())
        })
    }

    fn commit_pdu(&mut self, old: &str) -> Result<(), String> {
        let cur = self
            .doc
            .pdus
            .iter()
            .find(|p| p.name == old)
            .cloned()
            .ok_or_else(|| format!("没有 PDU {old}"))?;
        let (cur_len, cur_cycle, cur_comment) =
            (cur.length_bytes, cur.cycle_ms, cur.comment.clone());
        let new_name = self.edit.name.clone();
        let comment = self.edit.comment.clone();
        let len = self.edit.length.max(1) as u32;
        let cycle = parse_f(&self.edit.cycle);
        let apply_name = old.to_string();
        self.apply(move |doc, src| {
            use crate::system::edit as e;
            e::rename_pdu(doc, src, &apply_name, &new_name)?;
            if len != cur_len {
                e::set_pdu_length(doc, src, &new_name, len)?;
            }
            if let Some(ms) = cycle {
                if Some(ms) != cur_cycle {
                    e::set_pdu_cycle(doc, src, &new_name, ms)?;
                }
            }
            if comment != cur_comment {
                e::set_pdu_comment(doc, src, &new_name, &comment)?;
            }
            Ok(())
        })
    }

    fn commit_signal(&mut self, pdu: &str, old: &str) -> Result<(), String> {
        let cur = self
            .doc
            .pdus
            .iter()
            .find(|p| p.name == pdu)
            .and_then(|p| p.signals.iter().find(|s| s.name == old))
            .cloned()
            .ok_or_else(|| format!("没有信号 {old}"))?;
        let new_name = self.edit.name.clone();
        let comment = self.edit.comment.clone();
        let unit = self.edit.unit.clone();
        let values: Vec<(i64, String)> = self
            .edit
            .values
            .iter()
            .map(|(v, d)| (*v as i64, d.clone()))
            .collect();
        let start = self.edit.start.max(0) as u32;
        let bits = self.edit.bits.max(1) as u32;
        let order = if self.edit.order == 1 {
            crate::system::ByteOrder::LittleEndian
        } else {
            crate::system::ByteOrder::BigEndian
        };
        let factor = parse_f(&self.edit.factor).unwrap_or(cur.factor);
        let offset = parse_f(&self.edit.offset).unwrap_or(cur.offset);
        let min = parse_f(&self.edit.min).unwrap_or(cur.min);
        let max = parse_f(&self.edit.max).unwrap_or(cur.max);
        let apply_name = old.to_string();
        let apply_pdu = pdu.to_string();
        self.apply(move |doc, src| {
            use crate::system::edit as e;
            e::rename_signal(doc, src, &apply_pdu, &apply_name, &new_name)?;
            if start != cur.start_bit {
                e::set_signal_start(doc, src, &apply_pdu, &new_name, start)?;
            }
            if bits != cur.length_bits {
                e::set_signal_length(doc, src, &apply_pdu, &new_name, bits)?;
            }
            if order != cur.byte_order {
                e::set_signal_order(doc, src, &apply_pdu, &new_name, order)?;
            }
            if (factor - cur.factor).abs() > 1e-12
                || (offset - cur.offset).abs() > 1e-12
                || (min - cur.min).abs() > 1e-12
                || (max - cur.max).abs() > 1e-12
            {
                e::set_signal_scaling(doc, src, &apply_pdu, &new_name, factor, offset, min, max)?;
            }
            if unit != cur.unit {
                e::set_signal_unit(doc, src, &apply_pdu, &new_name, &unit)?;
            }
            if comment != cur.comment {
                e::set_signal_comment(doc, src, &apply_pdu, &new_name, &comment)?;
            }
            if values != cur.value_descriptions {
                e::set_signal_values(doc, src, &apply_pdu, &new_name, values)?;
            }
            Ok(())
        })
    }

    fn frame_panel(&mut self, ui: &Ui, f: &Frame) {
        use dear_imgui_rs::TreeNodeFlags;
        if ui.collapsing_header("Frame", TreeNodeFlags::DEFAULT_OPEN) {
            field(
                ui,
                "Short Name",
                |ui, b| {
                    ui.input_text("##fname", &mut b.name).build();
                },
                &mut self.edit,
            );
            field(
                ui,
                "Description",
                |ui, b| {
                    ui.input_text("##fcomment", &mut b.comment).build();
                },
                &mut self.edit,
            );
            field(
                ui,
                "Length [Byte]",
                |ui, b| {
                    ui.input_int("##flen", &mut b.length);
                },
                &mut self.edit,
            );
            field(ui, "Cluster", |ui, _b| ui.text(&f.cluster), &mut self.edit);
            if !f.path.is_empty() {
                kv(ui, "Path", crate::system::can_read::display_path(&f.path));
            }
        }
        if f.can.is_some() && ui.collapsing_header("CAN Properties", TreeNodeFlags::DEFAULT_OPEN) {
            field(
                ui,
                "Identifier",
                |ui, b| {
                    ui.input_int("##fid", &mut b.identifier);
                },
                &mut self.edit,
            );
            ui.align_text_to_frame_padding();
            ui.text("Addressing Mode");
            ui.same_line_with_pos(150.0);
            let mut addr = self.edit.addressing;
            if ui.combo_simple_string("##faddr", &mut addr, &["Standard", "Extended"]) {
                self.edit.addressing = addr;
            }
            ui.align_text_to_frame_padding();
            ui.text("Tx Frame Behaviour");
            ui.same_line_with_pos(150.0);
            let mut tx = self.edit.tx_behavior;
            if ui.combo_simple_string("##ftx", &mut tx, &["CAN-20", "CAN-FD"]) {
                self.edit.tx_behavior = tx;
            }
            ui.align_text_to_frame_padding();
            ui.text("Rx Frame Behaviour");
            ui.same_line_with_pos(150.0);
            let mut rx = self.edit.rx_behavior;
            if ui.combo_simple_string("##frx", &mut rx, &["CAN-20", "CAN-FD"]) {
                self.edit.rx_behavior = rx;
            }
        }
        if ui.collapsing_header("PDU Mappings", TreeNodeFlags::DEFAULT_OPEN) {
            if f.pdus.is_empty() {
                ui.text("(none)");
            }
            for m in &f.pdus {
                let len = self
                    .doc
                    .pdus
                    .iter()
                    .find(|p| p.name == m.pdu_name)
                    .map(|p| p.length_bytes.to_string())
                    .unwrap_or_else(|| "?".to_string());
                ui.text(format!(
                    "{}   start byte {}   (length {} B)",
                    m.pdu_name, m.start_position, len
                ));
            }
        }
        if ui.collapsing_header("Rx/Tx", TreeNodeFlags::DEFAULT_OPEN) {
            kv(ui, "Senders", f.senders.join(", "));
            kv(ui, "Receivers", f.receivers.join(", "));
            if let Some(t) = &f.triggering {
                kv(ui, "Frame Triggering", t);
            }
        }
    }

    fn pdu_panel(&mut self, ui: &Ui, p: &Pdu) {
        use dear_imgui_rs::TreeNodeFlags;
        if ui.collapsing_header("PDU", TreeNodeFlags::DEFAULT_OPEN) {
            field(
                ui,
                "Short Name",
                |ui, b| {
                    ui.input_text("##pname", &mut b.name).build();
                },
                &mut self.edit,
            );
            kv(ui, "PDU Type", p.pdu_type.label());
            field(
                ui,
                "Description",
                |ui, b| {
                    ui.input_text("##pcomment", &mut b.comment).build();
                },
                &mut self.edit,
            );
            field(
                ui,
                "Length [Byte]",
                |ui, b| {
                    ui.input_int("##plen", &mut b.length);
                },
                &mut self.edit,
            );
            field(
                ui,
                "Cycle [ms]",
                |ui, b| {
                    ui.input_text("##pcycle", &mut b.cycle).build();
                },
                &mut self.edit,
            );
            if !p.path.is_empty() {
                kv(ui, "Path", crate::system::can_read::display_path(&p.path));
            }
            if let Some(t) = &p.triggering {
                kv(ui, "PDU Triggering", t);
            }
        }
        if ui.collapsing_header("Rx/Tx", TreeNodeFlags::DEFAULT_OPEN) {
            kv(ui, "Senders", p.senders.join(", "));
            kv(ui, "Receivers", p.receivers.join(", "));
        }
    }

    fn signal_panel(&mut self, ui: &Ui, pdu_name: &str, s: &Signal) {
        use dear_imgui_rs::TreeNodeFlags;
        if ui.collapsing_header("Signal", TreeNodeFlags::DEFAULT_OPEN) {
            field(
                ui,
                "Short Name",
                |ui, b| {
                    ui.input_text("##sname", &mut b.name).build();
                },
                &mut self.edit,
            );
            field(ui, "PDU", |ui, _b| ui.text(pdu_name), &mut self.edit);
            field(
                ui,
                "Description",
                |ui, b| {
                    ui.input_text("##scomment", &mut b.comment).build();
                },
                &mut self.edit,
            );
            field(
                ui,
                "Start Bit",
                |ui, b| {
                    ui.input_int("##sstart", &mut b.start);
                },
                &mut self.edit,
            );
            field(
                ui,
                "Length [Bit]",
                |ui, b| {
                    ui.input_int("##sbits", &mut b.bits);
                },
                &mut self.edit,
            );
            ui.align_text_to_frame_padding();
            ui.text("Byte Order");
            ui.same_line_with_pos(150.0);
            let mut order = self.edit.order;
            if ui.combo_simple_string("##sorder", &mut order, &["Motorola (BE)", "Intel (LE)"]) {
                self.edit.order = order;
            }
            if !s.path.is_empty() {
                kv(ui, "Path", crate::system::can_read::display_path(&s.path));
            }
        }
        if ui.collapsing_header("Computation", TreeNodeFlags::DEFAULT_OPEN) {
            field(
                ui,
                "Factor",
                |ui, b| {
                    ui.input_text("##sfactor", &mut b.factor).build();
                },
                &mut self.edit,
            );
            field(
                ui,
                "Offset",
                |ui, b| {
                    ui.input_text("##soffset", &mut b.offset).build();
                },
                &mut self.edit,
            );
            field(
                ui,
                "Min",
                |ui, b| {
                    ui.input_text("##smin", &mut b.min).build();
                },
                &mut self.edit,
            );
            field(
                ui,
                "Max",
                |ui, b| {
                    ui.input_text("##smax", &mut b.max).build();
                },
                &mut self.edit,
            );
            field(
                ui,
                "Unit",
                |ui, b| {
                    ui.input_text("##sunit", &mut b.unit).build();
                },
                &mut self.edit,
            );
        }
        if ui.collapsing_header("Value Table", TreeNodeFlags::NONE) {
            let mut remove: Option<usize> = None;
            for (i, (v, d)) in self.edit.values.iter_mut().enumerate() {
                let _id = ui.push_id(i as i32);
                ui.set_next_item_width(70.0);
                ui.input_int("##vv", v);
                ui.same_line();
                ui.set_next_item_width(220.0);
                ui.input_text("##vd", d).build();
                ui.same_line();
                if ui.button("Remove") {
                    remove = Some(i);
                }
            }
            if let Some(i) = remove {
                self.edit.values.remove(i);
            }
            if ui.button("Add Value") {
                let next = self.edit.values.last().map(|(v, _)| v + 1).unwrap_or(0);
                self.edit.values.push((next, String::new()));
            }
        }
        if ui.collapsing_header("Rx/Tx", TreeNodeFlags::DEFAULT_OPEN) {
            kv(ui, "Senders", s.senders.join(", "));
            kv(ui, "Receivers", s.receivers.join(", "));
            if let Some(t) = &s.triggering {
                kv(ui, "Signal Triggering", t);
            }
        }
    }

    /// 编辑结果写到工具条上：成功清掉旧提示，失败说明原因
    fn note(&mut self, res: Result<(), String>) {
        match res {
            Ok(()) => self.save_note = None,
            Err(e) => self.save_note = Some(e),
        }
    }
}

/// 灰掉的按钮：不可用时画出来但点不动
fn button_enabled(ui: &Ui, label: &str, enabled: bool) -> bool {
    let _disabled = ui.begin_disabled_with_cond(!enabled);
    ui.button(label)
}

/// 一行"标签 + 输入框"：输入框绑到编辑缓冲区
fn field(ui: &Ui, label: &str, input: impl FnOnce(&Ui, &mut EditBuffers), b: &mut EditBuffers) {
    ui.align_text_to_frame_padding();
    ui.text(label);
    ui.same_line_with_pos(150.0);
    input(ui, b);
}

fn fmt_f(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 && v.abs() < 1e15 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn parse_f(t: &str) -> Option<f64> {
    let t = t.trim();
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok()
}

fn header(ui: &Ui, what: &str, cluster: &str, count: usize) {
    ui.text(format!("{what} of {cluster} ({count})"));
}

/// 表格骨架：建表、铺列、冻结表头，然后跑行
fn table_of(ui: &Ui, id: &str, height: f32, columns: &[&str], rows: impl FnOnce(&Ui)) {
    let options = TableOptions::new()
        .flags(
            TableFlags::RESIZABLE
                | TableFlags::BORDERS
                | TableFlags::ROW_BG
                | TableFlags::SCROLL_X
                | TableFlags::SCROLL_Y,
        )
        .sizing_policy(TableSizingPolicy::FixedFit);
    if let Some(_t) =
        ui.begin_table_with_sizing(id, columns.len(), options, [0.0, height.max(120.0)], 0.0)
    {
        for name in columns {
            ui.table_setup_column(name, TableColumnFlags::NONE, None);
        }
        ui.table_setup_scroll_freeze(0, 1);
        ui.table_headers_row();
        rows(ui);
    }
}

fn cell(ui: &Ui, text: impl AsRef<str>) {
    ui.table_next_column();
    ui.text(text);
}

fn cell_selectable(ui: &Ui, name: &str, prefix: &str, selected: bool) -> bool {
    ui.table_next_column();
    ui.selectable_config(format!("{name}##{prefix}_{name}"))
        .selected(selected)
        .build()
}

fn frames_table(ui: &Ui, frames: &[Frame], sel: &mut Option<String>, height: f32) {
    table_of(
        ui,
        "sys_frames",
        height,
        &[
            "Name",
            "Length [Byte]",
            "Identifier",
            "Addressing",
            "Behaviour",
            "PDUs",
            "Senders",
            "Receivers",
            "Comment",
        ],
        |ui| {
            for f in frames {
                ui.table_next_row();
                if cell_selectable(ui, &f.name, "frm", sel.as_deref() == Some(f.name.as_str())) {
                    *sel = Some(f.name.clone());
                }
                cell(ui, f.length_bytes.to_string());
                match f.can {
                    Some(can) => {
                        cell(ui, format!("0x{:03X} ({})", can.identifier, can.identifier));
                        cell(ui, if can.extended { "Extended" } else { "Standard" });
                        cell(
                            ui,
                            if can.fd || can.tx_behavior.as_str() == "CAN-FD" {
                                "CAN-FD"
                            } else {
                                "CAN-20"
                            },
                        );
                    }
                    // FlexRay 帧没有 CAN 标识符与寻址方式，留空而不是编一个值
                    None => {
                        cell(ui, "-");
                        cell(ui, "-");
                    }
                }
                cell(ui, f.pdus.len().to_string());
                cell(ui, f.senders.join(", "));
                cell(ui, f.receivers.join(", "));
                cell(ui, f.comment.clone());
            }
        },
    );
}

fn pdus_table(ui: &Ui, pdus: &[Pdu], sel: &mut Option<String>, height: f32, flexray: bool) {
    table_of(
        ui,
        "sys_pdus",
        height,
        &[
            "Name",
            "PDU Type",
            "Length [Byte]",
            "Kind",
            "Cycle [ms]",
            "Signals",
            "Senders",
            "Receivers",
        ],
        |ui| {
            for p in pdus {
                ui.table_next_row();
                if cell_selectable(ui, &p.name, "pdu", sel.as_deref() == Some(p.name.as_str())) {
                    *sel = Some(p.name.clone());
                }
                cell(ui, p.pdu_type.label());
                cell(ui, p.length_bytes.to_string());
                // Kind 是 FlexRay 的静态/动态段概念，CAN 集群没有
                cell(ui, if flexray { p.kind.label() } else { "-" });
                cell(
                    ui,
                    p.cycle_ms
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                );
                cell(ui, p.signals.len().to_string());
                cell(ui, p.senders.join(", "));
                cell(ui, p.receivers.join(", "));
            }
        },
    );
}

fn signals_table(ui: &Ui, signals: &[(String, Signal)], sel: &mut Option<String>, height: f32) {
    table_of(
        ui,
        "sys_signals",
        height,
        &[
            "PDU",
            "Name",
            "Start",
            "Length",
            "Order",
            "Type",
            "Factor",
            "Offset",
            "Min",
            "Max",
            "Unit",
            "Receivers",
        ],
        |ui| {
            for (pdu, s) in signals {
                ui.table_next_row();
                cell(ui, pdu.clone());
                if cell_selectable(ui, &s.name, "sig", sel.as_deref() == Some(s.name.as_str())) {
                    *sel = Some(s.name.clone());
                }
                cell(ui, s.start_bit.to_string());
                cell(ui, s.length_bits.to_string());
                cell(ui, format!("{:?}", s.byte_order));
                cell(ui, format!("{:?}", s.value_type));
                cell(ui, s.factor.to_string());
                cell(ui, s.offset.to_string());
                cell(ui, s.min.to_string());
                cell(ui, s.max.to_string());
                cell(ui, s.unit.clone());
                cell(ui, s.receivers.join(", "));
            }
        },
    );
}

/// PDUs 页种类过滤的下拉选项：第 0 项是全部
fn pdu_filter_labels() -> Vec<String> {
    let mut labels = vec!["All".to_string()];
    for t in [
        crate::system::PduType::ISignalIPdu,
        crate::system::PduType::UserDefined,
        crate::system::PduType::Nm,
        crate::system::PduType::Container,
        crate::system::PduType::Secured,
        crate::system::PduType::Multiplexed,
    ] {
        labels.push(t.label().to_string());
    }
    labels
}

fn filter_type(idx: usize) -> Option<crate::system::PduType> {
    use crate::system::PduType;
    match idx {
        1 => Some(PduType::ISignalIPdu),
        2 => Some(PduType::UserDefined),
        3 => Some(PduType::Nm),
        4 => Some(PduType::Container),
        5 => Some(PduType::Secured),
        6 => Some(PduType::Multiplexed),
        _ => None,
    }
}

/// 通信矩阵：行是发送 ECU，列是接收 ECU，格子里写有几帧；悬停列出帧名
fn comm_matrix(ui: &Ui, ecus: &[Ecu], frames: &[Frame], height: f32) {
    let names: Vec<String> = ecus.iter().map(|e| e.name.clone()).collect();
    let mut cols: Vec<&str> = vec!["Sender / Receiver"];
    cols.extend(names.iter().map(|s| s.as_str()));
    table_of(ui, "sys_matrix", height, &cols, |ui| {
        for row in &names {
            ui.table_next_row();
            cell(ui, row.clone());
            for col in &names {
                let hits: Vec<&str> = frames
                    .iter()
                    .filter(|f| {
                        f.senders.iter().any(|s| s == row) && f.receivers.iter().any(|r| r == col)
                    })
                    .map(|f| f.name.as_str())
                    .collect();
                ui.table_next_column();
                if !hits.is_empty() {
                    ui.text(format!("{} ✉", hits.len()));
                    if ui.is_item_hovered() {
                        ui.set_tooltip(format!("{row} → {col}:\n{}", hits.join("\n")));
                    }
                }
            }
        }
    });
}

/// 信号组：I-SIGNAL-I-PDU-GROUP，一个 ECU 收发的一组 PDU
fn groups_table(ui: &Ui, groups: &[crate::system::SignalGroup], height: f32) {
    table_of(
        ui,
        "sys_groups",
        height,
        &["Name", "ECU", "Direction", "PDUs", "Members"],
        |ui| {
            for g in groups {
                ui.table_next_row();
                cell(ui, g.name.clone());
                cell(ui, if g.ecu.is_empty() { "-" } else { &g.ecu });
                cell(ui, g.direction.label());
                cell(ui, g.pdus.len().to_string());
                cell(ui, g.pdus.join(", "));
            }
        },
    );
}

/// 表格下方明细：选中帧的各 PDU 与信号；点信号行选中它去 Signals 页编辑
fn detail_pane(ui: &Ui, f: &Frame, pdus: &[Pdu], sel: &mut Option<String>) {
    let height = ui.content_region_avail()[1];
    table_of(
        ui,
        "sys_detail_table",
        height,
        &[
            "PDU",
            "Signal",
            "Start",
            "Length [Bit]",
            "Order",
            "Senders",
            "Receivers",
        ],
        |ui| {
            for m in &f.pdus {
                let Some(p) = pdus.iter().find(|p| p.name == m.pdu_name) else {
                    continue;
                };
                ui.table_next_row();
                cell(ui, p.name.clone());
                cell(ui, "(PDU)");
                cell(ui, format!("byte {}", m.start_position));
                cell(ui, (p.length_bytes * 8).to_string());
                cell(ui, "");
                cell(ui, p.senders.join(", "));
                cell(ui, p.receivers.join(", "));
                for s in &p.signals {
                    ui.table_next_row();
                    cell(ui, "");
                    if cell_selectable(ui, &s.name, "det", sel.as_deref() == Some(s.name.as_str()))
                    {
                        *sel = Some(s.name.clone());
                    }
                    cell(ui, s.start_bit.to_string());
                    cell(ui, s.length_bits.to_string());
                    cell(ui, format!("{:?}", s.byte_order));
                    cell(ui, s.senders.join(", "));
                    cell(ui, s.receivers.join(", "));
                }
            }
        },
    );
}

/// FlexRay 排程：每帧每通道一条时隙记录
fn schedule_table(ui: &Ui, frames: &[Frame], height: f32) {
    table_of(
        ui,
        "sys_schedule",
        height,
        &[
            "Frame",
            "Channel",
            "Slot",
            "Base cycle",
            "Cycle repetition",
            "Startup frame",
        ],
        |ui| {
            for f in frames {
                if f.triggerings.is_empty() {
                    ui.table_next_row();
                    cell(ui, f.name.clone());
                    cell(ui, "-");
                    cell(ui, "-");
                    cell(ui, "-");
                    cell(ui, "-");
                    cell(ui, "-");
                    continue;
                }
                for t in &f.triggerings {
                    ui.table_next_row();
                    cell(ui, f.name.clone());
                    cell(ui, t.channel.label());
                    cell(ui, t.slot_id.to_string());
                    cell(ui, t.base_cycle.to_string());
                    cell(ui, t.cycle_repetition.to_string());
                    cell(ui, if t.startup { "yes" } else { "no" });
                }
            }
        },
    );
}

/// FlexRay 集群参数：gd*/g* 一组
fn params_table(ui: &Ui, cluster: &Cluster, height: f32) {
    let p = &cluster.fr;
    let rows: [(&str, String); 16] = [
        ("Speed [kbit/s]", p.speed_kbps.to_string()),
        ("Cycle time [ms]", p.cycle_time_ms.to_string()),
        ("Macrotick [us]", p.macrotick_duration_us.to_string()),
        ("Coldstart attempts", p.coldstart_attempts.to_string()),
        ("Action point offset", p.action_point_offset.to_string()),
        (
            "Minislot action point offset",
            p.minislot_action_point_offset.to_string(),
        ),
        (
            "Dynamic slot idle phase",
            p.dynamic_slot_idle_phase.to_string(),
        ),
        ("Minor version", p.minor_version.to_string()),
        ("Network idle time", p.network_idle_time.to_string()),
        ("Number of minislots", p.number_of_minislots.to_string()),
        (
            "Number of static slots",
            p.number_of_static_slots.to_string(),
        ),
        ("Minislot duration", p.minislot_duration.to_string()),
        ("Static slot duration", p.static_slot_duration.to_string()),
        ("Symbol window", p.symbol_window.to_string()),
        (
            "Symbol window idle phase",
            p.symbol_window_idle_phase.to_string(),
        ),
        (
            "Offset correction start",
            p.offset_correction_start.to_string(),
        ),
    ];
    table_of(ui, "sys_params", height, &["Parameter", "Value"], |ui| {
        for (name, value) in rows {
            ui.table_next_row();
            cell(ui, name);
            cell(ui, value);
        }
    });
}

/// 选中帧的属性面板：Frame / CAN Properties / PDU Mappings / Rx-Tx 四组
fn kv(ui: &Ui, key: &str, value: impl AsRef<str>) {
    ui.text(key);
    // 值列固定从 190px 起：面板在子窗口内，坐标就是子窗口内的偏移
    ui.same_line_with_pos(190.0);
    ui.text(value);
}
