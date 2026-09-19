//! FIBEX / ARXML 解析集成测试
//!
//! 用仓库内的样本文件验证解析结果与 Vector CANoe 中的内容一致：
//! PowerTrain.arxml 应解析出 48 个帧、24 个 PDU、非空的信号与 ECU 列表。

use crate::fibex::import::parse_content;

const POWERTRAIN_ARXML: &str = include_str!("../../fibex-sample/PowerTrain.arxml");
const DEMO_FIBEX_XML: &str = include_str!("../../fibex-sample/DemoFile_v3_FIBEX_3_0.xml");

#[test]
fn powertrain_arxml_parses_frames_pdus_signals() {
    let fibex = parse_content(POWERTRAIN_ARXML).expect("PowerTrain.arxml must parse");

    assert_eq!(fibex.frames().len(), 48, "48 frames expected");
    assert_eq!(fibex.pdus().len(), 24, "24 PDUs expected");
    assert!(
        !fibex.ecus().is_empty(),
        "ECU list should not be empty (Tester/Engine/BSC/BLU/GearBox/Dashboard...)"
    );

    // 信号总数：CANoe 中该集群有 38 个信号分布在各 PDU 中
    let signal_count: usize = fibex.pdus().iter().map(|p| p.signals().len()).sum();
    assert!(signal_count > 0, "PDU signal mappings must not be empty");

    // 抽查一个帧的时隙 / 周期信息（CANoe 帧页可见 Frame_13_0_2 slot=13, repetition=2）
    let frame = fibex
        .frames()
        .iter()
        .find(|f| f.name == "Frame_13_0_2")
        .expect("Frame_13_0_2 must exist");
    assert_eq!(frame.triggering().slot_id, 13);
    assert_eq!(frame.triggering().cycle_repetition, 2);

    // 抽查 PDU：ABSInfo 长度 8
    let pdu = fibex.pdus().iter().find(|p| p.name == "ABSInfo").unwrap();
    assert_eq!(pdu.length(), 8);
}

#[test]
fn fibex_xml_sample_parses() {
    let fibex = parse_content(DEMO_FIBEX_XML).expect("FIBEX 3.0 demo must parse");
    assert!(
        !fibex.frames().is_empty() || !fibex.pdus().is_empty(),
        "FIBEX demo should contain frames or PDUs"
    );
}
