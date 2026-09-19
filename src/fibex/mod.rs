//! FIBEX / AUTOSAR ARXML (FlexRay) 数据库支持
//!
//! 从 roxy-fibex 项目整合而来：
//! - `editable_fibex`: 可编辑数据模型（帧 / PDU / 信号 / ECU / 集群参数 + Undo/Redo）
//! - `import` / `export`: FIBEX XML 与 AUTOSAR ARXML 的解析与导出
//! - `ui`: FIBEX 查看与编辑窗口（帧 / PDU 列表 / 信号列表 / ECU 列表 / 集群参数标签页）
//! - `state`: FIBEX 侧的 UI 状态（窗口列表、剪贴板、确认对话框）

pub mod editable_fibex;
pub mod export;
pub mod import;
pub mod ui;

pub mod state;

#[cfg(test)]
mod tests;
