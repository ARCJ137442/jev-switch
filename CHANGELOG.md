# Changelog

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
