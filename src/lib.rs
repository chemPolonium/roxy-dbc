//! Library entry for roxy-dbc to allow integration tests and external usage.
pub mod app;
pub mod editable_dbc;
pub mod export;
pub mod fibex;
pub mod file_encoding;
pub mod import;
pub mod ui;
pub mod win_clipboard;

// Re-export commonly used types at crate root if desired
pub use crate::ui::UiState;
