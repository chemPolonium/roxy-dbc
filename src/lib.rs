//! Library entry for roxy-dbc to allow integration tests and external usage.
pub mod app;
pub mod editable_dbc;
pub mod ui;

// Re-export commonly used types at crate root if desired
pub use crate::ui::UiState;
