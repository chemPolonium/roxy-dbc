//! 系统模型的校验：Tools ▸ Validate 在焦点位于系统窗口时跑这些检查
//!
//! 只报数据库自身说得清的问题（重名、越界、悬空引用、没发送方），不猜外部工具的
//! 偏好；FlexRay 集群的时隙类检查在 FIBEX 窗口那一套里，这里不重复。

use crate::editable_dbc::{Severity, ValidationIssue};
use crate::system::{BusKind, SystemDoc};

#[derive(Default)]
struct Issues(Vec<ValidationIssue>);

impl Issues {
    fn err(&mut self, message: String) {
        self.0.push(ValidationIssue {
            severity: Severity::Error,
            message,
        });
    }

    fn warn(&mut self, message: String) {
        self.0.push(ValidationIssue {
            severity: Severity::Warning,
            message,
        });
    }
}

pub fn validate(doc: &SystemDoc) -> Vec<ValidationIssue> {
    let mut out = Issues::default();

    // 重名：各类对象各自的名字空间
    for (what, names) in [
        (
            "集群",
            doc.clusters
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
        ),
        (
            "帧",
            doc.frames
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
        ),
        (
            "PDU",
            doc.pdus.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        ),
        (
            "ECU",
            doc.ecus.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
        ),
    ] {
        for (i, a) in names.iter().enumerate() {
            if names[i + 1..].contains(a) {
                out.err(format!("{what}重名：{a}"));
            }
        }
    }
    let mut sig_names: Vec<&str> = Vec::new();
    for p in &doc.pdus {
        for s in &p.signals {
            if sig_names.contains(&s.name.as_str()) {
                out.err(format!("信号重名：{}", s.name));
            } else {
                sig_names.push(&s.name);
            }
        }
    }

    for cluster in &doc.clusters {
        let frames: Vec<&crate::system::Frame> = doc
            .frames
            .iter()
            .filter(|f| f.cluster == cluster.name)
            .collect();

        // 同一集群里标识符重复
        if cluster.kind == BusKind::Can {
            for (i, a) in frames.iter().enumerate() {
                let Some(ca) = a.can else { continue };
                for b in &frames[i + 1..] {
                    if b.can.map(|c| c.identifier) == Some(ca.identifier) {
                        out.err(format!(
                            "集群 {} 里标识符 0x{:X} 被两帧共用：{} 与 {}",
                            cluster.name, ca.identifier, a.name, b.name
                        ));
                    }
                }
            }
        }

        for f in &frames {
            if f.senders.is_empty() {
                out.warn(format!("帧 {} 没有发送方", f.name));
            }
            for m in &f.pdus {
                let Some(p) = doc.pdus.iter().find(|p| p.name == m.pdu_name) else {
                    out.err(format!("帧 {} 映射了不存在的 PDU {}", f.name, m.pdu_name));
                    continue;
                };
                let end = m.start_position as u64 + p.length_bytes as u64;
                if end > f.length_bytes as u64 {
                    out.err(format!(
                        "PDU {} 在帧 {} 里从第 {} 字节起、长 {} 字节，超出帧长 {} 字节",
                        p.name, f.name, m.start_position, p.length_bytes, f.length_bytes
                    ));
                }
            }
        }
    }

    for p in &doc.pdus {
        let bits = p.length_bytes as u64 * 8;
        for s in &p.signals {
            if s.start_bit as u64 + s.length_bits as u64 > bits {
                out.err(format!(
                    "信号 {} 在 PDU {} 里从第 {} 位起、长 {} 位，超出 PDU 的 {} 位",
                    s.name, p.name, s.start_bit, s.length_bits, bits
                ));
            }
            if s.min > s.max {
                out.warn(format!(
                    "信号 {} 的下限 {} 大于上限 {}",
                    s.name, s.min, s.max
                ));
            }
        }
        if p.signals.is_empty() && p.pdu_type == crate::system::PduType::ISignalIPdu {
            out.warn(format!("I-Signal-I-PDU {} 里一条信号都没有", p.name));
        }
    }

    for g in &doc.groups {
        for m in &g.pdus {
            if !doc.pdus.iter().any(|p| p.name == *m) {
                out.warn(format!("信号组 {} 引用了不存在的 PDU {}", g.name, m));
            }
        }
    }

    out.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::{CanFrameProps, Frame, FramePduMapping, Pdu, Signal};

    fn doc_with(frames: Vec<Frame>, pdus: Vec<Pdu>) -> SystemDoc {
        SystemDoc {
            name: "SYSTEM".to_string(),
            clusters: vec![crate::system::Cluster {
                name: "C".to_string(),
                kind: BusKind::Can,
                ..Default::default()
            }],
            frames,
            pdus,
            ..Default::default()
        }
    }

    fn frame(name: &str, id: u32, len: u32, pdus: &[(&str, u32)]) -> Frame {
        Frame {
            name: name.to_string(),
            cluster: "C".to_string(),
            length_bytes: len,
            pdus: pdus
                .iter()
                .map(|(p, s)| FramePduMapping::new(p, *s))
                .collect(),
            can: Some(CanFrameProps {
                identifier: id,
                ..Default::default()
            }),
            senders: vec!["ECU1".to_string()],
            ..Default::default()
        }
    }

    fn pdu(name: &str, len: u32, signals: &[(&str, u32, u32)]) -> Pdu {
        Pdu {
            name: name.to_string(),
            cluster: "C".to_string(),
            length_bytes: len,
            signals: signals
                .iter()
                .map(|(n, start, bits)| Signal {
                    name: n.to_string(),
                    start_bit: *start,
                    length_bits: *bits,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn clean_doc_has_no_issues() {
        let doc = doc_with(
            vec![frame("F1", 0x100, 8, &[("P1", 0)])],
            vec![pdu("P1", 8, &[("s1", 0, 16)])],
        );
        assert!(validate(&doc).is_empty(), "{:?}", validate(&doc));
    }

    #[test]
    fn duplicate_identifier_and_overflow_are_errors() {
        let doc = doc_with(
            vec![
                frame("F1", 0x100, 8, &[("P1", 0)]),
                frame("F2", 0x100, 4, &[("P1", 2)]),
            ],
            vec![pdu("P1", 4, &[("s1", 24, 16)])],
        );
        let issues = validate(&doc);
        let errors: Vec<&ValidationIssue> = issues
            .iter()
            .filter(|i| matches!(i.severity, Severity::Error))
            .collect();
        assert!(
            errors.iter().any(|i| i.message.contains("0x100")),
            "{issues:?}"
        );
        assert!(
            errors.iter().any(|i| i.message.contains("超出帧长")),
            "P1 从第 2 字节起长 4 字节，超出 F2 的 4 字节：{issues:?}"
        );
        assert!(
            errors.iter().any(|i| i.message.contains("超出 PDU")),
            "s1 从 24 位起长 16 位，超出 32 位：{issues:?}"
        );
    }

    #[test]
    fn missing_sender_is_a_warning_not_an_error() {
        let mut doc = doc_with(
            vec![frame("F1", 0x100, 8, &[("P1", 0)])],
            vec![pdu("P1", 8, &[])],
        );
        doc.frames[0].senders.clear();
        let issues = validate(&doc);
        assert!(
            issues.iter().any(
                |i| matches!(i.severity, Severity::Warning) && i.message.contains("没有发送方")
            ),
            "{issues:?}"
        );
        assert!(
            issues
                .iter()
                .any(|i| matches!(i.severity, Severity::Warning)
                    && i.message.contains("一条信号都没有")),
            "{issues:?}"
        );
    }
}
