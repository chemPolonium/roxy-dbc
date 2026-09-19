use crate::editable_dbc::{EditableDbc, EditableMessage, EditableSignal, FrameFormat};
use can_dbc::{ByteOrder, ValueType};

struct PduInfo {
    name: String,
    length: u64,
    signals: Vec<EditableSignal>,
}

pub fn parse_arxml(content: &str) -> Result<EditableDbc, String> {
    let doc = roxmltree::Document::parse(content).map_err(|e| format!("XML parse error: {}", e))?;

    let root = doc.root_element();
    let mut messages = Vec::new();
    let mut nodes = Vec::new();

    let mut pdu_nodes = Vec::new();
    collect_elements_by_tag(&root, "I-SIGNAL-I-PDU", &mut pdu_nodes);
    let mut pdus: Vec<PduInfo> = pdu_nodes.iter().map(parse_pdu_info).collect();

    let mut can_frames = Vec::new();
    collect_elements_by_tag(&root, "CAN-FRAME", &mut can_frames);

    let mut triggerings = Vec::new();
    collect_elements_by_tag(&root, "CAN-FRAME-TRIGGERING", &mut triggerings);

    for frame in &can_frames {
        let frame_name = get_short_name(frame);
        let pdu_name = format!("{}_PDU", frame_name);
        let pdu_pos = pdus.iter().position(|p| p.name == pdu_name);
        let pdu = pdu_pos.map(|i| pdus.remove(i));

        let triggering = triggerings
            .iter()
            .find(|t| get_short_name(t) == format!("{}_Trigger", frame_name));

        match parse_arxml_frame(frame, triggering, pdu.as_ref()) {
            Ok(message) => {
                for sig in message.signals() {
                    for r in sig.receivers() {
                        if !r.is_empty() && r != "Vector__XXX" && !nodes.contains(r) {
                            nodes.push(r.clone());
                        }
                    }
                }
                messages.push(message);
            }
            Err(e) => {
                eprintln!("Warning: skipping CAN-FRAME: {}", e);
            }
        }
    }

    // Standalone PDUs without a matching CAN-FRAME (external files)
    for pdu in pdus {
        let message = EditableMessage::build(
            0,
            FrameFormat::Standard,
            pdu.name,
            pdu.length,
            "Vector__XXX".to_string(),
            pdu.signals,
            String::new(),
        );
        messages.push(message);
    }

    Ok(EditableDbc::from_imported(messages, nodes))
}

fn collect_elements_by_tag<'a, 'b>(
    node: &roxmltree::Node<'a, 'b>,
    tag: &str,
    results: &mut Vec<roxmltree::Node<'a, 'b>>,
) {
    if node.is_element() && node.has_tag_name(tag) {
        results.push(*node);
    }
    for child in node.children() {
        collect_elements_by_tag(&child, tag, results);
    }
}

fn get_short_name(node: &roxmltree::Node) -> String {
    node.children()
        .find(|n| n.has_tag_name("SHORT-NAME"))
        .and_then(|n| n.text())
        .unwrap_or("Unnamed")
        .to_string()
}

fn parse_pdu_info(pdu_elem: &roxmltree::Node) -> PduInfo {
    let name = get_short_name(pdu_elem);

    let mut length_elems = Vec::new();
    collect_elements_by_tag(pdu_elem, "LENGTH", &mut length_elems);
    let length = length_elems
        .first()
        .and_then(|n| n.text())
        .and_then(|t| t.trim().parse::<u64>().ok())
        .unwrap_or(8);

    let mut signals = Vec::new();
    let mut mappings = Vec::new();
    collect_elements_by_tag(pdu_elem, "I-SIGNAL-TO-PDU-MAPPING", &mut mappings);
    for mapping in &mappings {
        if let Ok(signal) = parse_arxml_signal_mapping(mapping) {
            signals.push(signal);
        }
    }

    PduInfo {
        name,
        length,
        signals,
    }
}

fn parse_arxml_frame(
    frame_elem: &roxmltree::Node,
    triggering: Option<&roxmltree::Node>,
    pdu: Option<&PduInfo>,
) -> Result<EditableMessage, String> {
    let message_name = get_short_name(frame_elem);

    let mut identifiers = Vec::new();
    collect_elements_by_tag(frame_elem, "IDENTIFIER", &mut identifiers);
    if identifiers.is_empty()
        && let Some(t) = triggering {
            collect_elements_by_tag(t, "IDENTIFIER", &mut identifiers);
        }

    let message_id = identifiers
        .first()
        .and_then(|n| n.text())
        .and_then(|t| parse_arxml_id(t.trim()))
        .ok_or_else(|| format!("CAN-FRAME '{}' missing IDENTIFIER", message_name))?;

    let frame_format = if message_id > 0x7FF {
        FrameFormat::Extended
    } else {
        FrameFormat::Standard
    };

    let mut frame_lengths = Vec::new();
    collect_elements_by_tag(frame_elem, "FRAME-LENGTH", &mut frame_lengths);
    let message_size = frame_lengths
        .first()
        .and_then(|n| n.text())
        .and_then(|t| t.trim().parse::<u64>().ok())
        .or_else(|| pdu.map(|p| p.length))
        .unwrap_or(8);

    let signals = pdu.map(|p| p.signals.clone()).unwrap_or_default();

    Ok(EditableMessage::build(
        message_id,
        frame_format,
        message_name,
        message_size,
        "Vector__XXX".to_string(),
        signals,
        String::new(),
    ))
}

fn parse_arxml_signal_mapping(mapping: &roxmltree::Node) -> Result<EditableSignal, String> {
    let name = get_short_name(mapping);

    let mut start_elems = Vec::new();
    collect_elements_by_tag(mapping, "START-POSITION", &mut start_elems);

    let start_bit = start_elems
        .first()
        .and_then(|n| n.text())
        .and_then(|t| t.trim().parse::<u64>().ok())
        .unwrap_or(0);

    let mut length_elems = Vec::new();
    collect_elements_by_tag(mapping, "LENGTH", &mut length_elems);

    let signal_size = length_elems
        .first()
        .and_then(|n| n.text())
        .and_then(|t| t.trim().parse::<u64>().ok())
        .unwrap_or(1);

    let mut packing_elems = Vec::new();
    collect_elements_by_tag(mapping, "PACKING-BYTE-ORDER", &mut packing_elems);

    let byte_order = packing_elems
        .first()
        .and_then(|n| n.text())
        .map(|t| t.trim())
        .map(|t| {
            if t == "MOST-SIGNIFICANT-BYTE-FIRST" {
                ByteOrder::BigEndian
            } else {
                ByteOrder::LittleEndian
            }
        })
        .unwrap_or(ByteOrder::LittleEndian);

    Ok(EditableSignal::build(
        name,
        start_bit,
        signal_size,
        byte_order,
        ValueType::Unsigned,
        1.0,
        0.0,
        0.0,
        0.0,
        String::new(),
        Vec::new(),
        Vec::new(),
        String::new(),
    ))
}

fn parse_arxml_id(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u32>().ok()
    }
}
