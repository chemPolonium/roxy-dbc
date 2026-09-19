pub mod arxml;
pub mod fibex;

use crate::fibex::editable_fibex::EditableFibex;
use crate::file_encoding::decode_file_bytes;
use encoding_rs::Encoding;
use std::path::Path;

/// 读取并解析 FIBEX / AUTOSAR ARXML。
/// 编码自动识别（UTF-8 BOM → 严格 UTF-8 → GBK），并把检测到的编码随模型返回，
/// Save时按原编码写出。
pub fn import_file(path: &Path) -> Result<(EditableFibex, &'static Encoding, bool), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let decoded = decode_file_bytes(&bytes);
    let fibex = parse_content(&decoded.text)?;
    Ok((fibex, decoded.encoding, decoded.had_bom))
}

/// 根据内容自动识别 FIBEX 或 AUTOSAR ARXML
pub fn parse_content(content: &str) -> Result<EditableFibex, String> {
    let doc = roxmltree::Document::parse(content).map_err(|e| format!("XML parse error: {}", e))?;
    let root = doc.root_element();

    // 通过根元素名称识别格式（FIBEX 文件扩展名可能是 .fibex/.fx/.xml）
    let root_name = root.tag_name().name().to_uppercase();
    if root_name.contains("FIBEX") {
        fibex::parse_fibex_doc(&doc)
    } else if root_name.contains("AUTOSAR") {
        arxml::parse_arxml_doc(&doc)
    } else {
        // 尝试在文档内查找特征元素
        let mut has_fibex = false;
        let mut has_autosar = false;
        for node in doc.descendants() {
            if !node.is_element() {
                continue;
            }
            let name = node.tag_name().name().to_uppercase();
            if name == "FLEXRAY-CLUSTER" || name == "I-SIGNAL-I-PDU" || name == "ECU-INSTANCE" {
                has_autosar = true;
                break;
            }
            if name == "CLUSTER" || name == "SLOT" {
                has_fibex = true;
            }
        }
        if has_autosar {
            arxml::parse_arxml_doc(&doc)
        } else if has_fibex {
            fibex::parse_fibex_doc(&doc)
        } else {
            Err("Unrecognized XML format: neither FIBEX nor AUTOSAR".to_string())
        }
    }
}

/// 递归收集所有指定标签名的元素（忽略命名空间）
pub fn collect_elements_by_tag<'a, 'b>(
    node: &roxmltree::Node<'a, 'b>,
    tag: &str,
    results: &mut Vec<roxmltree::Node<'a, 'b>>,
) {
    if node.is_element() && node.tag_name().name() == tag {
        results.push(*node);
    }
    for child in node.children() {
        collect_elements_by_tag(&child, tag, results);
    }
}

/// 取元素的 SHORT-NAME 文本
pub fn get_short_name(node: &roxmltree::Node) -> String {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == "SHORT-NAME")
        .and_then(|n| n.text())
        .unwrap_or("Unnamed")
        .trim()
        .to_string()
}

/// 在元素子树内按候选标签列表查找第一个非空文本（忽略命名空间，容忍不同方言）
pub fn find_text_candidates(node: &roxmltree::Node, tags: &[&str]) -> Option<String> {
    for tag in tags {
        let mut found = Vec::new();
        collect_elements_by_tag(node, tag, &mut found);
        for f in found {
            if let Some(t) = f.text() {
                let t = t.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
    }
    None
}

/// 解析整数文本（支持 10/16 进制）
pub fn parse_u32(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u32>().ok()
    }
}

pub fn parse_f64(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok()
}

/// 解析 AUTOSAR Cycle Repetition枚举（CYCLE-REPETITION-4 / FR_CYCLE_REPETITION_4 / 4）
pub fn parse_cycle_repetition(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(v) = parse_u32(s) {
        return Some(v);
    }
    let upper = s.to_uppercase();
    let stripped = upper
        .trim_start_matches("FR_CYCLE_REPETITION")
        .trim_start_matches("CYCLE-REPETITION")
        .trim_start_matches(['-', '_']);
    stripped.parse::<u32>().ok()
}

/// 解析 AUTOSAR 引用路径（/Pkg/Sub/Name -> Name）
pub fn ref_short_name(s: &str) -> &str {
    s.rsplit('/').next().unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fibex::editable_fibex::{ByteOrder, FrChannel};

    const SAMPLE_FIBEX: &str = include_str!("../../../fibex-sample/powertrain.fibex");
    const SAMPLE_ARXML: &str = include_str!("../../../fibex-sample/chassis.arxml");

    #[test]
    fn parse_sample_fibex() {
        let fibex = parse_content(SAMPLE_FIBEX).expect("sample FIBEX should parse");

        assert_eq!(fibex.cluster().name, "PowertrainCluster");
        assert_eq!(fibex.cluster().params.speed_kbps, 10000);
        assert_eq!(fibex.cluster().params.cycle_time_ms, 5.0);
        assert_eq!(fibex.cluster().params.number_of_static_slots, 10);
        // MACROTICK-DURATION 0.005 ms -> 5 us
        assert_eq!(fibex.cluster().params.macrotick_duration_us, 5.0);

        assert_eq!(fibex.ecus().len(), 4);
        assert!(fibex.ecus().contains(&"EngECU".to_string()));

        assert_eq!(fibex.frames().len(), 4);
        assert_eq!(fibex.pdus().len(), 4);

        // EngineData 在 A/B 双通道 slot 1，重复周期 1
        let engine = fibex.get_frame("EngineData").unwrap();
        assert_eq!(engine.triggering().slot_id, 1);
        assert_eq!(engine.triggering().channel, FrChannel::Both);
        assert_eq!(engine.triggering().cycle_repetition, 1);
        assert_eq!(engine.pdus().len(), 1);
        assert_eq!(engine.comment(), "Engine status, transmitted every cycle");

        // TransmissionData 仅 A 通道；ChassisStatus 仅 B 通道
        let trans = fibex.get_frame("TransmissionData").unwrap();
        assert_eq!(trans.triggering().channel, FrChannel::A);
        assert_eq!(trans.triggering().slot_id, 2);
        let chassis = fibex.get_frame("ChassisStatus").unwrap();
        assert_eq!(chassis.triggering().channel, FrChannel::B);
        assert_eq!(chassis.triggering().slot_id, 3);
        assert_eq!(chassis.triggering().cycle_repetition, 4);

        // DashInfo 复用 Eng_PDU 且带两个映射
        let dash = fibex.get_frame("DashInfo").unwrap();
        assert_eq!(dash.pdus().len(), 2);
        assert_eq!(dash.pdus()[1].pdu_name, "Dash_PDU");
        assert_eq!(dash.pdus()[1].start_position, 4);

        // 信号属性（factor/offset/unit/值描述）从 CODING 解析
        let eng_pdu = fibex.get_pdu("Eng_PDU").unwrap();
        let speed = eng_pdu
            .signals()
            .iter()
            .find(|s| s.name() == "EngineSpeed")
            .unwrap();
        assert_eq!(speed.length_bits(), 16);
        assert_eq!(speed.byte_order(), ByteOrder::BigEndian);
        assert_eq!(speed.factor(), 0.25);
        assert_eq!(speed.unit(), "rpm");
        assert_eq!(speed.comment(), "Engine speed measured at crankshaft");

        let temp = eng_pdu
            .signals()
            .iter()
            .find(|s| s.name() == "CoolantTemp")
            .unwrap();
        assert_eq!(temp.offset(), -40.0);
        assert_eq!(temp.min(), -40.0);
        assert_eq!(temp.max(), 215.0);
        assert_eq!(temp.unit(), "DegC");

        let chassis_pdu = fibex.get_pdu("Chassis_PDU").unwrap();
        let status = chassis_pdu
            .signals()
            .iter()
            .find(|s| s.name() == "ChassisStatus")
            .unwrap();
        assert_eq!(
            status.value_descriptions(),
            &[
                (0, "OK".to_string()),
                (1, "Warning".to_string()),
                (2, "Error".to_string())
            ]
        );
    }

    #[test]
    fn parse_sample_arxml() {
        let fibex = parse_content(SAMPLE_ARXML).expect("sample ARXML should parse");

        assert_eq!(fibex.cluster().name, "PowertrainCluster");
        // FLEXRAY-CYCLE 1000 mt * 5 us = 5 ms
        assert_eq!(fibex.cluster().params.cycle_time_ms, 5.0);
        assert_eq!(fibex.cluster().params.macrotick_duration_us, 5.0);

        assert_eq!(fibex.ecus().len(), 4);
        assert_eq!(fibex.frames().len(), 4);
        assert_eq!(fibex.pdus().len(), 4);

        // EngineData 双通道（A+B）slot 1，startup
        let engine = fibex.get_frame("EngineData").unwrap();
        assert_eq!(engine.triggering().slot_id, 1);
        assert_eq!(engine.triggering().channel, FrChannel::Both);
        assert_eq!(engine.triggering().cycle_repetition, 1);
        assert!(engine.triggering().startup);

        // ChassisStatus 只在 B 通道，重复周期 4
        let chassis = fibex.get_frame("ChassisStatus").unwrap();
        assert_eq!(chassis.triggering().channel, FrChannel::B);
        assert_eq!(chassis.triggering().cycle_repetition, 4);
        assert_eq!(chassis.length(), 12);

        let eng_pdu = fibex.get_pdu("Eng_PDU").unwrap();
        assert_eq!(eng_pdu.signals().len(), 3);
        let speed = eng_pdu
            .signals()
            .iter()
            .find(|s| s.name() == "EngineSpeed")
            .unwrap();
        assert_eq!(speed.start_bit(), 0);
        assert_eq!(speed.length_bits(), 16);
        assert_eq!(speed.byte_order(), ByteOrder::BigEndian);
        assert_eq!(speed.comment(), "Engine speed measured at crankshaft");
    }
}
