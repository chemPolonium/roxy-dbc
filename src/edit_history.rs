//! Lightweight edit history manager using a single op sequence + cursor.
//!
//! This module provides a minimal `Operation` enum and `History` struct
//! implementing single-sequence undo/redo semantics suitable for adapting
//! into EditableDbcData. It's self-contained and tested with a small fake
//! state in unit tests.

use crate::dbc::MessageOverride;
use std::fmt;

// Operations store full `MessageOverride` values from `crate::dbc` so that
// history doesn't define its own snapshot type. Use `crate::dbc::MessageOverride`.

/// A lightweight signal representation for potential future use.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub struct SignalData {
    pub name: String,
    pub start_bit: u64,
    pub size: u64,
}

/// Operation enum: each variant contains enough data to undo and redo.
#[derive(Clone, Debug)]
pub enum Operation {
    RenameMessage {
        message_id: u32,
        old: String,
        new: String,
    },
    ModifyMessageId {
        original_message_id: u32,
        old_id: u32,
        new_id: u32,
    },
    ModifyMessageComment {
        message_id: u32,
        old: String,
        new: String,
    },
    ModifyMessageSize {
        message_id: u32,
        old: u8,
        new: u8,
    },
    ModifyMessageTransmitter {
        message_id: u32,
        old: String,
        new: String,
    },
    /// Modify a signal override on a message. `old` is optional (None means remove previous override)
    ModifySignal {
        message_id: u32,
        signal_name: String,
        old: Option<crate::dbc::SignalOverride>,
        new: crate::dbc::SignalOverride,
    },
    AddMessage {
        message: MessageOverride,
    },
    DeleteMessage {
        message: MessageOverride,
    },
    Composite(Vec<Operation>),
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operation::RenameMessage {
                message_id,
                old,
                new,
            } => {
                write!(f, "RenameMessage {}: '{}' -> '{}'", message_id, old, new)
            }
            Operation::ModifyMessageId {
                original_message_id,
                old_id,
                new_id,
            } => {
                write!(
                    f,
                    "ModifyMessageId {}: 0x{:X} -> 0x{:X}",
                    original_message_id, old_id, new_id
                )
            }
            Operation::ModifyMessageComment {
                message_id,
                old,
                new,
            } => write!(
                f,
                "ModifyMessageComment {}: '{}' -> '{}'",
                message_id, old, new
            ),
            Operation::ModifyMessageSize {
                message_id,
                old,
                new,
            } => write!(f, "ModifyMessageSize {}: {} -> {}", message_id, old, new),
            Operation::ModifyMessageTransmitter {
                message_id,
                old,
                new,
            } => write!(
                f,
                "ModifyMessageTransmitter {}: '{}' -> '{}'",
                message_id, old, new
            ),
            Operation::AddMessage { message } => {
                write!(
                    f,
                    "AddMessage {} ('{}')",
                    message.message_id, message.message_name
                )
            }
            Operation::DeleteMessage { message } => {
                write!(
                    f,
                    "DeleteMessage {} ('{}')",
                    message.message_id, message.message_name
                )
            }
            Operation::ModifySignal {
                message_id,
                signal_name,
                old: _,
                new: _,
            } => write!(f, "ModifySignal {}:{}", message_id, signal_name),
            Operation::Composite(ops) => write!(f, "Composite({} ops)", ops.len()),
        }
    }
}

/// A trait describing how to apply/undo a single Operation to a state.
/// This keeps the history manager generic and testable.
pub trait ApplyOp<S> {
    fn apply(&self, state: &mut S, forward: bool) -> Result<(), String>;
}

/// History manager that stores a single sequence of operations and a cursor.
#[derive(Debug, Default, Clone)]
pub struct History {
    ops: Vec<Operation>,
    cursor: usize, // 0..=ops.len(), points to next redo slot
    pub max_len: usize,
}

impl History {
    pub fn new(max_len: usize) -> Self {
        Self {
            ops: Vec::new(),
            cursor: 0,
            max_len,
        }
    }

    /// Apply a new operation: truncate tail if cursor not at end, push op,
    /// apply to state (via S implementation) and advance cursor.
    pub fn apply_new<S>(&mut self, op: Operation, state: &mut S) -> Result<(), String>
    where
        Operation: ApplyOp<S>,
    {
        // truncate any redo ops
        if self.cursor < self.ops.len() {
            self.ops.truncate(self.cursor);
        }

        // push
        self.ops.push(op.clone());

        // apply forward
        op.apply(state, true)?;

        // advance cursor
        self.cursor = self.ops.len();

        // enforce max length: drop oldest if over limit
        if self.ops.len() > self.max_len {
            let overflow = self.ops.len() - self.max_len;
            self.ops.drain(0..overflow);
            // adjust cursor accordingly
            if self.cursor >= overflow {
                self.cursor -= overflow;
            } else {
                self.cursor = 0;
            }
        }

        Ok(())
    }

    /// Undo one step (apply old values)
    pub fn undo<S>(&mut self, state: &mut S) -> Result<(), String>
    where
        Operation: ApplyOp<S>,
    {
        if self.cursor == 0 {
            return Err("Nothing to undo".into());
        }
        // move cursor back to the operation to undo
        self.cursor -= 1;
        let op = &self.ops[self.cursor];
        op.apply(state, false)
    }

    /// Redo one step
    pub fn redo<S>(&mut self, state: &mut S) -> Result<(), String>
    where
        Operation: ApplyOp<S>,
    {
        if self.cursor >= self.ops.len() {
            return Err("Nothing to redo".into());
        }
        let op = &self.ops[self.cursor];
        op.apply(state, true)?;
        self.cursor += 1;
        Ok(())
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor < self.ops.len()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// Description for the operation that would be undone next (if any)
    pub fn last_undo_description(&self) -> Option<String> {
        if self.cursor == 0 {
            None
        } else {
            Some(self.ops[self.cursor - 1].to_string())
        }
    }

    /// Description for the operation that would be redone next (if any)
    pub fn last_redo_description(&self) -> Option<String> {
        if self.cursor >= self.ops.len() {
            None
        } else {
            Some(self.ops[self.cursor].to_string())
        }
    }
}

// --------------------------------------------------------------------------------
// Minimal test state implementation and unit tests
// --------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// A tiny fake state representing set of messages, keyed by id.
    #[derive(Debug, Default)]
    struct TestState {
        msgs: BTreeMap<u32, MessageOverride>,
        signal_overrides: BTreeMap<(u32, String), SignalData>,
    }

    impl TestState {
        fn new() -> Self {
            Self {
                msgs: BTreeMap::new(),
                signal_overrides: BTreeMap::new(),
            }
        }
    }

    impl ApplyOp<TestState> for Operation {
        fn apply(&self, state: &mut TestState, forward: bool) -> Result<(), String> {
            match self {
                Operation::RenameMessage {
                    message_id,
                    old,
                    new,
                } => {
                    let id = *message_id;
                    if let Some(msg) = state.msgs.get_mut(&id) {
                        if forward {
                            // expect current equals old? not enforced in tests
                            msg.message_name = new.clone();
                        } else {
                            msg.message_name = old.clone();
                        }
                        Ok(())
                    } else {
                        Err(format!("Message {} not found", id))
                    }
                }
                Operation::ModifyMessageComment {
                    message_id,
                    old,
                    new,
                } => {
                    let id = *message_id;
                    if let Some(msg) = state.msgs.get_mut(&id) {
                        if forward {
                            msg.comment = Some(new.clone());
                        } else {
                            msg.comment = Some(old.clone());
                        }
                        Ok(())
                    } else {
                        Err(format!("Message {} not found", id))
                    }
                }
                Operation::ModifyMessageId {
                    original_message_id: _,
                    old_id,
                    new_id,
                } => {
                    // change key from old_id to new_id
                    if forward {
                        if let Some(mut msg) = state.msgs.remove(old_id) {
                            msg.message_id = *new_id;
                            state.msgs.insert(*new_id, msg);
                            Ok(())
                        } else {
                            Err(format!("Message 0x{:X} not found for id change", old_id))
                        }
                    } else {
                        if let Some(mut msg) = state.msgs.remove(new_id) {
                            msg.message_id = *old_id;
                            state.msgs.insert(*old_id, msg);
                            Ok(())
                        } else {
                            Err(format!("Message 0x{:X} not found for id revert", new_id))
                        }
                    }
                }
                Operation::ModifyMessageSize {
                    message_id,
                    old,
                    new,
                } => {
                    let id = *message_id;
                    if let Some(msg) = state.msgs.get_mut(&id) {
                        if forward {
                            msg.message_size = *new;
                        } else {
                            msg.message_size = *old;
                        }
                        Ok(())
                    } else {
                        Err(format!("Message {} not found", id))
                    }
                }
                Operation::ModifyMessageTransmitter {
                    message_id,
                    old,
                    new,
                } => {
                    let id = *message_id;
                    if let Some(msg) = state.msgs.get_mut(&id) {
                        if forward {
                            msg.transmitter = Some(new.clone());
                        } else {
                            msg.transmitter = Some(old.clone());
                        }
                        Ok(())
                    } else {
                        Err(format!("Message {} not found", id))
                    }
                }
                Operation::AddMessage { message } => {
                    let id = message.message_id;
                    if forward {
                        state.msgs.insert(id, message.clone());
                        Ok(())
                    } else {
                        state.msgs.remove(&id);
                        Ok(())
                    }
                }
                Operation::DeleteMessage { message } => {
                    let id = message.message_id;
                    if forward {
                        // forward means apply delete
                        if state.msgs.remove(&id).is_some() {
                            Ok(())
                        } else {
                            Err(format!("Message {} not found for delete", id))
                        }
                    } else {
                        // revert delete -> insert
                        state.msgs.insert(id, message.clone());
                        Ok(())
                    }
                }
                Operation::Composite(ops) => {
                    if forward {
                        for op in ops.iter() {
                            op.apply(state, true)?;
                        }
                    } else {
                        for op in ops.iter().rev() {
                            op.apply(state, false)?;
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
                    let key = (*message_id, signal_name.clone());
                    if forward {
                        state.signal_overrides.insert(
                            key,
                            SignalData {
                                name: new.name.clone(),
                                start_bit: new.start_bit,
                                size: new.signal_size,
                            },
                        );
                        Ok(())
                    } else {
                        match old {
                            Some(o) => {
                                state.signal_overrides.insert(
                                    key,
                                    SignalData {
                                        name: o.name.clone(),
                                        start_bit: o.start_bit,
                                        size: o.signal_size,
                                    },
                                );
                                Ok(())
                            }
                            None => {
                                state.signal_overrides.remove(&key);
                                Ok(())
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn add_rename_undo_redo() {
        let mut state = TestState::new();
        let mut history = History::new(100);

        // start empty, add message
        let msg = MessageOverride {
            message_id: 1,
            message_name: "Msg1".into(),
            comment: Some("c".into()),
            message_size: 8,
            transmitter: None,
            signals: vec![],
        };
        let add = Operation::AddMessage {
            message: msg.clone(),
        };
        history.apply_new(add, &mut state).unwrap();
        assert!(state.msgs.contains_key(&1));
        assert!(history.can_undo());
        assert!(!history.can_redo());

        // rename
        let op = Operation::RenameMessage {
            message_id: 1,
            old: "Msg1".into(),
            new: "X".into(),
        };
        history.apply_new(op, &mut state).unwrap();
        assert_eq!(state.msgs.get(&1).unwrap().message_name, "X");

        // undo rename
        history.undo(&mut state).unwrap();
        assert_eq!(state.msgs.get(&1).unwrap().message_name, "Msg1");
        assert!(history.can_redo());

        // redo rename
        history.redo(&mut state).unwrap();
        assert_eq!(state.msgs.get(&1).unwrap().message_name, "X");

        // undo rename, undo add => msg removed
        history.undo(&mut state).unwrap(); // undo rename
        history.undo(&mut state).unwrap(); // undo add
        assert!(!state.msgs.contains_key(&1));

        // redo add, redo rename
        history.redo(&mut state).unwrap();
        history.redo(&mut state).unwrap();
        assert!(state.msgs.contains_key(&1));
        assert_eq!(state.msgs.get(&1).unwrap().message_name, "X");
    }

    #[test]
    fn delete_undo_redo() {
        let mut state = TestState::new();
        let mut history = History::new(100);

        let msg = MessageOverride {
            message_id: 2,
            message_name: "Msg1".into(),
            comment: Some("c".into()),
            message_size: 8,
            transmitter: None,
            signals: vec![],
        };
        state.msgs.insert(2, msg.clone());

        let del = Operation::DeleteMessage {
            message: msg.clone(),
        };
        history.apply_new(del, &mut state).unwrap();
        assert!(!state.msgs.contains_key(&2));

        // undo delete
        history.undo(&mut state).unwrap();
        assert!(state.msgs.contains_key(&2));

        // redo delete
        history.redo(&mut state).unwrap();
        assert!(!state.msgs.contains_key(&2));
    }

    #[test]
    fn composite_operation() {
        let mut state = TestState::new();
        let mut history = History::new(100);

        let m1 = MessageOverride {
            message_id: 10,
            message_name: "A".into(),
            comment: Some("a".into()),
            message_size: 8,
            transmitter: None,
            signals: vec![],
        };
        let m2 = MessageOverride {
            message_id: 11,
            message_name: "B".into(),
            comment: Some("b".into()),
            message_size: 8,
            transmitter: None,
            signals: vec![],
        };

        let op = Operation::Composite(vec![
            Operation::AddMessage {
                message: m1.clone(),
            },
            Operation::AddMessage {
                message: m2.clone(),
            },
        ]);

        history.apply_new(op, &mut state).unwrap();
        assert!(state.msgs.contains_key(&10));
        assert!(state.msgs.contains_key(&11));

        history.undo(&mut state).unwrap();
        assert!(!state.msgs.contains_key(&10));
        assert!(!state.msgs.contains_key(&11));

        history.redo(&mut state).unwrap();
        assert!(state.msgs.contains_key(&10));
        assert!(state.msgs.contains_key(&11));
    }
}
