use crate::editable_dbc::EditableDbc;
use can_dbc::ByteOrder;

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Export the DBC contents as AUTOSAR 4.x style ARXML.
pub fn export_arxml(dbc: &EditableDbc) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<AUTOSAR xmlns=\"http://autosar.org/schema/r4.0\">\n");
    out.push_str("  <AR-PACKAGES>\n");
    out.push_str("    <AR-PACKAGE>\n");
    out.push_str("      <SHORT-NAME>CanDb</SHORT-NAME>\n");
    out.push_str("      <ELEMENTS>\n");

    for msg in dbc.messages().iter() {
        let name = escape_xml(msg.message_name());

        // CAN-FRAME
        out.push_str("        <CAN-FRAME>\n");
        out.push_str(&format!(
            "          <SHORT-NAME>{}</SHORT-NAME>\n",
            name
        ));
        if !msg.comment().is_empty() {
            out.push_str(&format!(
                "          <DESC><L-2 L=\"EN\">{}</L-2></DESC>\n",
                escape_xml(msg.comment())
            ));
        }
        out.push_str(&format!(
            "          <FRAME-LENGTH>{}</FRAME-LENGTH>\n",
            msg.message_size()
        ));
        out.push_str("          <PDU-TO-FRAME-MAPPINGS>\n");
        out.push_str("            <PDU-TO-FRAME-MAPPING>\n");
        out.push_str(&format!(
            "              <SHORT-NAME>{}_PduMapping</SHORT-NAME>\n",
            name
        ));
        out.push_str(&format!(
            "              <PDU-REF DEST=\"I-SIGNAL-I-PDU\">/CanDb/{}_PDU</PDU-REF>\n",
            name
        ));
        out.push_str("            </PDU-TO-FRAME-MAPPING>\n");
        out.push_str("          </PDU-TO-FRAME-MAPPINGS>\n");
        out.push_str("        </CAN-FRAME>\n");

        // CAN-FRAME-TRIGGERING
        out.push_str("        <CAN-FRAME-TRIGGERING>\n");
        out.push_str(&format!(
            "          <SHORT-NAME>{}_Trigger</SHORT-NAME>\n",
            name
        ));
        out.push_str(&format!(
            "          <IDENTIFIER>{}</IDENTIFIER>\n",
            msg.message_id()
        ));
        let addressing = match msg.frame_format() {
            crate::editable_dbc::FrameFormat::Standard
            | crate::editable_dbc::FrameFormat::StandardFd => "STANDARD",
            crate::editable_dbc::FrameFormat::Extended
            | crate::editable_dbc::FrameFormat::ExtendedFd => "EXTENDED",
        };
        out.push_str(&format!(
            "          <CAN-ADDRESSING-MODE>{}</CAN-ADDRESSING-MODE>\n",
            addressing
        ));
        out.push_str("        </CAN-FRAME-TRIGGERING>\n");

        // I-SIGNAL-I-PDU
        out.push_str("        <I-SIGNAL-I-PDU>\n");
        out.push_str(&format!(
            "          <SHORT-NAME>{}_PDU</SHORT-NAME>\n",
            name
        ));
        out.push_str(&format!(
            "          <LENGTH>{}</LENGTH>\n",
            msg.message_size()
        ));
        if !msg.signals().is_empty() {
            out.push_str("          <I-SIGNAL-TO-PDU-MAPPINGS>\n");
            for sig in msg.signals().iter() {
                out.push_str("            <I-SIGNAL-TO-PDU-MAPPING>\n");
                out.push_str(&format!(
                    "              <SHORT-NAME>{}</SHORT-NAME>\n",
                    escape_xml(sig.name())
                ));
                out.push_str(&format!(
                    "              <START-POSITION>{}</START-POSITION>\n",
                    sig.start_bit()
                ));
                out.push_str(&format!(
                    "              <LENGTH>{}</LENGTH>\n",
                    sig.signal_size()
                ));
                let order = match sig.byte_order() {
                    ByteOrder::LittleEndian => "MOST-SIGNIFICANT-BYTE-LAST",
                    ByteOrder::BigEndian => "MOST-SIGNIFICANT-BYTE-FIRST",
                };
                out.push_str(&format!(
                    "              <PACKING-BYTE-ORDER>{}</PACKING-BYTE-ORDER>\n",
                    order
                ));
                out.push_str("            </I-SIGNAL-TO-PDU-MAPPING>\n");
            }
            out.push_str("          </I-SIGNAL-TO-PDU-MAPPINGS>\n");
        }
        out.push_str("        </I-SIGNAL-I-PDU>\n");
    }

    out.push_str("      </ELEMENTS>\n");
    out.push_str("    </AR-PACKAGE>\n");
    out.push_str("  </AR-PACKAGES>\n");
    out.push_str("</AUTOSAR>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::export_arxml;
    use crate::editable_dbc::{EditableDbc, EditableMessage, EditableSignal, FrameFormat};
    use can_dbc::{ByteOrder, ValueType};

    #[test]
    fn export_contains_frame_pdu_and_signal() {
        let sig = EditableSignal::build(
            "Speed".to_string(),
            0,
            16,
            ByteOrder::LittleEndian,
            ValueType::Unsigned,
            0.1,
            0.0,
            0.0,
            250.0,
            "km/h".to_string(),
            vec!["ECU1".to_string()],
            Vec::new(),
            String::new(),
        );
        let msg = EditableMessage::build(
            0x123,
            FrameFormat::Standard,
            "EngineData".to_string(),
            8,
            "ECU1".to_string(),
            vec![sig],
            String::new(),
        );
        let dbc = EditableDbc::from_imported(vec![msg], vec!["ECU1".to_string()]);

        let xml = export_arxml(&dbc);
        assert!(xml.contains("<SHORT-NAME>EngineData</SHORT-NAME>"));
        assert!(xml.contains("<IDENTIFIER>291</IDENTIFIER>"));
        assert!(xml.contains("<FRAME-LENGTH>8</FRAME-LENGTH>"));
        assert!(xml.contains("<SHORT-NAME>Speed</SHORT-NAME>"));
        assert!(xml.contains("<START-POSITION>0</START-POSITION>"));
        assert!(xml.contains("<PACKING-BYTE-ORDER>MOST-SIGNIFICANT-BYTE-LAST</PACKING-BYTE-ORDER>"));

        // Round-trip: re-import the exported ARXML
        let reimported = crate::import::arxml::parse_arxml(&xml).expect("re-import should parse");
        assert_eq!(reimported.messages().len(), 1);
        let m = &reimported.messages()[0];
        assert_eq!(m.message_name(), "EngineData");
        assert_eq!(m.message_id(), 0x123);
        assert_eq!(m.message_size(), 8);
        assert_eq!(m.signals().len(), 1);
        let s = &m.signals()[0];
        assert_eq!(s.name(), "Speed");
        assert_eq!(s.signal_size(), 16);
        assert_eq!(s.byte_order(), &ByteOrder::LittleEndian);
    }
}
