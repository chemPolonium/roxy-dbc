//! All Signals / 通信矩阵窗口
//!
//! 把一个 DBC 中所有消息的信号展平成一张通信矩阵表：
//! 每行 = 一个信号，同时携带所属消息的上下文（ID、名称、DLC、发送节点）。
//! 支持：
//! - 按任意列排序、按名称过滤
//! - 双击信号打开信号编辑对话框；右键 Edit Message / Delete Signal
//! - Values（值表 VAL_）行内编辑，格式 `0=Off; 1=On`
//! - "+ Add Signal" 添加到选中行所在的消息（自动命名 NewSignal_NNNN）
//! - 导出 CSV（UTF-8 BOM，Excel 友好）

use can_dbc::{ByteOrder, ValueType};
use dear_imgui_rs::{MouseButton, SortDirection, TableFlags, Ui};

use crate::editable_dbc::{EditableDbc, EditableSignal};
use crate::ui::message_edit_window::MessageEditWindowState;
use crate::ui::signal_edit_window::SignalEditDialog;
use std::path::Path;

/// 矩阵窗口向 DbcWindow 上抛的事件（需要 UiState 配合的部分）
#[allow(dead_code)]
pub enum AllSignalsEvent {
    None,
    /// 请求删除一个信号（走统一的确认对话框）
    DeleteSignal { msg_id: u32, sig_name: String },
    /// 导出或其它操作失败
    Error(String),
}

/// 矩阵的一行：一个信号 + 所属消息上下文
#[derive(Clone, Debug)]
pub struct MatrixRow {
    pub msg_id: u32,
    pub msg_extended: bool,
    pub msg_name: String,
    pub dlc: u64,
    pub transmitter: String,
    pub sig_name: String,
    pub start_bit: u64,
    pub signal_size: u64,
    pub byte_order_is_little: bool,
    pub signed: bool,
    pub factor: f64,
    pub offset: f64,
    pub min: f64,
    pub max: f64,
    pub unit: String,
    pub receivers: String,
    pub comment: String,
    pub values: Vec<(i64, String)>,
}

impl MatrixRow {
    pub fn from_signal(msg: &crate::editable_dbc::EditableMessage, sig: &EditableSignal) -> Self {
        Self {
            msg_id: msg.message_id(),
            msg_extended: msg.is_extended(),
            msg_name: msg.message_name().to_string(),
            dlc: msg.message_size(),
            transmitter: msg.transmitter().to_string(),
            sig_name: sig.name().to_string(),
            start_bit: sig.start_bit(),
            signal_size: sig.signal_size(),
            byte_order_is_little: matches!(sig.byte_order(), can_dbc::ByteOrder::LittleEndian),
            signed: matches!(sig.value_type(), can_dbc::ValueType::Signed),
            factor: sig.factor(),
            offset: sig.offset(),
            min: sig.min(),
            max: sig.max(),
            unit: sig.unit().to_string(),
            receivers: sig.receivers().join(","),
            comment: sig.comment().to_string(),
            values: sig.value_descriptions().to_vec(),
        }
    }
}

/// 把值表格式化为 `0=Off; 1=On` 的单行文本
pub fn format_values(values: &[(i64, String)]) -> String {
    values
        .iter()
        .map(|(v, d)| format!("{}={}", v, d))
        .collect::<Vec<_>>()
        .join("; ")
}

/// 解析 `0=Off; 1=On` 格式的值表文本；非法时返回错误描述
pub fn parse_values(text: &str) -> Result<Vec<(i64, String)>, String> {
    let mut out: Vec<(i64, String)> = Vec::new();
    for part in text.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((v, d)) = part.split_once('=') else {
            return Err(format!("'{}' is missing '='", part));
        };
        let value = v
            .trim()
            .parse::<i64>()
            .map_err(|_| format!("'{}' is not a valid integer", v.trim()))?;
        out.push((value, d.trim().to_string()));
    }

    let mut seen = std::collections::HashSet::new();
    for (v, _) in &out {
        if !seen.insert(*v) {
            return Err(format!("duplicate value {}", v));
        }
    }
    // 与信号编辑对话框一致：应用时按值排序
    out.sort_by_key(|(v, _)| *v);
    Ok(out)
}

/// 生成下一个可用的信号名（NewSignal_0001 风格，跳过已存在的名字）
pub fn next_signal_name(dbc: &EditableDbc) -> String {
    let mut n = dbc.messages().iter().map(|m| m.signals().len()).sum::<usize>() + 1;
    loop {
        let name = format!("NewSignal_{:04}", n);
        let exists = dbc
            .messages()
            .iter()
            .any(|m| m.signals().iter().any(|s| s.name() == name));
        if !exists {
            return name;
        }
        n += 1;
    }
}

/// 通信矩阵 CSV（UTF-8 BOM 前缀由调用方处理，这里返回纯文本）
pub fn build_matrix_csv(rows: &[MatrixRow]) -> String {
    const HEADERS: [&str; 17] = [
        "ID", "Message", "DLC", "Transmitter", "Signal", "Start", "Length", "Order", "Type",
        "Factor", "Offset", "Min", "Max", "Unit", "Receivers", "Comment", "Values",
    ];

    fn csv_field(s: &str) -> String {
        if s.contains('"') || s.contains(',') || s.contains('\n') || s.contains('\r') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    }

    let mut out = String::new();
    out.push_str(
        &HEADERS
            .iter()
            .map(|h| csv_field(h))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push('\n');

    for r in rows {
        let fields = [
            format!("0x{:03X}{}", r.msg_id, if r.msg_extended { "x" } else { "" }),
            r.msg_name.clone(),
            r.dlc.to_string(),
            r.transmitter.clone(),
            r.sig_name.clone(),
            r.start_bit.to_string(),
            r.signal_size.to_string(),
            if r.byte_order_is_little { "Intel" } else { "Motorola" }.to_string(),
            if r.signed { "Signed" } else { "Unsigned" }.to_string(),
            r.factor.to_string(),
            r.offset.to_string(),
            r.min.to_string(),
            r.max.to_string(),
            r.unit.clone(),
            if r.receivers.is_empty() { "Vector__XXX".to_string() } else { r.receivers.clone() },
            r.comment.clone(),
            format_values(&r.values),
        ];
        out.push_str(
            &fields
                .iter()
                .map(|f| csv_field(f))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    }
    out
}

/// 列索引（与表头顺序一致，供排序使用）
mod col {
    pub const ID: u32 = 0;
    pub const MESSAGE: u32 = 1;
    pub const DLC: u32 = 2;
    pub const TRANSMITTER: u32 = 3;
    pub const SIGNAL: u32 = 4;
    pub const START: u32 = 5;
    pub const LENGTH: u32 = 6;
    pub const ORDER: u32 = 7;
    pub const TYPE: u32 = 8;
    pub const FACTOR: u32 = 9;
    pub const OFFSET: u32 = 10;
    pub const MIN: u32 = 11;
    pub const MAX: u32 = 12;
    pub const UNIT: u32 = 13;
    pub const RECEIVERS: u32 = 14;
    pub const COMMENT: u32 = 15;
    pub const VALUES: u32 = 16;
}

/// All Signals 标签页状态（每个 DbcWindow 一个）
#[derive(Clone)]
pub struct AllSignalsWindow {
    search: String,
    sort_column: u32,
    sort_ascending: bool,
    /// 当前选中行 (msg_id, sig_name)
    selected: Option<(u32, String)>,
    /// 正在行内编辑值表的信号
    values_edit_target: Option<(u32, String)>,
    values_edit_buffer: String,
}

impl AllSignalsWindow {
    pub fn new() -> Self {
        Self {
            search: String::new(),
            sort_column: col::ID,
            sort_ascending: true,
            selected: None,
            values_edit_target: None,
            values_edit_buffer: String::new(),
        }
    }
}

impl Default for AllSignalsWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl AllSignalsWindow {
    fn sorted_rows(&self, dbc: &EditableDbc) -> Vec<MatrixRow> {
        let mut rows: Vec<MatrixRow> = Vec::new();
        for msg in dbc.messages() {
            for sig in msg.signals() {
                rows.push(MatrixRow::from_signal(msg, sig));
            }
        }

        let query = self.search.to_lowercase();
        if !query.is_empty() {
            rows.retain(|r| {
                r.sig_name.to_lowercase().contains(&query)
                    || r.msg_name.to_lowercase().contains(&query)
                    || r.transmitter.to_lowercase().contains(&query)
            });
        }

        let col = self.sort_column;
        let asc = self.sort_ascending;
        rows.sort_by(|a, b| {
            let cmp = match col {
                col::ID => a.msg_id.cmp(&b.msg_id),
                col::MESSAGE => a.msg_name.cmp(&b.msg_name),
                col::DLC => a.dlc.cmp(&b.dlc),
                col::TRANSMITTER => a.transmitter.cmp(&b.transmitter),
                col::SIGNAL => a.sig_name.cmp(&b.sig_name),
                col::START => a.start_bit.cmp(&b.start_bit),
                col::LENGTH => a.signal_size.cmp(&b.signal_size),
                col::ORDER => a.byte_order_is_little.cmp(&b.byte_order_is_little),
                col::TYPE => a.signed.cmp(&b.signed),
                col::FACTOR => a.factor.partial_cmp(&b.factor).unwrap_or(std::cmp::Ordering::Equal),
                col::OFFSET => a.offset.partial_cmp(&b.offset).unwrap_or(std::cmp::Ordering::Equal),
                col::MIN => a.min.partial_cmp(&b.min).unwrap_or(std::cmp::Ordering::Equal),
                col::MAX => a.max.partial_cmp(&b.max).unwrap_or(std::cmp::Ordering::Equal),
                col::UNIT => a.unit.cmp(&b.unit),
                col::RECEIVERS => a.receivers.cmp(&b.receivers),
                col::COMMENT => a.comment.cmp(&b.comment),
                col::VALUES => format_values(&a.values).cmp(&format_values(&b.values)),
                _ => std::cmp::Ordering::Equal,
            };
            if asc { cmp } else { cmp.reverse() }
        });
        rows
    }
}

/// 渲染矩阵窗口需要的 DbcWindow 内部部件
pub struct MatrixContext<'a> {
    pub dbc: &'a mut EditableDbc,
    pub signal_edit_dialog: &'a mut SignalEditDialog,
    pub edit_windows: &'a mut Vec<MessageEditWindowState>,
    pub is_dirty: &'a mut bool,
    pub file_path: &'a str,
}

/// 生成默认的新信号（start 0、length 1、系数 1）
fn default_signal_named(name: &str) -> EditableSignal {
    EditableSignal::build(
        name.to_string(),
        0,
        1,
        ByteOrder::LittleEndian,
        ValueType::Unsigned,
        1.0,
        0.0,
        0.0,
        0.0,
        String::new(),
        Vec::new(),
        Vec::new(),
        String::new(),
    )
}

/// 在 "All Signals" 标签页中渲染通信矩阵内容（工具栏 + 展平表）
pub fn render(window: &mut AllSignalsWindow, ctx: MatrixContext, ui: &Ui) -> AllSignalsEvent {
    let mut event = AllSignalsEvent::None;

    if ui.small_button("+ Add Signal") {
        if let Some((msg_id, _)) = window.selected.clone() {
            if ctx.dbc.get_message(msg_id).is_some() {
                let name = next_signal_name(ctx.dbc);
                let sig = default_signal_named(&name);
                ctx.dbc.add_signal(msg_id, &sig);
                *ctx.is_dirty = true;
            }
        }
    }
    if window.selected.is_none() {
        ui.same_line();
        ui.text_disabled("(select a row to choose the target message)");
    }
    ui.same_line();
    if ui.small_button("Export CSV...") {
        let rows = window.sorted_rows(ctx.dbc);
        let csv = build_matrix_csv(&rows);
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(csv.as_bytes());
        let default_name = format!(
            "{}_matrix.csv",
            Path::new(ctx.file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("communication")
        );
        let Some(path) = rfd::FileDialog::new()
            .add_filter("CSV files", &["csv"])
            .set_file_name(&default_name)
            .save_file()
        else {
            return AllSignalsEvent::None;
        };
        if let Err(e) = std::fs::write(&path, &bytes) {
            event = AllSignalsEvent::Error(format!("Failed to export CSV: {}", e));
        }
    }
    ui.same_line();
    ui.input_text("##matrix_search", &mut window.search)
        .hint("Filter signal / message / transmitter...")
        .build();

    let rows = window.sorted_rows(ctx.dbc);
    ui.same_line();
    ui.text_disabled(format!("{} signal(s)", rows.len()));

    let avail = ui.content_region_avail();
    if let Some(_table) = ui.begin_table_with_sizing(
        "matrix_table",
        17,
        TableFlags::RESIZABLE
            | TableFlags::BORDERS
            | TableFlags::NO_BORDERS_IN_BODY
            | TableFlags::SCROLL_X
            | TableFlags::SCROLL_Y
            | TableFlags::SORTABLE,
        [0.0, avail[1]],
        0.0,
    ) {
                for header in [
                    "ID",
                    "Message",
                    "DLC",
                    "Transmitter",
                    "Signal",
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
                    "Comment",
                    "Values",
                ] {
                    ui.table_setup_column(header, dear_imgui_rs::TableColumnFlags::NONE, None);
                }
                // 冻结标题行（矩阵还有横向滚动，同时冻结 ID 列便于对照）
                ui.table_setup_scroll_freeze(1, 1);
                ui.table_headers_row();

                if let Some(mut sort_specs) = ui.table_get_sort_specs() {
                    if sort_specs.is_dirty() {
                        if let Some(spec) = sort_specs.iter().next() {
                            window.sort_column = usize::from(spec.column_index) as u32;
                        window.sort_ascending =
                            spec.sort_direction == SortDirection::Ascending;
                        }
                        sort_specs.clear_dirty(ui);
                    }
                }

                for row in &rows {
                    let key = (row.msg_id, row.sig_name.clone());
                    let is_selected = window.selected.as_ref() == Some(&key);

                    ui.table_next_row();

                    ui.table_set_column_index(col::ID as usize);
                    ui.text(format!(
                        "0x{:03X}{}",
                        row.msg_id,
                        if row.msg_extended { "x" } else { "" }
                    ));

                    ui.table_set_column_index(col::MESSAGE as usize);
                    ui.text(&row.msg_name);

                    ui.table_set_column_index(col::DLC as usize);
                    ui.text(row.dlc.to_string());

                    ui.table_set_column_index(col::TRANSMITTER as usize);
                    ui.text(&row.transmitter);

                    ui.table_set_column_index(col::SIGNAL as usize);
                    if ui
                        .selectable_config(&row.sig_name)
                        .selected(is_selected)
                        .span_all_columns(true)
                        .build()
                    {
                        window.selected = Some(key.clone());
                    }
                    if ui.is_item_hovered() && ui.is_mouse_double_clicked(MouseButton::Left) {
                        if let Some(msg) = ctx.dbc.get_message(row.msg_id) {
                            if let Some(sig) =
                                msg.signals().iter().find(|s| s.name() == row.sig_name)
                            {
                                ctx.signal_edit_dialog
                                    .open_from_signal(msg.message_id(), sig);
                            }
                        }
                    }

                    if let Some(_popup) = ui.begin_popup_context_item_with_label(Some(
                        &format!(
                            "matrix_ctx_{}_{}",
                            row.msg_id, row.sig_name
                        ),
                    )) {
                        if window.selected.as_ref() != Some(&key) {
                            window.selected = Some(key.clone());
                        }
                        if ui.menu_item("Edit Signal...") {
                            if let Some(msg) = ctx.dbc.get_message(row.msg_id) {
                                if let Some(sig) =
                                    msg.signals().iter().find(|s| s.name() == row.sig_name)
                                {
                                    ctx.signal_edit_dialog
                                        .open_from_signal(msg.message_id(), sig);
                                }
                            }
                        }
                        if ui.menu_item("Edit Values...") {
                            window.values_edit_target = Some(key.clone());
                            window.values_edit_buffer = format_values(&row.values);
                        }
                        if ui.menu_item("Edit Message...") {
                            if let Some(msg) = ctx.dbc.get_message(row.msg_id) {
                                ctx.edit_windows.push(MessageEditWindowState::open(msg));
                            }
                        }
                        ui.separator();
                        if ui.menu_item("Delete Signal") {
                            event = AllSignalsEvent::DeleteSignal {
                                msg_id: row.msg_id,
                                sig_name: row.sig_name.clone(),
                            };
                        }
                    }

                    ui.table_set_column_index(col::START as usize);
                    ui.text(row.start_bit.to_string());

                    ui.table_set_column_index(col::LENGTH as usize);
                    ui.text(row.signal_size.to_string());

                    ui.table_set_column_index(col::ORDER as usize);
                    ui.text(if row.byte_order_is_little { "Intel" } else { "Motorola" });

                    ui.table_set_column_index(col::TYPE as usize);
                    ui.text(if row.signed { "Signed" } else { "Unsigned" });

                    ui.table_set_column_index(col::FACTOR as usize);
                    ui.text(row.factor.to_string());

                    ui.table_set_column_index(col::OFFSET as usize);
                    ui.text(row.offset.to_string());

                    ui.table_set_column_index(col::MIN as usize);
                    ui.text(row.min.to_string());

                    ui.table_set_column_index(col::MAX as usize);
                    ui.text(row.max.to_string());

                    ui.table_set_column_index(col::UNIT as usize);
                    ui.text(&row.unit);

                    ui.table_set_column_index(col::RECEIVERS as usize);
                    if row.receivers.is_empty() {
                        ui.text_disabled("Vector__XXX");
                    } else {
                        ui.text(&row.receivers);
                    }

                    ui.table_set_column_index(col::COMMENT as usize);
                    ui.text(&row.comment);

                    // Values 列：双击进入行内编辑（0=Off; 1=On）
                    ui.table_set_column_index(col::VALUES as usize);
                    let editing_this = window.values_edit_target.as_ref() == Some(&key);
                    if editing_this {
                        ui.input_text("##matrix_values_edit", &mut window.values_edit_buffer)
                            .build();
                        ui.same_line();
                        let invalid = parse_values(&window.values_edit_buffer).is_err();
                        if ui.small_button("OK##matrix_values_apply") {
                            match parse_values(&window.values_edit_buffer) {
                                Ok(values) => {
                                    ctx.dbc.set_signal_value_descriptions(
                                        key.0,
                                        &key.1,
                                        values,
                                    );
                                    *ctx.is_dirty = true;
                                    window.values_edit_target = None;
                                }
                                Err(_) => {
                                    // 非法输入不退出编辑态，等待修正
                                }
                            }
                        }
                        ui.same_line();
                        if ui.small_button("X##matrix_values_cancel") {
                            window.values_edit_target = None;
                        }
                        if invalid {
                            ui.same_line();
                            ui.text_colored([1.0, 0.3, 0.3, 1.0], "invalid (use 0=Off; 1=On)");
                        }
                    } else {
                        let text = format_values(&row.values);
                        if text.is_empty() {
                            ui.text_disabled("-");
                        } else {
                            ui.text(&text);
                            if ui.is_item_hovered() {
                                ui.set_tooltip(&text);
                            }
                        }
                        if ui.is_item_hovered() && ui.is_mouse_double_clicked(MouseButton::Left) {
                            window.values_edit_target = Some(key.clone());
                            window.values_edit_buffer = text;
                        }
                    }
                }
    }
    event
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_format_parse_round_trip() {
        let values = vec![(0i64, "关闭".to_string()), (1i64, "开启".to_string())];
        let text = format_values(&values);
        assert_eq!(text, "0=关闭; 1=开启");
        let parsed = parse_values(&text).unwrap();
        assert_eq!(parsed, values);
    }

    #[test]
    fn parse_values_sorts_by_value() {
        let parsed = parse_values("3=three; -1=neg; 0=zero").unwrap();
        assert_eq!(
            parsed,
            vec![(-1, "neg".to_string()), (0, "zero".to_string()), (3, "three".to_string())]
        );
    }

    #[test]
    fn parse_values_rejects_missing_equals() {
        assert!(parse_values("0 zero").is_err());
    }

    #[test]
    fn parse_values_rejects_bad_integer() {
        assert!(parse_values("abc=x").is_err());
    }

    #[test]
    fn parse_values_rejects_duplicates() {
        assert!(parse_values("1=a; 1=b").is_err());
    }

    #[test]
    fn csv_escapes_commas_quotes_and_newlines() {
        let msg = crate::editable_dbc::EditableMessage::build(
            0x100,
            crate::editable_dbc::FrameFormat::Extended,
            "Msg,1".to_string(),
            8,
            "ECU".to_string(),
            Vec::new(),
            String::new(),
        );
        let sig = EditableSignal::build(
            "S\"1\"".to_string(),
            0,
            8,
            can_dbc::ByteOrder::LittleEndian,
            can_dbc::ValueType::Unsigned,
            0.1,
            0.0,
            0.0,
            1.0,
            "km/h".to_string(),
            vec!["A".to_string()],
            vec![(1i64, "on".to_string())],
            "line1\nline2".to_string(),
        );
        let row = MatrixRow::from_signal(&msg, &sig);
        let csv = build_matrix_csv(&[row]);
        assert!(csv.contains("\"Msg,1\""));
        assert!(csv.contains("\"S\"\"1\"\"\""));
        assert!(csv.contains("\"line1\nline2\""));
        assert!(csv.contains("0x100x"));
    }
}
