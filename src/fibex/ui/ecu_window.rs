use dear_imgui_rs::{TableFlags, TableSizingPolicy, Ui};

use crate::fibex::editable_fibex::EditableFibex;

/// ECU 列表（渲染在文件主窗口的标签页内）
#[allow(dead_code)]
#[derive(Clone, Default)]
pub struct EcuWindow {
    pub new_ecu_buffer: String,
    pub rename_target: Option<String>,
    pub rename_buffer: String,
}

impl EcuWindow {
    pub fn render_content(&mut self, ui: &Ui, fibex: &mut EditableFibex) {
        ui.input_text("##new_ecu", &mut self.new_ecu_buffer)
            .hint("New ECU name...")
            .build();
        ui.same_line();
        if ui.button("Add") && !self.new_ecu_buffer.trim().is_empty() {
            fibex.add_ecu(self.new_ecu_buffer.trim());
            self.new_ecu_buffer.clear();
        }

        let ecus: Vec<String> = fibex.ecus().clone();
        if ecus.is_empty() {
            ui.text_disabled("No ECUs defined");
            return;
        }

        // 工具栏行结束；实测剩余高度让表格正好填满
        let avail = ui.content_region_avail();
        let mut to_delete: Option<String> = None;
        let mut to_rename: Option<(String, String)> = None;

        ui.table("ecu_table")
            .flags(TableFlags::RESIZABLE | TableFlags::BORDERS | TableFlags::SCROLL_X | TableFlags::SCROLL_Y | TableFlags::ROW_BG)
            .sizing_policy(TableSizingPolicy::FixedFit)
            .freeze(0, 1)
            .outer_size([0.0, avail[1].max(120.0)])
            .column("ECU Name").weight(1.0).done()
            .column("Actions").done()
            .headers(true)
            .build(|ui| {
                for ecu_name in &ecus {
                    ui.table_next_row();

                    ui.table_set_column_index(0);
                    if self.rename_target.as_deref() == Some(ecu_name.as_str()) {
                        ui.set_next_item_width(-70.0);
                        ui.input_text("##rename_input", &mut self.rename_buffer).build();
                    } else {
                        ui.text(ecu_name);
                    }

                    ui.table_set_column_index(1);
                    if self.rename_target.as_deref() == Some(ecu_name.as_str()) {
                        if ui.small_button(format!("OK##{}", ecu_name)) {
                            let new_name = self.rename_buffer.trim().to_string();
                            if !new_name.is_empty() {
                                to_rename = Some((ecu_name.clone(), new_name));
                            }
                            self.rename_target = None;
                            self.rename_buffer.clear();
                        }
                        ui.same_line();
                        if ui.small_button(format!("Cancel##{}", ecu_name)) {
                            self.rename_target = None;
                            self.rename_buffer.clear();
                        }
                    } else {
                        if ui.small_button(format!("Rename##{}", ecu_name)) {
                            self.rename_target = Some(ecu_name.clone());
                            self.rename_buffer = ecu_name.clone();
                        }
                        ui.same_line();
                        if ui.small_button(format!("Delete##{}", ecu_name)) {
                            to_delete = Some(ecu_name.clone());
                        }
                    }
                }
            });

        if let Some(name) = to_delete {
            fibex.delete_ecu(&name);
        }
        if let Some((old, new)) = to_rename {
            fibex.rename_ecu(&old, &new);
        }
    }
}
