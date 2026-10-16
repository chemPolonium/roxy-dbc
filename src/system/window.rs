//! 系统窗口：一份文件里的 CAN 与 FlexRay 集群
//!
//! 布局照 Vector AUTOSAR Explorer：左边 `SYSTEM > 集群 > ECU` 的树；中间是选中集群
//! 的实体表（Frames / PDUs / Signals / Comm Matrix / Signal Groups，FlexRay 另有
//! Schedule 与 Cluster Params）；表格下方是选中帧的 PDU 与信号明细；右边只放表格里
//! 装不下的信息（引用路径、PDU 映射、收发与触发点名字）。
//!
//! 编辑在格子里做：勾上工具条的 Editable 后，格子里直接就是输入框与下拉框，回车或点
//! 别处提交，一步一撤销；行首那一窄列点一下选中整行。值表是列表，点 Signals 页的
//! Values 按钮开一个小对话框改。保存走"原文 + 补丁"，只替换改过的那一段字节。

use crate::fibex::editable_fibex::ByteOrder;
use crate::system::source::ArxmlSource;
use crate::system::{CanBehavior, Cluster, Ecu, Frame, Pdu, Signal, SysTab, SystemDoc};
use dear_imgui_rs::{
    Condition, MouseButton, NumericFormat, StyleVarVec2, TableColumnFlags, TableColumnWidth,
    TableFlags, TableOptions, TableSizingPolicy, Ui,
};

// ------------------------------------------------------------------ 行内编辑

/// 正在输入的单元格：只在这格被激活期间存在，存的是还没提交的文本
#[derive(Clone)]
pub struct Editing {
    pub key: String,
    pub text: String,
}

pub enum FrameField {
    Name(String),
    Comment(String),
    Length(u32),
    Identifier(u32),
    Extended(bool),
    Tx(CanBehavior),
    Rx(CanBehavior),
}

pub enum PduField {
    Name(String),
    Comment(String),
    Length(u32),
    Cycle(f64),
}

pub enum SignalField {
    Name(String),
    Comment(String),
    Start(u32),
    Length(u32),
    Order(ByteOrder),
    Factor(f64),
    Offset(f64),
    Min(f64),
    Max(f64),
    Unit(String),
}

/// 一次单元格提交
pub enum CellCommit {
    Frame(String, FrameField),
    Pdu(String, PduField),
    Signal(String, String, SignalField),
}

/// 画一张表时要用的编辑上下文：能不能改、正在改哪个格子、提交去哪、报错写哪
struct CellState<'a> {
    editable: bool,
    editing: &'a mut Option<Editing>,
    out: &'a mut Vec<CellCommit>,
    note: &'a mut Option<String>,
}

/// 输入框格子：ImGui 自己管输入中的状态，这里只把没提交的文本暂存在窗口的一格里，
/// 免得每帧被模型值盖回去。返回这一格是否刚被点中
fn edit_field(
    ui: &Ui,
    st: &mut CellState,
    key: &str,
    current: &str,
    make: impl FnOnce(&str) -> Result<CellCommit, String>,
) -> bool {
    let live = st
        .editing
        .as_ref()
        .filter(|e| e.key == key)
        .map(|e| e.text.clone());
    let has_live = live.is_some();
    let mut text = live.unwrap_or_else(|| current.to_string());
    ui.set_next_item_width(-1.0);
    ui.input_text(format!("##{key}").as_str(), &mut text)
        .build();
    let activated = ui.is_item_activated();
    if ui.is_item_deactivated_after_edit() {
        *st.editing = None;
        if text != current {
            match make(text.trim()) {
                Ok(c) => st.out.push(c),
                Err(e) => *st.note = Some(e),
            }
        }
    } else if ui.is_item_active() {
        *st.editing = Some(Editing {
            key: key.to_string(),
            text,
        });
    } else if has_live {
        *st.editing = None;
    }
    activated
}

/// 文本单元格：没勾 Editable 是文本，勾上直接就是输入框，回车或点到别处提交
fn text_cell(
    ui: &Ui,
    st: &mut CellState,
    key: &str,
    current: &str,
    label: impl AsRef<str>,
    make: impl FnOnce(&str) -> Result<CellCommit, String>,
) {
    ui.table_next_column();
    if !st.editable {
        ui.text(label);
    } else {
        edit_field(ui, st, key, current, make);
    }
}

/// 名字格：没勾 Editable 是行标签，点一下选中这一行；勾上是输入框，点它也选中这一行。
/// 返回 true 表示要把选中落到这一行
fn name_cell(
    ui: &Ui,
    st: &mut CellState,
    key: &str,
    current: &str,
    selected: bool,
    make: impl FnOnce(&str) -> Result<CellCommit, String>,
) -> bool {
    ui.table_next_column();
    if !st.editable {
        return ui
            .selectable_config(format!("{current}##{key}"))
            .selected(selected)
            .build();
    }
    edit_field(ui, st, key, current, make)
}

/// 整数格子交给 ImGui 的 InputInt：输入过滤、"敲的字不被模型值盖掉"、回车或失焦才落值
/// 都是它自带的，所以不用像文本格那样自己暂存
fn int_cell(
    ui: &Ui,
    st: &mut CellState,
    key: &str,
    current: i64,
    label: impl AsRef<str>,
    make: impl FnOnce(i64) -> CellCommit,
) {
    ui.table_next_column();
    if !st.editable {
        ui.text(label);
        return;
    }
    let mut v = current.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    ui.set_next_item_width(-1.0);
    // step 0 = 不要旁边的加减按钮，只要一个数字格
    if ui
        .input_int_config(format!("##{key}").as_str())
        .step(0)
        .build(&mut v)
    {
        st.out.push(make(i64::from(v)));
    }
}

/// 浮点格子：同样是 ImGui 自带的 InputDouble。`%.17g` 让 1 / 0.5 / 32767.5 都按原样显示，
/// 又保得住 double 的全部精度
fn f64_cell(
    ui: &Ui,
    st: &mut CellState,
    key: &str,
    current: f64,
    label: impl AsRef<str>,
    make: impl FnOnce(f64) -> CellCommit,
) {
    ui.table_next_column();
    if !st.editable {
        ui.text(label);
        return;
    }
    let fmt = NumericFormat::<f64>::new("%.17g").expect("写死的格式，校验不会失败");
    let mut v = current;
    ui.set_next_item_width(-1.0);
    if ui
        .input_double_config(format!("##{key}").as_str())
        .display_format(fmt)
        .build(&mut v)
    {
        st.out.push(make(v));
    }
}

/// 下拉单元格：没勾 Editable 是文本，勾上直接是下拉框，选中即提交
fn combo_cell(
    ui: &Ui,
    st: &mut CellState,
    key: &str,
    current: usize,
    items: &[&str],
    make: impl FnOnce(usize) -> CellCommit,
) {
    ui.table_next_column();
    if !st.editable {
        ui.text(items[current].to_string());
        return;
    }
    let mut idx = current;
    ui.set_next_item_width(-1.0);
    if ui.combo_simple_string(format!("##{key}").as_str(), &mut idx, items) {
        st.out.push(make(idx));
    }
}

// ------------------------------------------------------------------ 对话框

/// 双击 ECU / 集群、点 Values 格时弹出的编辑框
#[derive(Clone, Default)]
pub struct Modal {
    /// "ecu" / "cluster" / "values"
    pub kind: String,
    pub target: String,
    /// values 对话框里是信号所属 PDU
    pub extra: String,
    pub name: String,
    pub comment: String,
    /// 集群波特率文本
    pub baud: String,
    /// 信号值表
    pub values: Vec<(i32, String)>,
    /// 已经向 ImGui 请求过打开。只在没请求过的那一帧 open_popup 一次：
    /// 弹窗要到下一帧才真的出现，每帧都请求会把刚关掉的弹窗立刻复活
    pub shown: bool,
}

// ------------------------------------------------------------------ 窗口

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
    /// 勾选后才允许改动这份文件
    pub editable: bool,
    pub editing: Option<Editing>,
    pub modal: Option<Modal>,
    /// PDUs 页种类过滤：0 = 全部
    pub pdu_filter: usize,
    pub tab: SysTab,
    pub selected_cluster: Option<String>,
    pub selected_frame: Option<String>,
    pub selected_pdu: Option<String>,
    pub selected_signal: Option<String>,
    /// 工具条上的提示：保存结果或校验/编辑失败原因
    pub save_note: Option<String>,
    /// 保存警告的全文，显示在窗口下方的可关闭区域
    pub save_report: Option<String>,
    /// 关窗口时有未保存改动：先问
    pub close_ask: bool,
    /// 已经向 ImGui 请求打开那个"先问存不存"的弹窗
    pub close_ask_shown: bool,
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
            close_ask_shown: false,
            editable: false,
            editing: None,
            modal: None,
            pdu_filter: 0,
            doc,
            source,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
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
        self.editing = None;
    }

    pub fn redo(&mut self) {
        let Some(next) = self.redo_stack.pop() else {
            return;
        };
        self.undo_stack
            .push((self.doc.clone(), self.source.clone()));
        self.doc = next.0;
        self.source = next.1;
        self.editing = None;
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
                self.editing = None;
                Ok(())
            }
            Err(e) => {
                self.doc = snapshot.0;
                self.source = snapshot.1;
                Err(e)
            }
        }
    }

    /// 把一次单元格提交写进模型与补丁
    fn apply_commit(&mut self, commit: CellCommit) -> Result<(), String> {
        use crate::system::edit as e;
        match commit {
            CellCommit::Frame(name, field) => self.apply(move |doc, src| match field {
                FrameField::Name(v) => e::rename_frame(doc, src, &name, &v),
                FrameField::Comment(v) => e::set_frame_comment(doc, src, &name, &v),
                FrameField::Length(v) => e::set_frame_length(doc, src, &name, v.max(1)),
                FrameField::Identifier(v) => e::set_frame_identifier(doc, src, &name, v),
                FrameField::Extended(v) => e::set_frame_addressing(doc, src, &name, v),
                FrameField::Tx(v) => e::set_frame_tx_behavior(doc, src, &name, v),
                FrameField::Rx(v) => e::set_frame_rx_behavior(doc, src, &name, v),
            }),
            CellCommit::Pdu(name, field) => self.apply(move |doc, src| match field {
                PduField::Name(v) => e::rename_pdu(doc, src, &name, &v),
                PduField::Comment(v) => e::set_pdu_comment(doc, src, &name, &v),
                PduField::Length(v) => e::set_pdu_length(doc, src, &name, v.max(1)),
                PduField::Cycle(v) => e::set_pdu_cycle(doc, src, &name, v),
            }),
            CellCommit::Signal(pdu, name, field) => self.apply(move |doc, src| {
                let cur = doc
                    .pdus
                    .iter()
                    .find(|p| p.name == pdu)
                    .and_then(|p| p.signals.iter().find(|s| s.name == name))
                    .cloned()
                    .ok_or_else(|| format!("没有信号 {name}"))?;
                match field {
                    SignalField::Name(v) => e::rename_signal(doc, src, &pdu, &name, &v),
                    SignalField::Comment(v) => e::set_signal_comment(doc, src, &pdu, &name, &v),
                    SignalField::Start(v) => e::set_signal_start(doc, src, &pdu, &name, v),
                    SignalField::Length(v) => e::set_signal_length(doc, src, &pdu, &name, v),
                    SignalField::Order(v) => e::set_signal_order(doc, src, &pdu, &name, v),
                    SignalField::Unit(v) => e::set_signal_unit(doc, src, &pdu, &name, &v),
                    SignalField::Factor(v) => e::set_signal_scaling(
                        doc, src, &pdu, &name, v, cur.offset, cur.min, cur.max,
                    ),
                    SignalField::Offset(v) => e::set_signal_scaling(
                        doc, src, &pdu, &name, cur.factor, v, cur.min, cur.max,
                    ),
                    SignalField::Min(v) => e::set_signal_scaling(
                        doc, src, &pdu, &name, cur.factor, cur.offset, v, cur.max,
                    ),
                    SignalField::Max(v) => e::set_signal_scaling(
                        doc, src, &pdu, &name, cur.factor, cur.offset, cur.min, v,
                    ),
                }
            }),
        }
    }

    /// 工具条 Delete：按当前页签删选中对象
    fn delete_selected(&mut self) -> Result<(), String> {
        match self.tab {
            SysTab::Pdus => match self.selected_pdu.clone() {
                Some(n) => self.apply(move |doc, src| e_delete_pdu(doc, src, &n)),
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
                    self.apply(move |doc, src| e_delete_signal(doc, src, &pdu, &s))
                }
                None => Err("先选一条信号".to_string()),
            },
            _ => match self.selected_frame.clone() {
                Some(n) => self.apply(move |doc, src| e_delete_frame(doc, src, &n)),
                None => Err("先选一帧".to_string()),
            },
        }
    }

    /// 编辑结果写到工具条
    fn note(&mut self, res: Result<(), String>) {
        match res {
            Ok(()) => self.save_note = None,
            Err(e) => self.save_note = Some(e),
        }
    }

    /// 把当前文档写回 .arxml：原文上只替换改过的那一段
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

fn e_delete_frame(doc: &mut SystemDoc, src: &mut ArxmlSource, n: &str) -> Result<(), String> {
    crate::system::edit::delete_frame(doc, src, n)
}
fn e_delete_pdu(doc: &mut SystemDoc, src: &mut ArxmlSource, n: &str) -> Result<(), String> {
    crate::system::edit::delete_pdu(doc, src, n)
}
fn e_delete_signal(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    pdu: &str,
    n: &str,
) -> Result<(), String> {
    crate::system::edit::delete_signal(doc, src, pdu, n)
}

// ------------------------------------------------------------------ 渲染

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
            .size([1180.0, 760.0], Condition::FirstUseEver)
            .build(|| {
                // 焦点必须在 Begin/End 之间查询：build 闭包返回后当前窗口已出栈
                if ui.is_window_focused() {
                    *focused_slot = Some(i);
                }
                render_body(ui, w);
            });
        w.is_open = is_open;
        if w.is_open && !is_open && w.is_dirty() {
            w.is_open = true;
            w.close_ask = true;
        }
    }
    if let Some(i) = focused {
        ui_state.last_focused_sys_index = Some(i);
        ui_state.focus = crate::ui::state::FocusTarget::System;
    }
}

/// 关窗口前先问存不存：ImGui 自带的模态弹窗，关掉走标题栏的 X 或下面三个按钮。
/// 必须在系统窗口的绘制过程里调用：弹窗的 ID 取自当前窗口
fn render_close_ask(ui: &Ui, w: &mut SystemWindow) {
    let title = format!("Save changes?###sysclose_{}", w.sys_id);
    if !w.close_ask_shown {
        w.close_ask_shown = true;
        ui.open_popup(&title);
    }
    let mut opened = true;
    let mut done = false;
    if let Some(popup) = ui
        .begin_modal_popup_config(&title)
        .opened(&mut opened)
        .flags(dear_imgui_rs::WindowFlags::ALWAYS_AUTO_RESIZE)
        .begin()
    {
        ui.text(format!("{} 有未保存的改动。", w.file_path));
        if ui.button("Save") {
            w.save_as();
            if !w.is_dirty() {
                w.is_open = false;
            }
            done = true;
        }
        ui.same_line();
        if ui.button("Discard changes") {
            w.is_open = false;
            done = true;
        }
        ui.same_line();
        if ui.button("Keep editing") {
            done = true;
        }
        if done {
            ui.close_current_popup();
        }
        popup.end();
    }
    if done || !opened {
        w.close_ask = false;
        w.close_ask_shown = false;
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
    modal: Option<Modal>,
}

fn render_body(ui: &Ui, w: &mut SystemWindow) {
    if ui.button("Save ARXML...") {
        w.save_as();
    }
    ui.same_line();
    let mut editable = w.editable;
    if ui.checkbox("Editable", &mut editable) {
        w.editable = editable;
        w.editing = None;
        w.save_note = None;
    }
    ui.same_line();
    if editable && w.tab == SysTab::Pdus && w.selected_pdu.is_some() {
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
        ui.same_line();
    }
    if editable {
        if ui.button("Add Frame...") {
            add_frame(w);
        }
        ui.same_line();
        if ui.button("Delete") {
            let res = w.delete_selected();
            w.note(res);
        }
        ui.same_line();
    }
    if button_enabled(ui, "Undo", w.can_undo()) {
        w.undo();
    }
    ui.same_line();
    if button_enabled(ui, "Redo", w.can_redo()) {
        w.redo();
    }
    if !w.editable {
        ui.same_line();
        ui.text_disabled("(勾上 Editable 后格子里直接改)");
    }
    if let Some(note) = w.save_note.clone() {
        ui.same_line();
        ui.text_colored([0.95, 0.75, 0.25, 1.0], note);
    }
    ui.separator();

    let clusters = w.doc.clusters.clone();
    let ecus = w.doc.ecus.clone();
    let all_frames = w.doc.frames.clone();
    let all_pdus = w.doc.pdus.clone();
    let all_groups = w.doc.groups.clone();
    let mut view = View {
        tab: w.tab,
        cluster: w.selected_cluster.clone(),
        frame: w.selected_frame.clone(),
        pdu: w.selected_pdu.clone(),
        signal: w.selected_signal.clone(),
        pdu_filter: w.pdu_filter,
        report: w.save_report.clone(),
        modal: w.modal.clone(),
    };
    if view.cluster.is_none() {
        view.cluster = clusters.first().map(|c| c.name.clone());
    }

    let [avail_w, avail_h] = ui.content_region_avail();
    let left_w = 260.0f32.min(avail_w * 0.28);
    let body_h = (avail_h - 6.0).max(80.0);

    ui.child_window("sys_tree")
        .size([left_w, body_h])
        .build(ui, || {
            tree(ui, &clusters, &ecus, &mut view);
        });
    ui.same_line();

    let mut commits: Vec<CellCommit> = Vec::new();
    let mut edit_note: Option<String> = None;
    {
        let mut st = CellState {
            editable: w.editable,
            editing: &mut w.editing,
            out: &mut commits,
            note: &mut edit_note,
        };
        ui.child_window("sys_main")
            .size([avail_w - left_w - 12.0, body_h])
            .build(ui, || {
                let Some(cluster_name) = view.cluster.clone() else {
                    ui.text("No cluster in this file.");
                    return;
                };
                let Some(cluster) = clusters.iter().find(|c| c.name == cluster_name).cloned()
                else {
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
                let groups: Vec<crate::system::SignalGroup> = all_groups
                    .iter()
                    .filter(|g| g.cluster == cluster.name)
                    .cloned()
                    .collect();
                main_pane(
                    ui,
                    &mut st,
                    &mut view,
                    &cluster,
                    &frames,
                    &pdus,
                    &signals,
                    &ecus_on_cluster,
                    &groups,
                );
            });
    }

    for c in commits {
        let res = w.apply_commit(c);
        w.note(res);
    }
    if let Some(e) = edit_note {
        w.save_note = Some(e);
    }
    if w.editable {
        // 改名后旧选中可能已经不存在，清掉免得面板指到空对象
        if !w
            .doc
            .frames
            .iter()
            .any(|f| Some(&f.name) == w.selected_frame.as_ref())
        {
            w.selected_frame = w.doc.frames.first().map(|f| f.name.clone());
        }
        if !w
            .doc
            .pdus
            .iter()
            .any(|p| Some(&p.name) == w.selected_pdu.as_ref())
        {
            w.selected_pdu = w.doc.pdus.first().map(|p| p.name.clone());
        }
        if !w.doc.pdus.iter().any(|p| {
            p.signals
                .iter()
                .any(|s| Some(&s.name) == w.selected_signal.as_ref())
        }) {
            w.selected_signal = None;
        }
    }

    if let Some(report) = view.report.clone() {
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

    if let Some(m) = view.modal.clone() {
        view.modal = render_modal(ui, w, m);
    }

    w.tab = view.tab;
    w.selected_cluster = view.cluster;
    w.selected_frame = view.frame;
    w.selected_pdu = view.pdu;
    w.selected_signal = view.signal;
    w.pdu_filter = view.pdu_filter;
    w.save_report = view.report;
    w.modal = view.modal.clone();

    // 存不存的询问也在这里画：弹窗 ID 跟系统窗口绑定，别挪到窗口外面去请求
    if w.close_ask {
        render_close_ask(ui, w);
    }
}

fn add_frame(w: &mut SystemWindow) {
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
    let res =
        w.apply(|doc, src| crate::system::edit::add_frame(doc, src, &cluster, &name, next_id, 8));
    w.note(res);
}

/// 双击 ECU / 集群 / Values 格时弹出的小对话框。返回关窗后要留状态：
/// 关掉是 None，留着把输入带回去
/// 双击 ECU / 集群、点 Values 格时弹出的编辑框。用 ImGui 自带的模态弹窗：开没开由
/// ImGui 记（标题栏 X 与 Close / Apply 都算关），`w.modal` 只存"改谁、改成什么样"。
/// 标题里的 `###` 之后是固定 ID，换编辑对象不会另开一个弹窗
fn render_modal(ui: &Ui, w: &mut SystemWindow, mut m: Modal) -> Option<Modal> {
    let title = format!(
        "Edit {} - {}###sysmodal_{}",
        match m.kind.as_str() {
            "ecu" => "ECU",
            "values" => "Value Table",
            _ => "Cluster",
        },
        m.target,
        w.sys_id
    );
    if !m.shown {
        m.shown = true;
        ui.open_popup(&title);
    }
    let mut opened = true;
    let mut done = false;
    if let Some(popup) = ui
        .begin_modal_popup_config(&title)
        .opened(&mut opened)
        .begin()
    {
        match m.kind.as_str() {
            "values" => {
                let mut remove: Option<usize> = None;
                for (i, (v, d)) in m.values.iter_mut().enumerate() {
                    let _id = ui.push_id(i as i32);
                    ui.set_next_item_width(70.0);
                    ui.input_int("##vv", v);
                    ui.same_line();
                    ui.set_next_item_width(240.0);
                    ui.input_text("##vd", d).build();
                    ui.same_line();
                    if ui.button("Remove") {
                        remove = Some(i);
                    }
                }
                if let Some(i) = remove {
                    m.values.remove(i);
                }
                if ui.button("Add Value") {
                    let next = m.values.last().map(|(v, _)| v + 1).unwrap_or(0);
                    m.values.push((next, String::new()));
                }
            }
            _ => {
                ui.align_text_to_frame_padding();
                ui.text("Short Name");
                ui.same_line_with_pos(130.0);
                if m.kind == "ecu" {
                    ui.input_text("##mname", &mut m.name).build();
                } else {
                    ui.text(&m.name);
                }
                ui.align_text_to_frame_padding();
                ui.text("Description");
                ui.same_line_with_pos(130.0);
                ui.input_text("##mcomment", &mut m.comment).build();
                if m.kind == "cluster" {
                    ui.align_text_to_frame_padding();
                    ui.text("Baudrate [bit/s]");
                    ui.same_line_with_pos(130.0);
                    ui.input_text("##mbaud", &mut m.baud).build();
                } else if let Some(e) = w.doc.ecus.iter().find(|e| e.name == m.target) {
                    for c in &e.controllers {
                        ui.text(format!(
                            "Controller {}: quanta {}..{}, sample {}..{}",
                            c.name,
                            c.min_quanta.map(fmt_f).unwrap_or_else(|| "-".to_string()),
                            c.max_quanta.map(fmt_f).unwrap_or_else(|| "-".to_string()),
                            c.min_sample.map(fmt_f).unwrap_or_else(|| "-".to_string()),
                            c.max_sample.map(fmt_f).unwrap_or_else(|| "-".to_string()),
                        ));
                    }
                }
            }
        }
        if ui.button("Apply") {
            let res = apply_modal(w, &m);
            done = res.is_ok();
            w.note(res);
        }
        ui.same_line();
        if ui.button("Close") {
            done = true;
        }
        if done {
            ui.close_current_popup();
        }
        popup.end();
    }
    if done || !opened { None } else { Some(m) }
}

fn apply_modal(w: &mut SystemWindow, m: &Modal) -> Result<(), String> {
    use crate::system::edit as e;
    let kind = m.kind.clone();
    let target = m.target.clone();
    let name = m.name.clone();
    let comment = m.comment.clone();
    let baud = parse_f(&m.baud);
    let pdu = m.extra.clone();
    let values: Vec<(i64, String)> = m
        .values
        .iter()
        .map(|(v, d)| (*v as i64, d.clone()))
        .collect();
    w.apply(move |doc, src| match kind.as_str() {
        "ecu" => {
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
            Ok(())
        }
        "cluster" => {
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
            Ok(())
        }
        _ => e::set_signal_values(doc, src, &pdu, &target, values),
    })
}

fn tree(ui: &Ui, clusters: &[Cluster], ecus: &[Ecu], view: &mut View) {
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
            if clicked && ui.is_mouse_double_clicked(MouseButton::Left) {
                view.modal = Some(Modal {
                    kind: "cluster".to_string(),
                    target: c.name.clone(),
                    name: c.name.clone(),
                    comment: c.comment.clone(),
                    baud: c.baudrate.map(fmt_f).unwrap_or_default(),
                    ..Default::default()
                });
            }
            if node.is_some() {
                for e in on_bus {
                    ui.text(&e.name);
                    if ui.is_mouse_double_clicked(MouseButton::Left) && ui.is_item_hovered() {
                        view.modal = Some(Modal {
                            kind: "ecu".to_string(),
                            target: e.name.clone(),
                            name: e.name.clone(),
                            comment: e.comment.clone(),
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }
}

// ------------------------------------------------------------------ 中间那栏

#[allow(clippy::too_many_arguments)]
fn main_pane(
    ui: &Ui,
    st: &mut CellState,
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
    let table_h = (total_h * if with_detail { 0.40 } else { 0.52 }).max(140.0);

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
                frames_table(ui, st, frames, &mut view.frame, table_h);
            }
            SysTab::Pdus => {
                ui.text(format!("PDUs of {} ({})", cluster.name, pdus.len()));
                ui.same_line();
                ui.text("   PDU Type Filter:");
                ui.same_line();
                let labels = pdu_filter_labels();
                let mut idx = view.pdu_filter.min(labels.len() - 1);
                ui.set_next_item_width(130.0);
                if ui.combo_simple_string("##pdufilter", &mut idx, &labels) {
                    view.pdu_filter = idx;
                }
                let shown_pdus: Vec<Pdu> = match filter_type(view.pdu_filter) {
                    Some(t) => pdus.iter().filter(|p| p.pdu_type == t).cloned().collect(),
                    None => pdus.to_vec(),
                };
                pdus_table(ui, st, &shown_pdus, &mut view.pdu, table_h, flexray);
            }
            SysTab::Signals => {
                header(ui, "Signals", &cluster.name, signals.len());
                signals_table(ui, st, signals, &mut view.signal, &mut view.modal, table_h);
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

    // 下方明细：选中帧的各 PDU 与信号
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
                    detail_pane(ui, st, &f, &pdus_owned, &mut view.signal);
                });
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
                Some(p) => pdu_info(ui, &p),
                None => ui.text("Select a PDU."),
            },
            SysTab::Signals => match signal {
                Some((p, s)) => signal_info(ui, &p, &s),
                None => ui.text("Select a signal."),
            },
            SysTab::Frames => match frame {
                Some(f) => frame_info(ui, &f, pdus),
                None => ui.text("Select a frame."),
            },
            _ => {}
        });
}

/// 右侧只放表格里装不下的信息
fn frame_info(ui: &Ui, f: &Frame, pdus: &[Pdu]) {
    use dear_imgui_rs::TreeNodeFlags;
    if ui.collapsing_header("Frame", TreeNodeFlags::DEFAULT_OPEN) {
        kv(ui, "Cluster", &f.cluster);
        if !f.long_name.is_empty() {
            kv(ui, "Long Name", &f.long_name);
        }
        if !f.path.is_empty() {
            kv(ui, "Path", crate::system::can_read::display_path(&f.path));
        }
    }
    if let Some(can) = &f.can
        && (can.lower_can_id.is_some()
            || can.upper_can_id.is_some()
            || can.rx_mask.is_some()
            || can.tx_mask.is_some())
        && ui.collapsing_header("CAN Extras", TreeNodeFlags::DEFAULT_OPEN)
    {
        kv(ui, "Lower CAN ID", opt_u32(can.lower_can_id));
        kv(ui, "Upper CAN ID", opt_u32(can.upper_can_id));
        kv(ui, "Rx Mask", opt_u32(can.rx_mask));
        kv(ui, "Tx Mask", opt_u32(can.tx_mask));
    }
    if ui.collapsing_header("PDU Mappings", TreeNodeFlags::DEFAULT_OPEN) {
        if f.pdus.is_empty() {
            ui.text("(none)");
        }
        for m in &f.pdus {
            let len = pdus
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

fn pdu_info(ui: &Ui, p: &Pdu) {
    use dear_imgui_rs::TreeNodeFlags;
    if ui.collapsing_header("PDU", TreeNodeFlags::DEFAULT_OPEN) {
        kv(ui, "Cluster", &p.cluster);
        kv(ui, "PDU Type", p.pdu_type.label());
        if !p.long_name.is_empty() {
            kv(ui, "Long Name", &p.long_name);
        }
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

fn signal_info(ui: &Ui, pdu: &str, s: &Signal) {
    use dear_imgui_rs::TreeNodeFlags;
    if ui.collapsing_header("Signal", TreeNodeFlags::DEFAULT_OPEN) {
        kv(ui, "PDU", pdu);
        if !s.long_name.is_empty() {
            kv(ui, "Long Name", &s.long_name);
        }
        if !s.path.is_empty() {
            kv(ui, "Path", crate::system::can_read::display_path(&s.path));
        }
        if !s.compu_name.is_empty() {
            kv(ui, "Computation Method", &s.compu_name);
        }
        if let Some(v) = s.init_value {
            kv(ui, "Init Value", fmt_f(v));
        }
        if let Some(t) = &s.triggering {
            kv(ui, "Signal Triggering", t);
        }
    }
    if ui.collapsing_header("Rx/Tx", TreeNodeFlags::DEFAULT_OPEN) {
        kv(ui, "Senders", s.senders.join(", "));
        kv(ui, "Receivers", s.receivers.join(", "));
    }
    if ui.collapsing_header("Value Table", TreeNodeFlags::DEFAULT_OPEN) {
        if s.value_descriptions.is_empty() {
            ui.text("(none)");
        }
        for (v, d) in &s.value_descriptions {
            ui.text(format!("{v} = {d}"));
        }
        ui.text_disabled("点 Values 按钮可改值表");
    }
}

// ------------------------------------------------------------------ 表格

fn header(ui: &Ui, what: &str, cluster: &str, count: usize) {
    ui.text(format!("{what} of {cluster} ({count})"));
}

/// 表格的公共外壳。`columns` 是各列的表头与宽度，宽度填 0.0 表示与其它自动列平分
/// 剩下的宽度：放不下的下拉列（Addressing、Order 这些）要给固定宽度，不然格子上的字
/// 会被裁掉。`tight` 是编辑态：把输入框与下拉框自己的上下内边距去掉，它们就跟纯文本
/// 一样高，不会把整行撑大。
fn table_of(
    ui: &Ui,
    id: &str,
    height: f32,
    tight: bool,
    columns: &[(&str, f32)],
    rows: impl FnOnce(&Ui),
) {
    let _tight = tight.then(|| ui.push_style_var_y(StyleVarVec2::FramePadding, 0.0));
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
        for (name, width) in columns {
            let fixed = if *width > 0.0 {
                Some(TableColumnWidth::Fixed(*width))
            } else {
                None
            };
            ui.table_setup_column(*name, TableColumnFlags::NONE, fixed);
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

fn frames_table(
    ui: &Ui,
    st: &mut CellState,
    frames: &[Frame],
    sel: &mut Option<String>,
    height: f32,
) {
    table_of(
        ui,
        "sys_frames",
        height,
        st.editable,
        &[
            ("Name", 0.0),
            ("Length [Byte]", 0.0),
            ("Identifier", 0.0),
            ("Addressing", 92.0),
            ("Tx Behaviour", 92.0),
            ("Rx Behaviour", 92.0),
            ("PDUs", 0.0),
            ("Senders", 0.0),
            ("Receivers", 0.0),
            ("Comment", 0.0),
        ],
        |ui| {
            for f in frames {
                let selected = sel.as_deref() == Some(f.name.as_str());
                ui.table_next_row();
                let fname = f.name.clone();
                if name_cell(
                    ui,
                    st,
                    &format!("frm/{}/name", f.name),
                    &f.name,
                    selected,
                    move |v| Ok(CellCommit::Frame(fname, FrameField::Name(v.to_string()))),
                ) {
                    *sel = Some(f.name.clone());
                }
                let can = f.can.unwrap_or_default();
                int_cell(
                    ui,
                    st,
                    &format!("frm/{}/len", f.name),
                    f.length_bytes as i64,
                    f.length_bytes.to_string(),
                    move |v| CellCommit::Frame(f.name.clone(), FrameField::Length(v.max(0) as u32)),
                );
                // 标识符允许写 0x… 十六进制，InputInt 做不到，所以这格还是文本格
                let fname = f.name.clone();
                text_cell(
                    ui,
                    st,
                    &format!("frm/{}/id", f.name),
                    &can.identifier.to_string(),
                    format!("0x{:X} ({})", can.identifier, can.identifier),
                    move |t| {
                        let v = parse_int(t)?;
                        Ok(CellCommit::Frame(
                            fname.clone(),
                            FrameField::Identifier(v.max(0) as u32),
                        ))
                    },
                );
                combo_cell(
                    ui,
                    st,
                    &format!("frm/{}/addr", f.name),
                    usize::from(can.extended),
                    &["Standard", "Extended"],
                    move |v| CellCommit::Frame(f.name.clone(), FrameField::Extended(v == 1)),
                );
                combo_cell(
                    ui,
                    st,
                    &format!("frm/{}/tx", f.name),
                    usize::from(can.tx_behavior == CanBehavior::CanFd),
                    &["CAN-20", "CAN-FD"],
                    move |v| {
                        CellCommit::Frame(
                            f.name.clone(),
                            FrameField::Tx(if v == 1 {
                                CanBehavior::CanFd
                            } else {
                                CanBehavior::Can20
                            }),
                        )
                    },
                );
                let (fname, rx) = (f.name.clone(), can.rx_behavior);
                combo_cell(
                    ui,
                    st,
                    &format!("frm/{}/rx", f.name),
                    usize::from(rx == CanBehavior::CanFd),
                    &["CAN-20", "CAN-FD"],
                    move |v| {
                        CellCommit::Frame(
                            fname.clone(),
                            FrameField::Rx(if v == 1 {
                                CanBehavior::CanFd
                            } else {
                                CanBehavior::Can20
                            }),
                        )
                    },
                );
                cell(ui, f.pdus.len().to_string());
                cell(ui, f.senders.join(", "));
                cell(ui, f.receivers.join(", "));
                text_cell(
                    ui,
                    st,
                    &format!("frm/{}/comment", f.name),
                    &f.comment,
                    f.comment.clone(),
                    move |v| {
                        Ok(CellCommit::Frame(
                            f.name.clone(),
                            FrameField::Comment(v.to_string()),
                        ))
                    },
                );
            }
        },
    );
}

fn pdus_table(
    ui: &Ui,
    st: &mut CellState,
    pdus: &[Pdu],
    sel: &mut Option<String>,
    height: f32,
    flexray: bool,
) {
    table_of(
        ui,
        "sys_pdus",
        height,
        st.editable,
        &[
            ("Name", 0.0),
            ("PDU Type", 112.0),
            ("Length [Byte]", 0.0),
            ("Kind", 0.0),
            ("Cycle [ms]", 0.0),
            ("Signals", 0.0),
            ("Senders", 0.0),
            ("Receivers", 0.0),
            ("Comment", 0.0),
        ],
        |ui| {
            for p in pdus {
                let selected = sel.as_deref() == Some(p.name.as_str());
                ui.table_next_row();
                let pname = p.name.clone();
                if name_cell(
                    ui,
                    st,
                    &format!("pdu/{}/name", p.name),
                    &p.name,
                    selected,
                    move |v| Ok(CellCommit::Pdu(pname, PduField::Name(v.to_string()))),
                ) {
                    *sel = Some(p.name.clone());
                }
                cell(ui, p.pdu_type.label());
                int_cell(
                    ui,
                    st,
                    &format!("pdu/{}/len", p.name),
                    p.length_bytes as i64,
                    p.length_bytes.to_string(),
                    move |v| CellCommit::Pdu(p.name.clone(), PduField::Length(v.max(0) as u32)),
                );
                // 静态/动态段是 FlexRay 的概念
                cell(ui, if flexray { p.kind.label() } else { "-" });
                match p.cycle_ms {
                    Some(ms) => {
                        let name = p.name.clone();
                        f64_cell(
                            ui,
                            st,
                            &format!("pdu/{}/cycle", p.name),
                            ms,
                            fmt_f(ms),
                            move |v| CellCommit::Pdu(name.clone(), PduField::Cycle(v)),
                        )
                    }
                    None => {
                        let cur = String::new();
                        let name = p.name.clone();
                        text_cell(
                            ui,
                            st,
                            &format!("pdu/{}/cycle", p.name),
                            "",
                            cur,
                            move |v| {
                                let ms: f64 = v
                                    .parse()
                                    .map_err(|_| format!("周期要填数字，不能填「{v}」"))?;
                                Ok(CellCommit::Pdu(name.clone(), PduField::Cycle(ms)))
                            },
                        );
                    }
                }
                cell(ui, p.signals.len().to_string());
                cell(ui, p.senders.join(", "));
                cell(ui, p.receivers.join(", "));
                let name = p.name.clone();
                text_cell(
                    ui,
                    st,
                    &format!("pdu/{}/comment", p.name),
                    &p.comment,
                    p.comment.clone(),
                    move |v| {
                        Ok(CellCommit::Pdu(
                            name.clone(),
                            PduField::Comment(v.to_string()),
                        ))
                    },
                );
            }
        },
    );
}

fn signals_table(
    ui: &Ui,
    st: &mut CellState,
    signals: &[(String, Signal)],
    sel: &mut Option<String>,
    modal: &mut Option<Modal>,
    height: f32,
) {
    table_of(
        ui,
        "sys_signals",
        height,
        st.editable,
        &[
            ("PDU", 0.0),
            ("Name", 0.0),
            ("Start", 0.0),
            ("Length", 0.0),
            ("Order", 118.0),
            ("Type", 96.0),
            ("Factor", 0.0),
            ("Offset", 0.0),
            ("Min", 0.0),
            ("Max", 0.0),
            ("Unit", 0.0),
            ("Values", 86.0),
            ("Receivers", 0.0),
            ("Comment", 0.0),
        ],
        |ui| {
            for (pdu, s) in signals {
                let selected = sel.as_deref() == Some(s.name.as_str());
                let key = |field: &str| format!("sig/{pdu}/{}/{field}", s.name);
                ui.table_next_row();
                cell(ui, pdu.clone());
                let (p, n) = (pdu.clone(), s.name.clone());
                if name_cell(ui, st, &key("name"), &s.name, selected, move |v| {
                    Ok(CellCommit::Signal(
                        p.clone(),
                        n.clone(),
                        SignalField::Name(v.to_string()),
                    ))
                }) {
                    *sel = Some(s.name.clone());
                }
                let (p, n) = (pdu.clone(), s.name.clone());
                int_cell(
                    ui,
                    st,
                    &key("start"),
                    s.start_bit as i64,
                    s.start_bit.to_string(),
                    move |v| {
                        CellCommit::Signal(
                            p.clone(),
                            n.clone(),
                            SignalField::Start(v.max(0) as u32),
                        )
                    },
                );
                let (p, n) = (pdu.clone(), s.name.clone());
                int_cell(
                    ui,
                    st,
                    &key("len"),
                    s.length_bits as i64,
                    s.length_bits.to_string(),
                    move |v| {
                        CellCommit::Signal(
                            p.clone(),
                            n.clone(),
                            SignalField::Length(v.max(0) as u32),
                        )
                    },
                );
                let (p, n) = (pdu.clone(), s.name.clone());
                combo_cell(
                    ui,
                    st,
                    &key("order"),
                    usize::from(s.byte_order == ByteOrder::LittleEndian),
                    &["Motorola (BE)", "Intel (LE)"],
                    move |v| {
                        CellCommit::Signal(
                            p.clone(),
                            n.clone(),
                            SignalField::Order(if v == 1 {
                                ByteOrder::LittleEndian
                            } else {
                                ByteOrder::BigEndian
                            }),
                        )
                    },
                );
                cell(ui, format!("{:?}", s.value_type));
                for (field, value) in [
                    ("factor", s.factor),
                    ("offset", s.offset),
                    ("min", s.min),
                    ("max", s.max),
                ] {
                    let (p, n) = (pdu.clone(), s.name.clone());
                    let key = format!("sig/{pdu}/{}/{field}", s.name);
                    let make = move |v: f64| {
                        CellCommit::Signal(
                            p.clone(),
                            n.clone(),
                            match field {
                                "factor" => SignalField::Factor(v),
                                "offset" => SignalField::Offset(v),
                                "min" => SignalField::Min(v),
                                _ => SignalField::Max(v),
                            },
                        )
                    };
                    f64_cell(ui, st, &key, value, fmt_f(value), make);
                }
                let (p, n) = (pdu.clone(), s.name.clone());
                text_cell(ui, st, &key("unit"), &s.unit, s.unit.clone(), move |v| {
                    Ok(CellCommit::Signal(
                        p.clone(),
                        n.clone(),
                        SignalField::Unit(v.to_string()),
                    ))
                });
                // 值表是成对的 (原始值, 描述)，一格放不下：格子里放按钮，点开对话框
                ui.table_next_column();
                let summary = if s.value_descriptions.is_empty() {
                    "-".to_string()
                } else {
                    let n = s.value_descriptions.len();
                    format!("{n} value{}", if n == 1 { "" } else { "s" })
                };
                let open_values = if st.editable {
                    ui.button(format!("{summary}##{}", key("values")))
                } else {
                    ui.text(summary);
                    false
                };
                if open_values {
                    *modal = Some(Modal {
                        kind: "values".to_string(),
                        target: s.name.clone(),
                        extra: pdu.clone(),
                        values: s
                            .value_descriptions
                            .iter()
                            .map(|(v, d)| (*v as i32, d.clone()))
                            .collect(),
                        ..Default::default()
                    });
                }
                cell(ui, s.receivers.join(", "));
                let (p, n) = (pdu.clone(), s.name.clone());
                text_cell(
                    ui,
                    st,
                    &key("comment"),
                    &s.comment,
                    s.comment.clone(),
                    move |v| {
                        Ok(CellCommit::Signal(
                            p.clone(),
                            n.clone(),
                            SignalField::Comment(v.to_string()),
                        ))
                    },
                );
            }
        },
    );
}

/// 表格下方明细：选中帧的各 PDU 与信号，起始位/长度可直接改
fn detail_pane(ui: &Ui, st: &mut CellState, f: &Frame, pdus: &[Pdu], sel: &mut Option<String>) {
    let height = ui.content_region_avail()[1];
    table_of(
        ui,
        "sys_detail_table",
        height,
        st.editable,
        &[
            ("PDU", 0.0),
            ("Signal", 0.0),
            ("Start", 0.0),
            ("Length [Bit]", 0.0),
            ("Order", 106.0),
            ("Senders", 0.0),
            ("Receivers", 0.0),
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
                    let selected = sel.as_deref() == Some(s.name.as_str());
                    ui.table_next_row();
                    cell(ui, "");
                    ui.table_next_column();
                    if ui
                        .selectable_config(format!("{}##det_{}", s.name, s.name))
                        .selected(selected)
                        .build()
                    {
                        *sel = Some(s.name.clone());
                    }
                    let (pn, sn) = (p.name.clone(), s.name.clone());
                    int_cell(
                        ui,
                        st,
                        &format!("pdu/{pn}/map/{sn}/start"),
                        s.start_bit as i64,
                        s.start_bit.to_string(),
                        move |v| {
                            CellCommit::Signal(
                                pn.clone(),
                                sn.clone(),
                                SignalField::Start(v.max(0) as u32),
                            )
                        },
                    );
                    let (pn, sn) = (p.name.clone(), s.name.clone());
                    int_cell(
                        ui,
                        st,
                        &format!("sig/{sn}/len"),
                        s.length_bits as i64,
                        s.length_bits.to_string(),
                        move |v| {
                            CellCommit::Signal(
                                pn.clone(),
                                sn.clone(),
                                SignalField::Length(v.max(0) as u32),
                            )
                        },
                    );
                    cell(ui, format!("{:?}", s.byte_order));
                    cell(ui, s.senders.join(", "));
                    cell(ui, s.receivers.join(", "));
                }
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
    let mut cols: Vec<(&str, f32)> = vec![("Sender / Receiver", 150.0)];
    cols.extend(names.iter().map(|s| (s.as_str(), 0.0)));
    table_of(ui, "sys_matrix", height, false, &cols, |ui| {
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

fn groups_table(ui: &Ui, groups: &[crate::system::SignalGroup], height: f32) {
    table_of(
        ui,
        "sys_groups",
        height,
        false,
        &[
            ("Name", 0.0),
            ("ECU", 0.0),
            ("Direction", 0.0),
            ("PDUs", 0.0),
            ("Members", 0.0),
        ],
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

/// FlexRay 排程：每帧每通道一条时隙记录
fn schedule_table(ui: &Ui, frames: &[Frame], height: f32) {
    table_of(
        ui,
        "sys_schedule",
        height,
        false,
        &[
            ("Frame", 0.0),
            ("Channel", 0.0),
            ("Slot", 0.0),
            ("Base cycle", 0.0),
            ("Cycle repetition", 0.0),
            ("Startup frame", 0.0),
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
    table_of(
        ui,
        "sys_params",
        height,
        false,
        &[("Parameter", 220.0), ("Value", 0.0)],
        |ui| {
            for (name, value) in rows {
                ui.table_next_row();
                cell(ui, name);
                cell(ui, value);
            }
        },
    );
}

// ------------------------------------------------------------------ 小工具

fn button_enabled(ui: &Ui, label: &str, enabled: bool) -> bool {
    let _disabled = ui.begin_disabled_with_cond(!enabled);
    ui.button(label)
}

fn opt_u32(v: Option<u32>) -> String {
    v.map(|x| x.to_string()).unwrap_or_else(|| "-".to_string())
}

fn kv(ui: &Ui, key: &str, value: impl AsRef<str>) {
    ui.text(key);
    ui.same_line_with_pos(170.0);
    ui.text(value);
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

/// 十进制或 `0x` 十六进制
fn parse_int(t: &str) -> Result<i64, String> {
    let t = t.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return i64::from_str_radix(hex, 16).map_err(|_| format!("不是合法十六进制：{t}"));
    }
    t.parse::<i64>()
        .map_err(|_| format!("要填整数，不能填「{t}」"))
}
