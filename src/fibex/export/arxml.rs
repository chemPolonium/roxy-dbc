//! 将 EditableFibex 序列化为 AUTOSAR R4.x 风格 ARXML

use crate::fibex::editable_fibex::*;
use crate::fibex::export::escape_xml;

const PKG: &str = "FlexRayDb";

pub fn export_arxml(fibex: &EditableFibex) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<AUTOSAR xmlns=\"http://autosar.org/schema/r4.0\">\n");
    out.push_str("  <AR-PACKAGES>\n");
    out.push_str("    <AR-PACKAGE>\n");
    out.push_str(&format!("      <SHORT-NAME>{}</SHORT-NAME>\n", PKG));
    out.push_str("      <ELEMENTS>\n");

    let cluster = fibex.cluster();
    let p = &cluster.params;

    // ------------------------------------------------------------------
    // FLEXRAY-CLUSTER（含物理通道与帧触发）
    // ------------------------------------------------------------------
    out.push_str("        <FLEXRAY-CLUSTER>\n");
    out.push_str(&format!(
        "          <SHORT-NAME>{}</SHORT-NAME>\n",
        escape_xml(&cluster.name)
    ));
    out.push_str(&format!(
        "          <FLEXRAY-COLDSTART-ATTEMPTS>{}</FLEXRAY-COLDSTART-ATTEMPTS>\n",
        p.coldstart_attempts
    ));
    // FLEXRAY-CYCLE 以宏节拍数表示
    let cycle_mt = if p.macrotick_duration_us > 0.0 {
        (p.cycle_time_ms * 1000.0 / p.macrotick_duration_us).round() as u32
    } else {
        0
    };
    out.push_str(&format!(
        "          <FLEXRAY-CYCLE>{}</FLEXRAY-CYCLE>\n",
        cycle_mt
    ));
    out.push_str(&format!(
        "          <FLEXRAY-G-NUMBER-OF-MINISLOTS>{}</FLEXRAY-G-NUMBER-OF-MINISLOTS>\n",
        p.number_of_minislots
    ));
    out.push_str(&format!(
        "          <FLEXRAY-G-NUMBER-OF-STATIC-SLOTS>{}</FLEXRAY-G-NUMBER-OF-STATIC-SLOTS>\n",
        p.number_of_static_slots
    ));
    out.push_str(&format!(
        "          <FLEXRAY-GD-ACTION-POINT-OFFSET>{}</FLEXRAY-GD-ACTION-POINT-OFFSET>\n",
        p.action_point_offset
    ));
    out.push_str(&format!(
        "          <FLEXRAY-GD-DYNAMIC-SLOT-IDLE-PHASE>{}</FLEXRAY-GD-DYNAMIC-SLOT-IDLE-PHASE>\n",
        p.dynamic_slot_idle_phase
    ));
    out.push_str(&format!(
        "          <FLEXRAY-GD-MACROTICK-DURATION>{}</FLEXRAY-GD-MACROTICK-DURATION>\n",
        fmt_f64(p.macrotick_duration_us)
    ));
    out.push_str(&format!(
        "          <FLEXRAY-GD-MINISLOT>{}</FLEXRAY-GD-MINISLOT>\n",
        p.minislot_duration
    ));
    out.push_str(&format!(
        "          <FLEXRAY-GD-MINISLOT-ACTION-POINT-OFFSET>{}</FLEXRAY-GD-MINISLOT-ACTION-POINT-OFFSET>\n",
        p.minislot_action_point_offset
    ));
    out.push_str(&format!(
        "          <FLEXRAY-GD-MINOR-VERSION>{}</FLEXRAY-GD-MINOR-VERSION>\n",
        p.minor_version
    ));
    out.push_str(&format!(
        "          <FLEXRAY-GD-NIT>{}</FLEXRAY-GD-NIT>\n",
        p.network_idle_time
    ));
    out.push_str(&format!(
        "          <FLEXRAY-GD-STATIC-SLOT>{}</FLEXRAY-GD-STATIC-SLOT>\n",
        p.static_slot_duration
    ));
    out.push_str(&format!(
        "          <FLEXRAY-GD-SYMBOL-WINDOW>{}</FLEXRAY-GD-SYMBOL-WINDOW>\n",
        p.symbol_window
    ));
    out.push_str(&format!(
        "          <FLEXRAY-GD-SYMBOL-WINDOW-IDLE-PHASE>{}</FLEXRAY-GD-SYMBOL-WINDOW-IDLE-PHASE>\n",
        p.symbol_window_idle_phase
    ));
    out.push_str(&format!(
        "          <FLEXRAY-OFFSET-CORRECTION-START>{}</FLEXRAY-OFFSET-CORRECTION-START>\n",
        p.offset_correction_start
    ));

    out.push_str("          <FLEXRAY-PHYSICAL-CHANNELS>\n");
    for (channel_name, ch) in [("ChannelA", FrChannel::A), ("ChannelB", FrChannel::B)] {
        out.push_str("            <FLEXRAY-PHYSICAL-CHANNEL>\n");
        out.push_str(&format!(
            "              <SHORT-NAME>{}</SHORT-NAME>\n",
            channel_name
        ));

        let triggered: Vec<&EditableFrame> = fibex
            .frames()
            .iter()
            .filter(|f| f.triggering().channel.covers(ch))
            .collect();

        if !triggered.is_empty() {
            out.push_str("              <FRAME-TRIGGERINGS>\n");
            for frame in &triggered {
                let t = frame.triggering();
                out.push_str("                <FLEXRAY-FRAME-TRIGGERING>\n");
                out.push_str(&format!(
                    "                  <SHORT-NAME>{}_{}_Trigger</SHORT-NAME>\n",
                    channel_name,
                    escape_xml(frame.name())
                ));
                out.push_str(&format!(
                    "                  <FRAME-REF DEST=\"FLEXRAY-FRAME\">/{}/{} </FRAME-REF>\n",
                    PKG,
                    escape_xml(frame.name())
                ));
                out.push_str(&format!(
                    "                  <SLOT-ID>{}</SLOT-ID>\n",
                    t.slot_id
                ));
                out.push_str(&format!(
                    "                  <CYCLE-REPETITION>CYCLE-REPETITION-{}</CYCLE-REPETITION>\n",
                    t.cycle_repetition
                ));
                out.push_str(&format!(
                    "                  <BASE-CYCLE>{}</BASE-CYCLE>\n",
                    t.base_cycle
                ));
                out.push_str(&format!(
                    "                  <STARTUP-FRAME>{}</STARTUP-FRAME>\n",
                    if t.startup { "true" } else { "false" }
                ));
                out.push_str("                </FLEXRAY-FRAME-TRIGGERING>\n");
            }
            out.push_str("              </FRAME-TRIGGERINGS>\n");
        }

        out.push_str("            </FLEXRAY-PHYSICAL-CHANNEL>\n");
    }
    out.push_str("          </FLEXRAY-PHYSICAL-CHANNELS>\n");
    out.push_str("        </FLEXRAY-CLUSTER>\n");

    // ------------------------------------------------------------------
    // ECU-INSTANCE
    // ------------------------------------------------------------------
    for ecu in fibex.ecus() {
        out.push_str("        <ECU-INSTANCE>\n");
        out.push_str(&format!(
            "          <SHORT-NAME>{}</SHORT-NAME>\n",
            escape_xml(ecu)
        ));
        out.push_str("        </ECU-INSTANCE>\n");
    }

    // ------------------------------------------------------------------
    // I-SIGNAL-I-PDU（含 I-SIGNAL-TO-PDU-MAPPINGS）
    // ------------------------------------------------------------------
    for pdu in fibex.pdus() {
        out.push_str("        <I-SIGNAL-I-PDU>\n");
        out.push_str(&format!(
            "          <SHORT-NAME>{}</SHORT-NAME>\n",
            escape_xml(pdu.name())
        ));
        if !pdu.comment().is_empty() {
            out.push_str(&format!(
                "          <DESC><L-2 L=\"EN\">{}</L-2></DESC>\n",
                escape_xml(pdu.comment())
            ));
        }
        out.push_str(&format!("          <LENGTH>{}</LENGTH>\n", pdu.length()));
        if !pdu.signals().is_empty() {
            out.push_str("          <I-SIGNAL-TO-PDU-MAPPINGS>\n");
            for sig in pdu.signals() {
                out.push_str("            <I-SIGNAL-TO-PDU-MAPPING>\n");
                out.push_str(&format!(
                    "              <SHORT-NAME>{}_Map</SHORT-NAME>\n",
                    escape_xml(sig.name())
                ));
                out.push_str(&format!(
                    "              <START-POSITION>{}</START-POSITION>\n",
                    sig.start_bit()
                ));
                out.push_str(&format!(
                    "              <LENGTH>{}</LENGTH>\n",
                    sig.length_bits()
                ));
                out.push_str(&format!(
                    "              <PACKING-BYTE-ORDER>{}</PACKING-BYTE-ORDER>\n",
                    match sig.byte_order() {
                        ByteOrder::BigEndian => "MOST-SIGNIFICANT-BYTE-FIRST",
                        ByteOrder::LittleEndian => "MOST-SIGNIFICANT-BYTE-LAST",
                    }
                ));
                out.push_str(&format!(
                    "              <I-SIGNAL-REF DEST=\"I-SIGNAL\">/{}/{} </I-SIGNAL-REF>\n",
                    PKG,
                    escape_xml(sig.name())
                ));
                out.push_str("            </I-SIGNAL-TO-PDU-MAPPING>\n");
            }
            out.push_str("          </I-SIGNAL-TO-PDU-MAPPINGS>\n");
        }
        out.push_str("        </I-SIGNAL-I-PDU>\n");
    }

    // ------------------------------------------------------------------
    // I-SIGNAL（含长度与符号）
    // ------------------------------------------------------------------
    for pdu in fibex.pdus() {
        for sig in pdu.signals() {
            out.push_str("        <I-SIGNAL>\n");
            out.push_str(&format!(
                "          <SHORT-NAME>{}</SHORT-NAME>\n",
                escape_xml(sig.name())
            ));
            if !sig.comment().is_empty() {
                out.push_str(&format!(
                    "          <DESC><L-2 L=\"EN\">{}</L-2></DESC>\n",
                    escape_xml(sig.comment())
                ));
            }
            out.push_str(&format!(
                "          <LENGTH>{}</LENGTH>\n",
                sig.length_bits()
            ));
            out.push_str("        </I-SIGNAL>\n");
        }
    }

    // ------------------------------------------------------------------
    // FLEXRAY-FRAME（含 FRAME-PDU-MAPPINGS）
    // ------------------------------------------------------------------
    for frame in fibex.frames() {
        out.push_str("        <FLEXRAY-FRAME>\n");
        out.push_str(&format!(
            "          <SHORT-NAME>{}</SHORT-NAME>\n",
            escape_xml(frame.name())
        ));
        if !frame.comment().is_empty() {
            out.push_str(&format!(
                "          <DESC><L-2 L=\"EN\">{}</L-2></DESC>\n",
                escape_xml(frame.comment())
            ));
        }
        out.push_str(&format!(
            "          <FRAME-LENGTH>{}</FRAME-LENGTH>\n",
            frame.length()
        ));
        out.push_str(&format!(
            "          <PAYLOAD-PREAMBLE>{}</PAYLOAD-PREAMBLE>\n",
            if frame.payload_preamble() { "true" } else { "false" }
        ));
        if !frame.pdus().is_empty() {
            out.push_str("          <FRAME-PDU-MAPPINGS>\n");
            for m in frame.pdus() {
                out.push_str("            <FRAME-PDU-MAPPING>\n");
                out.push_str(&format!(
                    "              <SHORT-NAME>{}_Map</SHORT-NAME>\n",
                    escape_xml(&m.pdu_name)
                ));
                out.push_str(&format!(
                    "              <START-POSITION>{}</START-POSITION>\n",
                    m.start_position
                ));
                out.push_str(&format!(
                    "              <PDU-REF DEST=\"I-SIGNAL-I-PDU\">/{}/{} </PDU-REF>\n",
                    PKG,
                    escape_xml(&m.pdu_name)
                ));
                out.push_str("            </FRAME-PDU-MAPPING>\n");
            }
            out.push_str("          </FRAME-PDU-MAPPINGS>\n");
        }
        out.push_str("        </FLEXRAY-FRAME>\n");
    }

    out.push_str("      </ELEMENTS>\n");
    out.push_str("    </AR-PACKAGE>\n");
    out.push_str("  </AR-PACKAGES>\n");
    out.push_str("</AUTOSAR>\n");
    out
}

fn fmt_f64(v: f64) -> String {
    if (v - v.round()).abs() < f64::EPSILON {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fibex::export::fibex::export_fibex;
    use crate::fibex::import::parse_content;

    fn sample_fibex() -> EditableFibex {
        let mut fibex = EditableFibex::new();
        fibex.add_ecu("ECU_A");
        fibex.add_ecu("ECU_B");

        let sig = EditableSignal::build(
            "VehicleSpeed".to_string(),
            0,
            16,
            ByteOrder::BigEndian,
            ValueType::Unsigned,
            0.01,
            0.0,
            0.0,
            600.0,
            "km/h".to_string(),
            vec!["ECU_B".to_string()],
            Vec::new(),
            String::new(),
        );
        let pdu = EditablePdu::build(
            "Drivetrain_PDU".to_string(),
            4,
            PduKind::Static,
            vec![sig],
            String::new(),
        );
        fibex.add_pdu(&pdu);

        let frame = EditableFrame::build(
            "Drivetrain_Frame".to_string(),
            8,
            false,
            FrameTriggering {
                channel: FrChannel::A,
                slot_id: 3,
                base_cycle: 0,
                cycle_repetition: 4,
                startup: false,
            },
            vec![FramePduMapping::new("Drivetrain_PDU", 0)],
            String::new(),
        );
        fibex.add_frame(&frame);
        fibex
    }

    #[test]
    fn fibex_roundtrip() {
        let fibex = sample_fibex();
        let xml = export_fibex(&fibex);
        let imported = crate::fibex::import::fibex::parse_fibex_doc(
            &roxmltree::Document::parse(&xml).unwrap(),
        )
        .expect("re-import should parse");

        assert_eq!(imported.frames().len(), 1);
        let f = &imported.frames()[0];
        assert_eq!(f.name(), "Drivetrain_Frame");
        assert_eq!(f.length(), 8);
        assert_eq!(f.triggering().slot_id, 3);
        assert_eq!(f.triggering().cycle_repetition, 4);
        assert_eq!(f.pdus().len(), 1);
        assert_eq!(f.pdus()[0].pdu_name, "Drivetrain_PDU");

        assert_eq!(imported.pdus().len(), 1);
        let p = &imported.pdus()[0];
        assert_eq!(p.name(), "Drivetrain_PDU");
        assert_eq!(p.signals().len(), 1);
        let s = &p.signals()[0];
        assert_eq!(s.name(), "VehicleSpeed");
        assert_eq!(s.length_bits(), 16);
        assert_eq!(s.byte_order(), ByteOrder::BigEndian);
        assert_eq!(s.factor(), 0.01);
    }

    #[test]
    fn arxml_roundtrip() {
        let fibex = sample_fibex();
        let xml = export_arxml(&fibex);
        let imported = crate::fibex::import::arxml::parse_arxml_doc(
            &roxmltree::Document::parse(&xml).unwrap(),
        )
        .expect("re-import should parse");

        assert_eq!(imported.frames().len(), 1);
        let f = &imported.frames()[0];
        assert_eq!(f.name(), "Drivetrain_Frame");
        assert_eq!(f.triggering().slot_id, 3);
        assert_eq!(f.triggering().cycle_repetition, 4);
        assert_eq!(f.triggering().channel, FrChannel::A);
        assert_eq!(f.pdus().len(), 1);

        assert_eq!(imported.pdus().len(), 1);
        let s = &imported.pdus()[0].signals()[0];
        assert_eq!(s.name(), "VehicleSpeed");
        assert_eq!(s.start_bit(), 0);
        assert_eq!(s.length_bits(), 16);
        assert_eq!(s.byte_order(), ByteOrder::BigEndian);

        assert_eq!(imported.ecus(), &vec!["ECU_A".to_string(), "ECU_B".to_string()]);
    }

    #[test]
    fn format_sniffing_works() {
        let fibex = sample_fibex();
        let fibex_xml = export_fibex(&fibex);
        let arxml_xml = export_arxml(&fibex);

        let a = parse_content(&fibex_xml).unwrap();
        let b = parse_content(&arxml_xml).unwrap();
        assert_eq!(a.frames().len(), 1);
        assert_eq!(b.frames().len(), 1);
    }
}
