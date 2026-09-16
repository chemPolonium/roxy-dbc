//! DBC 文本文件的编码检测与转换
//!
//! CANdb++ / Vector 工具链在国内普遍输出 ANSI（GBK/cp936）编码的 DBC，
//! 而 UTF-8（可带 BOM）也常见。读取时嗅探 BOM -> 严格 UTF-8 -> GBK，
//! 并把检测到的编码记录在 DbcWindow 上，保存时按原编码写出，
//! 避免和其它工具交换文件时出现乱码。

use encoding_rs::{Encoding, GBK, UTF_16BE, UTF_16LE, UTF_8};

/// 一次解码的结果：文本、所用编码、是否带 BOM
#[derive(Clone)]
pub struct DecodedText {
    pub text: String,
    pub encoding: &'static Encoding,
    pub had_bom: bool,
}

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// 按顺序尝试 BOM、严格 UTF-8、GBK 解码
pub fn decode_file_bytes(bytes: &[u8]) -> DecodedText {
    if bytes.starts_with(UTF8_BOM) {
        let (text, _, _) = UTF_8.decode(&bytes[UTF8_BOM.len()..]);
        return DecodedText {
            text: text.into_owned(),
            encoding: UTF_8,
            had_bom: true,
        };
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (text, _, _) = UTF_16LE.decode(bytes);
        return DecodedText {
            text: text.into_owned(),
            encoding: UTF_8, // 罕见场景：解码后按 UTF-8 保存
            had_bom: false,
        };
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (text, _, _) = UTF_16BE.decode(bytes);
        return DecodedText {
            text: text.into_owned(),
            encoding: UTF_8,
            had_bom: false,
        };
    }

    match std::str::from_utf8(bytes) {
        Ok(s) => DecodedText {
            text: s.to_string(),
            encoding: UTF_8,
            had_bom: false,
        },
        // 不是合法 UTF-8：按国内工具链惯例视为 ANSI（GBK）
        Err(_) => {
            let (text, _, _) = GBK.decode(bytes);
            DecodedText {
                text: text.into_owned(),
                encoding: GBK,
                had_bom: false,
            }
        }
    }
}

/// 以指定编码编码文本（可选 UTF-8 BOM）；编码不了的字符替换为 U+FFFD
pub fn encode_to_bytes(text: &str, encoding: &'static Encoding, with_bom: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 3);
    if with_bom && encoding == UTF_8 {
        out.extend_from_slice(UTF8_BOM);
    }
    // 编码器永远不会失败，只做字符替换
    let (cow, _, _) = encoding.encode(text);
    out.extend_from_slice(&cow);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_with_bom_is_detected() {
        let mut bytes = UTF8_BOM.to_vec();
        bytes.extend_from_slice("CM_ BO_ 1 \"中文\";".as_bytes());
        let decoded = decode_file_bytes(&bytes);
        assert_eq!(decoded.encoding, UTF_8);
        assert!(decoded.had_bom);
        assert!(decoded.text.contains("中文"));
    }

    #[test]
    fn plain_utf8_is_detected() {
        let decoded = decode_file_bytes("BO_ 1 Msg: 8 Vector__XXX".as_bytes());
        assert_eq!(decoded.encoding, UTF_8);
        assert!(!decoded.had_bom);
    }

    #[test]
    fn gbk_is_detected_and_converted() {
        // 用 GBK 编码器生成真实 GBK 字节（模拟 CANdb++ ANSI 输出）
        let (bytes, _, _) = GBK.encode("CM_ BO_ 1 \"测试消息1\";\n");
        let decoded = decode_file_bytes(&bytes);
        assert_eq!(decoded.encoding, GBK);
        assert!(decoded.text.contains("测试消息1"));

        // 再编码回 GBK 应当还原
        let re = encode_to_bytes(&decoded.text, decoded.encoding, false);
        assert_eq!(re, bytes.into_owned());
    }

    #[test]
    fn utf8_bom_is_restored_on_save() {
        let text = "CM_ BO_ 1 \"中文\";";
        let out = encode_to_bytes(text, UTF_8, true);
        assert!(out.starts_with(UTF8_BOM));
        assert_eq!(&out[3..], text.as_bytes());
    }

    #[test]
    fn utf16le_bom_is_handled() {
        let mut bytes = vec![0xFF, 0xFE];
        let units: Vec<u8> = "中文"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        bytes.extend_from_slice(&units);
        let decoded = decode_file_bytes(&bytes);
        assert!(decoded.text.contains("中文"));
    }
}
