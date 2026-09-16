//! Node List 标签页：节点的添加 / 重命名 / 删除（内联渲染在 DBC 窗口标签中）

use dear_imgui_rs::Ui;

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
        return false;
    }

    let mut to_delete: Option<String> = None;
    let mut to_rename: Option<(String, String)> = None;

    for node_name in &nodes {
        if state.rename_target.as_deref() == Some(node_name.as_str()) {
            ui.input_text("##rename_input", &mut state.rename_buffer).build();
            ui.same_line();
            if ui.button("OK") {
                let new_name = state.rename_buffer.trim().to_string();
                if !new_name.is_empty() {
                    to_rename = Some((node_name.clone(), new_name));
                }
                state.rename_target = None;
                state.rename_buffer.clear();
            }
            ui.same_line();
            if ui.button("Cancel") {
                state.rename_target = None;
                state.rename_buffer.clear();
            }
        } else {
            ui.text(node_name);
            ui.same_line();
            if ui.small_button(format!("Rename##{}", node_name)) {
                state.rename_target = Some(node_name.clone());
                state.rename_buffer = node_name.clone();
            }
            ui.same_line();
            if ui.small_button(format!("Delete##{}", node_name)) {
                to_delete = Some(node_name.clone());
            }
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

    changed
}
