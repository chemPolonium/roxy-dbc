//! 读 .arxml 时记下的出处：保存只改改过的地方，其余字节原样写回
//!
//! 真实的 ARXML 里带着本工具不建模的内容（NM / TP 配置、I-PDU 组、ADMIN-DATA、
//! LONG-NAME、INIT-VALUE、TRANSFER-PROPERTY……）。整份重新生成会把这些全丢掉，
//! 所以保存走"原文 + 补丁"：改一个字段只替换它在原文里的那一段文本，删一个对象
//! 去掉它的元素区间，新增对象插进它所属 package 的 `<ELEMENTS>` 里。没碰过的部分
//! 一个字节都不动。

use std::collections::HashMap;
use std::ops::Range;

pub type Span = Range<usize>;

/// 一次待写回的替换
#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    pub span: Span,
    pub text: String,
}

#[derive(Clone, Default)]
pub struct ArxmlSource {
    /// 打开时的原文
    pub text: String,
    /// 对象元素的区间，键如 `frame/Console_1`
    pub elements: HashMap<String, Span>,
    /// 字段值文本的区间，键如 `frame/Console_1/identifier`。
    /// 一个字段在文件里可能出现多处（如周期同时写在 true / false 两段时序里），改时每处都替换
    pub fields: HashMap<String, Vec<Span>>,
    /// 可插入新增内容的区间：package 的 `<ELEMENTS>` 内层、映射列表内层
    pub containers: HashMap<String, Span>,
    /// 每个容器所属 package 的引用路径前缀，新增对象时拼完整引用用
    pub container_paths: HashMap<String, String>,
    /// 换算方法名 → 它引用的 UNIT 元素名：改单位时去改那个 UNIT 的显示名
    pub compu_unit: HashMap<String, String>,
    /// 元素里 SHORT-NAME 之后的位置：文件里缺某个字段时插到这儿
    pub anchors: HashMap<String, Span>,
    /// 待写回的改动
    pub patches: Vec<Patch>,
}

impl ArxmlSource {
    pub fn record_element(&mut self, key: &str, span: Span) {
        self.elements.insert(key.to_string(), span);
    }

    pub fn record_field(&mut self, key: &str, span: Span) {
        self.fields.entry(key.to_string()).or_default().push(span);
    }

    pub fn record_container(&mut self, key: &str, span: Span) {
        self.containers.insert(key.to_string(), span);
    }

    pub fn record_container_path(&mut self, key: &str, path: String) {
        self.container_paths.insert(key.to_string(), path);
    }

    /// 对象改名后，它名下所有键的前缀跟着换，后续编辑才找得到地方
    pub fn rename_prefix(&mut self, from: &str, to: &str) {
        remap_keys(&mut self.elements, from, to);
        remap_keys(&mut self.fields, from, to);
        remap_keys(&mut self.containers, from, to);
        remap_keys(&mut self.anchors, from, to);
        remap_keys(&mut self.container_paths, from, to);
    }

    pub fn record_anchor(&mut self, key: &str, span: Span) {
        self.anchors.insert(key.to_string(), span);
    }

    /// 改一个字段。文件里有这个字段就替换那一段；没有则把 `missing_xml` 整段插到
    /// 元素锚点处；连锚点都没有（对象本身是新增的）时返回 false，由调用方整体生成
    pub fn edit_field(&mut self, key: &str, missing_xml: &str, text: String) -> bool {
        if let Some(spans) = self.fields.get(key).cloned() {
            let mut any = false;
            for span in spans {
                if self.deleted_covers(&span) {
                    any = true;
                    continue;
                }
                self.push_patch(span, text.clone());
                any = true;
            }
            return any;
        }
        let Some(anchor) = self.anchors.get(key).cloned() else {
            return false;
        };
        if self.deleted_covers(&anchor) {
            return true;
        }
        self.push_patch(anchor, format!("\n{missing_xml}"));
        true
    }

    /// 字段第一处的区间（改名时改引用路径用）
    pub fn first_field(&self, key: &str) -> Option<&Span> {
        self.fields.get(key).and_then(|v| v.first())
    }

    /// 这段是否落在一个已删除的元素里
    fn deleted_covers(&self, span: &Span) -> bool {
        self.patches
            .iter()
            .any(|p| p.text.is_empty() && p.span.start <= span.start && p.span.end >= span.end)
    }

    /// 删一个对象：去掉元素区间，并丢掉落在这段里的其它改动
    pub fn delete_element(&mut self, key: &str) -> bool {
        let Some(span) = self.elements.get(key).cloned() else {
            return false;
        };
        self.patches
            .retain(|p| p.span.start < span.start || p.span.end > span.end);
        self.push_patch(span, String::new());
        true
    }

    /// 在容器末尾追加一段 XML（新增对象 / 新增映射）
    pub fn insert(&mut self, container: &str, text: String) -> bool {
        let Some(span) = self.containers.get(container).cloned() else {
            return false;
        };
        self.push_patch(span.end..span.end, text);
        true
    }

    fn push_patch(&mut self, span: Span, text: String) {
        // 同一段或落在它里面的旧改动：后一次为准
        self.patches
            .retain(|p| !(p.span.start >= span.start && p.span.end <= span.end));
        self.patches.push(Patch { span, text });
    }

    /// 原文 + 全部改动。补丁按位置从后往前套，前面的区间不受影响
    pub fn render(&self) -> String {
        let mut patches = self.patches.clone();
        patches.sort_by_key(|p| p.span.start);
        let mut out = self.text.clone();
        for p in patches.iter().rev() {
            let end = p.span.end.min(out.len());
            let start = p.span.start.min(end);
            out.replace_range(start..end, &p.text);
        }
        out
    }

    pub fn is_dirty(&self) -> bool {
        !self.patches.is_empty()
    }

    pub fn clear_patches(&mut self) {
        self.patches.clear();
    }
}

/// 对象改名后把它名下所有键的前缀换掉
fn remap_keys<V>(map: &mut HashMap<String, V>, from: &str, to: &str) {
    let moved: Vec<String> = map
        .keys()
        .filter(|k| k.starts_with(from))
        .cloned()
        .collect();
    for old in moved {
        if let Some(v) = map.remove(&old) {
            let new = format!("{to}{}", &old[from.len()..]);
            map.insert(new, v);
        }
    }
}

/// 取元素里第一个指定标签的文本及其在原文里的区间（去掉首尾空白后的区间）
pub fn text_span<'a, 'b>(node: &roxmltree::Node<'a, 'b>, tags: &[&str]) -> Option<(String, Span)> {
    for tag in tags {
        let mut out = Vec::new();
        collect_into(node, tag, &mut out);
        for el in out {
            // DESC / LONG-NAME 的文本在 L-2 / L-4 里，取第一个有内容的文本节点
            let Some(text_node) = el
                .descendants()
                .find(|n| n.is_text() && n.text().is_some_and(|t| !t.trim().is_empty()))
            else {
                continue;
            };
            let raw = text_node.text()?;
            let trimmed = raw.trim();
            let range = text_node.range();
            let lead = raw.len() - raw.trim_start().len();
            let trail = raw.len() - raw.trim_end().len();
            return Some((
                trimmed.to_string(),
                (range.start + lead)..(range.end - trail),
            ));
        }
    }
    None
}

/// 元素区间（含开闭标签）
pub fn element_span(node: &roxmltree::Node) -> Span {
    node.range()
}

/// 元素开闭标签之间的末尾位置：往这个列表里追加子元素时用
pub fn inner_span(el: &roxmltree::Node) -> Span {
    let r = el.range();
    let close = format!("</{}>", el.tag_name().name()).len();
    (r.end - close)..(r.end - close)
}

/// 元素所在 package 的 `<ELEMENTS>` 插入点：新增同类对象时插到它兄弟们后面
pub fn container_of(node: &roxmltree::Node) -> Option<Span> {
    let el = node.parent()?;
    if !el.is_element() || !el.has_tag_name("ELEMENTS") {
        return None;
    }
    Some(inner_span(&el))
}

/// 元素自身文本（去掉首尾空白）在原文里的区间
pub fn node_text_span(node: &roxmltree::Node) -> Option<Span> {
    let text_node = node.children().find(|n| n.is_text())?;
    let raw = text_node.text()?;
    let range = text_node.range();
    let lead = raw.len() - raw.trim_start().len();
    let trail = raw.len() - raw.trim_end().len();
    Some((range.start + lead)..(range.end - trail))
}

/// SHORT-NAME 元素结束的位置：文件里缺字段时往这儿插
pub fn anchor_after_short_name(node: &roxmltree::Node) -> Option<Span> {
    let sn = node
        .children()
        .find(|n| n.is_element() && n.has_tag_name("SHORT-NAME"))?;
    let end = sn.range().end;
    Some(end..end)
}

fn collect_into<'a, 'b>(
    node: &roxmltree::Node<'a, 'b>,
    tag: &str,
    out: &mut Vec<roxmltree::Node<'a, 'b>>,
) {
    if node.is_element() && node.has_tag_name(tag) {
        out.push(*node);
    }
    for child in node.children() {
        collect_into(&child, tag, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_applies_patches_in_order() {
        let mut s = ArxmlSource {
            text: "<a>1</a><b>2</b>".to_string(),
            ..Default::default()
        };
        s.push_patch(3..4, "10".to_string());
        s.push_patch(11..12, "20".to_string());
        assert_eq!(s.render(), "<a>10</a><b>20</b>");
    }

    #[test]
    fn delete_drops_patches_inside_the_element() {
        let mut s = ArxmlSource {
            text: "<x><a>1</a></x><b>2</b>".to_string(),
            ..Default::default()
        };
        s.record_element("x", 0..15);
        s.record_field("x/a", 6..7);
        s.edit_field("x/a", "<a>9</a>", "9".to_string());
        assert!(s.delete_element("x"));
        assert_eq!(s.render(), "<b>2</b>", "删掉的元素里不该留下改字段的补丁");
    }

    #[test]
    fn later_edit_of_same_field_wins() {
        let mut s = ArxmlSource {
            text: "<a>1</a>".to_string(),
            ..Default::default()
        };
        s.record_field("a", 3..4);
        s.edit_field("a", "<a>2</a>", "2".to_string());
        s.edit_field("a", "<a>3</a>", "3".to_string());
        assert_eq!(s.render(), "<a>3</a>");
    }

    #[test]
    fn missing_field_is_inserted_at_anchor() {
        let mut s = ArxmlSource {
            text: "<x><SHORT-NAME>n</SHORT-NAME></x>".to_string(),
            ..Default::default()
        };
        s.record_anchor("x/len", 29..29);
        assert!(s.edit_field("x/len", "<LENGTH>8</LENGTH>", "8".to_string()));
        assert_eq!(
            s.render(),
            "<x><SHORT-NAME>n</SHORT-NAME>\n<LENGTH>8</LENGTH></x>"
        );
    }
}
