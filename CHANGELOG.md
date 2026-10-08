# Changelog

## [0.10.2] - 2026-10-08

### Fixed

- 路由普通写入改为原子 `POST /v1/admin/routes/transaction`；整表 `PUT /v1/admin/routes` 不再暴露。
- 服务入口创建/更新使用边级 `route_operations`，入口元数据与路由变更在同一 SQLite 事务中提交。
- Provider 单项更新/删除不再通过内部整表重建，删除时只清理失效路由分支，保留有效分支与历史记录。
- 边身份使用 `[left, right, match, upstream_model]` 的 canonical JSON 表示，兼容唯一的旧边 ID。

### Verification

- 全 workspace Rust 测试、`ts-rs` 类型门禁、UI 69 项测试、TypeScript lint 和生产构建通过。
- 全 workspace `cargo fmt` 已执行，Clippy 使用 `-D warnings` 验证；GitHub Release 将本版本标记为预发布修复预览。

### 用户叙事与对照场景

#### 配置修改：从整表覆盖到单资源原子变更

- **起点**：用户在路由、入口或提供商页面修改一个对象。
- **问题**：整表读改写可能覆盖并发或无关配置，失败后还可能留下非法路由图。
- **操作**：UI 提交边级操作或单 provider 变更；daemon 在快照副本/SQLite 事务中校验完整图后一次提交。
- **结果**：环、未知引用、重复边、入口路由越权和 provider 删除悬空分支都会在提交前拒绝，旧快照与运行时 Router 保持不变。
- **对照**：显式 JSON/TOML 导入仍是带确认的批量恢复路径；普通 UI、CLI、Agent 不再使用整表覆盖。
- **边界**：本版不引入额外 ETag/version 实体；写入由当前 daemon 的 SQLite 写锁串行保护。

## [0.10.1] - 2026-10-07

### Fixed

- 修复 Laya 状态脚本在服务不可达时隐式返回退出码 `0`，导致停止后仍显示“运行中”的问题；状态命令现在明确使用 `0`/`3` readiness 退出码。
- Laya 启停脚本按启动脚本的精确父子进程链识别本次服务，停止时不按端口或模糊进程名误杀。
- Providers 页面保留每个提供商的服务启停展开状态；打开面板时自动刷新生命周期状态。

### Added

- 每个提供商独立的“自动探测”开关，默认开启；首页和提供商页分别按该开关执行 API 探测。
- 公开的本地服务脚本指南、Laya/StartLux 脱敏示例、UI 配置步骤、跨平台原理说明与故障排查 FAQ。
- 模型运行时路径由外部脚本私有配置管理；Jev Switch 不注入或解释模型专用环境变量。

### 用户叙事与对照场景

#### 本地服务：从“停止后仍显示运行中”到可重复启停

- **起点**：用户需要在 Providers 页面多次启动、检查和停止 Laya/StartLux 服务。
- **问题**：Laya 状态脚本在失败分支返回了成功退出码，停止后 API 已不可达但 UI 仍显示运行中。
- **操作**：修正状态退出码、按所有权识别进程，并连续执行三轮“状态→启动→状态→停止”验证。
- **结果**：三轮均返回 `stopped/not_ready`，停止后服务端口不再监听；StartLux 完成真实 Jev 请求和停止回收；OneJev 连续两轮启停通过；维护者随后人工确认 Laya 在 UI 中可重复启停。
- **对照**：没有生命周期配置的云端 provider 仍保持手工管理，不会被自动启停。
- **边界**：OneJev 仍需要用户提供 qev 和模型文件；Jev Switch 不下载模型，也不代替脚本管理模型运行时。

## [0.10.0] - 2026-10-05

### Added

- Provider 级受控模型生命周期：结构化启动、停止和状态命令，`manual`、`startup_check`、`on_demand` 三种启动策略，以及 `persistent`、`session`、`external` 进程策略。
- readiness 检查、provider 互斥、进程身份快照、独立生命周期审计和稳定的 Agent/API 错误码。
- 设置中的“允许主机命令”总开关，默认关闭；开启时使用 5 秒风险确认。
- Providers 高级生命周期面板，支持命令配置、状态检查、启动/停止、API key 注入警告和脱敏状态反馈。
- `GET/PUT /v1/admin/providers/{id}/lifecycle` 与 `GET/PUT /v1/admin/host-commands` 管理端点。
- 模型画像与属性路由计划明确排期至 `0.11.0`，本版本不改变现有画像/候选过滤语义。

### Verification

- Rust workspace 默认测试、`ts-rs` 生成门禁、UI 测试、TypeScript lint 和生产构建通过。
- 本版本尚未完成 StartLux/OpenJev/Laya 真实服务、Windows/Unix 进程组、Android/cloud 权限和 Agent scope 的人工验收；因此标记为预发布版。

### 用户叙事与对照场景

#### 本地模型服务：从人工切换到受控生命周期

- **起点**：用户需要先单独打开 Laya、OpenJev 或其他本地模型服务，再回到 Jev Switch 配置地址和模型；关闭 Jev Switch 时又担心误停仍在使用的服务。
- **问题**：`允许路由`、服务是否运行和服务是否由 Jev Switch 控制混在一起，命令失败也难以区分是进程启动失败还是网关调用失败。
- **操作**：在 Provider 高级面板登记结构化 argv、状态/停止命令、启动策略与进程策略；设置中显式开启主机命令；Jev Switch 在进入候选时等待 readiness，并把生命周期失败交给既有 failover。
- **结果**：用户可分别看到路由资格、可控服务和服务状态；持久服务不会因应用退出自动停止，失败会留下独立审计记录和可定位的错误码。
- **对照**：无 lifecycle 配置的 provider 仍保持原有纯人工行为；云端 provider 不会因为“允许路由”而被当作可控本地服务。
- **边界**：本版本只提供受控命令执行和契约闭环，不下载模型、不提供任意 shell、不按端口或进程名接管服务；真实平台进程组和本地服务仍需人工验收。

## [0.9.3] - 2026-10-05

### Fixed

- Windows 发布流程接入 Authenticode SHA-256 签名、RFC 3161 时间戳和签后 `/pa` 校验。
- daemon、Tauri 壳、MSI、NSIS、Standalone 及 Portable ZIP 使用同一签名链；Release 附带签名清单。
- 正式 tag 缺少 Windows PFX 或密码时阻断发布，避免产生“发布者：未知”的正式包；workflow_dispatch 仍可生成明确标注的未签名干跑包。

### Verification

- PowerShell 签名脚本和发布 workflow YAML 静态解析通过。
- SmartScreen 信誉仍取决于公开 CA 证书与下载历史，不承诺新证书首次下载立即跳过所有提示。

### 用户叙事与对照场景

#### Windows 下载安全提示：从未签名包到可验证发布者

- **起点**：用户下载 Standalone EXE 时看到“发布者：未知”，无法确认文件是否来自 Jev Switch。
- **问题**：旧流程只构建和上传 EXE，没有 Authenticode 签名，Windows 无法显示受信任发布者。
- **操作**：Release tag 注入 CA 签发的 PFX，依次签名 daemon、Tauri 壳、安装器和 Standalone，并使用时间戳；签名清单记录证书指纹和文件 SHA-256。
- **结果**：用户可在文件属性和清单中核对签名、发布者、时间戳与哈希，安装包和独立包共享同一发布身份。
- **对照**：手动 workflow_dispatch 仍可用于无证书构建验证，但不会冒充正式签名 Release；新证书的 SmartScreen 信誉积累单独处理。
- **边界**：代码签名证明发布身份和文件完整性，不保证每台机器立即消除 SmartScreen 的下载信誉提示，也不替代 Defender 扫描。

## [0.9.2] - 2026-10-05

### Added

- Tauri 桌面顶级页面支持 Ctrl+Tab 顺序切换和 Ctrl+Shift+Tab 逆序切换，覆盖 Dashboard、Providers、Endpoints、Routing、Playground、Statistics、Settings。
- 快捷键只在 Tauri APP 中接管；浏览器环境保留原生标签切换，输入框、文本域、选择框和 contenteditable 保留原生编辑行为。

### Verification

- UI tests：69 项通过；TypeScript lint 与生产构建通过。
- 版本字段与锁文件统一为 `0.9.2`。

### 用户叙事与对照场景

#### 顶级页面快捷切换

- **起点**：用户在 Jev Switch 中反复比较 Dashboard、Providers、Routing 和 Playground，需要连续查看多个顶级页面。
- **问题**：鼠标需要往返顶栏，页面切换打断对照节奏；普通浏览器标签快捷键又不应被网页版本抢走。
- **操作**：在 Tauri APP 中按 Ctrl+Tab 顺序切换，按 Ctrl+Shift+Tab 逆序切换；焦点位于输入控件或可编辑区域时快捷键让位。
- **结果**：顶级页面像浏览器标签一样循环切换，当前页面状态和路由 hash 保持一致，七个页面都可到达。
- **对照**：Tauri APP 接管页面循环，普通浏览器保留浏览器标签切换；输入框保留编辑快捷键，只读用户只循环访问有权限的页面。
- **边界**：这是页面导航效率改进，不改变页面内部 Tab 焦点顺序、浏览器环境行为或页面权限。

## [0.9.1] - 2026-10-05

### Fixed

- Routing DAG 画布恢复普通滚轮上下平移。
- Routing DAG 画布支持 Shift+滚轮左右平移，并保留触控板原生横向滚轮增量。
- 保留 Ctrl/⌘+滚轮缩放、节点拖拽、键盘操作与触摸双指缩放路径。

### Verification

- UI tests：66 项通过；TypeScript lint 与生产构建通过。
- 版本字段与锁文件统一为 `0.9.1`。

### 用户叙事与对照场景

#### 多模态兼容

- **起点**：用户已经有 OneJev 图片决策服务，但下游程序只会调用 Jev Switch 的标准 `/v1/systemone`。
- **问题**：官方 TypeSafe 的稳定协议没有图像字段；把图片强塞进 `state` 只会得到不可解释的文本噪声。
- **操作**：本地 provider 在同一请求中使用 `extensions.media` 或顶层 `media`，开启 `forward_extensions = true`；Jev Switch 记录字段名和转发状态，不记录图片内容。
- **结果**：下游仍只调用一个 Jev API，兼容的本地服务可以收到媒体；官方 TypeSafe 路由会剥离媒体并留下可追溯状态。
- **对照**：纯文本请求无需扩展字段；同一请求切换官方 TypeSafe 与本地兼容 provider 时，协议保持一致但媒体处理结果不同。
- **边界**：Jev Switch 不负责图片缩放、抽帧、视觉判断或概率校准；多模态模型质量必须另行测量。

#### Routing DAG 滚轮平移

- **起点**：用户在大路由图上查看多个入口、别名节点和 provider 卡片。
- **问题**：普通滚轮事件没有更新画布平移量，用户只能拖拽空白区域，长图浏览成本高。
- **操作**：普通滚轮上下移动画布，Shift+滚轮左右移动；触控板原生 `deltaX` 优先，Ctrl/⌘+滚轮仍执行缩放。
- **结果**：用户可以像浏览文档一样快速扫过路由图，缩放锚点、节点拖拽、键盘 Escape 和双指缩放仍保持原有行为。
- **对照**：普通滚轮改变视口位置，Ctrl/⌘+滚轮改变比例，拖拽节点改变路由布局；三者分别作用于不同状态。
- **边界**：输入框、选择框和按钮保留原生滚轮行为；原生 Tauri 设备上的滚轮手感仍需人工抽查。

## [0.9.0] - 2026-10-05

### Added

- 支持在统一 `POST /v1/systemone` 请求中携带显式 `extensions` 或兼容服务使用的未知顶层字段。
- local 模式默认保留扩展字段；`strip_unknown_fields` 可全局启用边界剥离。
- TypeSafe-compatible provider 增加 `forward_extensions` 选项，允许 OneJev 等本地/网关服务接收 `media`、`images` 等多模态字段。
- 响应与失败 route trace 记录扩展字段名及 gateway/provider disposition，不记录媒体内容或 Base64 数据。

### Compatibility

- 官方 TypeSafe provider 默认不会收到未知扩展字段，保持当前文本/JSON API 边界。
- 真实 OneJev/LLM2Jev/OpenJev 多模态推理、质量与校准仍需按独立实验报告验收。

### Verification

- Rust workspace 默认测试与 `ts-rs` 生成门禁通过。
- UI tests、TypeScript lint 通过；生产构建和远端 GitHub Actions 以 tag workflow 结果为准。
- 详细范围见 [`v0.9.0` 发布核验](docs/verification/v0.9.0-release-candidate-2026-10-05.md) 与[多模态兼容核验](docs/verification/multimodal-systemone-compatibility-2026-10-05.md)。
