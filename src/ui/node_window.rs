use imgui::Ui;

use crate::editable_dbc::EditableDbc;

pub struct NodeDialog {
    pub show: bool,
    pub new_node_buffer: String,
    pub rename_target: Option<String>,
    pub rename_buffer: String,
}

impl Default for NodeDialog {
    fn default() -> Self {
        Self {
            show: false,
            new_node_buffer: String::new(),
            rename_target: None,
            rename_buffer: String::new(),
        }
    }
}

impl NodeDialog {
    pub fn render(&mut self, ui: &Ui, dbc: &mut EditableDbc) {
        if !self.show {
            return;
        }

        let mut is_open = self.show;
        ui.window("Nodes")
            .size([300.0, 400.0], imgui::Condition::FirstUseEver)
            .opened(&mut is_open)
            .build(|| {
                ui.input_text("##new_node", &mut self.new_node_buffer)
                    .hint("New node name...")
                    .build();
                ui.same_line();
                if ui.button("Add") && !self.new_node_buffer.trim().is_empty() {
                    dbc.add_node(self.new_node_buffer.trim());
                    self.new_node_buffer.clear();
                }

                ui.separator();

                let nodes: Vec<String> = dbc.nodes().clone();
                let mut to_delete: Option<String> = None;
                let mut to_rename: Option<(String, String)> = None;

                for node_name in &nodes {
                    if self.rename_target.as_deref() == Some(node_name.as_str()) {
                        ui.input_text("##rename_input", &mut self.rename_buffer).build();
                        ui.same_line();
                        if ui.button("OK") {
                            let new_name = self.rename_buffer.trim().to_string();
                            if !new_name.is_empty() {
                                to_rename = Some((node_name.clone(), new_name));
                            }
                            self.rename_target = None;
                            self.rename_buffer.clear();
                        }
                        ui.same_line();
                        if ui.button("Cancel") {
                            self.rename_target = None;
                            self.rename_buffer.clear();
                        }
                    } else {
                        ui.text(node_name);
                        ui.same_line();
                        if ui.small_button(format!("Rename##{}", node_name)) {
                            self.rename_target = Some(node_name.clone());
                            self.rename_buffer = node_name.clone();
                        }
                        ui.same_line();
                        if ui.small_button(format!("Delete##{}", node_name)) {
                            to_delete = Some(node_name.clone());
                        }
                    }
                }

                if let Some(name) = to_delete {
                    dbc.delete_node(&name);
                }
                if let Some((old, new)) = to_rename {
                    dbc.rename_node(&old, &new);
                }
            });

        self.show = is_open;
    }
}
