# Roxy DBC

[![Rust](https://img.shields.io/badge/rust-2024%20edition-blue.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)

一个现代化的 DBC (Database CAN) 文件查看器和编辑器，使用 Rust 和 ImGui 构建。

## ✨ 特性

- 🚗 **完整的 DBC 支持** - 解析和显示 CAN 数据库文件
- ✏️ **消息编辑** - 支持修改消息的 ID、名称、大小、发送节点等属性
- 📊 **多窗口界面** - 支持同时打开多个 DBC 文件
- 🔍 **智能搜索** - 快速查找消息和信号
- 📋 **表格视图** - 清晰的消息和信号列表显示
- ↕️ **排序功能** - 按任意列对数据进行排序
- ↩️ **撤销/重做** - 完整的 Undo/Redo 支持 (Ctrl+Z / Ctrl+Y)
- 🎨 **现代化UI** - 基于 ImGui 的直观用户界面
- ⚡ **高性能** - 使用 wgpu 进行硬件加速渲染

## 🖼️ 界面预览

![Roxy DBC Screenshot](screenshot.png)

### 主要功能
- **消息表格**: 显示消息ID、名称、长度和信号数量
- **信号详情窗口**: 双击消息打开独立的信号详情窗口
- **消息编辑**: 右键点击消息选择 "Edit..." 编辑属性
- **悬停预览**: 鼠标悬停在消息上显示信号摘要
- **Docking 布局**: 灵活的窗口停靠和组织

## 📝 编辑功能

### 可编辑的消息属性

- **Message ID** - CAN 消息标识符 (支持 0x123 或 123 格式)
- **Message Name** - 消息名称
- **Message Size** - 消息长度 (0-8 字节)
- **Transmitter** - 发送该消息的 ECU/节点名称
- **Comment** - 消息注释说明

### 编辑模式

使用传统的三按钮模式：
- **OK** - 保存修改并关闭对话框
- **Cancel** - 放弃修改并关闭对话框
- **Apply** - 保存修改但保持对话框打开（支持持续编辑）

### 撤销/重做

- 所有编辑操作都支持完整的 Undo/Redo
- 快捷键：Ctrl+Z (撤销) / Ctrl+Y 或 Ctrl+Shift+Z (重做)
- 最多保存 100 条历史记录
- 在 Edit 菜单中显示操作描述

## 🛠️ 技术栈

- **语言**: Rust 2024 Edition
- **GUI框架**: ImGui + wgpu
- **窗口管理**: winit
- **DBC解析**: can-dbc
- **文件对话框**: rfd

## 📦 安装

### 前提条件
- Rust 1.75+

### 从源码构建
```bash
# 克隆仓库
git clone https://github.com/chemPolonium/roxy-dbc.git
cd roxy-dbc

# 构建项目
cargo build --release

# 运行
cargo run --release
```

## 🚀 使用方法

### 基本操作
1. **启动应用** - 运行 `cargo run --release` 或直接执行编译后的程序
2. **打开DBC文件** - 点击 `File -> Load DBC File`
3. **浏览消息** - 在消息表格中查看所有CAN消息
4. **查看信号** - 双击消息打开独立的信号详情窗口
5. **搜索过滤** - 使用搜索框快速找到特定的消息

### 编辑消息
1. **右键点击** 消息行
2. 选择 **"Edit..."**
3. 在对话框中修改需要的属性
4. 点击 **OK** 保存并关闭，或 **Apply** 保存但继续编辑
5. 使用 **Ctrl+Z** 撤销修改

### 表格功能
- **排序**: 点击列标题对数据进行升序/降序排列
- **选择**: 点击消息行的任意列都可以选中该消息
- **高亮**: 选中的行会显示蓝色背景
- **悬停预览**: 鼠标悬停显示前10个信号的摘要

### 信号详情窗口
显示完整的信号信息：
- **信号名称** - 信号的标识名
- **类型** - Signed/Unsigned
- **字节序** - Intel (Little Endian) / Motorola (Big Endian)
- **起始位** - 信号在消息中的起始位置
- **长度** - 信号的位长度
- **系数** - 信号的比例因子
- **偏移量** - 信号的偏移值
- **最小值** - 信号的最小有效值
- **最大值** - 信号的最大有效值
- **单位** - 信号的物理单位

## 📁 项目结构

```
src/
├── main.rs          # 程序入口点和应用初始化
├── app.rs           # 窗口和图形上下文管理
├── dbc.rs           # DBC数据层和编辑覆盖层
└── ui/              # UI模块
    ├── mod.rs           # UI模块入口
    ├── state.rs         # UI状态和Undo/Redo系统
    ├── dbc_window.rs    # DBC窗口渲染
    ├── signal_window.rs # Signal窗口渲染
    ├── dialogs.rs       # 对话框管理
    └── menu.rs          # 菜单栏和快捷键
```

## ⚠️ 重要说明

### 数据持久化
- ✅ 所有修改在应用运行时保存在内存中
- ✅ 支持完整的 Undo/Redo
- ❌ 关闭应用后修改会丢失
- ❌ 暂不支持导出为 DBC 文件

### 数据覆盖机制
Roxy DBC 使用**非破坏性覆盖层**：
- 原始 DBC 文件不会被修改
- 所有编辑保存在覆盖层中
- 可以随时清除所有修改
- 内存占用极小（相比完整克隆节省 99.995%）

## 🔮 未来计划

### 短期目标
- [ ] 实时输入验证和错误提示
- [ ] Message ID 重复检查
- [ ] Signal 属性编辑
- [ ] 批量编辑功能

### 中期目标
- [ ] DBC 文件导出（保存修改）
- [ ] JSON/XML 格式导出
- [ ] 导入外部修改

### 长期目标
- [ ] CAN FD 支持（最大64字节）
- [ ] 可视化 Signal 布局编辑器
- [ ] 网络拓扑图
- [ ] 实时 CAN 数据监控

## 📄 许可证

本项目基于 GPLv3 许可证开源 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 🙏 致谢

- [can-dbc](https://github.com/marcelbuesing/can-dbc) - DBC 文件解析库
- [Dear ImGui](https://github.com/ocornut/imgui) - 即时模式 GUI 库
- [imgui-rs](https://github.com/imgui-rs/imgui-rs) - ImGui 的 Rust 绑定
- [wgpu](https://github.com/gfx-rs/wgpu) - 现代图形 API

## 📝 更新日志

查看 [CHANGELOG.md](CHANGELOG.md) 了解详细的版本历史和更新内容。

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 💬 联系方式

- GitHub Issues: [https://github.com/chemPolonium/roxy-dbc/issues](https://github.com/chemPolonium/roxy-dbc/issues)