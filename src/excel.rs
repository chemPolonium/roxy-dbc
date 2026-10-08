//! Excel 通信矩阵模板 ↔ DBC
//!
//! 模板结构见 dbc-sample/DbcDemo.xlsx：单个工作表 CanMatrix，第 1 行是中英双语表头，
//! 之后报文行与信号行混排——报文名称非空的是报文行，信号名称非空的是信号行，
//! 信号行属于它上面最近的那个报文行。末尾若干列的表头是节点名，格子里填 S（发送）
//! 或 R（接收）。

use std::path::Path;

use calamine::{Data, Reader, Xlsx, open_workbook};
use can_dbc::{ByteOrder, ValueType};
use rust_xlsxwriter::{Format, Workbook, Worksheet};

use crate::editable_dbc::{
    AttrTarget, AttrValue, EditableDbc, EditableMessage, EditableSignal, FrameFormat, attr_names,
};

/// 工作表名，与模板一致
const SHEET_NAME: &str = "CanMatrix";

/// (表头英文键, 双语表头文本)：顺序即模板列顺序
/// (列键, 表头里可识别的英文写法, 写模板时用的双语表头)。
/// 认列时按写法长度从长到短匹配，所以 Signal_Min_Value 不会抢走 Signal_Min_Value_Phy。
pub const COLUMNS: [(&str, &[&str], &str); 29] = [
    ("msg_name", &["msg_name"], "报文名称\r\nMsg_Name"),
    ("msg_type", &["msg_type"], "报文类型\r\nMsg_Type"),
    ("msg_id", &["msg_id"], "报文标识符\r\nMsg_ID"),
    (
        "msg_send_type",
        &["msg_send_type"],
        "报文发送类型\r\nMsg_Send_Type",
    ),
    (
        "msg_cycle_time",
        &["msg_cycle_time"],
        "报文周期时间\r\nMsg_Cycle_Time",
    ),
    (
        "msg_cycle_time_fast",
        &["msg_cycle_time_fast"],
        "报文发送的快速周期(ms)\r\nMsg_Cycle_Time_Fast",
    ),
    (
        "msg_nr_of_repetition",
        &["msg_nr_of_reption", "msg_nr_of_repetition"],
        "报文快速发送的次数\r\nMsg_Nr_Of_Reption",
    ),
    (
        "msg_delay_time",
        &["msg_delay_time"],
        "报文延时时间(ms)\r\nMsg_Delay_Time",
    ),
    ("msg_length", &["msg_length"], "报文长度\r\nMsg_Length"),
    ("signal_name", &["signal_name"], "信号名称\r\nSignal_Name"),
    (
        "signal_description",
        &["signal_description"],
        "信号描述\r\nSignal_Description",
    ),
    ("byte_order", &["byte_order"], "排列格式\r\nByte_Order"),
    ("start_byte", &["start_byte"], "起始字节\r\nStart_Byte"),
    ("start_bit", &["start_bit"], "起始位\r\nStart_Bit"),
    (
        "signal_send_type",
        &["signal_send_type"],
        "信号发送类型\r\nSignal_Send_Type",
    ),
    ("bit_length", &["bit_length"], "信号长度\r\nBit_Length"),
    (
        "date_type",
        &["date_type", "data_type"],
        "数据类型\r\nDate_Type",
    ),
    ("factor", &["factor"], "精度\r\nFactor"),
    ("offset", &["offset"], "偏移量\r\nOffset"),
    (
        "min_phy",
        &["signal_min_value_phy", "min_phy"],
        "物理最小值\r\nSignal_Min_Value_Phy",
    ),
    (
        "max_phy",
        &["signal_max_value_phy", "max_phy"],
        "物理最大值\r\nSignal_Max_Value_Phy",
    ),
    (
        "min_bus",
        &["signal_min_value", "min_bus"],
        "总线最小值\r\nSignal_Min_Value",
    ),
    (
        "max_bus",
        &["signal_max_value", "max_bus"],
        "总线最大值\r\nSignal_Max_Value",
    ),
    (
        "initial_value",
        &["initial_value"],
        "初始值\r\nInitial_Value",
    ),
    (
        "invalid_value",
        &["invalid_value"],
        "无效值\r\nInvalid_Value",
    ),
    (
        "inactive_value",
        &["inactive_value"],
        "非使能值\r\nInactive_Value",
    ),
    ("unit", &["unit"], "单位\r\nUnit"),
    (
        "signal_values",
        &["signal_value_descriptions", "signal_values"],
        "信号值描述\r\nSignal_Value_Descriptions",
    ),
    ("comment", &["comments", "comment"], "备注\r\nComments"),
];

/// 模板列 -> DBC 属性：(列键, 属性名, 挂在什么对象上, 是否按数字解释)
///
/// 按数字解释的列接受 `0x` 十六进制或十进制；其余按文本存。属性声明由
/// [EditableDbc] 在赋值时补齐，导出模板时再按同样的名字读回来。
pub const ATTRIBUTE_COLUMNS: [(&str, &str, crate::editable_dbc::AttrTarget, bool); 10] = [
    ("msg_type", attr_names::MSG_TYPE, AttrTarget::Message, false),
    (
        "msg_send_type",
        attr_names::MSG_SEND_TYPE,
        AttrTarget::Message,
        false,
    ),
    (
        "msg_cycle_time",
        attr_names::MSG_CYCLE_TIME,
        AttrTarget::Message,
        true,
    ),
    (
        "msg_cycle_time_fast",
        attr_names::MSG_CYCLE_TIME_FAST,
        AttrTarget::Message,
        true,
    ),
    (
        "msg_nr_of_repetition",
        attr_names::MSG_NR_OF_REPETITION,
        AttrTarget::Message,
        true,
    ),
    (
        "msg_delay_time",
        attr_names::MSG_DELAY_TIME,
        AttrTarget::Message,
        true,
    ),
    (
        "signal_send_type",
        attr_names::SIG_SEND_TYPE,
        AttrTarget::Signal,
        false,
    ),
    (
        "initial_value",
        attr_names::SIG_START_VALUE,
        AttrTarget::Signal,
        true,
    ),
    (
        "invalid_value",
        attr_names::SIG_INVALID_VALUE,
        AttrTarget::Signal,
        true,
    ),
    (
        "inactive_value",
        attr_names::SIG_INACTIVE_VALUE,
        AttrTarget::Signal,
        true,
    ),
];

/// 一行里的单元格：列键 -> 文本
#[derive(Debug, Clone, Default)]
pub struct MatrixRow {
    pub excel_row: usize,
    pub values: Vec<(String, String)>,
    /// 节点列：节点名 -> 标记（S / R / 空）
    pub nodes: Vec<(String, String)>,
}

impl MatrixRow {
    fn get(&self, key: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .filter(|v| !v.trim().is_empty())
    }

    fn marked(&self, want: &str) -> Vec<String> {
        self.nodes
            .iter()
            .filter(|(_, mark)| mark.eq_ignore_ascii_case(want))
            .map(|(name, _)| name.clone())
            .collect()
    }
}

/// 解析结果
pub struct ParsedMatrix {
    pub rows: Vec<MatrixRow>,
    pub nodes: Vec<String>,
    /// 解析与转换过程中需要操作者知道的事
    pub report: Vec<String>,
}

/// 读取模板。sheet_index 为 None 时取第一个工作表。
pub fn read_matrix(path: &Path) -> Result<ParsedMatrix, String> {
    let mut book: Xlsx<_> =
        open_workbook(path).map_err(|e| format!("无法打开 Excel 文件: {}", e))?;

    let names = book.sheet_names().to_vec();
    let target = if names.iter().any(|n| n == SHEET_NAME) {
        SHEET_NAME.to_string()
    } else {
        names
            .first()
            .cloned()
            .ok_or_else(|| "文件里没有工作表".to_string())?
    };
    let range = book
        .worksheet_range(&target)
        .map_err(|e| format!("读取工作表失败: {}", e))?;

    let mut rows_iter = range.rows();
    let header = match rows_iter.next() {
        Some(h) => h,
        None => return Err("文件里没有表头行".to_string()),
    };

    // 表头 -> 列键；认不出来的按节点列处理（记下列号，数据行按列号取标记）
    let mut columns: Vec<(usize, String)> = Vec::new();
    let mut node_cols: Vec<(usize, String)> = Vec::new();
    for (idx, cell) in header.iter().enumerate() {
        let text = cell_text(cell);
        if text.trim().is_empty() {
            continue;
        }
        match classify_header(&text) {
            HeaderKind::Column(key) => columns.push((idx, key.to_string())),
            HeaderKind::Node => {
                let name = node_name(&text);
                if !name.is_empty() && !node_cols.iter().any(|(_, n)| *n == name) {
                    node_cols.push((idx, name));
                }
            }
        }
    }
    if !columns.iter().any(|(_, k)| k == "msg_name")
        || !columns.iter().any(|(_, k)| k == "signal_name")
    {
        return Err(
            "表头里找不到 Msg_Name / Signal_Name 列，不是本工具认识的通信矩阵模板".to_string(),
        );
    }

    let mut rows = Vec::new();
    for (row_idx, cells) in rows_iter.enumerate() {
        let mut row = MatrixRow {
            excel_row: row_idx + 2,
            values: Vec::new(),
            nodes: Vec::new(),
        };
        for (col, key) in &columns {
            row.values.push((
                key.clone(),
                cell_text(cells.get(*col).unwrap_or(&Data::Empty)),
            ));
        }
        for (col, name) in &node_cols {
            let mark = cell_text(cells.get(*col).unwrap_or(&Data::Empty))
                .trim()
                .to_uppercase();
            if !mark.is_empty() {
                row.nodes.push((name.clone(), mark));
            }
        }
        if row.values.iter().any(|(_, v)| !v.trim().is_empty()) || !row.nodes.is_empty() {
            rows.push(row);
        }
    }

    Ok(ParsedMatrix {
        report: Vec::new(),
        rows,
        nodes: node_cols.into_iter().map(|(_, name)| name).collect(),
    })
}

enum HeaderKind {
    Column(&'static str),
    Node,
}

/// 按表头里的英文写法识别列；写法从长到短匹配，避免 Signal_Min_Value 抢走
/// Signal_Min_Value_Phy、Msg_Cycle_Time 抢走 Msg_Cycle_Time_Fast
fn classify_header(header: &str) -> HeaderKind {
    let lower = header.to_lowercase();
    let mut tokens: Vec<(&'static str, &'static str)> = COLUMNS
        .iter()
        .flat_map(|(key, aliases, _)| aliases.iter().map(move |a| (*key, *a)))
        .collect();
    tokens.sort_by_key(|(_, token)| -(token.len() as i32));
    for (key, token) in tokens {
        if lower.contains(token) {
            return HeaderKind::Column(key);
        }
    }
    HeaderKind::Node
}

/// 节点列的表头可能只有一行（如 Demo1），取第一行做节点名
fn node_name(header: &str) -> String {
    header
        .split(['\n', '\r'])
        .map(|s| s.trim())
        .find(|s| !s.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Float(f) => format_number(*f),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

/// 表格里的数字：整数不带小数点，其余按最短形式
fn format_number(v: f64) -> String {
    if (v - v.round()).abs() < f64::EPSILON && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

fn parse_u64(text: &str) -> Option<u64> {
    let t = text.trim().replace('_', "");
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(hex.trim(), 16).ok()
    } else {
        t.parse::<u64>().ok()
    }
}

fn parse_f64(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok()
}

/// 值描述：`0x00:Off\r\n0x01:On`，也接受分号或逗号分隔
fn parse_value_descriptions(text: &str) -> Vec<(i64, String)> {
    let mut out = Vec::new();
    for part in text.split(['\n', '\r', ';']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((raw, desc)) = part.split_once(':') else {
            continue;
        };
        if let Some(v) = parse_u64(raw) {
            out.push((v as i64, desc.trim().to_string()));
        }
    }
    out
}

/// 报文行先收集成这个，最后连同它下面的信号行一起构造 EditableMessage
struct PendingMessage {
    id: u32,
    frame_format: FrameFormat,
    name: String,
    size: u64,
    transmitter: String,
    comment: String,
    /// 报文行标的 R 节点：信号行没标 R 时沿用
    receivers: Vec<String>,
    signals: Vec<EditableSignal>,
    /// 报文级属性（周期时间、发送类型等）
    attributes: Vec<(String, AttrValue)>,
}

/// 单元格文本 -> 属性值。数字列接受 `0x` 十六进制与十进制，带小数的按 FLOAT。
fn attr_value(text: &str, numeric: bool) -> Option<AttrValue> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if numeric {
        if let Some(v) = parse_u64(t) {
            return Some(AttrValue::Int(v as i64));
        }
        if let Some(v) = parse_f64(t) {
            return Some(AttrValue::Float(v));
        }
    }
    Some(AttrValue::Text(t.to_string()))
}

/// 一行里属于某个对象的全部属性列
fn row_attributes(row: &MatrixRow, target: AttrTarget) -> Vec<(String, AttrValue)> {
    let mut out = Vec::new();
    for (key, name, kind, numeric) in ATTRIBUTE_COLUMNS {
        if kind != target {
            continue;
        }
        if let Some(value) = row.get(key).and_then(|v| attr_value(v, numeric)) {
            out.push((name.to_string(), value));
        }
    }
    out
}

/// 把模板转成可编辑的 DBC 模型。
///
/// 报文类型、发送类型、周期时间、快速周期、重发次数、延时、信号发送类型、初始值、
/// 无效值、非使能值写进 DBC 属性（GenMsg* / GenSig*），保存时会带上 `BA_DEF_` 与 `BA_`。
pub fn to_dbc(parsed: &mut ParsedMatrix) -> EditableDbc {
    let mut nodes: Vec<String> = parsed.nodes.clone();
    let mut pending: Vec<PendingMessage> = Vec::new();
    let mut current: Option<usize> = None;

    for row in &parsed.rows {
        let msg_name = row.get("msg_name").map(|s| s.to_string());
        let sig_name = row.get("signal_name").map(|s| s.to_string());

        if let Some(name) = msg_name {
            let Some(raw_id) = row.get("msg_id").and_then(parse_u64) else {
                parsed.report.push(format!(
                    "第 {} 行：报文 {} 的 Msg_ID 缺失或不是数字，已跳过该报文",
                    row.excel_row, name
                ));
                current = None;
                continue;
            };

            let size = row.get("msg_length").and_then(parse_u64).unwrap_or(8);
            let frame_format = FrameFormat::compose(raw_id > 0x7FF, size > 8);
            let transmitter = row
                .marked("S")
                .into_iter()
                .next()
                .unwrap_or_else(|| "Vector__XXX".to_string());
            if transmitter != "Vector__XXX" && !nodes.contains(&transmitter) {
                nodes.push(transmitter.clone());
            }

            let receivers = row.marked("R");
            for node in &receivers {
                if !nodes.contains(node) {
                    nodes.push(node.clone());
                }
            }
            let mut signals = Vec::new();
            if let Some(signal) = build_signal(row, sig_name.as_deref(), &receivers, &mut nodes) {
                signals.push(signal);
            }
            pending.push(PendingMessage {
                id: raw_id as u32,
                frame_format,
                name: name.to_string(),
                size,
                transmitter,
                comment: row.get("comment").unwrap_or("").to_string(),
                receivers,
                signals,
                attributes: row_attributes(row, AttrTarget::Message),
            });
            current = Some(pending.len() - 1);
            continue;
        }

        if sig_name.is_some() {
            let Some(idx) = current else {
                parsed.report.push(format!(
                    "第 {} 行：信号上面没有报文行，已跳过",
                    row.excel_row
                ));
                continue;
            };
            let receivers = pending[idx].receivers.clone();
            if let Some(signal) = build_signal(row, sig_name.as_deref(), &receivers, &mut nodes) {
                pending[idx].signals.push(signal);
            }
        }
    }

    let messages = pending
        .into_iter()
        .map(|p| {
            let mut msg = EditableMessage::build(
                p.id,
                p.frame_format,
                p.name,
                p.size,
                p.transmitter,
                p.signals,
                p.comment,
            );
            for (name, value) in p.attributes {
                msg.apply_attribute(&name, Some(value));
            }
            msg
        })
        .collect();

    let mut dbc = EditableDbc::from_imported(messages, nodes);
    dbc.ensure_definitions_for_used_attributes();
    dbc
}

fn build_signal(
    row: &MatrixRow,
    name: Option<&str>,
    message_receivers: &[String],
    nodes: &mut Vec<String>,
) -> Option<EditableSignal> {
    let name = name?;
    let start_bit = row
        .get("start_bit")
        .and_then(parse_u64)
        .or_else(|| row.get("start_byte").and_then(parse_u64).map(|b| b * 8))
        .unwrap_or(0);
    let size = row.get("bit_length").and_then(parse_u64).unwrap_or(1);
    let byte_order = match row.get("byte_order").unwrap_or("").to_lowercase().as_str() {
        s if s.contains("intel") || s.contains("little") => ByteOrder::LittleEndian,
        _ => ByteOrder::BigEndian,
    };
    let value_type = match row.get("date_type").unwrap_or("").to_lowercase().as_str() {
        s if s.contains("signed") && !s.contains("unsigned") => ValueType::Signed,
        _ => ValueType::Unsigned,
    };
    let factor = row.get("factor").and_then(parse_f64).unwrap_or(1.0);
    let offset = row.get("offset").and_then(parse_f64).unwrap_or(0.0);
    let min = row
        .get("min_phy")
        .and_then(parse_f64)
        .or_else(|| row.get("min_bus").and_then(parse_f64))
        .unwrap_or(0.0);
    let max = row
        .get("max_phy")
        .and_then(parse_f64)
        .or_else(|| row.get("max_bus").and_then(parse_f64))
        .unwrap_or(0.0);
    let receivers: Vec<String> = {
        let own = row.marked("R");
        let list = if own.is_empty() {
            message_receivers.to_vec()
        } else {
            own
        };
        for node in &list {
            if !nodes.contains(node) {
                nodes.push(node.clone());
            }
        }
        list
    };

    let mut signal = EditableSignal::build(
        name.to_string(),
        start_bit,
        size,
        byte_order,
        value_type,
        factor,
        offset,
        min,
        max,
        row.get("unit").unwrap_or("").to_string(),
        receivers,
        parse_value_descriptions(row.get("signal_values").unwrap_or("")),
        row.get("signal_description").unwrap_or("").to_string(),
    );
    for (attr, value) in row_attributes(row, AttrTarget::Signal) {
        signal.apply_attribute(&attr, Some(value));
    }
    Some(signal)
}

/// 写出模板。dbc 为 None 时只写表头。
pub fn write_template(path: &Path, dbc: Option<&EditableDbc>) -> Result<(), String> {
    let mut workbook = Workbook::new();
    let mut sheet = Worksheet::new();
    sheet
        .set_name(SHEET_NAME)
        .map_err(|e| format!("设置工作表名失败: {}", e))?;

    let header_format = Format::new().set_bold().set_text_wrap();
    let nodes: Vec<String> = match dbc {
        Some(db) => db
            .nodes()
            .iter()
            .filter(|n| n.as_str() != "Vector__XXX")
            .cloned()
            .collect(),
        None => vec!["Node1".to_string()],
    };

    let headers: Vec<String> = COLUMNS
        .iter()
        .map(|(_, _, text)| text.to_string())
        .chain(nodes.iter().map(|n| format!("{}\r\nSend=S, Receive=R", n)))
        .collect();
    sheet
        .write_row_with_format(0, 0, headers.iter(), &header_format)
        .map_err(|e| format!("写表头失败: {}", e))?;
    sheet
        .set_freeze_panes(1, 0)
        .map_err(|e| format!("冻结表头失败: {}", e))?;
    for col in 0..headers.len() as u16 {
        let _ = sheet.set_column_width(col, 18.0);
    }

    let mut row_idx = 1;
    if let Some(db) = dbc {
        for message in db.messages() {
            let mut cells = vec![String::new(); COLUMNS.len() + nodes.len()];
            set(&mut cells, "msg_name", message.message_name());
            set(
                &mut cells,
                "msg_id",
                &format!("0x{:X}", message.message_id()),
            );
            set(
                &mut cells,
                "msg_length",
                &message.message_size().to_string(),
            );
            set(&mut cells, "comment", message.comment());
            for (key, attr, target, _) in ATTRIBUTE_COLUMNS {
                if target != AttrTarget::Message {
                    continue;
                }
                if let Some(value) = db.message_attribute(message.message_id(), attr) {
                    set(&mut cells, key, &value.display());
                }
            }
            if message.transmitter() != "Vector__XXX" {
                set_node(&mut cells, &nodes, message.transmitter(), "S");
            }
            write_row(&mut sheet, row_idx, &cells)?;
            row_idx += 1;

            for signal in message.signals() {
                let mut cells = vec![String::new(); COLUMNS.len() + nodes.len()];
                set(&mut cells, "signal_name", signal.name());
                set(&mut cells, "signal_description", signal.comment());
                for (key, attr, target, _) in ATTRIBUTE_COLUMNS {
                    if target != AttrTarget::Signal {
                        continue;
                    }
                    if let Some(value) =
                        db.signal_attribute(message.message_id(), signal.name(), attr)
                    {
                        set(&mut cells, key, &value.display());
                    }
                }
                set(
                    &mut cells,
                    "byte_order",
                    match signal.byte_order() {
                        ByteOrder::LittleEndian => "Intel",
                        ByteOrder::BigEndian => "Motorola",
                    },
                );
                set(&mut cells, "start_bit", &signal.start_bit().to_string());
                set(
                    &mut cells,
                    "start_byte",
                    &(signal.start_bit() / 8).to_string(),
                );
                set(&mut cells, "bit_length", &signal.signal_size().to_string());
                set(
                    &mut cells,
                    "date_type",
                    match signal.value_type() {
                        ValueType::Signed => "Signed",
                        ValueType::Unsigned => "Unsigned",
                    },
                );
                set(&mut cells, "factor", &signal.factor().to_string());
                set(&mut cells, "offset", &signal.offset().to_string());
                set(&mut cells, "min_phy", &signal.min().to_string());
                set(&mut cells, "max_phy", &signal.max().to_string());
                set(&mut cells, "unit", signal.unit());
                let values: Vec<String> = signal
                    .value_descriptions()
                    .iter()
                    .map(|(v, d)| format!("0x{:02X}:{}", v, d))
                    .collect();
                set(&mut cells, "signal_values", &values.join("\r\n"));
                for node in signal.receivers() {
                    set_node(&mut cells, &nodes, node, "R");
                }
                write_row(&mut sheet, row_idx, &cells)?;
                row_idx += 1;
            }
        }
    }

    workbook.push_worksheet(sheet);
    workbook
        .save(path)
        .map_err(|e| format!("保存 Excel 文件失败: {}", e))?;
    Ok(())
}

fn set(cells: &mut [String], key: &str, value: &str) {
    if let Some(idx) = COLUMNS.iter().position(|(k, _, _)| *k == key) {
        cells[idx] = value.to_string();
    }
}

fn set_node(cells: &mut [String], nodes: &[String], node: &str, mark: &str) {
    if let Some(offset) = nodes.iter().position(|n| n == node) {
        let idx = COLUMNS.len() + offset;
        if idx < cells.len() {
            cells[idx] = mark.to_string();
        }
    }
}

fn write_row(sheet: &mut Worksheet, row: u32, cells: &[String]) -> Result<(), String> {
    for (col, value) in cells.iter().enumerate() {
        if value.is_empty() {
            continue;
        }
        sheet
            .write_string(row, col as u16, value.as_str())
            .map_err(|e| format!("写入单元格失败: {}", e))?;
    }
    Ok(())
}

/// 供界面显示：模板里那些非几何列落到 .dbc 的什么地方
pub fn attribute_columns_hint() -> String {
    "报文类型、发送类型、周期时间、快速周期、重发次数、延时、信号发送类型、初始值、无效值、非使能值按 DBC 属性保存（GenMsgType / GenMsgSendType / GenMsgCycleTime / GenMsgCycleTimeFast / GenMsgNrOfRepetition / GenMsgDelayTime / GenSigSendType / GenSigStartValue / GenSigInvalidValue / GenSigInactiveValue），Excel 与 .dbc 之间来回不丢。".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn template_path() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("dbc-sample/DbcDemo.xlsx")
    }

    #[test]
    fn reads_the_sample_template() {
        let mut parsed = read_matrix(&template_path()).expect("template should parse");
        assert_eq!(parsed.nodes, vec!["Demo1", "Demo2", "Demo3", "Demo4"]);
        let dbc = to_dbc(&mut parsed);

        assert_eq!(dbc.messages().len(), 2, "Demo_2F3 与 Demo_500");
        let demo = dbc.get_message_by_name("Demo_500").expect("Demo_500");
        assert_eq!(demo.message_id(), 0x500);
        assert_eq!(demo.message_size(), 8);
        assert_eq!(demo.transmitter(), "Demo1");
        assert_eq!(demo.signals().len(), 6);

        let s1 = &demo.signals()[0];
        assert_eq!(s1.name(), "Demo_Signal_Name1");
        assert_eq!(s1.start_bit(), 0);
        assert_eq!(s1.signal_size(), 1);
        assert!(matches!(s1.value_type(), ValueType::Signed));

        let s6 = &demo.signals()[5];
        assert_eq!(s6.start_bit(), 56);
        assert_eq!(s6.signal_size(), 32);
        assert!((s6.factor() - 0.001).abs() < 1e-12);
        assert!((s6.offset() - 100.0).abs() < 1e-12);
        assert_eq!(s6.unit(), "%");
        assert_eq!(s6.value_descriptions().len(), 6);
        assert_eq!(s6.value_descriptions()[0], (0, "Demo_12".to_string()));
        // 第 9 行的节点标记是 S=R Demo1, R Demo2, R Demo3, Demo4 空白
        assert_eq!(
            s6.receivers(),
            &vec!["Demo2".to_string(), "Demo3".to_string()]
        );

        let first = dbc.get_message_by_name("Demo_2F3").expect("Demo_2F3");
        assert_eq!(first.message_id(), 0x2F3);
        assert_eq!(first.message_size(), 4);
    }

    /// 模板里的周期、发送类型、初始值这些列要落到 DBC 属性上
    #[test]
    fn attribute_columns_reach_the_dbc() {
        let mut parsed = read_matrix(&template_path()).expect("parse");
        let dbc = to_dbc(&mut parsed);
        assert!(
            parsed.report.is_empty(),
            "没有列被丢掉了，报告应为空: {:?}",
            parsed.report
        );

        let msg = dbc.get_message_by_name("Demo_500").expect("Demo_500");
        assert_eq!(
            dbc.message_attribute(msg.message_id(), attr_names::MSG_CYCLE_TIME),
            Some(AttrValue::Int(100)),
            "Msg_Cycle_Time=100"
        );
        assert_eq!(
            dbc.message_attribute(msg.message_id(), attr_names::MSG_CYCLE_TIME_FAST),
            Some(AttrValue::Int(20))
        );
        assert_eq!(
            dbc.message_attribute(msg.message_id(), attr_names::MSG_NR_OF_REPETITION),
            Some(AttrValue::Int(3))
        );
        assert_eq!(
            dbc.message_attribute(msg.message_id(), attr_names::MSG_SEND_TYPE),
            Some(AttrValue::Text("cycle".to_string()))
        );
        assert_eq!(
            dbc.message_attribute(msg.message_id(), attr_names::MSG_TYPE),
            Some(AttrValue::Text("NM".to_string()))
        );

        let first = &msg.signals()[0];
        assert_eq!(
            dbc.signal_attribute(msg.message_id(), first.name(), attr_names::SIG_START_VALUE),
            Some(AttrValue::Int(1)),
            "Initial_Value 写作 0x01，按十六进制读成 1"
        );
        assert_eq!(
            dbc.signal_attribute(msg.message_id(), first.name(), attr_names::SIG_SEND_TYPE),
            Some(AttrValue::Text("cycle".to_string()))
        );

        // 保存时既写声明也写取值
        let text = dbc.to_dbc_string();
        assert!(
            text.contains("BA_DEF_ BO_ \"GenMsgCycleTime\""),
            "缺 BA_DEF_ 声明"
        );
        assert!(text.contains("BA_ \"GenMsgCycleTime\" BO_ 1280 100;"));
        assert!(text.contains("BA_ \"GenSigStartValue\" SG_ 1280 Demo_Signal_Name1 1;"));
    }

    /// 模板 -> DBC -> 模板 -> DBC：属性两边对得上
    #[test]
    fn attributes_survive_the_excel_round_trip() {
        let mut parsed = read_matrix(&template_path()).expect("parse");
        let dbc = to_dbc(&mut parsed);

        let dir = std::env::temp_dir().join("roxy_dbc_excel_attr");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("attrs.xlsx");
        write_template(&file, Some(&dbc)).expect("write template");

        let mut again = read_matrix(&file).expect("re-read");
        let dbc2 = to_dbc(&mut again);
        let msg = dbc2.get_message_by_name("Demo_500").expect("Demo_500");
        assert_eq!(
            dbc2.message_attribute(msg.message_id(), attr_names::MSG_CYCLE_TIME),
            Some(AttrValue::Int(100)),
            "导出的模板里周期时间没写回去"
        );
        assert_eq!(
            dbc2.message_attribute(msg.message_id(), attr_names::MSG_SEND_TYPE),
            Some(AttrValue::Text("cycle".to_string()))
        );
        let first = &msg.signals()[0];
        assert_eq!(
            dbc2.signal_attribute(msg.message_id(), first.name(), attr_names::SIG_START_VALUE),
            Some(AttrValue::Int(1))
        );
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn exported_template_reads_back_as_the_same_database() {
        let mut parsed = read_matrix(&template_path()).expect("parse");
        let dbc = to_dbc(&mut parsed);

        let dir = std::env::temp_dir().join("roxy_dbc_excel_test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("roundtrip.xlsx");
        write_template(&file, Some(&dbc)).expect("write template");

        let mut again = read_matrix(&file).expect("re-read");
        let dbc2 = to_dbc(&mut again);

        assert_eq!(dbc2.messages().len(), dbc.messages().len(), "报文数不一致");
        for message in dbc.messages() {
            let other = dbc2
                .get_message_by_name(message.message_name())
                .unwrap_or_else(|| panic!("丢了报文 {}", message.message_name()));
            assert_eq!(other.message_id(), message.message_id());
            assert_eq!(other.message_size(), message.message_size());
            assert_eq!(other.transmitter(), message.transmitter());
            assert_eq!(other.signals_count(), message.signals_count());
            for signal in message.signals() {
                let s2 = other
                    .signals()
                    .iter()
                    .find(|s| s.name() == signal.name())
                    .expect("信号名应一致");
                assert_eq!(s2.start_bit(), signal.start_bit());
                assert_eq!(s2.signal_size(), signal.signal_size());
                assert_eq!(s2.value_descriptions(), signal.value_descriptions());
                assert_eq!(s2.receivers(), signal.receivers());
            }
        }
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn empty_template_has_headers_only() {
        let dir = std::env::temp_dir().join("roxy_dbc_excel_test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("empty.xlsx");
        write_template(&file, None).expect("write empty template");

        let parsed = read_matrix(&file).expect("re-read");
        assert!(parsed.rows.is_empty(), "空模板不该有数据行");
        let _ = std::fs::remove_file(&file);
    }
}
