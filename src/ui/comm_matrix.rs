//! Communication Matrix 标签页：CANdb++ 风格的 信号/消息 × 节点 TX/RX 关系矩阵
//!
//! - 行 = 消息（可展开为信号），列 = 网络节点
//! - 消息行的发送节点列显示 TX；接收了该消息任一信号的节点列显示 RX
//! - 展开后每个信号行显示各自的 RX 节点
//! - 首列与表头冻结，支持纵向 / 横向滚动

use dear_imgui_rs::{TableColumnFlags, TableFlags, Ui};

use crate::editable_dbc::EditableDbc;

const TX_COLOR: [f32; 4] = [0.15, 0.15, 0.15, 1.0];
const RX_COLOR: [f32; 4] = [0.85, 0.15, 0.15, 1.0];

/// 在标签页内容区渲染通信矩阵
pub fn render_comm_matrix(ui: &Ui, dbc: &EditableDbc) {
    let nodes = dbc.nodes();
    if nodes.is_empty() {
        ui.text_disabled(
            "No nodes defined. Add nodes in the Node List tab first, then the TX/RX matrix shows here.",
        );
        return;
    }

    let avail = ui.content_region_avail();
    if let Some(_table) = ui.begin_table_with_sizing(
        "comm_matrix",
        1 + nodes.len(),
        dear_imgui_rs::TableOptions::new()
            .flags(
                TableFlags::RESIZABLE
                    | TableFlags::BORDERS
                    | TableFlags::NO_BORDERS_IN_BODY
                    | TableFlags::SCROLL_X
                    | TableFlags::SCROLL_Y,
            )
            .sizing_policy(dear_imgui_rs::TableSizingPolicy::FixedFit),
        [0.0, avail[1]],
        0.0,
    ) {
        ui.table_setup_column(
            "Signals / Receive-Nodes",
            TableColumnFlags::NONE,
            None,
        );
        for node in nodes {
            ui.table_setup_column(node, TableColumnFlags::NONE, None);
        }
        // 冻结首列与表头
        ui.table_setup_scroll_freeze(1, 1);
        ui.table_headers_row();

        for msg in dbc.messages() {
            // 该消息 TX 节点与所有信号 RX 节点的列索引（按列序渲染，ImGui 单元格只能前进）
            let mut marks: Vec<(usize, bool)> = Vec::new(); // (列索引, 是否 TX)
            if let Some(col) = nodes.iter().position(|n| *n == msg.transmitter()) {
                marks.push((col + 1, true));
            }
            for sig in msg.signals() {
                for r in sig.receivers() {
                    if let Some(col) = nodes.iter().position(|n| n == r) {
                        marks.push((col + 1, false));
                    }
                }
            }
            // 同列既是发送又是接收时保留 TX；再按列序排好
            marks.sort_by_key(|(c, is_tx)| (*c, !*is_tx));
            marks.dedup_by_key(|(c, _)| *c);

            ui.table_next_row();
            ui.table_set_column_index(0);
            let tree = ui.tree_node(format!(
                "{}##msg_{}",
                msg.message_name(),
                msg.message_id()
            ));
            for (col, is_tx) in &marks {
                ui.table_set_column_index(*col);
                if *is_tx {
                    ui.text_colored(TX_COLOR, "TX");
                } else {
                    ui.text_colored(RX_COLOR, "RX");
                }
            }

            if let Some(_tree_token) = tree {
                for sig in msg.signals() {
                    ui.table_next_row();
                    ui.table_set_column_index(0);
                    ui.indent_by(18.0);
                    ui.text(sig.name());
                    ui.unindent_by(18.0);

                    let mut rx_cols: Vec<usize> = sig
                        .receivers()
                        .iter()
                        .filter_map(|r| nodes.iter().position(|n| n == r).map(|i| i + 1))
                        .collect();
                    rx_cols.sort_unstable();
                    rx_cols.dedup();
                    for col in rx_cols {
                        ui.table_set_column_index(col);
                        ui.text_colored(RX_COLOR, "RX");
                    }
                }
            }
        }
    }
}
