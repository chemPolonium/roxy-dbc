//! 属性编辑行：报文与信号编辑窗口共用
//!
//! 行由 DBC 里的属性声明（`BA_DEF_`）决定：声明了什么就显示什么，
//! 枚举用下拉（候选值就是声明里那串），其余用文本框。没填文本的属性不落盘。

use dear_imgui_rs::Ui;

use crate::editable_dbc::{
    AttrTarget, AttrType, AttrValue, EditableDbc, EditableMessage, EditableSignal,
};

/// 一份属性表单：属性名 -> 编辑中的文本，顺序与声明列表一致
#[derive(Clone, Default)]
pub struct AttributeFields {
    rows: Vec<(String, String)>,
}

/// 把文本按声明类型转成属性值；转不了返回 None（当作没填）
fn parse_as(kind: &AttrType, text: &str) -> Option<AttrValue> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    match kind {
        AttrType::Int { .. } | AttrType::Hex { .. } => {
            if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                i64::from_str_radix(hex.trim(), 16).ok().map(AttrValue::Int)
            } else {
                t.parse::<i64>().ok().map(AttrValue::Int)
            }
        }
        AttrType::Float { .. } => t.parse::<f64>().ok().map(AttrValue::Float),
        AttrType::Enum(_) | AttrType::String => Some(AttrValue::Text(t.to_string())),
    }
}

impl AttributeFields {
    /// 报文上的属性表单
    pub fn for_message(dbc: &EditableDbc, msg: &EditableMessage) -> Self {
        Self::build(dbc, AttrTarget::Message, |name| {
            msg.attribute(name).map(|v| v.display())
        })
    }

    /// 信号上的属性表单
    pub fn for_signal(dbc: &EditableDbc, msg_id: u32, sig: &EditableSignal) -> Self {
        Self::build(dbc, AttrTarget::Signal, |name| {
            dbc.signal_attribute(msg_id, sig.name(), name)
                .map(|v| v.display())
        })
    }

    /// 行 = 该类型上的全部声明；取值由调用方按对象取（含默认值回落）
    fn build(
        dbc: &EditableDbc,
        target: AttrTarget,
        current: impl Fn(&str) -> Option<String>,
    ) -> Self {
        let rows: Vec<(String, String)> = dbc
            .attribute_definitions_for(target)
            .into_iter()
            .map(|def| {
                let value = current(&def.name).unwrap_or_default();
                (def.name.clone(), value)
            })
            .collect();
        Self { rows }
    }

    /// 画表单；返回是否有改动
    pub fn render(&mut self, ui: &Ui, dbc: &EditableDbc, target: AttrTarget) {
        if self.rows.is_empty() {
            return;
        }
        ui.separator();
        ui.text_disabled("Attributes");
        for (name, text) in &mut self.rows {
            let kind = dbc
                .attribute_definition(name, target)
                .map(|d| d.kind.clone())
                .unwrap_or(AttrType::String);
            let field_id = format!("##attr_{}", name);
            let current = text.as_str();
            ui.align_text_to_frame_padding();
            ui.text(name.as_str());
            ui.same_line_with_pos(220.0);
            match &kind {
                AttrType::Enum(options) => {
                    // 当前值不在声明列表里时追加进去，避免下拉把已有值改掉
                    let mut options = options.clone();
                    if !current.trim().is_empty() && !options.iter().any(|o| o == current) {
                        options.push(current.to_string());
                    }
                    let refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
                    let sel = options.iter().position(|o| o == current).unwrap_or(0);
                    let mut idx = sel;
                    ui.set_next_item_width(180.0);
                    if ui.combo_simple_string(field_id.as_str(), &mut idx, &refs)
                        && idx < options.len()
                    {
                        *text = options[idx].clone();
                    }
                }
                _ => {
                    ui.set_next_item_width(180.0);
                    ui.input_text(field_id.as_str(), text).build();
                }
            }
        }
    }

    /// 有多少行的取值确实变了（用于把一次编辑合并成一步撤销）。
    /// 只比对象上显式挂的值：与 setter 的判断一致，避免算出不存在的改动。
    pub fn changed_message(&self, dbc: &EditableDbc, message_id: u32) -> usize {
        let explicit = |name: &str| {
            dbc.get_message(message_id)
                .and_then(|m| m.attribute(name))
                .cloned()
        };
        self.changed(dbc, AttrTarget::Message, explicit)
    }

    pub fn changed_signal(&self, dbc: &EditableDbc, message_id: u32, signal_name: &str) -> usize {
        let explicit = |name: &str| {
            dbc.get_message(message_id)
                .and_then(|m| m.signals().iter().find(|s| s.name() == signal_name))
                .and_then(|s| s.attribute(name))
                .cloned()
        };
        self.changed(dbc, AttrTarget::Signal, explicit)
    }

    fn changed(
        &self,
        dbc: &EditableDbc,
        target: AttrTarget,
        explicit: impl Fn(&str) -> Option<AttrValue>,
    ) -> usize {
        self.rows
            .iter()
            .filter(|(name, text)| {
                dbc.attribute_definition(name, target)
                    .map(|d| parse_as(&d.kind, text) != explicit(name))
                    .unwrap_or(false)
            })
            .count()
    }

    /// 把表单里的值写回报文（走撤销历史的 setter）
    pub fn apply_to_message(&self, dbc: &mut EditableDbc, message_id: u32) {
        for (name, text) in &self.rows {
            let Some(kind) = dbc
                .attribute_definition(name, AttrTarget::Message)
                .map(|d| d.kind.clone())
            else {
                continue;
            };
            let value = parse_as(&kind, text);
            dbc.set_message_attribute(message_id, name, value);
        }
    }

    /// 把表单里的值写回信号（走撤销历史的 setter）
    pub fn apply_to_signal(&self, dbc: &mut EditableDbc, message_id: u32, signal_name: &str) {
        for (name, text) in &self.rows {
            let Some(kind) = dbc
                .attribute_definition(name, AttrTarget::Signal)
                .map(|d| d.kind.clone())
            else {
                continue;
            };
            let value = parse_as(&kind, text);
            dbc.set_signal_attribute(message_id, signal_name, name, value);
        }
    }
}
