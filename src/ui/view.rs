//! UI view-layer for messages and signals (decoupled from can_dbc types)
use crate::dbc::{ByteOrder, EditableDbcData, MessageRef, SignalOverride, ValueType};

/// A lightweight, UI-friendly representation of a Signal.
#[derive(Clone, Debug)]
pub struct SignalView {
    pub name: String,
    pub start_bit: u64,
    pub signal_size: u64,
    pub byte_order: ByteOrder,
    pub value_type: ValueType,
    pub factor: f64,
    pub offset: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub unit: String,
    pub comment: String,
    // (removed) overridden flag was unused at runtime; keep views minimal
}

/// A lightweight representation of a Message for UI rendering.
#[derive(Clone, Debug)]
pub struct MessageView {
    pub message_id: u32,
    pub message_name: String,
    pub signals: Vec<SignalView>,
}

impl SignalView {
    pub fn from_original(sig: &can_dbc::Signal) -> Self {
        Self {
            name: sig.name().to_string(),
            start_bit: *sig.start_bit(),
            signal_size: *sig.signal_size(),
            byte_order: *sig.byte_order(),
            value_type: *sig.value_type(),
            factor: *sig.factor(),
            offset: *sig.offset(),
            minimum: *sig.min(),
            maximum: *sig.max(),
            unit: sig.unit().to_string(),
            comment: String::new(),
        }
    }

    pub fn from_override(name: String, ov: &SignalOverride) -> Self {
        Self {
            name,
            start_bit: ov.start_bit,
            signal_size: ov.signal_size,
            byte_order: ov.byte_order,
            value_type: ov.value_type,
            factor: ov.factor,
            offset: ov.offset,
            minimum: ov.minimum,
            maximum: ov.maximum,
            unit: ov.unit.clone(),
            comment: ov.comment.clone(),
        }
    }
}

impl MessageView {
    /// Build a MessageView from a MessageRef and the editable overlay.
    pub fn from_message_ref(msg_ref: &MessageRef, editable: &EditableDbcData) -> Self {
        let message_id = msg_ref.message_id();
        let message_name = msg_ref.message_name().to_string();
        let _message_size = editable.get_message_size(message_id, msg_ref.message_size());

        let mut signals = Vec::new();

        for sig in msg_ref.signals().iter() {
            // if override exists, use it
            if let Some(ov) = editable
                .signal_overrides
                .get(&(message_id, sig.name().to_string()))
            {
                signals.push(SignalView::from_override(sig.name().to_string(), ov));
            } else {
                signals.push(SignalView::from_original(sig));
            }
        }

        // If this is a MessageOverride (added messages), its signals are already in msg_ref.signals()

        MessageView {
            message_id,
            message_name,
            signals,
        }
    }

    // Compatibility accessors to reduce call-site changes
    pub fn message_id(&self) -> u32 {
        self.message_id
    }

    pub fn message_name(&self) -> &str {
        &self.message_name
    }

    // Note: keep fields public for direct access; removed unused accessors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dbc::EditableDbcData;
    use std::path::PathBuf;

    #[test]
    fn message_view_from_sample() {
        // Load a sample DBC from the repository's dbc-sample directory
        let mut editable = EditableDbcData::new();
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let dbc_path = manifest.join("dbc-sample").join("can_data_ev1.dbc");
        editable
            .load_dbc_file(&dbc_path)
            .expect("failed to load sample dbc");

        let msgs = editable.get_all_messages();
        assert!(!msgs.is_empty(), "no messages found in sample dbc");

        let msg_ref = &msgs[0];
        let view = MessageView::from_message_ref(msg_ref, &editable);
        assert!(!view.message_name.is_empty());
        assert!(view.signals.len() > 0, "message should contain signals");
    }
}
