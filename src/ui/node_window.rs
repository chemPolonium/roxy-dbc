//! Node List 标签页：节点的添加 / 重命名 / 删除（内联渲染在 DBC 窗口标签中）

use dear_imgui_rs::{TableFlags, Ui};

use crate::editable_dbc::EditableDbc;

/// Node List 标签页的交互状态（每个 DbcWindow 一个）
#[derive(Clone, Default)]
pub struct NodeListState {
    pub new_node_buffer: String,
    pub rename_target: Option<String>,
    pub rename_buffer: String,
}

/// 在标签页内容区渲染节点列表；返回是否有增删改（用于标记脏状态）
pub fn render_node_list(ui: &Ui, dbc: &mut EditableDbc, state: &mut NodeListState) -> bool {
    let mut changed = false;

    ui.set_next_item_width(260.0);
    ui.input_text("##new_node", &mut state.new_node_buffer)
        .hint("New node name...")
        .build();
    ui.same_line();
    if ui.button("Add") && !state.new_node_buffer.trim().is_empty() {
        dbc.add_node(state.new_node_buffer.trim());
        state.new_node_buffer.clear();
        changed = true;
    }

    ui.separator();

    let nodes: Vec<String> = dbc.nodes().clone();
    if nodes.is_empty() {
        ui.text_disabled("No nodes defined. Add a node above, or they will be inferred from TX/RX references.");
        return changed;
    }

    // 统计每个节点的发送报文数与接收信号数，让列表信息更完整
    let tx_count = |name: &str| -> usize {
        dbc.messages()
            .iter()
            .filter(|m| m.transmitter() == name)
            .count()
    };
    let rx_count = |name: &str| -> usize {
        dbc.messages()
            .iter()
            .flat_map(|m| m.signals())
            .filter(|s| s.receivers().iter().any(|r| r == name))
            .count()
    };

    // 表格填满标签页剩余空间，窗口不出现滚动条
    let avail_h = ui.content_region_avail()[1];
    if let Some(_table) = ui.begin_table_with_sizing(
        "node_list",
        4,
        TableFlags::RESIZABLE
            | TableFlags::BORDERS
            | TableFlags::ROW_BG
            | TableFlags::SCROLL_X
            | TableFlags::SCROLL_Y,
        [0.0, avail_h],
        0.0,
    ) {
        // Node 列占满剩余宽度；其余列宽自适应内容
        ui.table_setup_column(
            "Node",
            dear_imgui_rs::TableColumnFlags::NONE,
            Some(dear_imgui_rs::TableColumnWidth::Stretch(1.0)),
        );
        ui.table_setup_column("TX Messages", dear_imgui_rs::TableColumnFlags::NONE, None);
        ui.table_setup_column("RX Signals", dear_imgui_rs::TableColumnFlags::NONE, None);
        ui.table_setup_column("Actions", dear_imgui_rs::TableColumnFlags::NONE, None);
        ui.table_setup_scroll_freeze(0, 1);
        ui.table_headers_row();

        let mut to_delete: Option<String> = None;
        let mut to_rename: Option<(String, String)> = None;

        for node_name in &nodes {
            ui.table_next_row();

            ui.table_set_column_index(0);
            if state.rename_target.as_deref() == Some(node_name.as_str()) {
                ui.set_next_item_width(160.0);
                ui.input_text("##rename_input", &mut state.rename_buffer).build();
                ui.same_line();
                if ui.small_button("OK") {
                    let new_name = state.rename_buffer.trim().to_string();
                    if !new_name.is_empty() {
                        to_rename = Some((node_name.clone(), new_name));
                    }
                    state.rename_target = None;
                    state.rename_buffer.clear();
                }
                ui.same_line();
                if ui.small_button("Cancel") {
                    state.rename_target = None;
                    state.rename_buffer.clear();
                }
            } else {
                ui.text(node_name);
            }

            ui.table_set_column_index(1);
            ui.text(tx_count(node_name).to_string());

            ui.table_set_column_index(2);
            ui.text(rx_count(node_name).to_string());

            ui.table_set_column_index(3);
            if ui.small_button(format!("Rename##{}", node_name)) {
                state.rename_target = Some(node_name.clone());
                state.rename_buffer = node_name.clone();
            }
            ui.same_line();
            if ui.small_button(format!("Delete##{}", node_name)) {
                to_delete = Some(node_name.clone());
            }
        }

        if let Some(name) = to_delete {
            dbc.delete_node(&name);
            changed = true;
        }
        if let Some((old, new)) = to_rename {
            dbc.rename_node(&old, &new);
            changed = true;
        }
    }

    changed
}
