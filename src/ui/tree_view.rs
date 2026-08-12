use crate::editable_dbc::EditableDbc;
use imgui::Ui;
use std::collections::HashSet;

#[allow(dead_code)]
pub enum TreeEvent {
    None,
    SelectMessage(u32),
    OpenMessage(u32),
    SelectSignal(u32, String),
    EditSignal(u32, String),
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct TreeState {
    pub selected_message_id: Option<u32>,
    pub selected_signal_name: Option<String>,
    pub search_query: String,
    expanded_messages: HashSet<u32>,
}

impl Default for TreeState {
    fn default() -> Self {
        Self {
            selected_message_id: None,
            selected_signal_name: None,
            search_query: String::new(),
            expanded_messages: HashSet::new(),
        }
    }
}

impl TreeState {
    pub fn render(&mut self, ui: &Ui, dbc: &EditableDbc, file_name: &str) -> TreeEvent {
        let mut event = TreeEvent::None;

        ui.input_text("##tree_search", &mut self.search_query)
            .hint("Filter messages...")
            .build();
        ui.separator();

        let messages = dbc.messages();
        let msg_count = messages.len();
        let query_lower = self.search_query.to_lowercase();

        ui.tree_node_config(format!("Messages ({})", msg_count))
            .default_open(true)
            .build(|| {
                for msg in messages.iter() {
                    let name = msg.message_name();
                    if !query_lower.is_empty() && !name.to_lowercase().contains(&query_lower) {
                        continue;
                    }

                    let msg_id = msg.message_id();
                    let is_expanded = self.expanded_messages.contains(&msg_id);

                    let arrow = if is_expanded { "\u{25BC}" } else { "\u{25B6}" };
                    let label = format!(
                        "{} {} (0x{:03X}, {})##msg_{}",
                        arrow,
                        name,
                        msg_id,
                        msg.message_size(),
                        msg_id
                    );

                    let is_msg_selected = self.selected_message_id == Some(msg_id)
                        && self.selected_signal_name.is_none();

                    if ui.selectable_config(&label).selected(is_msg_selected).build() {
                        self.selected_message_id = Some(msg_id);
                        self.selected_signal_name = None;
                        self.toggle_message_expanded(msg_id);
                        event = TreeEvent::SelectMessage(msg_id);
                    }
                    if ui.is_item_hovered() && ui.is_mouse_double_clicked(imgui::MouseButton::Left) {
                        if !is_expanded {
                            self.expanded_messages.insert(msg_id);
                        }
                        event = TreeEvent::OpenMessage(msg_id);
                    }

                    if is_expanded {
                        ui.indent();
                        for sig in msg.signals().iter() {
                            let sig_name = sig.name();
                            let sig_label = format!(
                                "  {} ({}:{})##sig_{}_{}",
                                sig_name,
                                sig.start_bit(),
                                sig.signal_size(),
                                msg_id,
                                sig_name
                            );

                            let is_sig_selected = self.selected_message_id == Some(msg_id)
                                && self.selected_signal_name.as_deref() == Some(sig_name);

                            if ui.selectable_config(&sig_label).selected(is_sig_selected).build() {
                                self.selected_message_id = Some(msg_id);
                                self.selected_signal_name = Some(sig_name.to_string());
                                event = TreeEvent::SelectSignal(msg_id, sig_name.to_string());
                            }
                            if ui.is_item_hovered()
                                && ui.is_mouse_double_clicked(imgui::MouseButton::Left)
                            {
                                event =
                                    TreeEvent::EditSignal(msg_id, sig_name.to_string());
                            }
                        }
                        ui.unindent();
                    }
                }
            });

        let nodes = dbc.nodes();
        if !nodes.is_empty() {
            ui.tree_node_config(format!("Nodes ({})", nodes.len()))
                .default_open(true)
                .build(|| {
                    for node in nodes.iter() {
                        let label = format!("{}##node_{}", node, file_name);
                        ui.selectable_config(&label).selected(false).build();
                    }
                });
        }

        event
    }

    pub fn toggle_message_expanded(&mut self, message_id: u32) {
        if self.expanded_messages.contains(&message_id) {
            self.expanded_messages.remove(&message_id);
        } else {
            self.expanded_messages.insert(message_id);
        }
    }
}
