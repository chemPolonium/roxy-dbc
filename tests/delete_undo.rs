use roxy_dbc::dbc::CustomMessage;
use roxy_dbc::dbc::OverridesSnapshot;
use roxy_dbc::ui::state::{DbcWindowState, UiState, UndoOperationKind};

fn setup_state_with_message() -> (UiState, u32) {
    // Create editable data
    let editable = roxy_dbc::dbc::EditableDbcData::new();

    // Build UiState and add a DbcWindowState
    let mut ui_state = UiState::default();
    let mut window = DbcWindowState::new(0, editable);

    // Construct a simple custom message and add it
    let msg = CustomMessage {
        message_id: 0x100,
        message_name: "TestMsg".to_string(),
        message_size: 8,
        transmitter: "".to_string(),
        comment: "".to_string(),
        signals: vec![],
    };
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

    // Simulate deletion logic (create snapshots, delete, push undo) without using private menu::handle_delete_message
    let before_snapshot = OverridesSnapshot::from_editable(&ui_state.dbc_windows[0].editable_data);
    ui_state.dbc_windows[0]
        .editable_data
        .delete_message(message_id);
    let after_snapshot = OverridesSnapshot::from_editable(&ui_state.dbc_windows[0].editable_data);
    ui_state.dbc_windows[0].push_undo(
        UndoOperationKind::DeleteMessage { message_id },
        &before_snapshot,
        &after_snapshot,
    );

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
