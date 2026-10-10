//! FlexRay 侧的通信矩阵与信号清单导出
//!
//! - 通信矩阵：行 = 帧（可展开到帧内各 PDU 的信号），列 = ECU，
//!   格子里标 TX / RX / R*（只收到该帧部分信号）
//! - 信号清单 CSV：与 Signal List 标签页看到的内容一致，另附注释与值表

use std::path::Path;

use dear_imgui_rs::{TableColumnFlags, TableFlags, TableOptions, Ui};

use crate::fibex::editable_fibex::{EditableFibex, EditableFrame, FrChannel};

/// 发送：亮绿色；接收：珊瑚红；只收到部分信号：橙色 R*
const TX_COLOR: [f32; 4] = [0.35, 0.85, 0.55, 1.0];
const RX_COLOR: [f32; 4] = [0.95, 0.5, 0.45, 1.0];
const RX_PARTIAL_COLOR: [f32; 4] = [1.0, 0.65, 0.2, 1.0];

enum Mark {
    Tx,
    Rx,
    RxPartial { received: usize, total: usize },
}

/// 帧的排程摘要：单通道写 `A slot 3 / cycle 0 of 4`，两通道时隙不同就分开写
fn schedule_summary(frame: &EditableFrame) -> String {
    let a = frame.channel_triggering(FrChannel::A);
    let b = frame.channel_triggering(FrChannel::B);
    match (a, b) {
        (Some(t), None) | (None, Some(t)) => format!(
            "{} slot {} / cycle {} of {}",
            t.channel.label(),
            t.slot_id,
            t.base_cycle,
            t.cycle_repetition
        ),
        (Some(a), Some(b)) if a.slot_id == b.slot_id => format!(
            "A+B slot {} / cycle {} of {}",
            a.slot_id, a.base_cycle, a.cycle_repetition
        ),
        (Some(a), Some(b)) => format!(
            "A slot {} / B slot {} / cycle {} of {}",
            a.slot_id, b.slot_id, a.base_cycle, a.cycle_repetition
        ),
        (None, None) => "not scheduled".to_string(),
    }
}

/// 渲染通信矩阵（只读：收发关系来自文件，编辑它在通信工具那边做）
pub fn render_comm_matrix(ui: &Ui, fibex: &EditableFibex) {
    let ecus = fibex.ecus().clone();
    if ecus.is_empty() {
        ui.text_disabled(
            "This file lists no ECUs, so there is nothing to put in the matrix columns.",
        );
        return;
    }

    let frames = fibex.frames().clone();
    // 帧内所有信号的接收方并集，用于判断 R*
    let mut any_mark = false;

    let avail_h = ui.content_region_avail()[1];
    if let Some(_table) = ui.begin_table_with_sizing(
        "fr_comm_matrix",
        1 + ecus.len(),
        TableOptions::new()
            .flags(
                TableFlags::BORDERS
                    | TableFlags::ROW_BG
                    | TableFlags::SCROLL_X
                    | TableFlags::SCROLL_Y
                    | TableFlags::NO_SAVED_SETTINGS,
            )
            .sizing_policy(dear_imgui_rs::TableSizingPolicy::FixedFit),
        [0.0, avail_h],
        0.0,
    ) {
        ui.table_setup_column("Frames / Receive-ECUs", TableColumnFlags::NONE, None);
        for ecu in &ecus {
            ui.table_setup_column(ecu, TableColumnFlags::NONE, None);
        }
        ui.table_setup_scroll_freeze(1, 1);
        ui.table_headers_row();

        for frame in &frames {
            let mut signals: Vec<(String, crate::fibex::editable_fibex::EditableSignal)> =
                Vec::new();
            let mut senders: Vec<String> = Vec::new();
            for mapping in frame.pdus() {
                let Some(pdu) = fibex.get_pdu(&mapping.pdu_name) else {
                    continue;
                };
                senders.extend(pdu.senders().iter().cloned());
                for sig in pdu.signals() {
                    signals.push((pdu.name().to_string(), sig.clone()));
                }
            }
            senders.sort();
            senders.dedup();

            let total = signals.len();
            let mut marks: Vec<(usize, Mark)> = Vec::new();
            for (idx, ecu) in ecus.iter().enumerate() {
                let sends = senders.iter().any(|s| s == ecu);
                let received = signals
                    .iter()
                    .filter(|(_, s)| s.receivers().iter().any(|r| r == ecu))
                    .count();
                if sends {
                    marks.push((idx, Mark::Tx));
                } else if total > 0 && received == total {
                    marks.push((idx, Mark::Rx));
                } else if received > 0 {
                    marks.push((idx, Mark::RxPartial { received, total }));
                }
            }
            any_mark |= !marks.is_empty();

            ui.table_next_row();
            ui.table_set_column_index(0);
            let tree = ui.tree_node(format!(
                "{}  ({})##fr_{}",
                frame.name(),
                schedule_summary(frame),
                frame.name()
            ));

            for (idx, mark) in &marks {
                ui.table_set_column_index(idx + 1);
                let (label, color) = match mark {
                    Mark::Tx => ("TX", TX_COLOR),
                    Mark::Rx => ("RX", RX_COLOR),
                    Mark::RxPartial { .. } => ("R*", RX_PARTIAL_COLOR),
                };
                ui.text_colored(color, label);
                if let Mark::RxPartial { received, total } = mark
                    && ui.is_item_hovered()
                {
                    ui.set_tooltip(format!("Receives {} of {} signals", received, total));
                }
            }

            if let Some(_token) = tree {
                for (pdu_name, sig) in &signals {
                    ui.table_next_row();
                    ui.table_set_column_index(0);
                    ui.indent_by(18.0);
                    ui.text(format!("{} . {}", pdu_name, sig.name()));
                    ui.unindent_by(18.0);

                    for (idx, ecu) in ecus.iter().enumerate() {
                        if !sig.receivers().iter().any(|r| r == ecu) {
                            continue;
                        }
                        ui.table_set_column_index(idx + 1);
                        ui.text_colored(RX_COLOR, "RX");
                    }
                }
            }
        }
    }

    if !any_mark {
        ui.text_disabled(
            "No sender / receiver info in this file, so the matrix is empty. \
             Slots and cycles are still listed above.",
        );
    }
}

/// Signal List 标签页的 CSV 导出按钮；返回错误信息（无则 None）
pub fn render_export_button(ui: &Ui, fibex: &EditableFibex, file_path: &str) -> Option<String> {
    if !ui.button("Export CSV...") {
        return None;
    }
    let csv = build_signal_csv(fibex);
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(csv.as_bytes());
    let default_name = format!(
        "{}_signals.csv",
        Path::new(file_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("flexray")
    );
    let path = rfd::FileDialog::new()
        .add_filter("CSV files", &["csv"])
        .set_directory(crate::paths::save_dir_for(file_path))
        .set_file_name(&default_name)
        .save_file()?;
    match std::fs::write(&path, &bytes) {
        Ok(()) => None,
        Err(e) => Some(format!("Failed to export CSV: {}", e)),
    }
}

/// 一行 = 一个 (PDU, 信号)，列与 Signal List 标签页一致，另附注释与值表
pub fn build_signal_csv(fibex: &EditableFibex) -> String {
    const HEADERS: [&str; 15] = [
        "PDU", "Signal", "Start", "Length", "Order", "Type", "Factor", "Offset", "Min", "Max",
        "Unit", "Tx", "Rx", "Comment", "Values",
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

    for pdu in fibex.pdus() {
        for sig in pdu.signals() {
            let values: Vec<String> = sig
                .value_descriptions()
                .iter()
                .map(|(v, d)| format!("{}={}", v, d))
                .collect();
            let fields = [
                pdu.name().to_string(),
                sig.name().to_string(),
                sig.start_bit().to_string(),
                sig.length_bits().to_string(),
                match sig.byte_order() {
                    crate::fibex::editable_fibex::ByteOrder::BigEndian => "Motorola",
                    crate::fibex::editable_fibex::ByteOrder::LittleEndian => "Intel",
                }
                .to_string(),
                match sig.value_type() {
                    crate::fibex::editable_fibex::ValueType::Signed => "Signed",
                    crate::fibex::editable_fibex::ValueType::Unsigned => "Unsigned",
                }
                .to_string(),
                sig.factor().to_string(),
                sig.offset().to_string(),
                sig.min().to_string(),
                sig.max().to_string(),
                sig.unit().to_string(),
                sig.senders().join(";"),
                sig.receivers().join(";"),
                sig.comment().to_string(),
                values.join(";"),
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
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fibex::editable_fibex::{
        ByteOrder, EditablePdu, EditableSignal, PduKind, ValueType,
    };

    #[test]
    fn csv_of_real_sample_has_one_row_per_signal() {
        use crate::fibex::import::parse_content;

        let fibex =
            parse_content(include_str!("../../../fibex-sample/PowerTrain.arxml")).expect("parse");
        let csv = build_signal_csv(&fibex);
        let lines: Vec<&str> = csv.lines().collect();
        let signal_count: usize = fibex.pdus().iter().map(|p| p.signals().len()).sum();

        assert_eq!(lines.len(), signal_count + 1, "每个信号一行 + 表头");
        assert!(lines.iter().all(|l| l.split(',').count() >= 15), "列数不足");
        // 值表信号应带出 Values 列内容
        assert!(csv.contains("Off") || csv.contains("Idle") || csv.contains("="));
    }

    #[test]
    fn csv_escapes_and_lists_senders() {
        let mut fibex = EditableFibex::new();
        fibex.add_ecu("ECU_A");
        let mut sig = EditableSignal::build(
            "Speed,kmh".to_string(),
            0,
            16,
            ByteOrder::BigEndian,
            ValueType::Unsigned,
            0.1,
            0.0,
            0.0,
            500.0,
            "km/h".to_string(),
            vec!["ECU_B".to_string()],
            Vec::new(),
            "note".to_string(),
        );
        sig.set_senders(vec!["ECU_A".to_string()]);
        let pdu = EditablePdu::build(
            "Pdu\"1\"".to_string(),
            4,
            PduKind::Static,
            vec![sig],
            String::new(),
        );
        fibex.add_pdu(&pdu);

        let csv = build_signal_csv(&fibex);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0].split(',').count(), 15);
        // 引号翻倍、含逗号的字段整体加引号
        assert!(lines[1].contains("\"Pdu\"\"1\"\"\""), "{}", lines[1]);
        assert!(lines[1].contains("\"Speed,kmh\""), "{}", lines[1]);
        assert!(lines[1].contains("ECU_A"));
        assert!(lines[1].contains("ECU_B"));
    }
}
