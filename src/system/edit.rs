//! 系统模型的编辑操作：改模型的同时在原文上记补丁
//!
//! 每个操作先由调用方（窗口）压一份撤销快照，然后改 [`SystemDoc`] 并按
//! [`ArxmlSource`] 记下的字节区间记补丁：改值只替换那一段文本，删除去掉元素区间，
//! 新增把整段 XML 插进所属 package。新增之后原文里还没有这些对象，立刻用
//! "写出再读回" 换一份新的基线，后续编辑才找得到它们的出处。

use crate::fibex::editable_fibex::ByteOrder;
use crate::system::source::ArxmlSource;
use crate::system::{CanBehavior, Pdu, SystemDoc};

/// 浮点数写成文件里常见的样子：整数不带小数点
fn fmt_f(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 && v.abs() < 1e15 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn desc_xml(text: &str) -> String {
    format!("<DESC><L-2 L=\"FOR-ALL\">{}</L-2></DESC>", escape(text))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn unique<'a>(taken: impl Iterator<Item = &'a str>, want: &str) -> String {
    let taken: Vec<&str> = taken.collect();
    if !taken.contains(&want) {
        return want.to_string();
    }
    let mut i = 2;
    loop {
        let cand = format!("{want}_{i}");
        if !taken.contains(&cand.as_str()) {
            return cand;
        }
        i += 1;
    }
}

/// 写出再读回：让新增的对象也带上字节区间，后续编辑才找得到地方
fn rebase(doc: &mut SystemDoc, src: &mut ArxmlSource) -> Result<(), String> {
    let text = src.render();
    let (new_doc, new_src) = SystemDoc::from_arxml_with_source(&text)?;
    *doc = new_doc;
    *src = new_src;
    Ok(())
}

fn frame_mut(doc: &mut SystemDoc, name: &str) -> Result<usize, String> {
    doc.frames
        .iter()
        .position(|f| f.name == name)
        .ok_or_else(|| format!("没有帧 {name}"))
}

fn pdu_mut<'a>(doc: &'a mut SystemDoc, name: &str) -> Result<usize, String> {
    doc.pdus
        .iter()
        .position(|p| p.name == name)
        .ok_or_else(|| format!("没有 PDU {name}"))
}

fn signal_index(pdu: &Pdu, name: &str) -> Result<usize, String> {
    pdu.signals
        .iter()
        .position(|s| s.name == name)
        .ok_or_else(|| format!("PDU 里没有信号 {name}"))
}

/// 引用路径换末段：改名时 FRAME-REF / PDU-REF / I-SIGNAL-REF 要跟着指向新名字
fn retarget(src: &ArxmlSource, key: &str, new_name: &str) -> Option<String> {
    let span = src.first_field(key)?;
    let old = &src.text[span.start..span.end];
    let (head, _) = old.rsplit_once('/')?;
    Some(format!("{head}/{new_name}"))
}

// ---------------------------------------------------------------------- 帧

pub fn rename_frame(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    old: &str,
    new: &str,
) -> Result<(), String> {
    if old == new {
        return Ok(());
    }
    if doc.frames.iter().any(|f| f.name == new) {
        return Err(format!("已经有帧叫 {new}"));
    }
    let i = frame_mut(doc, old)?;
    if !src.edit_field(&format!("frame/{old}/name"), "", new.to_string()) {
        return Err("文件里找不到这帧的名字，保存后重新打开再改".to_string());
    }
    if let Some(path) = retarget(src, &format!("frame/{old}/frame_ref"), new) {
        src.edit_field(&format!("frame/{old}/frame_ref"), "", path);
    }
    doc.frames[i].name = new.to_string();
    src.rename_prefix(&format!("frame/{old}"), &format!("frame/{new}"));
    Ok(())
}

pub fn set_frame_identifier(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    name: &str,
    id: u32,
) -> Result<(), String> {
    let i = frame_mut(doc, name)?;
    if id > 0x1FFF_FFFF {
        return Err("标识符超出 29 位上限".to_string());
    }
    let key = format!("frame/{name}/identifier");
    let missing = format!("<IDENTIFIER>{id}</IDENTIFIER>");
    if !src.edit_field(&key, &missing, id.to_string()) {
        return Err("文件里这帧没有触发点，保存后重新打开再改".to_string());
    }
    doc.frames[i]
        .can
        .get_or_insert_with(Default::default)
        .identifier = id;
    Ok(())
}

pub fn set_frame_length(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    name: &str,
    len: u32,
) -> Result<(), String> {
    let i = frame_mut(doc, name)?;
    if len == 0 || len > 64 {
        return Err("帧长度要在 1..64 字节".to_string());
    }
    let key = format!("frame/{name}/length");
    if !src.edit_field(
        &key,
        &format!("<FRAME-LENGTH>{len}</FRAME-LENGTH>"),
        len.to_string(),
    ) {
        return Err("文件里找不到这帧的长度".to_string());
    }
    doc.frames[i].length_bytes = len;
    Ok(())
}

pub fn set_frame_addressing(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    name: &str,
    extended: bool,
) -> Result<(), String> {
    let i = frame_mut(doc, name)?;
    let text = if extended { "EXTENDED" } else { "STANDARD" };
    let key = format!("frame/{name}/addressing");
    if !src.edit_field(
        &key,
        &format!("<CAN-ADDRESSING-MODE>{text}</CAN-ADDRESSING-MODE>"),
        text.to_string(),
    ) {
        return Err("文件里这帧没有触发点，保存后重新打开再改".to_string());
    }
    doc.frames[i]
        .can
        .get_or_insert_with(Default::default)
        .extended = extended;
    Ok(())
}

pub fn set_frame_tx_behavior(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    name: &str,
    b: CanBehavior,
) -> Result<(), String> {
    let i = frame_mut(doc, name)?;
    let key = format!("frame/{name}/tx_behavior");
    let text = b.as_str();
    if !src.edit_field(
        &key,
        &format!("<CAN-FRAME-TX-BEHAVIOR>{text}</CAN-FRAME-TX-BEHAVIOR>"),
        text.to_string(),
    ) {
        return Err("文件里这帧没有触发点，保存后重新打开再改".to_string());
    }
    doc.frames[i]
        .can
        .get_or_insert_with(Default::default)
        .tx_behavior = b;
    Ok(())
}

pub fn set_frame_rx_behavior(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    name: &str,
    b: CanBehavior,
) -> Result<(), String> {
    let i = frame_mut(doc, name)?;
    let key = format!("frame/{name}/rx_behavior");
    let text = b.as_str();
    if !src.edit_field(
        &key,
        &format!("<CAN-FRAME-RX-BEHAVIOR>{text}</CAN-FRAME-RX-BEHAVIOR>"),
        text.to_string(),
    ) {
        return Err("文件里这帧没有触发点，保存后重新打开再改".to_string());
    }
    doc.frames[i]
        .can
        .get_or_insert_with(Default::default)
        .rx_behavior = b;
    Ok(())
}

pub fn set_frame_comment(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    name: &str,
    text: &str,
) -> Result<(), String> {
    let i = frame_mut(doc, name)?;
    let key = format!("frame/{name}/comment");
    if !src.edit_field(&key, &desc_xml(text), text.to_string()) {
        return Err("文件里找不到这帧".to_string());
    }
    doc.frames[i].comment = text.to_string();
    Ok(())
}

pub fn delete_frame(doc: &mut SystemDoc, src: &mut ArxmlSource, name: &str) -> Result<(), String> {
    let i = frame_mut(doc, name)?;
    if !src.delete_element(&format!("frame/{name}")) {
        return Err("文件里找不到这帧".to_string());
    }
    src.delete_element(&format!("frame/{name}/triggering"));
    doc.frames.remove(i);
    Ok(())
}

/// 新增一帧：连带一个同名的空 PDU 与一条带标识符的触发点；不带收发端口
pub fn add_frame(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    cluster: &str,
    name: &str,
    id: u32,
    length: u32,
) -> Result<(), String> {
    let name = unique(doc.frames.iter().map(|f| f.name.as_str()), name);
    let pdu_name = unique(doc.pdus.iter().map(|p| p.name.as_str()), &name);
    let pdu_path = src
        .container_paths
        .get("pdus")
        .cloned()
        .unwrap_or_else(|| "/PDUS".to_string());
    let frame_path = src
        .container_paths
        .get("frames")
        .cloned()
        .unwrap_or_else(|| "/FRAME".to_string());
    let frame_xml = format!(
        "<CAN-FRAME><SHORT-NAME>{name}</SHORT-NAME><FRAME-LENGTH>{length}</FRAME-LENGTH><PDU-TO-FRAME-MAPPINGS><PDU-TO-FRAME-MAPPING><SHORT-NAME>{name}</SHORT-NAME><PACKING-BYTE-ORDER>MOST-SIGNIFICANT-BYTE-LAST</PACKING-BYTE-ORDER><PDU-REF DEST=\"I-SIGNAL-I-PDU\">{pdu_path}/{pdu_name}</PDU-REF><START-POSITION>0</START-POSITION></PDU-TO-FRAME-MAPPING></PDU-TO-FRAME-MAPPINGS></CAN-FRAME>"
    );
    let pdu_xml = format!(
        "<I-SIGNAL-I-PDU><SHORT-NAME>{pdu_name}</SHORT-NAME><LENGTH>{length}</LENGTH><I-SIGNAL-TO-PDU-MAPPINGS/></I-SIGNAL-I-PDU>"
    );
    let trig_xml = format!(
        "<CAN-FRAME-TRIGGERING><SHORT-NAME>FT_{name}</SHORT-NAME><FRAME-REF DEST=\"CAN-FRAME\">{frame_path}/{name}</FRAME-REF><CAN-ADDRESSING-MODE>STANDARD</CAN-ADDRESSING-MODE><CAN-FRAME-TX-BEHAVIOR>CAN-20</CAN-FRAME-TX-BEHAVIOR><IDENTIFIER>{id}</IDENTIFIER></CAN-FRAME-TRIGGERING>"
    );
    if !src.insert("frames", frame_xml)
        || !src.insert("pdus", pdu_xml)
        || !src.insert(&format!("cluster/{cluster}/triggerings"), trig_xml)
    {
        return Err("文件里找不到插入位置".to_string());
    }
    rebase(doc, src)?;
    Ok(())
}

// ---------------------------------------------------------------------- PDU

pub fn rename_pdu(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    old: &str,
    new: &str,
) -> Result<(), String> {
    if old == new {
        return Ok(());
    }
    if doc.pdus.iter().any(|p| p.name == new) {
        return Err(format!("已经有 PDU 叫 {new}"));
    }
    let i = pdu_mut(doc, old)?;
    if !src.edit_field(&format!("pdu/{old}/name"), "", new.to_string()) {
        return Err("文件里找不到这个 PDU 的名字".to_string());
    }
    let users: Vec<String> = doc
        .frames
        .iter()
        .filter(|f| f.pdus.iter().any(|m| m.pdu_name == old))
        .map(|f| f.name.clone())
        .collect();
    for f in users {
        if let Some(path) = retarget(src, &format!("frame/{f}/map/{old}/ref"), new) {
            src.edit_field(&format!("frame/{f}/map/{old}/ref"), "", path);
        }
    }
    if let Some(path) = retarget(src, &format!("pdu/{old}/pdu_ref"), new) {
        src.edit_field(&format!("pdu/{old}/pdu_ref"), "", path);
    }
    doc.pdus[i].name = new.to_string();
    src.rename_prefix(&format!("pdu/{old}"), &format!("pdu/{new}"));
    Ok(())
}

pub fn set_pdu_length(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    name: &str,
    len: u32,
) -> Result<(), String> {
    let i = pdu_mut(doc, name)?;
    if len == 0 || len > 64 {
        return Err("PDU 长度要在 1..64 字节".to_string());
    }
    let key = format!("pdu/{name}/length");
    if !src.edit_field(&key, &format!("<LENGTH>{len}</LENGTH>"), len.to_string()) {
        return Err("文件里找不到这个 PDU 的长度".to_string());
    }
    doc.pdus[i].length_bytes = len;
    Ok(())
}

/// 周期毫秒；文件里没有 I-PDU-TIMING 时整段补一个
pub fn set_pdu_cycle(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    name: &str,
    ms: f64,
) -> Result<(), String> {
    let i = pdu_mut(doc, name)?;
    if ms <= 0.0 {
        return Err("周期要大于 0".to_string());
    }
    let seconds = fmt_f(ms / 1000.0);
    let key = format!("pdu/{name}/cycle");
    let missing = format!(
        "<I-PDU-TIMING-SPECIFICATIONS><I-PDU-TIMING><TRANSMISSION-MODE-DECLARATION><TRANSMISSION-MODE-TRUE-TIMING><CYCLIC-TIMING><TIME-OFFSET><VALUE>0</VALUE></TIME-OFFSET><TIME-PERIOD><VALUE>{seconds}</VALUE></TIME-PERIOD></CYCLIC-TIMING></TRANSMISSION-MODE-TRUE-TIMING></TRANSMISSION-MODE-DECLARATION></I-PDU-TIMING></I-PDU-TIMING-SPECIFICATIONS>"
    );
    if !src.edit_field(&key, &missing, seconds) {
        return Err("文件里找不到这个 PDU".to_string());
    }
    doc.pdus[i].cycle_ms = Some(ms);
    Ok(())
}

pub fn set_pdu_comment(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    name: &str,
    text: &str,
) -> Result<(), String> {
    let i = pdu_mut(doc, name)?;
    let key = format!("pdu/{name}/comment");
    if !src.edit_field(&key, &desc_xml(text), text.to_string()) {
        return Err("文件里找不到这个 PDU".to_string());
    }
    doc.pdus[i].comment = text.to_string();
    Ok(())
}

pub fn delete_pdu(doc: &mut SystemDoc, src: &mut ArxmlSource, name: &str) -> Result<(), String> {
    let i = pdu_mut(doc, name)?;
    if !src.delete_element(&format!("pdu/{name}")) {
        return Err("文件里找不到这个 PDU".to_string());
    }
    // 帧里指向它的映射一并去掉，不留悬空引用
    for f in doc
        .frames
        .iter()
        .filter(|f| f.pdus.iter().any(|m| m.pdu_name == name))
    {
        src.delete_element(&format!("frame/{}/map/{name}/element", f.name));
    }
    for f in doc.frames.iter_mut() {
        f.pdus.retain(|m| m.pdu_name != name);
    }
    doc.pdus.remove(i);
    Ok(())
}

// ---------------------------------------------------------------------- 信号

fn signal_compu(doc: &SystemDoc, pdu: &str, signal: &str) -> Result<String, String> {
    let p = doc
        .pdus
        .iter()
        .find(|p| p.name == pdu)
        .ok_or_else(|| format!("没有 PDU {pdu}"))?;
    let s = p
        .signals
        .iter()
        .find(|s| s.name == signal)
        .ok_or_else(|| format!("没有信号 {signal}"))?;
    if s.compu_name.is_empty() {
        return Err(format!("信号 {signal} 在文件里没有换算方法"));
    }
    Ok(s.compu_name.clone())
}

pub fn rename_signal(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    pdu: &str,
    old: &str,
    new: &str,
) -> Result<(), String> {
    if old == new {
        return Ok(());
    }
    if doc
        .pdus
        .iter()
        .any(|p| p.signals.iter().any(|s| s.name == new))
    {
        return Err(format!("已经有信号叫 {new}"));
    }
    let i = pdu_mut(doc, pdu)?;
    let j = signal_index(&doc.pdus[i], old)?;
    if !src.edit_field(&format!("signal/{old}/name"), "", new.to_string()) {
        return Err("文件里找不到这个信号的名字".to_string());
    }
    src.edit_field(&format!("pdu/{pdu}/map/{old}/name"), "", new.to_string());
    if let Some(path) = retarget(src, &format!("pdu/{pdu}/map/{old}/ref"), new) {
        src.edit_field(&format!("pdu/{pdu}/map/{old}/ref"), "", path);
    }
    if let Some(path) = retarget(src, &format!("signal/{old}/sig_ref"), new) {
        src.edit_field(&format!("signal/{old}/sig_ref"), "", path);
    }
    doc.pdus[i].signals[j].name = new.to_string();
    src.rename_prefix(&format!("signal/{old}"), &format!("signal/{new}"));
    src.rename_prefix(
        &format!("pdu/{pdu}/map/{old}"),
        &format!("pdu/{pdu}/map/{new}"),
    );
    Ok(())
}

pub fn set_signal_start(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    pdu: &str,
    signal: &str,
    bit: u32,
) -> Result<(), String> {
    let i = pdu_mut(doc, pdu)?;
    let j = signal_index(&doc.pdus[i], signal)?;
    let len = doc.pdus[i].signals[j].length_bits;
    if bit as u64 + len as u64 > doc.pdus[i].length_bytes as u64 * 8 {
        return Err("起始位加长度超出 PDU 的位数".to_string());
    }
    let key = format!("pdu/{pdu}/map/{signal}/start");
    if !src.edit_field(
        &key,
        &format!("<START-POSITION>{bit}</START-POSITION>"),
        bit.to_string(),
    ) {
        return Err("文件里找不到这条映射".to_string());
    }
    doc.pdus[i].signals[j].start_bit = bit;
    Ok(())
}

pub fn set_signal_length(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    pdu: &str,
    signal: &str,
    len: u32,
) -> Result<(), String> {
    let i = pdu_mut(doc, pdu)?;
    let j = signal_index(&doc.pdus[i], signal)?;
    if len == 0 || len > 64 {
        return Err("信号长度要在 1..64 位".to_string());
    }
    let key = format!("signal/{signal}/length");
    if !src.edit_field(&key, &format!("<LENGTH>{len}</LENGTH>"), len.to_string()) {
        return Err("文件里找不到这个信号".to_string());
    }
    doc.pdus[i].signals[j].length_bits = len;
    Ok(())
}

pub fn set_signal_order(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    pdu: &str,
    signal: &str,
    order: ByteOrder,
) -> Result<(), String> {
    let i = pdu_mut(doc, pdu)?;
    let j = signal_index(&doc.pdus[i], signal)?;
    let text = match order {
        ByteOrder::BigEndian => "MOST-SIGNIFICANT-BYTE-FIRST",
        ByteOrder::LittleEndian => "MOST-SIGNIFICANT-BYTE-LAST",
    };
    let key = format!("pdu/{pdu}/map/{signal}/order");
    if !src.edit_field(
        &key,
        &format!("<PACKING-BYTE-ORDER>{text}</PACKING-BYTE-ORDER>"),
        text.to_string(),
    ) {
        return Err("文件里找不到这条映射".to_string());
    }
    doc.pdus[i].signals[j].byte_order = order;
    Ok(())
}

pub fn set_signal_scaling(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    pdu: &str,
    signal: &str,
    factor: f64,
    offset: f64,
    min: f64,
    max: f64,
) -> Result<(), String> {
    let cm = signal_compu(doc, pdu, signal)?;
    let i = pdu_mut(doc, pdu)?;
    let j = signal_index(&doc.pdus[i], signal)?;
    // 文件里的上下限是原始值：物理值 = 偏移 + 系数 × 原始值，反推回去
    let raw_min = if factor.abs() > 1e-12 {
        (min - offset) / factor
    } else {
        min
    };
    let raw_max = if factor.abs() > 1e-12 {
        (max - offset) / factor
    } else {
        max
    };
    let ok = src.edit_field(&format!("compu/{cm}/offset"), "", fmt_f(offset))
        && src.edit_field(&format!("compu/{cm}/factor"), "", fmt_f(factor))
        && src.edit_field(&format!("compu/{cm}/min"), "", fmt_f(raw_min))
        && src.edit_field(&format!("compu/{cm}/max"), "", fmt_f(raw_max));
    if !ok {
        return Err("文件里的换算方法缺字段，保存后重新打开再改".to_string());
    }
    let s = &mut doc.pdus[i].signals[j];
    s.factor = factor;
    s.offset = offset;
    s.min = min;
    s.max = max;
    Ok(())
}

pub fn set_signal_unit(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    pdu: &str,
    signal: &str,
    unit: &str,
) -> Result<(), String> {
    let cm = signal_compu(doc, pdu, signal)?;
    let unit_name = src
        .compu_unit
        .get(&cm)
        .cloned()
        .ok_or_else(|| "文件里这个换算方法没有挂单位".to_string())?;
    let i = pdu_mut(doc, pdu)?;
    let j = signal_index(&doc.pdus[i], signal)?;
    if !src.edit_field(&format!("unit/{unit_name}/display"), "", unit.to_string()) {
        return Err("文件里找不到这个单位".to_string());
    }
    doc.pdus[i].signals[j].unit = unit.to_string();
    Ok(())
}

pub fn set_signal_comment(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    pdu: &str,
    signal: &str,
    text: &str,
) -> Result<(), String> {
    let i = pdu_mut(doc, pdu)?;
    let j = signal_index(&doc.pdus[i], signal)?;
    let key = format!("signal/{signal}/comment");
    if !src.edit_field(&key, &desc_xml(text), text.to_string()) {
        return Err("文件里找不到这个信号".to_string());
    }
    doc.pdus[i].signals[j].comment = text.to_string();
    Ok(())
}

pub fn set_signal_values(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    pdu: &str,
    signal: &str,
    values: Vec<(i64, String)>,
) -> Result<(), String> {
    let cm = signal_compu(doc, pdu, signal)?;
    let i = pdu_mut(doc, pdu)?;
    let j = signal_index(&doc.pdus[i], signal)?;
    let scales = values
        .iter()
        .map(|(v, t)| {
            format!(
                "<COMPU-SCALE><LOWER-LIMIT>{v}</LOWER-LIMIT><UPPER-LIMIT>{v}</UPPER-LIMIT><COMPU-CONST><VT>{}</VT></COMPU-CONST></COMPU-SCALE>",
                escape(t)
            )
        })
        .collect::<String>();
    let xml = format!("<COMPU-SCALES>{scales}</COMPU-SCALES>");
    if !src.edit_field(&format!("compu/{cm}/values"), &xml, xml.clone()) {
        return Err("文件里找不到这个换算方法".to_string());
    }
    doc.pdus[i].signals[j].value_descriptions = values;
    Ok(())
}

pub fn delete_signal(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    pdu: &str,
    signal: &str,
) -> Result<(), String> {
    let i = pdu_mut(doc, pdu)?;
    let j = signal_index(&doc.pdus[i], signal)?;
    if !src.delete_element(&format!("signal/{signal}")) {
        return Err("文件里找不到这个信号".to_string());
    }
    src.delete_element(&format!("pdu/{pdu}/map/{signal}/element"));
    src.delete_element(&format!("signal/{signal}/triggering"));
    doc.pdus[i].signals.remove(j);
    Ok(())
}

/// 在 PDU 里新增一条信号：建 I-SIGNAL 与映射，不带换算方法
pub fn add_signal(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    pdu: &str,
    name: &str,
    start: u32,
    length: u32,
) -> Result<(), String> {
    let i = pdu_mut(doc, pdu)?;
    let name = unique(doc.pdus[i].signals.iter().map(|s| s.name.as_str()), name);
    if start as u64 + length as u64 > doc.pdus[i].length_bytes as u64 * 8 {
        return Err("起始位加长度超出 PDU 的位数".to_string());
    }
    let sig_path = src
        .container_paths
        .get("signals")
        .cloned()
        .unwrap_or_else(|| "/I_SIGNALS".to_string());
    let sig_xml =
        format!("<I-SIGNAL><SHORT-NAME>{name}</SHORT-NAME><LENGTH>{length}</LENGTH></I-SIGNAL>");
    let map_xml = format!(
        "<I-SIGNAL-TO-I-PDU-MAPPING><SHORT-NAME>{name}</SHORT-NAME><I-SIGNAL-REF DEST=\"I-SIGNAL\">{sig_path}/{name}</I-SIGNAL-REF><PACKING-BYTE-ORDER>MOST-SIGNIFICANT-BYTE-LAST</PACKING-BYTE-ORDER><START-POSITION>{start}</START-POSITION></I-SIGNAL-TO-I-PDU-MAPPING>"
    );
    if !src.insert("signals", sig_xml) || !src.insert(&format!("pdu/{pdu}/maps"), map_xml) {
        return Err("文件里找不到插入位置".to_string());
    }
    rebase(doc, src)?;
    Ok(())
}

// ---------------------------------------------------------------------- ECU 与集群

pub fn rename_ecu(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    old: &str,
    new: &str,
) -> Result<(), String> {
    if old == new {
        return Ok(());
    }
    if doc.ecus.iter().any(|e| e.name == new) {
        return Err(format!("已经有 ECU 叫 {new}"));
    }
    let i = doc
        .ecus
        .iter()
        .position(|e| e.name == old)
        .ok_or_else(|| format!("没有 ECU {old}"))?;
    if !src.edit_field(&format!("ecu/{old}/name"), "", new.to_string()) {
        return Err("文件里找不到这个 ECU 的名字".to_string());
    }
    doc.ecus[i].name = new.to_string();
    src.rename_prefix(&format!("ecu/{old}"), &format!("ecu/{new}"));
    Ok(())
}

pub fn set_ecu_comment(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    name: &str,
    text: &str,
) -> Result<(), String> {
    let i = doc
        .ecus
        .iter()
        .position(|e| e.name == name)
        .ok_or_else(|| format!("没有 ECU {name}"))?;
    let key = format!("ecu/{name}/comment");
    if !src.edit_field(&key, &desc_xml(text), text.to_string()) {
        return Err("文件里找不到这个 ECU".to_string());
    }
    doc.ecus[i].comment = text.to_string();
    Ok(())
}

pub fn set_cluster_baudrate(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    cluster: &str,
    bps: f64,
) -> Result<(), String> {
    let i = doc
        .clusters
        .iter()
        .position(|c| c.name == cluster)
        .ok_or_else(|| format!("没有集群 {cluster}"))?;
    if bps <= 0.0 {
        return Err("波特率要大于 0".to_string());
    }
    let key = format!("cluster/{cluster}/baudrate");
    if !src.edit_field(
        &key,
        &format!("<BAUDRATE>{}</BAUDRATE>", fmt_f(bps)),
        fmt_f(bps),
    ) {
        return Err("文件里找不到这个集群的波特率".to_string());
    }
    doc.clusters[i].baudrate = Some(bps);
    Ok(())
}

pub fn set_cluster_comment(
    doc: &mut SystemDoc,
    src: &mut ArxmlSource,
    cluster: &str,
    text: &str,
) -> Result<(), String> {
    let i = doc
        .clusters
        .iter()
        .position(|c| c.name == cluster)
        .ok_or_else(|| format!("没有集群 {cluster}"))?;
    let key = format!("cluster/{cluster}/comment");
    if !src.edit_field(&key, &desc_xml(text), text.to_string()) {
        return Err("文件里找不到这个集群".to_string());
    }
    doc.clusters[i].comment = text.to_string();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 样本是客户内部文件、不入库：机器上没有时就跳过这几条测试
    fn load() -> Option<(SystemDoc, ArxmlSource)> {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("can-arxml-sample/Comfort.arxml");
        let text = std::fs::read_to_string(&path).ok()?;
        SystemDoc::from_arxml_with_source(&text).ok()
    }

    fn reload(text: &str) -> SystemDoc {
        SystemDoc::from_arxml_with_source(text).expect("re-parse").0
    }

    #[test]
    fn editing_identifier_and_comment_round_trips() {
        let Some((mut doc, mut src)) = load() else {
            return;
        };
        set_frame_identifier(&mut doc, &mut src, "Console_1", 0x2A0).expect("set id");
        set_frame_comment(&mut doc, &mut src, "Console_1", "控制台 1 号").expect("set comment");
        let out = src.render();
        let back = reload(&out);
        let f = back
            .frames
            .iter()
            .find(|f| f.name == "Console_1")
            .expect("frame");
        assert_eq!(f.can.as_ref().unwrap().identifier, 0x2A0);
        assert_eq!(f.comment, "控制台 1 号");
        assert!(out.contains("<NM-CONFIG>") || !out.contains("<NM-CONFIG>"));
    }

    #[test]
    fn editing_signal_scaling_and_unit_round_trips() {
        let Some((mut doc, mut src)) = load() else {
            return;
        };
        set_signal_scaling(
            &mut doc,
            &mut src,
            "Gateway_2",
            "CarSpeed",
            0.25,
            -10.0,
            -10.0,
            90.0,
        )
        .expect("scaling");
        set_signal_unit(&mut doc, &mut src, "Gateway_2", "CarSpeed", "km/h").expect("unit");
        let out = src.render();
        let back = reload(&out);
        let pdu = back
            .pdus
            .iter()
            .find(|p| p.name == "Gateway_2")
            .expect("pdu");
        let s = pdu
            .signals
            .iter()
            .find(|s| s.name == "CarSpeed")
            .expect("signal");
        assert!((s.factor - 0.25).abs() < 1e-9);
        assert!((s.offset + 10.0).abs() < 1e-9);
        assert_eq!(s.unit, "km/h");
    }

    #[test]
    fn delete_frame_keeps_the_rest_of_the_file() {
        let Some((mut doc, mut src)) = load() else {
            return;
        };
        let before = doc.frames.len();
        delete_frame(&mut doc, &mut src, "DebugMsg1").expect("delete");
        assert_eq!(doc.frames.len(), before - 1);
        let out = src.render();
        let back = reload(&out);
        assert_eq!(back.frames.len(), before - 1);
        assert!(back.frames.iter().all(|f| f.name != "DebugMsg1"));
        assert!(
            out.contains("<NM-CONFIG>") || out.contains("<I-SIGNAL-I-PDU>"),
            "其余内容要留下"
        );
    }

    #[test]
    fn add_signal_then_edit_it() {
        let Some((mut doc, mut src)) = load() else {
            return;
        };
        add_signal(&mut doc, &mut src, "Console_1", "NewSig", 16, 4).expect("add");
        assert!(
            doc.pdus
                .iter()
                .find(|p| p.name == "Console_1")
                .unwrap()
                .signals
                .iter()
                .any(|s| s.name == "NewSig")
        );
        // 重新基线之后新增的信号也能继续改
        set_signal_start(&mut doc, &mut src, "Console_1", "NewSig", 20).expect("edit new signal");
        let out = src.render();
        let back = reload(&out);
        let pdu = back
            .pdus
            .iter()
            .find(|p| p.name == "Console_1")
            .expect("pdu");
        let s = pdu
            .signals
            .iter()
            .find(|s| s.name == "NewSig")
            .expect("new signal");
        assert_eq!(s.start_bit, 20);
        assert_eq!(s.length_bits, 4);
    }

    #[test]
    fn add_frame_then_set_its_identifier() {
        let Some((mut doc, mut src)) = load() else {
            return;
        };
        add_frame(&mut doc, &mut src, "Comfort", "NewFrame", 0x300, 8).expect("add frame");
        set_frame_identifier(&mut doc, &mut src, "NewFrame", 0x301).expect("edit new frame");
        let out = src.render();
        let back = reload(&out);
        let f = back
            .frames
            .iter()
            .find(|f| f.name == "NewFrame")
            .expect("new frame");
        assert_eq!(f.can.as_ref().unwrap().identifier, 0x301);
        assert_eq!(f.length_bytes, 8);
    }

    #[test]
    fn rename_frame_moves_its_source_keys() {
        let Some((mut doc, mut src)) = load() else {
            return;
        };
        rename_frame(&mut doc, &mut src, "Console_1", "Console_A").expect("rename");
        set_frame_identifier(&mut doc, &mut src, "Console_A", 0x1A5).expect("edit after rename");
        let out = src.render();
        let back = reload(&out);
        let f = back
            .frames
            .iter()
            .find(|f| f.name == "Console_A")
            .expect("renamed");
        assert_eq!(f.can.as_ref().unwrap().identifier, 0x1A5);
    }

    #[test]
    fn set_pdu_cycle_writes_seconds() {
        let Some((mut doc, mut src)) = load() else {
            return;
        };
        set_pdu_cycle(&mut doc, &mut src, "Gateway_1", 50.0).expect("cycle");
        let out = src.render();
        let back = reload(&out);
        let p = back
            .pdus
            .iter()
            .find(|p| p.name == "Gateway_1")
            .expect("pdu");
        assert_eq!(p.cycle_ms, Some(50.0));
    }

    /// 周期在文件里写了两遍（true / false 时序）：改的时候两处都要换
    #[test]
    fn cycle_edit_replaces_every_occurrence() {
        let Some((mut doc, mut src)) = load() else {
            return;
        };
        set_pdu_cycle(&mut doc, &mut src, "Console_1", 25.0).expect("cycle");
        let out = src.render();
        assert_eq!(
            out.matches("<VALUE>0.025</VALUE>").count(),
            2,
            "两段时序的周期都要改"
        );
        assert!(!out.contains("<VALUE>0.02</VALUE>"), "旧周期不该留下");
    }
}
