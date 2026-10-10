# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.11.1] - 2026-10-10

### Added
- 命令行帮助补全写法：`roxy-dbc.exe --help` / `-h` / `/?` / `-?` 列命令与退出码，`roxy-dbc.exe dbc help <命令>`（如 `dbc help message add`）与 `dbc <命令> --help` 一样只列那条命令的选项；帮助文本里也说明了直接给文件名是开窗口

## [0.11.0] - 2026-10-10

### Added
- 命令行改 DBC（不开窗口，给脚本和 AI 代理用）
  - 同一个 exe 多了一条路：`roxy-dbc.exe dbc <命令> <文件> [选项]`；`roxy-dbc.exe --help` 列命令，`roxy-dbc.exe dbc signal add --help` 列一条命令的选项
  - `show` 打印节点、报文、信号、注释、属性、值表（`--json` 出机器读的形式）；`validate` 跑与 Tools > Validate 相同的检查
  - `comment` 加 / 改 / 删报文与信号注释（`--message EngineData` / `--signal EngineData.EngSpeed` / `--clear`）
  - `message add|set|delete`、`signal add|set|delete`、`attribute set`、`node add|rename|delete` 改数据库内容；目标写名字或 ID 都认，写错时报错并列出文件里有什么
  - 写回沿用打开时的编码（UTF-8 / UTF-8 BOM / GBK），先写同目录临时文件再替换；`--out` 写副本不动原文件，`--backup` 先存 `.bak`
  - 结果走 stdout、错误走 stderr，退出码 0 成功 / 1 validate 查出错误 / 2 命令没做成
  - 文件里带本工具不建模的段（`CM_ BU_` 节点注释、`EV_` 环境变量、`SIG_GROUP_`、`VAL_TABLE_`）时，写回前在 stderr 说明这些行会丢
- 程序图标
  - exe 文件本身的图标：`build.rs` 用 winresource 把 `roxy-dbc.ico` 嵌进 PE 资源，资源管理器与快捷方式上显示它
  - 运行时的窗口图标：`src/icon.rs` 从同一个 .ico 里取 32 位的那条图（自下而上的 BGRA 转 RGBA，alpha 全 0 时按 AND 掩码补形状），交给 winit；标题栏与任务栏用这个图标，读不出可用条目时保持系统默认图标

## [0.10.0] - 2026-10-08

### Added
- DBC 属性
  - 打开文件时读入属性声明（`BA_DEF_`）、默认值（`BA_DEF_DEF_`）与取值（`BA_`），保存时写回；`VFrameFormat` 仍按帧格式（标准/扩展、经典/CAN FD）自己写出，不重复登记
  - 消息与信号的编辑窗口底部多一节 **Attributes**：文件里声明了什么就列什么，枚举属性用下拉（当前值不在列表里时把它追加进下拉，不会被改掉），其余用文本框；清空文本即删除该取值
  - 给没声明过的属性赋值时自动补一条声明，范围取到能容下这个值
  - Tools > Validate 报出取值超出声明的 INT/FLOAT 范围、或不在枚举列表里的属性
  - 属性改动走撤销历史，并与同一次 Apply 里的其他改动合并成一步撤销
  - Messages & Signals 表新增 **Cycle** 列：按 `GenMsgCycleTime`、`CycleTime` 的优先顺序取文件里声明过的那个，取不到（含文件里的默认值）时显示 `-`；点列标题可按周期排序
- CAN 数据库导出为 ARXML 现在带上整张数据库
  - 除 CAN-FRAME 与 I-SIGNAL-I-PDU 外，还写 CAN-CLUSTER（含通道与每条帧的 CAN-FRAME-TRIGGERING：标识符、标准/扩展寻址、CAN FD 收发行为）、I-SIGNAL 与 COMPU-METHOD（因子/偏移、上下限、单位、值表、符号类型）、ECU-INSTANCE 与端口（帧的 FRAME-PORT 表达发送/接收，信号级 I-SIGNAL-PORT 表达每个信号的接收者）、帧与信号的注释
  - 读回侧按引用解析（FRAME-REF / PDU-REF / I-SIGNAL-REF / COMPU-METHOD-REF / 端口名后缀），不再只靠命名约定；缺引用时才回退到 `{帧名}_PDU` / `{帧名}_Trigger`
  - Ctrl+O / 拖放 / 命令行打开 .arxml / .xml 时按内容分流：CAN 快照进 DBC 编辑窗口，FlexRay 进 FIBEX 查看窗口，因此导出的 CAN ARXML 可以直接再打开回来
- FlexRay 帧按通道排程
  - 帧编辑窗口的排程改成每通道一行：Channel A / Channel B 各有 Send 勾选框和时隙、起始周期、重复周期、Startup 字段；勾掉 Send 表示该通道不再发送这帧
  - Schedule 页点空格子只改所点通道的排程，另一通道不受影响；Unschedule 一次清除两个通道，算一步撤销
  - Frames 表的 Slot / Base Cycle / Repetition / Startup 列在两通道取值不同时写成 `A 1 / B 2`，相同或只有一个通道时只写一个值；Channel 列写 A / B / A+B
- FlexRay 关键时隙与通信周期
  - ECU List 标签页新增 **Key Slot** 列：显示该 ECU 控制器用的关键时隙号与用途，例如 `16 (sync + startup)`；文件里没配的显示 `-`
  - Cluster Parameters 标签页新增 **Macroticks per Cycle (gCycle)**：一个通信周期包含多少个宏节拍，与上面的周期时间和宏节拍时长对应
  - 保存回 ARXML / FIBEX 时写出这两项，往返不丢
- 通信矩阵 Excel 模板 ↔ DBC
  - File 菜单新增 **Import Excel Matrix...**：读取填好的 .xlsx 模板生成一个 DBC 编辑窗口，报文行下面的信号行归属它上面最近的报文行，节点列的 S 是发送、R 是接收。表头认不出 Msg_Name / Signal_Name 时直接说明这不是本工具的模板。生成后窗口记录的文件名是同名 .dbc，`Ctrl+S` 写出 DBC，不会覆盖 Excel 源文件
  - File 菜单新增 **Export Excel Template...**：导出只有表头的空白模板，供填写
  - DBC 窗口的 **Export** 标签页新增 **Export Excel...**：把当前 DBC 按模板写成 .xlsx（报文一行、信号一行，节点列标 S/R），改完可再导回
  - Ctrl+O / 拖放 / 命令行走同一个入口，现在也能直接打开 .xlsx
  - 导入过程中被跳过的行（缺 Msg_ID、信号上面没有报文行）在 Validation Results 窗口里按行号列出。模板里的报文类型、发送类型、周期时间、快速周期、重发次数、延时、信号发送类型、初始值、无效值、非使能值存为 DBC 属性（GenMsgType / GenMsgSendType / GenMsgCycleTime / GenMsgCycleTimeFast / GenMsgNrOfRepetition / GenMsgDelayTime / GenSigSendType / GenSigStartValue / GenSigInvalidValue / GenSigInactiveValue），初始值一类支持 `0x` 十六进制；导出模板时按同样的名字读回来
  - `Ctrl+S` / Save As 的默认文件名沿用窗口当前文件名，不再固定为 output.dbc
- File 菜单新增 **New FIBEX**（`Ctrl+Shift+N`）：新建空的 FlexRay 数据库，首次保存时选择路径与格式
- FIBEX 窗口新增 **Schedule** 标签页：按通道显示静态段的"周期 × 时隙"矩阵，同一格出现多个帧时标红
- 在 Schedule 页可直接改排程：选中一个帧（Frames 页里选，或点网格里已排程的格子），再点空格子即把它排到该时隙与该周期；`Unschedule` 清除排程；`Ctrl+Z` 撤销。目标时隙在该周期已被占用时拒绝改动，并说明是哪一帧占用；改动后一行提示说明实际生效的发送规律
- PDU 列表支持右键复制 / 剪切 / 粘贴，粘贴出的副本自动取不重名的名字
- Signal List 标签页可按任意列排序（点列标题），与其余表格一致
- FIBEX 窗口新增 **Communication Matrix** 标签页：行是帧（可展开到帧内各 PDU 的信号），列是 ECU，格子里标 TX / RX；只收到该帧部分信号时标 R\*，鼠标悬停显示收到几个。帧名后面附带时隙号与周期
- Signal List 标签页新增 **Export CSV...**：每个信号一行，含所属 PDU、起始位、长度、字节序、数据类型、因子/偏移、上下限、单位、发送与接收、注释和值表；文件带 UTF-8 BOM，Excel 直接打开不乱码
- 焦点在 FIBEX 窗口时，Tools > Validate 校验当前 FlexRay 数据库

### Fixed
- 帧在 A、B 两个通道占用不同时隙时只保住一个通道：现在每通道各存一条触发，读入、显示与保存回 ARXML / FIBEX 都不再丢掉另一个通道的时隙
- Validate 的时隙冲突改为按通道比较：A 通道与 B 通道使用同一编号的时隙不再被误报为冲突
- 一帧没有任何通道触发时不再显示成默认的第 1 时隙，而是显示 `-`，Validate 提示 "not triggered on any channel"
- 打开 ARXML 时，帧引用的传输层 PDU 与网络管理 PDU 现在会出现在 PDU 列表里，不再只剩一个引用名
- 帧内 PDU 的起始位置改为按位读取；此前按字节读取，导致同一帧内第二个 PDU 的位置远超帧长
- ARXML 的集群参数（静态时隙数、宏节拍、周期、速率、各类偏移与空闲时间）现在显示文件里的真实值，而不是内置默认值；Validate 里大量"时隙号超出 gNumberOfStaticSlots"的误报随之消失
- 保存为 ARXML 不再丢失信号的因子/偏移、值表、符号类型、上下限与单位；保存为 FIBEX 不再丢失帧内的多个 PDU
- ARXML 里的 payload preamble 标志现在能读出（该标志写在触发点上，不在帧里）
- FIBEX 文件里的 `EVENT-PDU` 显示为 Event 类型，不再被并入 Dynamic
- 焦点在 FIBEX 窗口时，Edit 菜单的撤销/重做/复制/粘贴/删除/新建帧以及 `Ctrl+Z`、`Ctrl+Y` 真正作用于 FlexRay 数据
- 校验对话框可以拖动边缘调整大小，问题列表在框内滚动
- 调度表格里点击格子，选中的是被点的那一格
- 保存 ARXML 时，出现在多个 PDU 里的同名信号只写一份定义

## [0.9.0] - 2026-09-19

### Changed
- **Export ARXML 从菜单栏移入 DBC 窗口的独立标签页**：导出动作随窗口走，不再依赖全局焦点状态（只打开 FIBEX 配置时菜单项也不会误置灰）
- **Edit 菜单按焦点分发**：焦点在 FIBEX 窗口时，Undo / Redo / Copy / Paste / Delete / Add Frame 作用于 FIBEX 数据（此前编辑菜单只对 DBC 窗口生效）
- **统一文件打开入口（Ctrl+O / Load File）**：一个入口即可打开所有支持的文件——`.dbc` / `.kcd` 进 DBC 编辑窗口，`.xml` / `.arxml`（含 FIBEX 格式的 XML，按文件内容自动识别格式）进 FIBEX 查看窗口；拖放与命令行走同一分流逻辑
  - File > Import as DBC... 保留：将 ARXML/KCD 显式转换为可编辑的 DBC 模型
- **所有表格启用横向滚动与列宽自适应**（ImGui 原生 `SCROLL_X` + `SIZING_FIXED FIT` 语义，非手写逻辑）
  - 列宽自动适配当前内容（内容长则列宽随之变宽，过宽时出现表格水平滚动条）
  - 注释 / Message / Node 等"长文本"列改为 `WidthStretch`，自动占满表格剩余宽度
  - fibex 各表格移除手写的固定列宽，交由 ImGui 按内容计算

### Added
- **整合 roxy-fibex：支持 FIBEX XML 与 AUTOSAR ARXML (FlexRay) 数据库**
  - 新增 FIBEX 查看窗口（五标签页）：帧（名称/长度/通道/时隙/基础周期/重复周期/启动/PDU 数）、PDU 列表、信号列表、ECU 列表、集群参数
  - 打开 .fibex / .fx / .xml / .arxml 文件自动识别格式（FIBEX 3.0 / AUTOSAR R4.x），拖放与命令行同样支持
  - File 菜单新增 Load FIBEX / ARXML、Save FIBEX / Save FIBEX As（按扩展名选择 FIBEX / ARXML 保存格式）
  - FIBEX 编辑功能完整移植：帧 / PDU / 信号增删改与撤销重做、位布局、调度表视图

### Fixed
- **Ctrl+O 弹出两次文件选择对话框**：文件级快捷键（Ctrl+O / Ctrl+N / Ctrl+S）与 Edit 菜单的全局快捷键段重复注册，同一按键触发两次；现已去除重复，Ctrl+Z/Y/C/X/V 等 DBC 编辑快捷键按焦点分发
- **ARXML 解析读不全的问题**：PDU 的信号映射元素标签为 `I-SIGNAL-TO-I-PDU-MAPPING`（此前只匹配容器名 `I-SIGNAL-TO-PDU-MAPPINGS`，导致一个信号都解析不到，PDU 信号数恒为 0）
- 集成测试以 PowerTrain.arxml 断言解析结果与 Vector CANoe 一致：48 帧 / 24 PDU / 信号非空 / 时隙与重复周期正确

### Changed
- **ARXML 解析增强（对齐 Vector CANoe 显示内容）**
  - PDU / 信号的发送与接收节点（Senders / Receivers）：从 ECU 端口 `I-PDU-PORT` / `I-SIGNAL-PORT` 的方向后缀（_Tx / _Rx）解析；PDU 列表与信号列表新增"发送 / 接收"列
  - 物理值换算：解析 `COMPU-METHOD`（`COMPU-RATIONAL-COEFFS` 分子 [offset, factor]）填入信号因子 / 偏移；枚举 `COMPU-SCALE` 解析为信号值表

## [0.8.0] - 2026-09-18

### Added
- **Message 编辑的发送节点改为下拉选择**：Transmitter 从 Node List 的节点列表中选择（含 `Vector__XXX` 选项）；文件中的历史值不在节点列表时兜底追加显示，不会被静默丢弃

### Changed
- **移除 DBC 窗口底部的文件信息行**，消除窗口右侧的滚动条：表格填满标签页全部剩余空间
  - 文件完整路径（含所在文件夹）移到 DBC 窗口标题栏显示
  - 消息数显示在 Messages & Signals 标签页过滤框右侧（随过滤实时变化），信号数显示在 All Signals 标签页过滤框右侧

### Fixed / Improved
- **同名文件窗口重叠**：DBC / Message / 编辑窗口的标题 ID 改为包含完整文件路径——两个不同目录的同名 DBC 不再互相覆盖成重叠窗口；窗口脏标记 `*` 的变化也不再导致布局重置
- **最近文件去重**：打开文件的路径统一规范化（解析相对 / 绝对、`/` 与 `\`、`..` 等），同一文件的不同写法在最近文件中只保留一条；Recent Files 菜单项 ID 带上完整路径，同名文件不再触发 ImGui ID 冲突警告
- **ID 冲突警告**：All Signals 中不同消息下的同名信号（如 motbus.dbc 的 WheelSpeedFR）、Message 窗口中的重复信号名，控件 ID 均已唯一化，不再弹出 "conflicting ID" 错误弹窗
- **Communication Matrix 配色**：RX 改为柔和的珊瑚红，TX 改为醒目的亮绿色，部分接收仍为橙色 R*
- **表格高度**：所有标签页的表格为底部状态栏预留 20px，既不挤出状态栏也不留大片空白
- **表格布局**：所有标签页的表格高度为底部文件信息状态栏让位，不再把状态栏挤出窗口；搜索输入框改为固定宽度，不再触发横向滚动
- **All Signals**：移除多余的行选择提示；无选中行时 "+ Add Signal" 显示为禁用态；工具栏改用标准按钮
- **Node List**：改为表格布局（Node / TX Messages / RX Signals / Actions），新增每个节点的发送报文数与接收信号数统计，操作按钮对齐
- **Communication Matrix 可编辑**：点击信号行节点单元格切换该节点的接收状态；点击消息行节点单元格切换发送节点（再次点击当前 TX 恢复 Vector__XXX）
- **Communication Matrix 部分接收标注**：节点只接收报文中的部分信号时显示橙色 `R*`（悬停提示 `Receives K of N signals (partial)`），全部接收才显示 `RX`
- **表格样式**：所有表格恢复列边框线并启用 ImGui 自带的奇偶行斑马纹（`ROW_BG`）
- **Message 窗口位布局图限高**：FD 长报文（最多 64 字节行）的位布局在可滚动的子区域内显示（约窗口 35% 高度），不再把信号表格挤出窗口
- **Validation Results 窗口**：改为固定默认尺寸 + 结果表格填满窗口
- **粘贴去重**：粘贴消息 / 信号时名称自动去重（`_copy` / `_copy2` ...），消息 ID 分配跳过已占用 ID 并在超过 29 位上限后回绕查找空闲 ID

### Removed
- 移除未使用的 `MessageCreateDialog` 模块及 UI 状态中的死字段（`next_dbc_id`、`message_window_focus_request`、`last_focused_message_window`、`MessageWindow::parent_dbc_id`）；合并 Edit 菜单与右键菜单中重复的复制 / 粘贴 / 新建消息实现

## [0.7.0] - 2026-09-17

### Added
- **命令行打开文件**
  - 启动时可通过命令行参数直接打开文件：`roxy-dbc.exe <file.dbc> [more.dbc ...]`
  - 支持 .dbc / .arxml / .kcd，可传多个文件（按顺序打开，聚焦最后一个）
  - 传入的文件计入最近文件列表；解析失败弹出错误对话框而不会崩溃
  - 支持注册 Windows"打开方式"关联
- **DBC 窗口标签页重构**（CANdb++ 风格）
  - DBC 窗口顶部改为四个标签页：Messages & Signals / All Signals / Node List / Communication Matrix
  - Messages & Signals 标签页的 "All Signals" 按钮直接切换到 All Signals 标签页
  - Node List 从独立对话框移入标签页（Tools 菜单原入口移除），节点增删改现在会正确标记脏状态
- **Communication Matrix 标签页**（CANdb++ 风格 TX/RX 关系矩阵）
  - 行 = 消息（可展开为信号），列 = 网络节点
  - 发送节点列显示 TX，接收该消息任一信号的节点显示 RX；展开后显示每个信号各自的 RX
  - 首列与表头冻结，支持纵向 / 横向滚动
- **All Signals / 通信矩阵窗口**（现位于 All Signals 标签页）
  - 将 DBC 中所有消息的信号展平为一张通信矩阵：ID、Message、DLC、Transmitter + 信号全部属性共 17 列
  - 任意列排序、按信号名 / 消息名 / 发送节点过滤
  - 双击信号打开信号编辑对话框；右键 Edit Signal / Edit Message / Delete Signal
  - "+ Add Signal" 添加到选中行所在的消息，自动命名（NewSignal_0001 风格，跳过重名）
  - 表格标题行冻结，滚动时保持可见（所有表格均已应用）
- **信号 Values（值表）编辑**
  - 通信矩阵 Values 列双击进入行内编辑，格式 `0=Off; 1=On`，非法输入红字提示并阻断应用
  - 应用时按值排序并合并重复检查，走统一的 Undo/Redo；悬停显示完整值表
  - 结构化的值表编辑仍保留在信号编辑对话框中（含剪贴板批量导入）
- **通信矩阵导出** - "Export CSV..." 一键导出当前过滤后的矩阵（UTF-8 BOM，Excel 打开中文不乱码）
- **中文支持**
  - 中文显示：UI 字体从系统加载 CJK 回退字体（微软雅黑 / 黑体 / 宋体，按序探测），与 Inconsolata 合并为同一图集，拉丁保持 Inconsolata、中文走系统字体（与 roxy-can 方案一致）
  - GBK 编码：打开 DBC/ARXML/KCD 时嗅探编码（UTF-8 BOM → 严格 UTF-8 → GBK），国内工具链（CANdb++ ANSI）导出的 GBK 文件不再乱码
  - 保存保持原编码：GBK 文件保存后仍是 GBK，与其它工具互换无障碍；另存为新路径默认 UTF-8
- **CAN FD 支持**
  - 消息帧格式扩展为 Standard / Extended / Standard FD / Extended FD，编辑对话框新增 CAN FD 开关
  - DLC 上限按帧类型区分：经典 CAN 8 字节、CAN FD 64 字节（含合法 FD 长度提示）
  - 保存时写出 Vector 风格的 `VFrameFormat` / `BusType` 属性，打开时自动解析 CAN FD 标志（DLC > 8 兜底识别）
- **消息 ID 可编辑**
  - 编辑对话框支持十六进制（0x 前缀）与十进制输入，带 ID 范围与重复校验，错误时阻断保存
  - ID 修改后已打开的 Message 窗口 / 信号编辑对话框自动跟随新 ID
- **信号接收节点（Receivers）编辑** - 信号编辑对话框新增逗号分隔的 Receivers 输入框
- **信号表格补充列** - Message 窗口信号表新增 Min、Max、Receivers 列（对齐 CANdb++ 风格）
- **校验规则增强**（Tools > Validate）
  - 经典 CAN 超过 8 字节报错、CAN FD 非法 DLC 警告、ID 超出标准/扩展范围报错
  - factor 为 0 报错、min > max 警告
  - 消息 / 信号 / 节点名称不符合 DBC 标识符规则时报错
  - 发送节点与接收节点未在节点列表中定义时警告
  - Motorola 信号按实际位号判断越界（原先按起始位 + 长度线性相加会误报）
- **节点重命名自动传播** - 重命名节点时同步更新消息发送节点与信号接收节点，并合并为单步撤销
- **自研 Win32 剪贴板后端** - 输入框 Ctrl+C/V 与值表"从剪贴板导入"在迁移后依然可用

### Changed
- **UI 框架迁移**：从已停止维护的 imgui-rs 0.12（imgui / imgui-wgpu / imgui-winit-support）迁移至
  [dear-imgui-rs](https://github.com/Latias94/dear-imgui-rs) 0.18（dear-imgui-rs + dear-imgui-wgpu + dear-imgui-winit），
  升级至 Dear ImGui 1.92.9b（docking 分支），动态字体系统按 DPI 自动光栅化
- 消息表 ID 列对扩展帧显示 `x` 后缀（CANdb++ 风格）
- 最近文件列表改存 `%APPDATA%\roxy-dbc\recent_files.txt`，避免安装到 Program Files 时写入失败
- 键盘导航（方向键 / Enter / Del）在文本输入框激活时不再误触发

### Fixed
- 打开 DBC 时信号注释（`CM_ SG_`）丢失的问题
- 扩展帧消息注释（`CM_ BO_`）保存时未使用 raw ID，导致重新打开后注释丢失的问题
- 消息被删除后仍打开的编辑窗口中点击 Apply 会崩溃的问题；被删消息的 Message / 编辑窗口现在会自动关闭
- 从错误路径打开 DBC 时直接 panic 的问题，解析失败的错误详情现在会完整显示

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
