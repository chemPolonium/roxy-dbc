use crate::editable_dbc::{EditableDbc, EditableMessage, EditableSignal, FrameFormat};
use can_dbc::{ByteOrder, ValueType};

pub fn parse_kcd(content: &str) -> Result<EditableDbc, String> {
    let doc = roxmltree::Document::parse(content).map_err(|e| format!("XML parse error: {}", e))?;

    let mut messages = Vec::new();
    let mut nodes = Vec::new();

    let root = doc.root_element();

    for bus in root.children().filter(|n| n.has_tag_name("Bus")) {
        for msg_elem in bus.children().filter(|n| n.has_tag_name("Message")) {
            let id_str = msg_elem
                .attribute("id")
                .ok_or_else(|| "Message missing 'id' attribute".to_string())?;
            let message_id = parse_id(id_str)?;

            let message_name = msg_elem
                .attribute("name")
                .unwrap_or("Unnamed")
                .to_string();

            let message_size = msg_elem
                .attribute("length")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(8);

            let frame_format = if message_id > 0x7FF {
                FrameFormat::Extended
            } else {
                FrameFormat::Standard
            };

            let transmitter = msg_elem
                .attribute("producer")
                .unwrap_or("Vector__XXX")
                .to_string();

            if !transmitter.is_empty()
                && transmitter != "Vector__XXX"
                && !nodes.contains(&transmitter)
            {
                nodes.push(transmitter.clone());
            }

            let comment = msg_elem
                .children()
                .find(|n| n.has_tag_name("Notes"))
                .map(|n| n.text().unwrap_or("").to_string())
                .unwrap_or_default();

            let mut signals = Vec::new();
            for sig_elem in msg_elem
                .children()
                .filter(|n| n.has_tag_name("Signal"))
            {
                let signal = parse_kcd_signal(&sig_elem)?;
                signals.push(signal);
            }

            for sig_elem in msg_elem
                .children()
                .filter(|n| n.has_tag_name("SignalGroup"))
            {
                for inner_sig in sig_elem
                    .children()
                    .filter(|n| n.has_tag_name("Signal"))
                {
                    let signal = parse_kcd_signal(&inner_sig)?;
                    signals.push(signal);
                }
            }

            let message = EditableMessage::build(
                message_id,
                frame_format,
                message_name,
                message_size,
                transmitter,
                signals,
                comment,
            );
            messages.push(message);
        }
    }

    Ok(EditableDbc::from_imported(messages, nodes))
}

fn parse_kcd_signal(elem: &roxmltree::Node) -> Result<EditableSignal, String> {
    let name = elem
        .attribute("name")
        .unwrap_or("Unnamed")
        .to_string();

    let start_bit = elem
        .attribute("offset")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    let signal_size = elem
        .attribute("length")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1);

    let byte_order = match elem.attribute("endianess") {
        Some("big") => ByteOrder::BigEndian,
        _ => ByteOrder::LittleEndian,
    };

    let value_type = match elem.attribute("type") {
        Some("signed") => ValueType::Signed,
        _ => ValueType::Unsigned,
    };

    let factor = elem
        .attribute("slope")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(1.0);

    let offset = elem
        .attribute("intercept")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);

    let min = elem
        .attribute("min")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);

    let max = elem
        .attribute("max")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);

    let unit = elem
        .attribute("unit")
        .unwrap_or("")
        .to_string();

    let comment = elem
        .children()
        .find(|n| n.has_tag_name("Notes"))
        .map(|n| n.text().unwrap_or("").to_string())
        .unwrap_or_default();

    let mut value_descriptions = Vec::new();
    for label_set in elem.children().filter(|n| n.has_tag_name("LabelSet")) {
        for label in label_set.children().filter(|n| n.has_tag_name("Label")) {
            let label_name = label.attribute("name").unwrap_or("").to_string();
            let label_value = label
                .attribute("value")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0);
            value_descriptions.push((label_value, label_name));
        }
    }

    Ok(EditableSignal::build(
        name,
        start_bit,
        signal_size,
        byte_order,
        value_type,
        factor,
        offset,
        min,
        max,
        unit,
        Vec::new(),
        value_descriptions,
        comment,
    ))
}

fn parse_id(s: &str) -> Result<u32, String> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).map_err(|e| format!("Invalid hex id '{}': {}", s, e))
    } else {
        s.parse::<u32>()
            .map_err(|e| format!("Invalid id '{}': {}", s, e))
    }
}
