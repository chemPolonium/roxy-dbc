#[cfg(test)]
mod tests {
    use crate::dbc::{CustomMessage, DbcData, EditableDbcData};
    use crate::ui::menu;
    use crate::ui::state::{DbcWindowState, UiState, UndoOperationKind};

    // Helper to create a UiState with one DBC window containing one message
    fn setup_state_with_message() -> (UiState, u32) {
        // Create a minimal DBC data and editable wrapper
        let mut dbc = DbcData::new();

        // Construct a simple custom message
        let msg = CustomMessage {
            message_id: 0x100,
            message_name: "TestMsg".to_string(),
            message_size: 8,
            transmitter: "".to_string(),
            comment: "".to_string(),
            signals: vec![],
        };

        let editable = EditableDbcData::from_dbc_data(dbc);

        // Build UiState and add a DbcWindowState
        let mut ui_state = UiState::new();
        let mut window = DbcWindowState::new(0, editable);

        // add message
        window.editable_data.add_message(msg.clone());
        ui_state.dbc_windows.push(window);
        (ui_state, msg.message_id)
    }

    #[test]
    fn delete_message_records_undo_and_undo_restores() {
        let (mut ui_state, message_id) = setup_state_with_message();

        // focus the first dbc window
        ui_state.last_focused_dbc_index = Some(0);

        // Ensure message exists before delete
        assert!(
            ui_state.dbc_windows[0]
                .editable_data
                .get_all_messages()
                .iter()
                .any(|m| m.message_id() == message_id)
        );

        // Call delete (this uses UiState.get_focused_dbc_window internally)
        menu::handle_delete_message(&mut ui_state, message_id);

        // After delete, message should be gone
        assert!(
            !ui_state.dbc_windows[0]
                .editable_data
                .get_all_messages()
                .iter()
                .any(|m| m.message_id() == message_id)
        );

        // Undo stack should have an entry
        assert!(ui_state.dbc_windows[0].can_undo());

        // Perform undo
        ui_state.dbc_windows[0].undo();

        // After undo, message should be present again
        assert!(
            ui_state.dbc_windows[0]
                .editable_data
                .get_all_messages()
                .iter()
                .any(|m| m.message_id() == message_id)
        );
    }
}
