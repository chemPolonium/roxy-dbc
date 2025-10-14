# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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