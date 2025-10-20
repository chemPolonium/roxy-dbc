use roxy_dbc::dbc::MessageOverride;
use roxy_dbc::edit_history::Operation;
use roxy_dbc::ui::state::{DbcWindowState, UiState};

fn setup_state_with_message() -> (UiState, u32) {
    // Create editable data
    let editable = roxy_dbc::dbc::EditableDbcData::new();

    // Build UiState and add a DbcWindowState
    let mut ui_state = UiState::default();
    let mut window = DbcWindowState::new(0, editable);

    // Construct a simple custom message and add it
    let msg = MessageOverride {
        message_id: 0x100,
        message_name: "TestMsg".to_string(),
        message_size: 8,
        transmitter: None,
        comment: None,
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

    // Simulate deletion via history Operation
    // Build a MessageOverride from existing data
    let cm = ui_state.dbc_windows[0]
        .editable_data
        .get_message_ref_by_id(message_id)
        .map(|m| m.to_message_override())
        .unwrap_or_else(|| MessageOverride {
            message_id,
            message_name: "Deleted".to_string(),
            message_size: 8,
            transmitter: None,
            comment: None,
            signals: vec![],
        });
    // Apply delete via history using full MessageOverride
    let window = &mut ui_state.dbc_windows[0];
    window
        .history
        .apply_new(
            Operation::DeleteMessage {
                message: cm.clone(),
            },
            &mut window.editable_data,
        )
        .expect("apply delete");

    // After delete, message should be gone (use `window` to avoid borrow issues)
    assert!(
        !window
            .editable_data
            .get_all_messages()
            .iter()
            .any(|m| m.message_id() == message_id)
    );

    // History should allow undo
    assert!(window.history.can_undo());

    // Perform undo via history
    window
        .history
        .undo(&mut window.editable_data)
        .expect("undo");

    // After undo, message should be present again
    assert!(
        window
            .editable_data
            .get_all_messages()
            .iter()
            .any(|m| m.message_id() == message_id)
    );
}
