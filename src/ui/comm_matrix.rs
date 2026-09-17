//! Communication Matrix 标签页：CANdb++ 风格的 信号/消息 × 节点 TX/RX 关系矩阵
//!
//! - 行 = 消息（可展开为信号），列 = 网络节点
//! - 消息行：发送节点列显示 TX；接收了该消息全部信号的节点显示 RX，
//!   只接收部分信号时显示橙色 R*（悬停提示 n/m）
//! - 展开后每个信号行显示各自的 RX 节点
//! - 可编辑：点击信号行节点的任意单元格切换该节点的接收状态；
//!   点击消息行节点的单元格把发送节点切换为该节点（再次点击当前 TX 清为 Vector__XXX）
//! - 首列与表头冻结，纵向 / 横向滚动

use dear_imgui_rs::{TableColumnFlags, TableFlags, TableOptions, Ui};

use crate::editable_dbc::EditableDbc;

/// 发送节点：亮绿色，醒目但不刺眼
const TX_COLOR: [f32; 4] = [0.35, 0.85, 0.55, 1.0];
/// 接收节点：柔和的珊瑚红
const RX_COLOR: [f32; 4] = [0.95, 0.5, 0.45, 1.0];
/// 部分接收：橙色 R*
const RX_PARTIAL_COLOR: [f32; 4] = [1.0, 0.65, 0.2, 1.0];

/// 一行内某个节点列的标记
enum Mark {
    Tx,
    Rx,
    RxPartial { received: usize, total: usize },
}

/// 在标签页内容区渲染通信矩阵；直接编辑数据库并按需置脏
pub fn render_comm_matrix(ui: &Ui, dbc: &mut EditableDbc, is_dirty: &mut bool) {
    let nodes = dbc.nodes().clone();
    if nodes.is_empty() {
        ui.text_disabled(
            "No nodes defined. Add nodes in the Node List tab first, then the TX/RX matrix shows here.",
        );
        return;
    }

    let messages: Vec<u32> = dbc.messages().iter().map(|m| m.message_id()).collect();

    // 表格填满标签页剩余空间，窗口不出现滚动条
    let avail = ui.content_region_avail();
    let avail_h = avail[1];
    if let Some(_table) = ui.begin_table_with_sizing(
        "comm_matrix",
        1 + nodes.len(),
        TableOptions::new()
            .flags(
                TableFlags::RESIZABLE
                    | TableFlags::BORDERS
                    | TableFlags::ROW_BG
                    | TableFlags::SCROLL_X
                    | TableFlags::SCROLL_Y,
            )
            .sizing_policy(dear_imgui_rs::TableSizingPolicy::FixedFit),
        [0.0, avail_h],
        0.0,
    ) {
        ui.table_setup_column("Signals / Receive-Nodes", TableColumnFlags::NONE, None);
        for node in &nodes {
            ui.table_setup_column(node, TableColumnFlags::NONE, None);
        }
        // 冻结首列与表头
        ui.table_setup_scroll_freeze(1, 1);
        ui.table_headers_row();

        for msg_id in &messages {
            let Some(msg) = dbc.get_message(*msg_id) else {
                continue;
            };
            let msg = msg.clone();
            let total = msg.signals().len();

            // 汇总每个节点列的接收信号数
            let mut marks: Vec<(usize, Mark)> = Vec::new();
            for (node_idx, node) in nodes.iter().enumerate() {
                let received = msg
                    .signals()
                    .iter()
                    .filter(|s| s.receivers().iter().any(|r| r == node))
                    .count();
                if node == &msg.transmitter() {
                    marks.push((node_idx, Mark::Tx));
                } else if total > 0 && received == total {
                    marks.push((node_idx, Mark::Rx));
                } else if received > 0 {
                    marks.push((node_idx, Mark::RxPartial { received, total }));
                }
            }

            ui.table_next_row();
            ui.table_set_column_index(0);
            let tree = ui.tree_node(format!(
                "{}##msg_{}",
                msg.message_name(),
                msg.message_id()
            ));

            for (node_idx, mark) in &marks {
                let col = node_idx + 1;
                ui.table_set_column_index(col);
                let (label, color) = match mark {
                    Mark::Tx => ("TX", TX_COLOR),
                    Mark::Rx => ("RX", RX_COLOR),
                    Mark::RxPartial { .. } => ("R*", RX_PARTIAL_COLOR),
                };
                let cell_start = ui.cursor_screen_pos();
                ui.text_colored(color, label);
                if let Mark::RxPartial { received, total } = mark {
                    if ui.is_item_hovered() {
                        ui.set_tooltip(&format!(
                            "Receives {} of {} signals (partial)",
                            received, total
                        ));
                    }
                }

                // 整格点击区：消息行点击 = 切换发送节点
                ui.set_cursor_screen_pos(cell_start);
                let w = ui.content_region_avail()[0].max(30.0);
                let id = format!(
                    "##cmtx_{}_{}",
                    msg.message_id(),
                    nodes[*node_idx]
                );
                if ui.invisible_button(&id, [w, ui.frame_height()]) {
                    let new_tx = if msg.transmitter() == nodes[*node_idx] {
                        "Vector__XXX"
                    } else {
                        &nodes[*node_idx]
                    };
                    dbc.set_message_transmitter(msg.message_id(), new_tx);
                    *is_dirty = true;
                }
            }

            if let Some(_tree_token) = tree {
                for sig in msg.signals().clone() {
                    ui.table_next_row();
                    ui.table_set_column_index(0);
                    ui.indent_by(18.0);
                    ui.text(sig.name());
                    ui.unindent_by(18.0);

                    for (node_idx, node) in nodes.iter().enumerate() {
                        let receives = sig.receivers().iter().any(|r| r == node);
                        let col = node_idx + 1;
                        ui.table_set_column_index(col);
                        let cell_start = ui.cursor_screen_pos();
                        if receives {
                            ui.text_colored(RX_COLOR, "RX");
                        }

                        // 整格点击区：信号行点击 = 切换接收
                        ui.set_cursor_screen_pos(cell_start);
                        let w = ui.content_region_avail()[0].max(30.0);
                        let id = format!(
                            "##cmrx_{}_{}_{}",
                            msg.message_id(),
                            sig.name(),
                            node
                        );
                        if ui.invisible_button(&id, [w, ui.frame_height()]) {
                            let mut receivers = sig.receivers().clone();
                            if let Some(pos) = receivers.iter().position(|r| r == node) {
                                receivers.remove(pos);
                            } else {
                                receivers.push(node.clone());
                            }
                            dbc.set_signal_receivers(msg.message_id(), sig.name(), receivers);
                            *is_dirty = true;
                        }
                    }
                }
            }
        }
    }
}
