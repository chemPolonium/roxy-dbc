pub mod arxml;
pub mod kcd;

use crate::editable_dbc::EditableDbc;
use crate::file_encoding::decode_file_bytes;
use std::path::Path;

pub fn import_file(path: &Path) -> Result<EditableDbc, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    // 与 DBC 一致地嗅探编码：UTF-8 之外按 GBK 处理（国内工具链导出的 XML 也可能是 ANSI）
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let content = decode_file_bytes(&bytes).text;
    match ext.as_str() {
        "kcd" => kcd::parse_kcd(&content),
        "arxml" => arxml::parse_arxml(&content),
        _ => Err(format!("Unsupported format: {}", ext)),
    }
}
