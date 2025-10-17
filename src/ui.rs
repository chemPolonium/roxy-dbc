//! UI 渲染和界面逻辑
use crate::dbc::DbcData;
use imgui::{
    Condition, Key, TableColumnFlags, TableColumnSetup, TableFlags, TableSortDirection, Ui,
    WindowFocusedFlags,
};
use std::collections::HashMap;
use std::time::Duration;

/// DBC 窗口状态
#[derive(Clone)]
pub struct DbcWindowState {
    pub id: usize,
    pub dbc_data: DbcData,
    pub search_query: String,
    pub selected_message_id: Option<u32>,
    pub is_open: bool,
    // ---- Undo/Redo 支持 ----
    pub undo_stack: Vec<UndoEntry>,
    pub redo_stack: Vec<UndoEntry>,
    // ---- Message 重命名支持 ----
    pub message_name_overrides: HashMap<u32, String>,
}

/// 可撤销的操作类型（骨架，后续可扩展携带更多上下文）
#[derive(Clone, Debug)]
pub enum UndoOperationKind {
    AddMessage {
        message_id: u32,
    },
    RemoveMessage {
        message_id: u32,
    },
    ModifyMessage {
        message_id: u32,
        field: String,
        old_value: String,
        new_value: String,
    }, // 通用字段修改
    RenameMessage {
        message_id: u32,
        old_name: String,
        new_name: String,
    },
    AddSignal {
        message_id: u32,
        signal_name: String,
    },
    RemoveSignal {
        message_id: u32,
        signal_name: String,
    },
    ModifySignal {
        message_id: u32,
        signal_name: String,
        field: String,
    },
    // 未来: 批量操作 / 重排序
}

/// Undo 条目：当前采用全量快照策略（简单可靠，DBC体量小）
#[derive(Clone)]
pub struct UndoEntry {
    pub op: UndoOperationKind,
    pub before: DbcData, // 操作前快照
    pub after: DbcData,  // 操作后快照
}

impl DbcWindowState {
    const MAX_UNDO_ENTRIES: usize = 100;

    /// 创建新的 DBC 窗口状态
    pub fn new(id: usize, dbc_data: DbcData) -> Self {
        Self {
            id,
            dbc_data,
            search_query: String::new(),
            selected_message_id: None,
            is_open: true,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            message_name_overrides: HashMap::new(),
        }
    }

    /// 记录一次可撤销操作
    pub fn push_undo(&mut self, op: UndoOperationKind, before: &DbcData, after: &DbcData) {
        self.redo_stack.clear();
        self.undo_stack.push(UndoEntry {
            op,
            before: before.clone(),
            after: after.clone(),
        });

        self.limit_undo_stack_size();
    }

    /// 执行撤销操作
    pub fn undo(&mut self) {
        if let Some(entry) = self.undo_stack.pop() {
            let current = self.dbc_data.clone();
            self.apply_undo_operation(&entry.op);
            self.dbc_data = entry.before.clone();

            self.redo_stack.push(UndoEntry {
                op: entry.op,
                before: entry.before,
                after: current,
            });
        }
    }

    /// 执行重做操作
    pub fn redo(&mut self) {
        if let Some(entry) = self.redo_stack.pop() {
            let current = self.dbc_data.clone();
            self.apply_redo_operation(&entry.op);
            self.dbc_data = entry.after.clone();

            self.undo_stack.push(UndoEntry {
                op: entry.op,
                before: current,
                after: entry.after,
            });
        }
    }

    /// 应用撤销时的名称覆盖逻辑
    fn apply_undo_operation(&mut self, op: &UndoOperationKind) {
        if let UndoOperationKind::RenameMessage {
            message_id,
            old_name,
            ..
        } = op
        {
            if old_name.is_empty() {
                self.message_name_overrides.remove(message_id);
            } else {
                self.message_name_overrides
                    .insert(*message_id, old_name.clone());
            }
        }
    }

    /// 应用重做时的名称覆盖逻辑
    fn apply_redo_operation(&mut self, op: &UndoOperationKind) {
        if let UndoOperationKind::RenameMessage {
            message_id,
            new_name,
            ..
        } = op
        {
            if new_name.is_empty() {
                self.message_name_overrides.remove(message_id);
            } else {
                self.message_name_overrides
                    .insert(*message_id, new_name.clone());
            }
        }
    }

    /// 限制 undo 栈的大小
    fn limit_undo_stack_size(&mut self) {
        if self.undo_stack.len() > Self::MAX_UNDO_ENTRIES {
            let overflow = self.undo_stack.len() - Self::MAX_UNDO_ENTRIES;
            self.undo_stack.drain(0..overflow);
        }
    }

    /// 检查是否可以撤销
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// 检查是否可以重做
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

fn describe_undo(entry: &UndoEntry) -> &'static str {
    match entry.op {
        UndoOperationKind::RenameMessage { .. } => "Rename Message",
        UndoOperationKind::AddMessage { .. } => "Add Message",
        UndoOperationKind::RemoveMessage { .. } => "Remove Message",
        UndoOperationKind::ModifyMessage { .. } => "Modify Message",
        UndoOperationKind::AddSignal { .. } => "Add Signal",
        UndoOperationKind::RemoveSignal { .. } => "Remove Signal",
        UndoOperationKind::ModifySignal { .. } => "Modify Signal",
    }
}

fn describe_redo(entry: &UndoEntry) -> &'static str {
    describe_undo(entry)
}

/// 错误对话框状态
pub struct ErrorDialog {
    pub show: bool,
    pub message: String,
}

/// Message 编辑对话框状态
pub struct MessageEditDialog {
    pub show: bool,
    pub parent_dbc_id: usize,
    pub message_id: u32,

    // 字段缓冲区
    pub name_buffer: String,
    pub comment_buffer: String,

    // 原始值（用于检测变化）
    pub original_name: String,
    pub original_comment: String,

    // 跟踪哪个字段刚刚失去焦点
    pub name_had_focus: bool,
    pub comment_had_focus: bool,
}

impl MessageEditDialog {
    pub fn new() -> Self {
        Self {
            show: false,
            parent_dbc_id: 0,
            message_id: 0,
            name_buffer: String::new(),
            comment_buffer: String::new(),
            original_name: String::new(),
            original_comment: String::new(),
            name_had_focus: false,
            comment_had_focus: false,
        }
    }

    pub fn open(&mut self, parent_dbc_id: usize, message: &crate::dbc::Message) {
        self.show = true;
        self.parent_dbc_id = parent_dbc_id;
        self.message_id = message.message_id().raw();

        // 初始化缓冲区
        self.name_buffer = message.message_name().to_string();
        self.original_name = message.message_name().to_string();

        // TODO: 从 DBC 读取 comment
        self.comment_buffer = String::new();
        self.original_comment = String::new();

        // 重置焦点跟踪
        self.name_had_focus = false;
        self.comment_had_focus = false;
    }

    pub fn close(&mut self) {
        self.show = false;
    }
}

/// Signal详细窗口状态
#[derive(Clone)]
pub struct SignalWindowState {
    pub id: usize,
    pub message: crate::dbc::Message,
    pub is_open: bool,
    pub parent_dbc_id: usize, // 对应的父DBC窗口id
}

/// UI 状态管理
pub struct UiState {
    pub show_performance_window: bool,
    pub show_about_dialog: bool,
    pub dbc_windows: Vec<DbcWindowState>,
    pub signal_windows: Vec<SignalWindowState>,
    pub next_dbc_id: usize,
    pub error_dialog: ErrorDialog,
    pub message_edit_dialog: MessageEditDialog,
    pub last_focused_dbc_index: Option<usize>, // 最近聚焦的DBC窗口索引
    pub dbc_window_focus_request: Option<usize>, // 需要聚焦的DBC窗口id
    pub signal_window_focus_request: Option<usize>, // 需要聚焦的signal窗口id
    pub last_focused_signal_window: Option<usize>, // 最近聚焦的Signal窗口id（index in signal_windows vec）
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            show_performance_window: false,
            show_about_dialog: false,
            dbc_windows: Vec::new(),
            signal_windows: Vec::new(),
            next_dbc_id: 1,
            error_dialog: ErrorDialog {
                show: false,
                message: String::new(),
            },
            message_edit_dialog: MessageEditDialog::new(),
            last_focused_dbc_index: None,
            dbc_window_focus_request: None,
            signal_window_focus_request: None,
            last_focused_signal_window: None,
        }
    }
}

impl UiState {
    /// 检查指定 message 是否有对应的 Signal 窗口打开。
    /// 若存在打开的信号窗口则弹出错误对话框并返回 Err。
    pub fn ensure_message_not_in_open_signal_windows(&mut self, message_id: u32) -> Result<(), ()> {
        if let Some(sw) = self
            .signal_windows
            .iter()
            .find(|w| w.message.message_id().raw() == message_id)
        {
            self.error_dialog.message = format!(
                "无法修改或删除消息: '{}' (0x{:03X})，其 Signal 窗口仍然打开。请先关闭对应的 Signal 窗口。",
                sw.message.message_name(),
                sw.message.message_id().raw()
            );
            self.error_dialog.show = true;
            return Err(());
        }
        Ok(())
    }
}

/// 设置主dockspace占满整个窗口
fn setup_main_dockspace(ui: &Ui, ui_state: &mut UiState) {
    // 主菜单栏
    render_main_menu_bar(ui, ui_state);

    // 使用dockspace_over_main_viewport API创建全屏dockspace
    ui.dockspace_over_main_viewport();
}

/// 渲染主界面  
/// 主 UI 渲染函数
pub fn render_ui(ui: &Ui, delta_s: Duration, target_frame_time: Duration, ui_state: &mut UiState) {
    setup_main_dockspace(ui, ui_state);

    if ui_state.show_performance_window {
        render_performance_window(ui, delta_s, target_frame_time);
    }

    render_dbc_windows(ui, ui_state);
    handle_global_shortcuts(ui, ui_state);
    render_signal_windows(ui, ui_state);
    render_dialogs(ui, ui_state);
}

/// 渲染所有 DBC 窗口并处理相关逻辑
fn render_dbc_windows(ui: &Ui, ui_state: &mut UiState) {
    let mut windows_to_remove = Vec::new();
    let mut clicked_messages = Vec::new(); // 收集双击事件，稍后处理
    let mut edit_requests = Vec::new(); // 收集编辑请求，稍后处理

    for (index, window) in ui_state.dbc_windows.iter_mut().enumerate() {
        request_window_focus_if_needed(ui_state.dbc_window_focus_request, window.id);

        if !window.is_open {
            continue;
        }

        let (still_open, double_clicked, edit_requested, focused) = render_dbc_window(ui, window);

        if !still_open {
            windows_to_remove.push(index);
        }

        if focused {
            ui_state.last_focused_dbc_index = Some(index);
            ui_state.last_focused_signal_window = None;
        }

        if let Some(message) = double_clicked {
            clicked_messages.push((message, window.id));
        }

        if let Some(message) = edit_requested {
            edit_requests.push((message, window.id));
        }
    }

    // 处理所有双击事件
    for (message, parent_dbc_id) in clicked_messages {
        handle_message_double_click(ui_state, &message, parent_dbc_id);
    }

    // 处理所有编辑请求
    for (message, parent_dbc_id) in edit_requests {
        ui_state.message_edit_dialog.open(parent_dbc_id, &message);
    }

    cleanup_closed_dbc_windows(ui_state, windows_to_remove);
}

/// 处理消息双击事件，打开或聚焦对应的 Signal 窗口
fn handle_message_double_click(
    ui_state: &mut UiState,
    message: &crate::dbc::Message,
    parent_dbc_id: usize,
) {
    let msg_id = message.message_id().raw();

    if let Some(existing_idx) = ui_state
        .signal_windows
        .iter()
        .position(|w| w.message.message_id().raw() == msg_id && w.parent_dbc_id == parent_dbc_id)
    {
        // 窗口已存在，请求聚焦
        let existing_id = ui_state.signal_windows[existing_idx].id;
        ui_state.signal_window_focus_request = Some(existing_id);
    } else {
        // 创建新窗口
        let new_id = ui_state.signal_windows.len();
        ui_state.signal_windows.push(SignalWindowState {
            id: new_id,
            message: message.clone(),
            is_open: true,
            parent_dbc_id,
        });
        ui_state.signal_window_focus_request = Some(new_id);
    }
}

/// 清理已关闭的 DBC 窗口及其关联的 Signal 窗口
fn cleanup_closed_dbc_windows(ui_state: &mut UiState, windows_to_remove: Vec<usize>) {
    if windows_to_remove.is_empty() {
        return;
    }

    // 收集被关闭窗口的 ID
    let closed_parent_ids: std::collections::HashSet<_> = windows_to_remove
        .iter()
        .filter_map(|&idx| ui_state.dbc_windows.get(idx).map(|w| w.id))
        .collect();

    // 移除关联的 Signal 窗口
    if !closed_parent_ids.is_empty() {
        ui_state
            .signal_windows
            .retain(|sw| !closed_parent_ids.contains(&sw.parent_dbc_id));

        // 修正焦点索引
        if let Some(sig_idx) = ui_state.last_focused_signal_window {
            if sig_idx >= ui_state.signal_windows.len() {
                ui_state.last_focused_signal_window = None;
            }
        }
    }

    // 逆序移除 DBC 窗口
    for &idx in windows_to_remove.iter().rev() {
        ui_state.dbc_windows.remove(idx);
    }

    // 修正 DBC 焦点索引
    if let Some(focused_idx) = ui_state.last_focused_dbc_index {
        if focused_idx >= ui_state.dbc_windows.len() {
            ui_state.last_focused_dbc_index = None;
        }
    }
}

/// 渲染所有 Signal 窗口
fn render_signal_windows(ui: &Ui, ui_state: &mut UiState) {
    let mut windows_to_remove = Vec::new();

    for (index, window) in ui_state.signal_windows.iter_mut().enumerate() {
        request_window_focus_if_needed(ui_state.signal_window_focus_request, window.id);

        if !window.is_open {
            continue;
        }

        let (still_open, focused) = render_signal_window(ui, window);

        if focused {
            ui_state.last_focused_signal_window = Some(index);
            ui_state.last_focused_dbc_index = None;
        }

        if !still_open {
            windows_to_remove.push(index);
        }
    }

    // 清空聚焦请求
    ui_state.dbc_window_focus_request = None;
    ui_state.signal_window_focus_request = None;

    cleanup_closed_signal_windows(ui_state, windows_to_remove);
}

/// 清理已关闭的 Signal 窗口
fn cleanup_closed_signal_windows(ui_state: &mut UiState, windows_to_remove: Vec<usize>) {
    for &index in windows_to_remove.iter().rev() {
        ui_state.signal_windows.remove(index);

        // 修正焦点索引
        if let Some(focused_sig) = ui_state.last_focused_signal_window {
            if focused_sig == index {
                ui_state.last_focused_signal_window = None;
            } else if focused_sig > index {
                ui_state.last_focused_signal_window = Some(focused_sig - 1);
            }
        }
    }
}

/// 请求窗口聚焦（如果需要）
fn request_window_focus_if_needed(focus_request: Option<usize>, window_id: usize) {
    if focus_request == Some(window_id) {
        unsafe {
            imgui::sys::igSetNextWindowFocus();
        }
    }
}

/// 渲染所有对话框
fn render_dialogs(ui: &Ui, ui_state: &mut UiState) {
    if ui_state.error_dialog.show {
        render_error_dialog(ui, &mut ui_state.error_dialog);
    }

    if ui_state.show_about_dialog {
        render_about_dialog(ui, &mut ui_state.show_about_dialog);
    }

    if ui_state.message_edit_dialog.show {
        render_message_edit_dialog(ui, ui_state);
    }
}

/// 渲染主菜单栏
fn render_main_menu_bar(ui: &Ui, ui_state: &mut UiState) {
    ui.main_menu_bar(|| {
        render_context_label(ui, ui_state);
        ui.same_line();

        render_file_menu(ui, ui_state);
        render_edit_menu(ui, ui_state);
        render_view_menu(ui, ui_state);
        render_help_menu(ui, ui_state);
    });
}

/// 渲染菜单栏左侧的上下文标签
fn render_context_label(ui: &Ui, ui_state: &UiState) {
    if let Some(label) = get_context_label(ui_state) {
        ui.text(label);
    } else {
        ui.text("(No file)");
    }
}

/// 获取当前上下文标签（显示当前聚焦的文件/消息）
fn get_context_label(ui_state: &UiState) -> Option<String> {
    // 优先显示 Signal 窗口上下文
    if let Some(sig_idx) = ui_state.last_focused_signal_window {
        if let Some(sw) = ui_state.signal_windows.get(sig_idx) {
            if let Some(parent) = ui_state
                .dbc_windows
                .iter()
                .find(|w| w.id == sw.parent_dbc_id)
            {
                let file_part = format_file_label(&parent.dbc_data.file_path, "(message)");
                return Some(format!("{}/{}", file_part, sw.message.message_name()));
            }
        }
    }

    // 否则显示 DBC 窗口上下文
    if let Some(idx) = ui_state.last_focused_dbc_index {
        if let Some(win) = ui_state.dbc_windows.get(idx) {
            return Some(format_file_label(&win.dbc_data.file_path, "(dbc)"));
        }
    }

    None
}

/// 格式化文件标签
fn format_file_label(file_path: &str, prefix: &str) -> String {
    if file_path.is_empty() {
        "(No file)".to_string()
    } else {
        let path = std::path::Path::new(file_path);
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| format!("{} {}", prefix, s))
            .unwrap_or_else(|| format!("{} Unknown", prefix))
    }
}

/// 渲染文件菜单
fn render_file_menu(ui: &Ui, ui_state: &mut UiState) {
    ui.menu("File", || {
        if ui.menu_item("Load DBC File") {
            handle_load_dbc_file(ui_state);
        }
        ui.separator();
        if ui.menu_item("Exit") {
            std::process::exit(0);
        }
    });
}

/// 处理加载 DBC 文件
fn handle_load_dbc_file(ui_state: &mut UiState) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("DBC files", &["dbc"])
        .pick_file()
    else {
        return;
    };

    let path_str = path.to_string_lossy().to_string();

    // 检查文件是否已经打开
    if let Some(existing_idx) = ui_state
        .dbc_windows
        .iter()
        .position(|w| w.dbc_data.file_path == path_str)
    {
        focus_existing_dbc_window(ui_state, existing_idx);
    } else {
        load_new_dbc_file(ui_state, &path);
    }
}

/// 聚焦已存在的 DBC 窗口
fn focus_existing_dbc_window(ui_state: &mut UiState, window_index: usize) {
    if let Some(window) = ui_state.dbc_windows.get_mut(window_index) {
        window.is_open = true;
        ui_state.dbc_window_focus_request = Some(window.id);
    }
    ui_state.last_focused_dbc_index = Some(window_index);
    ui_state.last_focused_signal_window = None;
}

/// 加载新的 DBC 文件
fn load_new_dbc_file(ui_state: &mut UiState, path: &std::path::Path) {
    let mut dbc_data = DbcData::new();
    match dbc_data.load_dbc_file(path) {
        Ok(_) => {
            ui_state.dbc_windows.push(DbcWindowState {
                id: ui_state.next_dbc_id,
                is_open: true,
                dbc_data,
                search_query: String::new(),
                selected_message_id: None,
                undo_stack: Vec::new(),
                redo_stack: Vec::new(),
                message_name_overrides: HashMap::new(),
            });
            ui_state.next_dbc_id += 1;
        }
        Err(e) => {
            ui_state.error_dialog.message = format!("Failed to load DBC file: {}", e);
            ui_state.error_dialog.show = true;
        }
    }
}

/// 渲染编辑菜单
fn render_edit_menu(ui: &Ui, ui_state: &mut UiState) {
    ui.menu("Edit", || {
        if let Some(idx) = ui_state.last_focused_dbc_index {
            if let Some(win) = ui_state.dbc_windows.get_mut(idx) {
                render_undo_redo_menu_items(ui, win);
            } else {
                ui.text_disabled("No active DBC window");
            }
        } else {
            ui.text_disabled("No active DBC window");
        }
    });
}

/// 渲染撤销/重做菜单项
fn render_undo_redo_menu_items(ui: &Ui, window: &mut DbcWindowState) {
    let undo_label = if let Some(last) = window.undo_stack.last() {
        format!("Undo {}\tCtrl+Z", describe_undo(last))
    } else {
        "Undo\tCtrl+Z".to_string()
    };

    let redo_label = if let Some(last) = window.redo_stack.last() {
        format!("Redo {}\tCtrl+Y", describe_redo(last))
    } else {
        "Redo\tCtrl+Y".to_string()
    };

    if ui
        .menu_item_config(&undo_label)
        .enabled(!window.undo_stack.is_empty())
        .build()
    {
        window.undo();
    }

    if ui
        .menu_item_config(&redo_label)
        .enabled(!window.redo_stack.is_empty())
        .build()
    {
        window.redo();
    }
}

/// 渲染视图菜单
fn render_view_menu(ui: &Ui, ui_state: &mut UiState) {
    ui.menu("View", || {
        ui.checkbox("Performance Window", &mut ui_state.show_performance_window);
    });
}

/// 渲染帮助菜单
fn render_help_menu(ui: &Ui, ui_state: &mut UiState) {
    ui.menu("Help", || {
        if ui.menu_item("About") {
            ui_state.show_about_dialog = true;
        }
    });
}

/// 渲染性能信息窗口
fn render_performance_window(ui: &Ui, delta_s: Duration, target_frame_time: Duration) {
    let window = ui.window("Performance Information");
    window
        .size([300.0, 150.0], Condition::FirstUseEver)
        .position([400.0, 50.0], Condition::FirstUseEver)
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

/// 渲染单个 DBC 浏览器窗口
fn render_dbc_window(
    ui: &Ui,
    window_state: &mut DbcWindowState,
) -> (
    bool,
    Option<crate::dbc::Message>,
    Option<crate::dbc::Message>,
    bool,
) {
    let window_title = format!(
        "DBC Browser {} - {}",
        window_state.id,
        if window_state.dbc_data.file_path.is_empty() {
            "No file"
        } else {
            // 只显示文件名，不显示完整路径
            std::path::Path::new(&window_state.dbc_data.file_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Unknown file")
        }
    );

    let mut is_open = window_state.is_open;
    let mut double_clicked_message = None;
    let mut edit_requested_message = None;
    let mut focused = false;

    if is_open {
        let window = ui
            .window(&window_title)
            .opened(&mut is_open)
            .size([800.0, 600.0], Condition::FirstUseEver)
            .position(
                [
                    50.0 + (window_state.id as f32 * 30.0),
                    50.0 + (window_state.id as f32 * 30.0),
                ],
                Condition::FirstUseEver,
            );

        window.build(|| {
            let (double_click, edit_request) = render_dbc_window_content(ui, window_state);
            double_clicked_message = double_click;
            edit_requested_message = edit_request;
            if ui.is_window_focused_with_flags(WindowFocusedFlags::ROOT_AND_CHILD_WINDOWS) {
                focused = true;
            }
        });
    }

    if !is_open {
        window_state.is_open = false;
    }

    (
        window_state.is_open,
        double_clicked_message,
        edit_requested_message,
        focused,
    )
}

/// 处理全局快捷键（依赖 last_focused_dbc_index）
fn handle_global_shortcuts(ui: &Ui, ui_state: &mut UiState) {
    let io = ui.io();
    if !io.key_ctrl {
        return;
    }
    let Some(idx) = ui_state.last_focused_dbc_index else {
        return;
    };
    if let Some(win) = ui_state.dbc_windows.get_mut(idx) {
        let shift = io.key_shift;
        // 优先 Undo
        if ui.is_key_pressed(Key::Z) && !shift {
            win.undo();
            return;
        }
        // Redo: Ctrl+Shift+Z 或 Ctrl+Y
        if (ui.is_key_pressed(Key::Z) && shift) || ui.is_key_pressed(Key::Y) {
            win.redo();
        }
    }
}

/// 渲染单个 Signal 详细窗口
fn render_signal_window(ui: &Ui, window_state: &mut SignalWindowState) -> (bool, bool) {
    let window_title = format!(
        "Signals - {} (0x{:03X})",
        window_state.message.message_name(),
        window_state.message.message_id().raw()
    );

    let mut is_open = window_state.is_open;
    let mut focused = false;

    if is_open {
        let window = ui
            .window(&window_title)
            .opened(&mut is_open)
            .size([800.0, 600.0], Condition::FirstUseEver)
            .position(
                [
                    100.0 + (window_state.id as f32 * 30.0),
                    100.0 + (window_state.id as f32 * 30.0),
                ],
                Condition::FirstUseEver,
            );

        window.build(|| {
            render_signal_window_content(ui, &window_state.message);
            if ui.is_window_focused_with_flags(WindowFocusedFlags::ROOT_AND_CHILD_WINDOWS) {
                focused = true;
            }
        });
    }

    if !is_open {
        window_state.is_open = false;
    }

    (window_state.is_open, focused)
}

/// 渲染Signal窗口的内容
fn render_signal_window_content(ui: &Ui, message: &crate::dbc::Message) {
    ui.text(format!(
        "Message: {} (0x{:03X}) - {} signals",
        message.message_name(),
        message.message_id().raw(),
        message.signals().len()
    ));
    ui.separator();

    // 创建完整的信号表格
    if let Some(_table) = ui.begin_table_with_flags(
        "full_signals_table",
        10,
        TableFlags::RESIZABLE
            | TableFlags::REORDERABLE
            | TableFlags::HIDEABLE
            | TableFlags::BORDERS
            | TableFlags::SIZING_FIXED_FIT
            | TableFlags::SCROLL_Y
            | TableFlags::SORTABLE,
    ) {
        // 设置完整的表格列
        let columns = [
            (
                "Signal",
                TableColumnFlags::DEFAULT_SORT | TableColumnFlags::WIDTH_STRETCH,
                0.0,
            ),
            ("Type", TableColumnFlags::default(), 0.0),
            ("Order", TableColumnFlags::default(), 0.0),
            ("Start", TableColumnFlags::default(), 0.0),
            ("Length", TableColumnFlags::default(), 0.0),
            ("Factor", TableColumnFlags::default(), 0.0),
            ("Offset", TableColumnFlags::default(), 0.0),
            ("Min", TableColumnFlags::default(), 0.0),
            ("Max", TableColumnFlags::default(), 0.0),
            ("Unit", TableColumnFlags::default(), 0.0),
        ];

        for (name, flags, width) in &columns {
            ui.table_setup_column_with(TableColumnSetup {
                name,
                flags: *flags,
                init_width_or_weight: *width,
                user_id: ui.new_id_str(&format!("full_{}", name.to_lowercase())),
            });
        }

        ui.table_headers_row();

        // 显示所有信号
        for signal in message.signals().iter() {
            ui.table_next_row();

            ui.table_set_column_index(0);
            ui.text(signal.name());

            ui.table_set_column_index(1);
            let data_type = match signal.value_type() {
                can_dbc::ValueType::Signed => "signed",
                can_dbc::ValueType::Unsigned => "unsigned",
            };
            ui.text(data_type);

            ui.table_set_column_index(2);
            let byte_order = match signal.byte_order() {
                can_dbc::ByteOrder::LittleEndian => "Intel",
                can_dbc::ByteOrder::BigEndian => "Motorola",
            };
            ui.text(byte_order);

            ui.table_set_column_index(3);
            ui.text(format!("{}", signal.start_bit()));

            ui.table_set_column_index(4);
            ui.text(format!("{}", signal.signal_size()));

            ui.table_set_column_index(5);
            ui.text(format!("{}", signal.factor()));

            ui.table_set_column_index(6);
            ui.text(format!("{}", signal.offset()));

            ui.table_set_column_index(7);
            ui.text(format!("{}", signal.min()));

            ui.table_set_column_index(8);
            ui.text(format!("{}", signal.max()));

            ui.table_set_column_index(9);
            ui.text(signal.unit());
        }
    }
}

/// 渲染DBC窗口的内容
fn render_dbc_window_content(
    ui: &Ui,
    window_state: &mut DbcWindowState,
) -> (Option<crate::dbc::Message>, Option<crate::dbc::Message>) {
    // 文件信息区域
    render_dbc_file_info(ui, window_state);

    // 搜索栏
    render_dbc_search_bar(ui, window_state);

    // 消息列表表格
    render_messages_table(ui, window_state)
}

/// 渲染DBC文件信息区域
fn render_dbc_file_info(ui: &Ui, window_state: &mut DbcWindowState) {
    // 显示当前加载的文件
    if !window_state.dbc_data.file_path.is_empty() {
        ui.text(format!("Loaded: {}", window_state.dbc_data.file_path));
        ui.text(format!(
            "Messages: {}",
            window_state.dbc_data.message_count()
        ));
    } else {
        ui.text("No DBC file loaded");
    }

    // 显示错误信息
    if !window_state.dbc_data.error_message.is_empty() {
        ui.text_colored([1.0, 0.0, 0.0, 1.0], &window_state.dbc_data.error_message);
    }

    ui.separator();
}

/// 渲染搜索栏
fn render_dbc_search_bar(ui: &Ui, window_state: &mut DbcWindowState) {
    ui.text("Search Messages & Signals:");
    ui.input_text("##search", &mut window_state.search_query)
        .build();
    ui.separator();
}

/// 渲染消息列表表格
fn render_messages_table(
    ui: &Ui,
    window_state: &mut DbcWindowState,
) -> (Option<crate::dbc::Message>, Option<crate::dbc::Message>) {
    ui.child_window("messages_list")
        .size([0.0, 0.0])
        .build(|| {
            let filtered_messages = window_state
                .dbc_data
                .search_messages(&window_state.search_query);

            if let Some(_table) = ui.begin_table_with_flags(
                "messages_table",
                4,
                TableFlags::RESIZABLE
                    | TableFlags::REORDERABLE
                    | TableFlags::SIZING_FIXED_FIT
                    | TableFlags::BORDERS
                    | TableFlags::SCROLL_Y
                    | TableFlags::SORTABLE,
            ) {
                setup_messages_table_columns(ui);
                ui.table_headers_row();

                let sorted_messages = sort_messages(ui, filtered_messages);

                return render_messages_rows(
                    ui,
                    &mut window_state.selected_message_id,
                    sorted_messages,
                    &window_state.message_name_overrides,
                    window_state.id,
                );
            }
            (None, None)
        })
        .unwrap_or((None, None))
}

/// 设置消息表格的列
fn setup_messages_table_columns(ui: &Ui) {
    ui.table_setup_column_with(TableColumnSetup {
        name: "ID",
        flags: TableColumnFlags::DEFAULT_SORT,
        init_width_or_weight: 80.0,
        user_id: ui.new_id_str("id_col"),
    });
    ui.table_setup_column_with(TableColumnSetup {
        name: "Name",
        flags: TableColumnFlags::WIDTH_STRETCH,
        init_width_or_weight: 0.0,
        user_id: ui.new_id_str("name_col"),
    });
    ui.table_setup_column_with(TableColumnSetup {
        name: "Length",
        flags: TableColumnFlags::default(),
        init_width_or_weight: 60.0,
        user_id: ui.new_id_str("len_col"),
    });
    ui.table_setup_column_with(TableColumnSetup {
        name: "Signals",
        flags: TableColumnFlags::default(),
        init_width_or_weight: 60.0,
        user_id: ui.new_id_str("sig_col"),
    });
}

/// 渲染消息表格的行，返回双击的消息（如果有的话）
fn render_messages_rows(
    ui: &Ui,
    current_selection: &mut Option<u32>,
    messages: Vec<&crate::dbc::Message>,
    name_overrides: &std::collections::HashMap<u32, String>,
    _parent_dbc_id: usize,
) -> (Option<crate::dbc::Message>, Option<crate::dbc::Message>) {
    let mut double_clicked_message = None;
    let mut edit_requested_message = None;

    for message in messages {
        ui.table_next_row();

        let selected = *current_selection == Some(message.message_id().raw());

        if selected {
            ui.table_set_bg_color(imgui::TableBgTarget::ROW_BG0, [0.3, 0.3, 0.7, 0.65]);
        }

        ui.table_set_column_index(0);
        if ui
            .selectable_config(format!("0x{:03X}", message.message_id().raw()))
            .selected(selected)
            .span_all_columns(true)
            .build()
        {
            *current_selection = Some(message.message_id().raw());
        }

        // 检测鼠标悬停在这一行上
        if ui.is_item_hovered() {
            render_signal_popup(ui, message);

            // 检测双击事件
            if ui.is_mouse_double_clicked(imgui::MouseButton::Left) {
                double_clicked_message = Some(message.clone());
            }
        }

        // 右键菜单
        let popup_id = format!("message_context_menu_{}", message.message_id().raw());
        if ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
            ui.open_popup(&popup_id);
        }

        ui.popup(&popup_id, || {
            ui.text(format!(
                "Message: {} (0x{:03X})",
                message.message_name(),
                message.message_id().raw()
            ));
            ui.separator();

            if ui.menu_item("Edit...") {
                edit_requested_message = Some(message.clone());
            }

            // 未来可以添加更多选项
            // if ui.menu_item("Delete") { ... }
            // if ui.menu_item("Duplicate") { ... }
        });

        ui.table_set_column_index(1);
        let shown_name = name_overrides
            .get(&message.message_id().raw())
            .map(|s| s.as_str())
            .unwrap_or(message.message_name());
        ui.text(shown_name);

        ui.table_set_column_index(2);
        ui.text(format!("{}", message.message_size()));

        ui.table_set_column_index(3);
        ui.text(format!("{}", message.signals().len()));
    }

    (double_clicked_message, edit_requested_message)
}

/// 渲染信号详情弹出窗口
fn render_signal_popup(ui: &Ui, message: &crate::dbc::Message) {
    ui.tooltip(|| {
        ui.text(format!(
            "Message: {} (0x{:03X})",
            message.message_name(),
            message.message_id().raw()
        ));
        ui.separator();

        if message.signals().is_empty() {
            ui.text("No signals in this message");
            return;
        }

        // 创建一个简化的信号表格
        if let Some(_table) = ui.begin_table_with_flags(
            "popup_signals_table",
            5, // 减少列数，只显示关键信息
            TableFlags::BORDERS | TableFlags::SIZING_FIXED_FIT,
        ) {
            // 设置表格列
            ui.table_setup_column("Signal");
            ui.table_setup_column("Start");
            ui.table_setup_column("Length");
            ui.table_setup_column("Factor");
            ui.table_setup_column("Unit");
            ui.table_headers_row();

            // 显示前几个信号（避免popup过大）
            for signal in message.signals().iter().take(10) {
                ui.table_next_row();

                ui.table_set_column_index(0);
                ui.text(signal.name());

                ui.table_set_column_index(1);
                ui.text(format!("{}", signal.start_bit()));

                ui.table_set_column_index(2);
                ui.text(format!("{}", signal.signal_size()));

                ui.table_set_column_index(3);
                ui.text(format!("{}", signal.factor()));

                ui.table_set_column_index(4);
                ui.text(signal.unit());
            }

            if message.signals().len() > 10 {
                ui.table_next_row();
                ui.table_set_column_index(0);
                ui.text(format!(
                    "... and {} more signals",
                    message.signals().len() - 10
                ));
            }
        }
    });
}

/// 排序消息列表
fn sort_messages<'a>(
    ui: &Ui,
    mut messages: Vec<&'a crate::dbc::Message>,
) -> Vec<&'a crate::dbc::Message> {
    if let Some(sort_specs) = ui.table_sort_specs_mut() {
        let specs = sort_specs.specs();
        for (i, spec) in specs.iter().enumerate() {
            if i == 0 {
                let ascending = spec.sort_direction() == Some(TableSortDirection::Ascending);
                messages.sort_by(|a, b| {
                    let ordering = match spec.column_idx() {
                        0 => a.message_id().raw().cmp(&b.message_id().raw()),
                        1 => a.message_name().cmp(b.message_name()),
                        2 => a.message_size().cmp(b.message_size()),
                        3 => a.signals().len().cmp(&b.signals().len()),
                        _ => std::cmp::Ordering::Equal,
                    };
                    if ascending {
                        ordering
                    } else {
                        ordering.reverse()
                    }
                });
                break;
            }
        }
    }
    messages
}

/// 渲染错误对话框
fn render_error_dialog(ui: &Ui, error_dialog: &mut ErrorDialog) {
    if error_dialog.show {
        ui.open_popup("Error");
    }

    ui.modal_popup_config("Error").resizable(false).build(|| {
        ui.text("Error");
        ui.separator();

        // 使用固定大小的滚动区域显示错误消息
        ui.child_window("error_message")
            .size([400.0, 100.0])
            .build(|| {
                ui.text_wrapped(&error_dialog.message);
            });

        ui.separator();

        if ui.button("OK") {
            ui.close_current_popup();
            error_dialog.show = false;
        }
    });
}

/// 渲染关于对话框
fn render_about_dialog(ui: &Ui, show_about: &mut bool) {
    if *show_about {
        ui.open_popup("About");
    }

    ui.modal_popup_config("About").resizable(false).build(|| {
        ui.text("Roxy dbc viewer");
        ui.separator();
        ui.text("Version: 0.4.0");
        ui.text("Built with Rust and ImGui");
        ui.separator();
        ui.text("An application for viewing CAN DBC files.");

        ui.separator();
        if ui.button("Close") {
            ui.close_current_popup();
            *show_about = false;
        }
    });
}

/// 渲染 Message 编辑对话框
fn render_message_edit_dialog(ui: &Ui, ui_state: &mut UiState) {
    if !ui_state.message_edit_dialog.show {
        return;
    }

    // 在渲染前记录焦点状态
    let prev_name_had_focus = ui_state.message_edit_dialog.name_had_focus;
    let prev_comment_had_focus = ui_state.message_edit_dialog.comment_had_focus;

    // 用于记录当前焦点状态和窗口是否关闭
    let mut name_has_focus = false;
    let mut comment_has_focus = false;
    let mut should_close = false;

    // 提取需要的值以避免借用冲突
    let message_id = ui_state.message_edit_dialog.message_id;

    ui.window("Edit Message")
        .size([500.0, 400.0], imgui::Condition::FirstUseEver)
        .position([100.0, 100.0], imgui::Condition::FirstUseEver)
        .opened(&mut ui_state.message_edit_dialog.show)
        .build(|| {
            ui.text(format!("Editing Message 0x{:03X}", message_id));
            ui.separator();

            // Message Name
            ui.text("Message Name:");
            ui.set_next_item_width(-1.0);
            ui.input_text(
                "##message_name",
                &mut ui_state.message_edit_dialog.name_buffer,
            )
            .build();

            name_has_focus = ui.is_item_active();

            ui.spacing();
            ui.spacing();

            // Comment
            ui.text("Comment:");
            ui.set_next_item_width(-1.0);
            ui.input_text_multiline(
                "##message_comment",
                &mut ui_state.message_edit_dialog.comment_buffer,
                [0.0, 100.0],
            )
            .build();

            comment_has_focus = ui.is_item_active();

            ui.spacing();
            ui.separator();

            // 提示信息
            ui.text_colored([0.6, 0.6, 0.6, 1.0], "Changes are saved automatically");
            ui.text_colored([0.6, 0.6, 0.6, 1.0], "Use Ctrl+Z/Ctrl+Y to undo/redo");

            ui.spacing();

            // Close 按钮
            if ui.button("Close") {
                should_close = true;
            }
        });

    // 处理窗口关闭
    if should_close {
        ui_state.message_edit_dialog.show = false;
    }

    // 渲染后处理焦点变化
    ui_state.message_edit_dialog.name_had_focus = name_has_focus;
    ui_state.message_edit_dialog.comment_had_focus = comment_has_focus;

    // 检测 Name 字段失去焦点
    if prev_name_had_focus && !name_has_focus {
        handle_name_change(ui_state);
    }

    // 检测 Comment 字段失去焦点
    if prev_comment_had_focus && !comment_has_focus {
        handle_comment_change(ui_state);
    }
}

/// 处理 Message Name 变化（在失去焦点时）
fn handle_name_change(ui_state: &mut UiState) {
    let dialog = &ui_state.message_edit_dialog;
    let parent_dbc_id = dialog.parent_dbc_id;
    let message_id = dialog.message_id;
    let new_name = dialog.name_buffer.trim();
    let old_name = &dialog.original_name;

    // 检查是否有实际变化
    if new_name == old_name || new_name.is_empty() {
        return;
    }

    // 查找对应的 DBC 窗口
    if let Some(dbc_window) = ui_state
        .dbc_windows
        .iter_mut()
        .find(|w| w.id == parent_dbc_id)
    {
        // 记录 undo（在修改之前）
        let before = dbc_window.dbc_data.clone();

        // 更新名称覆盖映射
        dbc_window
            .message_name_overrides
            .insert(message_id, new_name.to_string());

        let after = dbc_window.dbc_data.clone();

        dbc_window.push_undo(
            UndoOperationKind::RenameMessage {
                message_id,
                old_name: old_name.clone(),
                new_name: new_name.to_string(),
            },
            &before,
            &after,
        );

        // 更新 original_name，以便下次比较
        ui_state.message_edit_dialog.original_name = new_name.to_string();
    }
}

/// 处理 Message Comment 变化（在失去焦点时）
fn handle_comment_change(ui_state: &mut UiState) {
    let dialog = &ui_state.message_edit_dialog;
    let parent_dbc_id = dialog.parent_dbc_id;
    let message_id = dialog.message_id;
    let new_comment = dialog.comment_buffer.trim();
    let old_comment = &dialog.original_comment;

    // 检查是否有实际变化
    if new_comment == old_comment {
        return;
    }

    // 查找对应的 DBC 窗口
    if let Some(dbc_window) = ui_state
        .dbc_windows
        .iter_mut()
        .find(|w| w.id == parent_dbc_id)
    {
        // 记录 undo（在修改之前）
        let before = dbc_window.dbc_data.clone();

        // TODO: 实现 message_comment_overrides 映射
        // dbc_window.message_comment_overrides.insert(message_id, new_comment.to_string());

        let after = dbc_window.dbc_data.clone();

        dbc_window.push_undo(
            UndoOperationKind::ModifyMessage {
                message_id,
                field: "comment".to_string(),
                old_value: old_comment.clone(),
                new_value: new_comment.to_string(),
            },
            &before,
            &after,
        );

        // 更新 original_comment，以便下次比较
        ui_state.message_edit_dialog.original_comment = new_comment.to_string();
    }
}
