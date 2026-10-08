//! UI 状态管理模块

use crate::editable_dbc::{EditableMessage, EditableSignal, ValidationIssue};
use crate::ui::dbc_window::DbcWindow;

#[allow(dead_code)]
pub enum DeleteTarget {
    Messages(Vec<u32>),
    Signals(u32, Vec<String>),
}

#[allow(dead_code)]
#[derive(Default)]
pub struct ConfirmDeleteDialog {
    pub show: bool,
    pub target: Option<DeleteTarget>,
    pub display_name: String,
    pub was_open: bool,
}

#[derive(Default)]
pub struct CloseConfirmDialog {
    pub show: bool,
    pub dbc_window_index: Option<usize>,
}

#[allow(dead_code)]
/// 错误对话框状态
#[derive(Default)]
pub struct ErrorDialog {
    pub show: bool,
    pub message: String,
}

/// 完成提示对话框：标题说明是哪件事，正文给出结果（如导出到的路径）
#[derive(Default)]
pub struct NoticeDialog {
    pub show: bool,
    pub title: String,
    pub message: String,
}

#[derive(Default)]
pub struct ValidationDialog {
    pub show: bool,
    pub issues: Vec<ValidationIssue>,
}

/// 剪贴板状态（用于复制/粘贴）
#[derive(Default)]
pub struct ClipboardState {
    pub copied_messages: Vec<EditableMessage>,
    pub copied_signals: Vec<EditableSignal>,
}

#[allow(dead_code)]
/// 当前获得键盘焦点的窗口类型，Edit 菜单据此把操作分发到 DBC 或 FIBEX 窗口
#[derive(Clone, Copy, PartialEq, Default)]
pub enum FocusTarget {
    #[default]
    None,
    Dbc,
    Fibex,
}

#[allow(dead_code)]
/// 主 UI 状态管理
pub struct UiState {
    pub show_performance_window: bool,
    pub show_about_dialog: bool,
    pub dbc_windows: Vec<DbcWindow>,
    pub error_dialog: ErrorDialog,
    pub notice_dialog: NoticeDialog,
    pub validation_dialog: ValidationDialog,
    pub last_focused_dbc_index: Option<usize>,
    pub dbc_window_focus_request: Option<usize>,
    pub clipboard: ClipboardState,
    pub confirm_delete_dialog: ConfirmDeleteDialog,
    pub close_confirm_dialog: CloseConfirmDialog,
    pub recent_files: Vec<String>,
    pub file_hovering: bool,
    /// FIBEX / ARXML (FlexRay) 查看与编辑窗口（从 roxy-fibex 整合）
    pub fibex: crate::fibex::state::FibexUiState,
    /// 当前聚焦的窗口类型
    pub focus: FocusTarget,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            show_performance_window: false,
            show_about_dialog: false,
            dbc_windows: Vec::new(),
            error_dialog: ErrorDialog::default(),
            notice_dialog: NoticeDialog::default(),
            validation_dialog: ValidationDialog::default(),
            last_focused_dbc_index: None,
            dbc_window_focus_request: None,
            clipboard: ClipboardState::default(),
            confirm_delete_dialog: ConfirmDeleteDialog::default(),
            close_confirm_dialog: CloseConfirmDialog::default(),
            recent_files: Vec::new(),
            file_hovering: false,
            fibex: crate::fibex::state::FibexUiState::default(),
            focus: FocusTarget::None,
        }
    }
}

/// 规范化路径用于比较：解析符号链接与 `..`，去掉 Windows 扩展前缀 `\\?\`。
/// 文件不存在时原样返回。
pub fn normalize_path(path: &str) -> String {
    std::path::Path::new(path)
        .canonicalize()
        .map(|p| p.to_string_lossy().trim_start_matches(r"\\?\").to_string())
        .unwrap_or_else(|_| path.to_string())
}

#[allow(dead_code)]
impl UiState {
    /// 获取当前聚焦的 DBC 窗口
    pub fn get_focused_dbc_window(&mut self) -> Option<&mut DbcWindow> {
        let idx = self.last_focused_dbc_index?;
        self.dbc_windows.get_mut(idx)
    }

    // copy_message removed; use handle_copy_message in menu.rs which already performs copy and logs

    /// 检查剪贴板是否有内容
    pub fn has_clipboard_message(&self) -> bool {
        !self.clipboard.copied_messages.is_empty()
    }

    /// 统一的文件打开入口（Ctrl+O / 拖放 / 命令行 / 最近文件都走这里）：
    /// - .dbc  → DBC 编辑窗口
    /// - .kcd  → 导入为 DBC 编辑窗口
    /// - .xlsx → 按通信矩阵模板生成 DBC 编辑窗口
    /// - .arxml / .xml / .fibex / .fx → FIBEX / AUTOSAR 查看窗口
    ///
    /// 已打开的同一文件（按规范化路径比较）只会聚焦已有窗口，不会重复打开。
    pub fn open_path(&mut self, path: &std::path::Path) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        // 统一为规范化的绝对路径：相对 / 绝对写法指向同一文件时只算一个
        let path_str = normalize_path(&path.to_string_lossy());
        let norm_path = std::path::Path::new(&path_str);

        match ext.as_str() {
            "dbc" => match DbcWindow::from_path(norm_path) {
                Ok(dbc_window) => {
                    self.add_recent_file(&path_str);
                    self.dbc_windows.push(dbc_window);
                    self.last_focused_dbc_index = Some(self.dbc_windows.len() - 1);
                }
                Err(e) => {
                    self.error_dialog.message = format!("Failed to load file: {}", e);
                    self.error_dialog.show = true;
                }
            },
            "kcd" => match crate::import::import_file(norm_path) {
                Ok(editable_dbc) => {
                    let dbc_window = DbcWindow::new(&path_str, editable_dbc);
                    self.add_recent_file(&path_str);
                    self.dbc_windows.push(dbc_window);
                    self.last_focused_dbc_index = Some(self.dbc_windows.len() - 1);
                }
                Err(e) => {
                    self.error_dialog.message = format!("Import failed: {}", e);
                    self.error_dialog.show = true;
                }
            },
            "xlsx" => self.open_excel_matrix(norm_path, &path_str),
            // FIBEX XML / AUTOSAR ARXML：按文件内容分流——CAN 快照进 DBC 编辑窗口，
            // FlexRay（FIBEX 3.0 / AUTOSAR R4.x）进 FIBEX 查看窗口
            "arxml" | "xml" | "fibex" | "fx" => {
                if is_can_snapshot(norm_path) {
                    match crate::import::import_file(norm_path) {
                        Ok(editable_dbc) => {
                            let dbc_window = DbcWindow::new(&path_str, editable_dbc);
                            self.add_recent_file(&path_str);
                            self.dbc_windows.push(dbc_window);
                            self.last_focused_dbc_index = Some(self.dbc_windows.len() - 1);
                        }
                        Err(e) => {
                            self.error_dialog.message = format!("Import failed: {}", e);
                            self.error_dialog.show = true;
                        }
                    }
                    return;
                }
                self.fibex.open_file_path(norm_path);
                if !self.fibex.error_dialog.show {
                    // 打开成功才计入最近文件
                    self.add_recent_file(&path_str);
                    self.last_focused_dbc_index = None;
                }
            }
            _ => {
                self.error_dialog.message = format!(
                    "Unsupported file type: {} (expected .dbc / .arxml / .xml / .kcd / .xlsx)",
                    path_str
                );
                self.error_dialog.show = true;
            }
        }
    }

    /// 按通信矩阵模板生成 DBC 编辑窗口。窗口记录的文件名改成同目录的 .dbc，
    /// 保存时写 .dbc，不会覆盖 Excel 源文件；导入过程中被跳过或没有写入的列
    /// 会在校验结果窗口里列出。
    fn open_excel_matrix(&mut self, path: &std::path::Path, source_path: &str) {
        let mut parsed = match crate::excel::read_matrix(path) {
            Ok(parsed) => parsed,
            Err(e) => {
                self.error_dialog.message = e;
                self.error_dialog.show = true;
                return;
            }
        };
        let dbc = crate::excel::to_dbc(&mut parsed);
        let mut dbc_window = DbcWindow::new(&path.with_extension("dbc").to_string_lossy(), dbc);
        // 还没有对应的 .dbc 文件，保存时才落盘
        dbc_window.is_dirty = true;
        self.add_recent_file(source_path);
        self.dbc_windows.push(dbc_window);
        self.last_focused_dbc_index = Some(self.dbc_windows.len() - 1);

        if !parsed.report.is_empty() {
            self.validation_dialog.issues = parsed
                .report
                .into_iter()
                .map(|message| crate::editable_dbc::ValidationIssue {
                    severity: crate::editable_dbc::Severity::Warning,
                    message,
                })
                .collect();
            self.validation_dialog.show = true;
        }
    }

    /// 生成下一个可用的 Message ID：优先取 max+1 起的第一个空闲 ID，
    /// 超过 29 位扩展上限后从 1 开始找空洞
    pub fn generate_next_message_id(&self, dbc_window_index: usize) -> u32 {
        const MAX_ID: u32 = 0x1FFF_FFFF;
        if let Some(window) = self.dbc_windows.get(dbc_window_index) {
            let used: std::collections::HashSet<u32> = window
                .dbc
                .messages()
                .iter()
                .map(|m| m.message_id())
                .collect();
            let max_id = used.iter().copied().max().unwrap_or(0);
            for cand in (max_id + 1)..=MAX_ID {
                if !used.contains(&cand) {
                    return cand;
                }
            }
            for cand in 1..=max_id {
                if !used.contains(&cand) {
                    return cand;
                }
            }
        }
        1
    }

    pub fn add_recent_file(&mut self, path: &str) {
        // 以规范化路径为准去重：相对 / 绝对路径指向同一文件时只保留一条
        let norm = normalize_path(path);
        self.recent_files.retain(|p| normalize_path(p) != norm);
        self.recent_files.insert(0, norm);
        if self.recent_files.len() > 10 {
            self.recent_files.truncate(10);
        }
        self.save_recent_files();
    }

    pub fn load_recent_files(&mut self) {
        if let Some(path) = recent_files_path()
            && let Ok(content) = std::fs::read_to_string(&path)
        {
            let mut seen: Vec<String> = Vec::new();
            for line in content.lines().filter(|l| !l.is_empty()) {
                let norm = normalize_path(line);
                if seen.contains(&norm) {
                    continue;
                }
                seen.push(norm);
                if seen.len() >= 10 {
                    break;
                }
            }
            self.recent_files = seen;
        }
    }

    pub fn save_recent_files(&self) {
        if let Some(path) = recent_files_path() {
            // 确保配置目录存在（如 %APPDATA%\roxy-dbc）
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let content = self.recent_files.join("\n");
            let _ = std::fs::write(path, content);
        }
    }
}

/// 文件内容是 CAN 通信快照（本工具导出的 ARXML）而不是 FlexRay：按元素名判断。
/// 读不出文本时返回 false，交给 FIBEX 一侧去报错。
fn is_can_snapshot(path: &std::path::Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let content = crate::file_encoding::decode_file_bytes(&bytes).text;
    (content.contains("<CAN-FRAME>") || content.contains("<CAN-CLUSTER>"))
        && !content.contains("FLEXRAY-CLUSTER")
}

/// 最近文件列表的存放位置：优先 %APPDATA%\roxy-dbc\recent_files.txt，
/// 避免写入程序目录失败（如安装在 Program Files）；无 APPDATA 时回退到 exe 目录。
fn recent_files_path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .map(|p| p.join("roxy-dbc"))
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        })?;
    Some(base.join("recent_files.txt"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_path_resolves_relative_and_dots() {
        let cwd = std::env::current_dir().unwrap();
        let norm = normalize_path("src\\..\\src\\ui");
        let expect = cwd
            .join("src")
            .join("ui")
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_string();
        assert_eq!(norm, expect);
    }

    #[test]
    fn normalize_path_keeps_missing_file_as_is() {
        assert_eq!(normalize_path("Untitled.dbc"), "Untitled.dbc");
    }

    #[test]
    fn add_recent_file_dedupes_same_file_different_spelling() {
        // canonicalize 依赖文件真实存在：先在临时目录创建
        let dir = std::env::temp_dir().join(format!("roxy-dbc-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let abs_path = dir.join("same.dbc");
        std::fs::write(&abs_path, b"").unwrap();

        let mut state = UiState::default();
        let abs = abs_path.to_string_lossy().to_string();
        // Windows 绝对路径里 / 与 \ 等价，canonicalize 后应指向同一文件
        let alt = abs.replace('\\', "/");
        state.add_recent_file(&abs);
        state.add_recent_file(&alt);
        assert_eq!(
            state.recent_files.len(),
            1,
            "same file via different spelling must not duplicate"
        );
        assert_eq!(state.recent_files[0], normalize_path(&abs));

        std::fs::remove_file(&abs_path).ok();
        std::fs::remove_dir(&dir).ok();
    }
}
