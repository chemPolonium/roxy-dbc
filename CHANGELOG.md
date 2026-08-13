# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0] - 2026-08-13

### Added
- **节点（ECU）管理**
  - Tools > Nodes 窗口：添加 / 删除 / 重命名节点
  - 节点操作支持 Undo/Redo，保存时写入 `BU_` 段
- **信号位布局图**
  - 消息窗口中按字节行可视化信号占位，颜色区分不同信号
  - 支持 Intel（小端）与 Motorola（大端）位序，选中信号高亮描边
- **多选与批量操作**
  - 消息表和信号表支持 Ctrl+点击切换、Shift+点击/方向键范围选择
  - 批量复制 / 剪切 / 删除，批量删除合并为单步撤销
  - 右键菜单与 Edit 菜单对选区整体生效，剪贴板支持多条目
- **导入 / 导出**
  - 导入 ARXML（CAN-FRAME + I-SIGNAL-I-PDU 关联解析）和 KCD 文件（File > Import）
  - 导出 AUTOSAR 4.x 风格 ARXML（File > Export ARXML...）
  - 拖放 DBC/ARXML/KCD 文件直接打开，拖拽悬停时显示视觉提示
- **内容创建**
  - 新建 DBC（Ctrl+N）、"+ Add Message"、"+ Add Signal"，自动分配 ID 与默认值
- **值表（VAL_）编辑增强**
  - 非法整数与重复值实时提示，应用时自动按值排序
  - 支持从剪贴板批量导入（如 `0 "Off" 1 "On"`）
- **DBC 校验** - Tools > Validate 以表格列出错误与警告
- **快捷键** - Ctrl+N / Ctrl+O / Ctrl+S / Ctrl+Z / Ctrl+Y / Ctrl+C / X / V

### Changed
- UI 重构为 CANdb++ 风格界面：消息表格主窗口 + 独立 Message 窗口（含信号表格）
- 数据层重构为 `EditableDbc`，所有属性可直接编辑，操作级 Undo/Redo，复合操作合并撤销
- 编辑对话框采用 OK / Cancel / Apply 三按钮模式
- 保存：Ctrl+S / File > Save；未保存过的新文件自动弹出另存为对话框；标题栏以 `*` 标记脏状态
- 依赖升级：can-dbc 10.0、wgpu 28、imgui 0.12（docking + tables）、winit 0.30

### Fixed
- About 与错误对话框点击后不显示的问题（对话框渲染在重构中丢失）
- DBC 保存时注释与值表描述的引号转义（原为空操作）
- 新建 DBC 保存时默认写到程序目录的问题（现强制弹出另存为对话框）
- 导入 ARXML 时信号与消息 ID 关联丢失的问题（帧-PDU 按引用合并）
- 信号窗口 z-order 与渲染问题

### Removed
- 悬停信号预览弹窗（由 Message 窗口 + 位布局图替代）

## [0.5.0] - 2025-11-19

### Added
- 完整的 Message 属性编辑：Message ID（支持十六进制/十进制输入）、Message Name、Message Size、Transmitter、Comment，全部支持 Undo/Redo
- 消息编辑对话框采用 OK / Cancel / Apply 三按钮模式，支持持续编辑工作流
- 主菜单栏与文件操作入口
- 自定义可编辑 DBC 数据结构（Editable DBC），替代只读解析结果
- 悬停信号预览：鼠标悬停消息行显示最多 10 个信号的摘要弹窗
- 双击消息打开独立的信号详情窗口，支持多窗口对比

### Changed
- 数据层从覆盖映射（overrides）重构为单一可编辑结构
- Undo/Redo 迁移为按窗口的历史记录
- UI 布局改为 message 与 signal 独立的布局
- 升级 can-dbc 至 8.0

## [0.4.0] - 2025-10-14

### Added
- 完整的主界面 Docking 支持
  - 使用 `dockspace_over_main_viewport()` 实现全屏 docking 布局
  - 所有窗口现在可以自由停靠和组织

### Changed
- 优化信号表格列顺序
  - Type 和 Order 列移至 Start 列之前，便于快速查看信号类型和字节序
- 简化数值格式化逻辑
  - 移除复杂的小数位数计算函数
  - 直接使用 Rust 默认浮点数格式化，自动处理整数和小数显示
- 简化错误对话框
  - 使用固定大小（400x100px）替代复杂的动态布局
  - 长错误消息支持滚动显示
  - 移除不必要的按钮居中和手动间距逻辑

### Removed
- 移除无用的 CAN 数据显示窗口
- 移除无用的图表绘制窗口
- 移除复杂的 `get_decimal_places` 函数及相关测试代码
- 简化消息选择逻辑，移除不必要的变化检测

### Improved
- 代码结构优化
  - 提取消息表格行渲染函数 `render_messages_rows`
  - 改善函数职责分离和代码可读性
- 界面更加简洁专注于 DBC 文件浏览核心功能

## [0.3.0] - 2025-10-14

### Added
- 添加 ImGui Docking 支持
  - 支持创建标签页组合

### Improved
- 优化用户界面体验
  - 提供更灵活的多窗口管理功能

## [0.2.1] - 2025-10-14

### Fixed
- 修正数值显示精度问题
  - Factor、Offset、Min、Max 列现在智能检测整数值，整数显示为整数格式（如 "1" 而不是 "1.0"）
  - 小数值按照适当精度显示，避免不必要的尾随零
  - Min/Max 列的精度现在基于 Factor 的精度，保持逻辑一致性

### Improved
- 优化数值显示逻辑，提供更自然和简洁的用户界面

## [0.1.0] - 2025-10-13

### Added
- 初始版本发布
- DBC 文件解析和显示功能
- 多窗口界面支持
- 消息和信号表格显示
- 搜索和排序功能
- 整行选择支持
- 动态布局调整
