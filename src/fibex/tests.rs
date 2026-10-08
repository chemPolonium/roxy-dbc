//! FIBEX / ARXML 解析集成测试
//!
//! 用仓库内的样本文件验证解析结果与 Vector CANoe 中的内容一致：
//! PowerTrain.arxml 应解析出 48 个帧、24 个 PDU、非空的信号与 ECU 列表。

use crate::fibex::editable_fibex::{
    EditableFibex, EditableSignal, FrChannel, FrameTriggering, PduKind, ValueType,
};
use crate::fibex::import::parse_content;

const POWERTRAIN_ARXML: &str = include_str!("../../fibex-sample/PowerTrain.arxml");
const CHASSIS_ARXML: &str = include_str!("../../fibex-sample/chassis.arxml");
const DEMO_FIBEX_XML: &str = include_str!("../../fibex-sample/DemoFile_v3_FIBEX_3_0.xml");

#[test]
fn powertrain_arxml_parses_frames_pdus_signals() {
    let fibex = parse_content(POWERTRAIN_ARXML).expect("PowerTrain.arxml must parse");

    assert_eq!(fibex.frames().len(), 48, "48 frames expected");
    // 24 I-SIGNAL-I-PDU + 24 N-PDU（TP）+ 6 NM-PDU，全部都会被帧引用
    assert_eq!(fibex.pdus().len(), 54, "54 PDUs expected");
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
    assert_eq!(frame.channel_triggering(FrChannel::A).unwrap().slot_id, 13);
    assert_eq!(
        frame
            .channel_triggering(FrChannel::A)
            .unwrap()
            .cycle_repetition,
        2
    );

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

/// 帧内 PDU 起始位统计（用于往返比较）
fn fingerprint(f: &EditableFibex) -> (usize, usize, usize, usize, usize, usize, usize) {
    let signals: Vec<&EditableSignal> = f.pdus().iter().flat_map(|p| p.signals()).collect();
    (
        f.frames().len(),
        f.pdus().len(),
        signals.len(),
        signals
            .iter()
            .filter(|s| s.factor() != 1.0 || s.offset() != 0.0)
            .count(),
        signals
            .iter()
            .filter(|s| !s.value_descriptions().is_empty())
            .count(),
        signals
            .iter()
            .filter(|s| s.value_type() == ValueType::Signed)
            .count(),
        f.frames().iter().map(|fr| fr.pdus().len()).sum(),
    )
}

/// AUTOSAR 里帧内 PDU 的 START-POSITION 以位计，模型按字节存：
/// Frame_76_0_1 映射 EngineDev4(bit 7)/EngineDev5(bit 71) → 字节 0/8
#[test]
fn powertrain_arxml_pdu_mappings_use_byte_positions() {
    let f = parse_content(POWERTRAIN_ARXML).expect("parse");
    let frame = f
        .frames()
        .iter()
        .find(|fr| fr.name == "Frame_76_0_1")
        .expect("frame");
    let starts: Vec<(String, u32)> = frame
        .pdus()
        .iter()
        .map(|m| (m.pdu_name.clone(), m.start_position))
        .collect();
    assert_eq!(
        starts,
        vec![("EngineDev4".to_string(), 0), ("EngineDev5".to_string(), 8),]
    );
    // 24 I-SIGNAL-I-PDU + 24 N-PDU(TP) + 6 NM-PDU：帧引用的 PDU 必须都能在列表里找到
    assert_eq!(
        f.pdus().len(),
        54,
        "N-PDU / NM-PDU must be imported, not left dangling"
    );
    for frame in f.frames() {
        for m in frame.pdus() {
            assert!(
                f.get_pdu(&m.pdu_name).is_some(),
                "dangling PDU ref {}",
                m.pdu_name
            );
        }
    }
}

/// PowerTrain 的 payload preamble 标志写在 FLEXRAY-FRAME-TRIGGERING 上而非帧上，
/// 48 帧中恰有 24 帧为 true
#[test]
fn powertrain_arxml_reads_payload_preamble() {
    let f = parse_content(POWERTRAIN_ARXML).expect("parse");
    let with_preamble = f.frames().iter().filter(|fr| fr.payload_preamble()).count();
    assert_eq!(
        with_preamble, 24,
        "payload preamble 标志读不到（全为 false）"
    );
}

/// ARXML 保存后再读回：信号换算、值表、符号类型与帧内 PDU 映射都不应丢
#[test]
fn arxml_roundtrip_is_lossless() {
    let orig = parse_content(POWERTRAIN_ARXML).expect("parse");
    let xml = crate::fibex::export::arxml::export_arxml(&orig);
    let back = parse_content(&xml).expect("re-import exported arxml");
    let before = fingerprint(&orig);
    let after = fingerprint(&back);
    assert!(
        before.4 > 0 && before.3 > 0,
        "样本应含值表与线性换算，否则此测试无意义"
    );
    assert_eq!(
        before, after,
        "arxml 往返丢失内容 (frames, pdus, signals, factor, valtab, signed, pdu_maps)"
    );
    assert_eq!(
        orig.cluster().params,
        back.cluster().params,
        "集群参数在往返中改变"
    );
    assert_eq!(
        orig.ecu_key_slots(),
        back.ecu_key_slots(),
        "ECU 的关键时隙在 arxml 往返中改变"
    );
}

/// PowerTrain 的集群参数用无前缀标签，且时间量以秒计（MACROTICK-DURATION 1.375E-06）
#[test]
fn powertrain_arxml_reads_cluster_params() {
    let f = parse_content(POWERTRAIN_ARXML).expect("parse");
    let p = f.cluster().params;
    assert_eq!(
        p.number_of_static_slots, 70,
        "静态时隙数没读到（落在默认 10 上）"
    );
    assert_eq!(p.number_of_minislots, 291);
    assert_eq!(p.action_point_offset, 2);
    assert_eq!(p.network_idle_time, 11);
    assert_eq!(p.offset_correction_start, 3632);
    assert_eq!(p.minislot_duration, 5);
    assert_eq!(p.static_slot_duration, 31);
    assert!(
        (p.macrotick_duration_us - 1.375).abs() < 1e-9,
        "macrotick {}",
        p.macrotick_duration_us
    );
    assert!(
        (p.cycle_time_ms - 5.0).abs() < 1e-9,
        "cycle {}",
        p.cycle_time_ms
    );
    assert_eq!(p.speed_kbps, 10000, "速率应由 BIT 1E-07 换算");
    assert_eq!(
        p.macro_per_cycle, 3636,
        "MACRO-PER-CYCLE 没读到（一个周期多少个宏节拍）"
    );
}

/// 关键时隙写在 ECU 的 FlexRay 控制器里：PowerTrain 有 BSC/Engine/GearBox 三个
#[test]
fn arxml_reads_ecu_key_slots() {
    let f = parse_content(POWERTRAIN_ARXML).expect("parse");
    assert_eq!(f.ecu_key_slots().len(), 3, "三个 ECU 配了关键时隙");
    let bsc = f.ecu_key_slot("BSC").expect("BSC key slot");
    assert_eq!(bsc.slot_id, 16);
    assert!(
        bsc.used_for_sync && bsc.used_for_startup,
        "BSC 同时用于同步与冷启动"
    );
    assert_eq!(f.ecu_key_slot("Engine").expect("Engine").slot_id, 25);
    assert_eq!(f.ecu_key_slot("GearBox").expect("GearBox").slot_id, 52);
    assert!(
        f.ecu_key_slot("Tester").is_none(),
        "没配关键时隙的 ECU 不该凭空出现"
    );
}

/// 两通道各一个时隙号：改成不同值后保存再读回，两个通道的时隙都要保住
#[test]
fn per_channel_slots_survive_save_and_reload() {
    let mut fibex = parse_content(CHASSIS_ARXML).expect("chassis parse");
    let a = fibex
        .get_frame("EngineData")
        .unwrap()
        .channel_triggering(FrChannel::A)
        .unwrap();
    fibex.set_channel_triggering(
        "EngineData",
        FrChannel::B,
        Some(FrameTriggering { slot_id: 7, ..a }),
    );

    for (name, xml) in [
        ("arxml", crate::fibex::export::arxml::export_arxml(&fibex)),
        ("fibex", crate::fibex::export::fibex::export_fibex(&fibex)),
    ] {
        let back = parse_content(&xml).unwrap_or_else(|e| panic!("re-import {name}: {e}"));
        let frame = back.get_frame("EngineData").expect("EngineData");
        assert_eq!(
            frame.channel_triggering(FrChannel::A).map(|t| t.slot_id),
            Some(1),
            "{name} 往返改动了 A 通道的时隙"
        );
        assert_eq!(
            frame.channel_triggering(FrChannel::B).map(|t| t.slot_id),
            Some(7),
            "{name} 往返丢了 B 通道的时隙"
        );
    }
}

/// Vector FIBEX 的 KEY-SLOT-USAGE/STARTUP-SYNC 就是既用于同步又用于冷启动的时隙号
#[test]
fn fibex_demo_reads_ecu_key_slots() {
    let f = parse_content(DEMO_FIBEX_XML).expect("parse");
    let ecm = f.ecu_key_slot("ECM").expect("ECM key slot");
    assert_eq!(ecm.slot_id, 15, "ECM 的 STARTUP-SYNC");
    assert!(ecm.used_for_sync && ecm.used_for_startup);
    assert_eq!(f.ecu_key_slot("ESP").expect("ESP").slot_id, 1);
    assert_eq!(f.ecu_key_slot("CMM").expect("CMM").slot_id, 45);
}

/// FIBEX 保存后再读回：帧内 PDU 映射与 PDU 类型不应丢
#[test]
fn fibex_roundtrip_is_lossless() {
    let orig = parse_content(POWERTRAIN_ARXML).expect("parse");
    let xml = crate::fibex::export::fibex::export_fibex(&orig);
    let back = parse_content(&xml).expect("re-import exported fibex");
    let before = fingerprint(&orig);
    let after = fingerprint(&back);
    assert_eq!(
        before, after,
        "fibex 往返丢失内容 (frames, pdus, signals, factor, valtab, signed, pdu_maps)"
    );
    let kinds = |f: &EditableFibex| {
        f.pdus()
            .iter()
            .filter(|p| p.kind() != PduKind::Static)
            .count()
    };
    assert_eq!(
        kinds(&orig),
        kinds(&back),
        "PDU 类型（动态/事件）在往返中被抹平"
    );
    assert!(kinds(&orig) > 0, "样本应含非静态 PDU");
    assert_eq!(
        orig.ecu_key_slots(),
        back.ecu_key_slots(),
        "ECU 的关键时隙在 fibex 往返中改变"
    );
    assert_eq!(
        orig.cluster().params.macro_per_cycle,
        back.cluster().params.macro_per_cycle,
        "一个周期的宏节拍数在 fibex 往返中改变"
    );
}
