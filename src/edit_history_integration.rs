//! Integration glue: implement `ApplyOp<EditableDbcData>` for `edit_history::Operation`
//! in a separate module to avoid circular module resolution issues.

use crate::dbc::{EditableDbcData, MessageOverride};
use crate::edit_history::ApplyOp;
use crate::edit_history::Operation;

impl ApplyOp<EditableDbcData> for Operation {
    fn apply(&self, data: &mut EditableDbcData, forward: bool) -> Result<(), String> {
        match self {
            Operation::RenameMessage {
                message_id,
                old,
                new,
            } => {
                if forward {
                    data.set_message_name(*message_id, new.clone());
                } else {
                    data.set_message_name(*message_id, old.clone());
                }
                Ok(())
            }
            Operation::ModifyMessageComment {
                message_id,
                old,
                new,
            } => {
                if forward {
                    data.set_message_comment(*message_id, new.clone());
                } else {
                    data.set_message_comment(*message_id, old.clone());
                }
                Ok(())
            }
            Operation::ModifyMessageSize {
                message_id,
                old,
                new,
            } => {
                if forward {
                    data.set_message_size(*message_id, *new);
                } else {
                    data.set_message_size(*message_id, *old);
                }
                Ok(())
            }
            Operation::ModifyMessageTransmitter {
                message_id,
                old,
                new,
            } => {
                if forward {
                    data.set_message_transmitter(*message_id, new.clone());
                } else {
                    data.set_message_transmitter(*message_id, old.clone());
                }
                Ok(())
            }
            Operation::ModifyMessageId {
                original_message_id,
                old_id,
                new_id,
            } => {
                if forward {
                    data.set_message_id(*original_message_id, *new_id);
                } else {
                    data.set_message_id(*original_message_id, *old_id);
                }
                Ok(())
            }
            Operation::AddMessage { message } => {
                let cm = MessageOverride {
                    message_id: message.message_id,
                    message_name: message.message_name.clone(),
                    message_size: message.message_size,
                    transmitter: message.transmitter.clone(),
                    comment: message.comment.clone(),
                    signals: message.signals.clone(),
                };
                if forward {
                    data.add_message(cm);
                } else {
                    data.delete_message(message.message_id);
                }
                Ok(())
            }
            Operation::DeleteMessage { message } => {
                let cm = MessageOverride {
                    message_id: message.message_id,
                    message_name: message.message_name.clone(),
                    message_size: message.message_size,
                    transmitter: message.transmitter.clone(),
                    comment: message.comment.clone(),
                    signals: message.signals.clone(),
                };
                if forward {
                    data.delete_message(message.message_id);
                } else {
                    data.add_message(cm);
                }
                Ok(())
            }
            Operation::Composite(ops) => {
                if forward {
                    for op in ops.iter() {
                        op.apply(data, true)?;
                    }
                } else {
                    for op in ops.iter().rev() {
                        op.apply(data, false)?;
                    }
                }
                Ok(())
            }
            Operation::ModifySignal {
                message_id,
                signal_name,
                old,
                new,
            } => {
                if forward {
                    data.signal_overrides
                        .insert((*message_id, signal_name.clone()), new.clone());
                } else {
                    match old {
                        Some(o) => {
                            data.signal_overrides
                                .insert((*message_id, signal_name.clone()), o.clone());
                        }
                        None => {
                            data.signal_overrides
                                .remove(&(*message_id, signal_name.clone()));
                        }
                    }
                }
                Ok(())
            }
        }
    }
}
