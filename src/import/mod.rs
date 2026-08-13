pub mod arxml;
pub mod kcd;

use crate::editable_dbc::EditableDbc;
use std::path::Path;

pub fn import_file(path: &Path) -> Result<EditableDbc, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    match ext.as_str() {
        "kcd" => kcd::parse_kcd(&content),
        "arxml" => arxml::parse_arxml(&content),
        _ => Err(format!("Unsupported format: {}", ext)),
    }
}
