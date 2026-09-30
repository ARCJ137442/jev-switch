# Android APP 实验性计划

**作者：** GPT-6 Luna xhigh（OpenAI Codex）  
**AI 披露：** 本文由 AI 协助整理；Android 构建、安装和真机结论必须由后续 CI 与人工设备验收产生。

## 定位与完整性承诺

Android APP 是面向没有 Termux 环境的普通移动端用户的实验性 Tauri 2 交付形态。它必须是“下载、安装、点击即可使用”的完整 Jev-Switch 应用：同一个 APK 包含可在 Android 上编译的 daemon 核心，用户可以在 APP 内启动、停止和查看本机网关；默认首次安装不启动网关，避免用户尚未配置提供商时消耗资源或暴露端口。

远端/局域网连接仍然是可选模式，用于把 APP 当作已有 Jev-Switch 的控制台；它不能替代本地网关能力，也不能成为 Android 首版的唯一工作方式。Windows sidecar 不能直接搬进 APK，Android 目标必须使用进程内 daemon/library 或明确的 Android 原生服务边界。

设计上以 [FlClash](https://github.com/chen08209/FlClash) 的后台服务实现为主参考，以 [LocalSend](https://github.com/localsend/localsend) 的“同一功能核心、各平台提供原生运行边界”为辅参考：React/Tauri 负责跨平台控制台与用户意图，daemon 负责协议、路由、SQLite 和上游适配，Android 原生 service 负责前台服务、通知、系统 action、Quick Settings tile、进程恢复和系统权限。这样旋转屏幕或外接键盘变化只重建界面，不重建已启动的网关；用户停止网关时才明确结束服务生命周期。

这两个公开项目给出的可迁移事实是：LocalSend 维护跨平台共享协议/核心，并为 Android、桌面、CLI 分别提供交付渠道，同时把便携数据目录和后台隐藏等运行选项写成明确行为；FlClash 更直接地把网关/代理当作后台服务，通过 Android action（START/STOP/TOGGLE）、前台通知和独立的 Android 生命周期代码控制服务。对 Jev-Switch 来说，FlClash 的 service/通知/action/tile 体系是主要参考，LocalSend 的核心复用与发行渠道是次要参考；不复制它们的协议、VPN 或传输实现。

### FlClash 风格的服务控制原则

1. **服务是事实源**：Dashboard、命令面板、通知按钮和 Quick Settings tile 都只发送 `START`、`STOP`、`TOGGLE` 意图，运行状态由 Android service 与 daemon listener 返回；UI 进程重建不能自行推断“已启动”。
2. **前台通知是服务契约**：用户启动本机网关后显示通知，通知上呈现运行/停止状态、实际监听地址和停止动作；保活开关只控制通知策略，不改变 daemon 的路由语义。关闭通知前提示 Android 可能回收后台服务。
3. **系统动作可追溯**：通知、tile 和 APP 内操作都写入同一生命周期日志/状态序列，启动失败、端口冲突、权限拒绝和系统回收必须可见；不得出现“tile 显示开、端口已关”的双重事实。
4. **生命周期与 Activity 分离**：配置变化、旋转、键盘、返回桌面只影响 Activity；service 继续持有 daemon、SQLite 和端口。真正的进程被系统杀死时，按持久化的期望运行状态执行恢复或保持关闭，并把结果反馈给通知和 UI。

系统级控制也属于完整性的一部分：Android 通知操作和 Quick Settings tile 至少提供 `START`、`STOP`、`TOGGLE` 三个动作，调用与 Dashboard 相同的生命周期服务，不另造一套状态。网关运行时默认显示常驻保活通知，设置中提供“保活通知”开关；关闭前提示系统可能回收后台服务，且不能把“通知消失”误报为“网关仍稳定运行”。Android 13+ 通知权限、Quick Settings tile 安装/可见性和锁屏操作权限都要单独验收。

调试日志同样采用“默认关闭、按需打开”的方式：设置页临时开启后，Android service 将生命周期、监听地址、端口冲突、权限拒绝、恢复结果和错误类别追加到应用私有目录；关闭后停止追加。日志不得写入请求体、答案、provider key、调用 Token 或完整 Authorization 头，并应提供清理/导出路径，供后续 APK 真机问题复现而不扩大敏感数据面。

```text
普通 Android 用户
  └─ 实验性 Jev-Switch APK
       ├─ 触屏控制台：Dashboard / Providers / Entries / Routing / Playground
       ├─ 本机网关：默认关闭 → 用户启动 → Android 生命周期托管
       ├─ 可选连接：本机局域网或云端 Jev API
       └─ 不承诺：桌面托盘、Windows sidecar、稳定推送通知
```

## 分阶段交付

### Phase A：CI 可构建

- 在 GitHub Actions Windows runner 上安装 Java 17、Android SDK/NDK 和 Rust Android targets。
- 执行 Tauri Android 初始化，生成受版本控制边界约束的 Android 工程。
- 构建明确标注实验性的 debug-signed APK，优先 `aarch64`，可选 `x86_64` 模拟器 APK；使用 Android debug keystore 只供安装验收，不作为正式签名或 Play 发布凭据；不依赖本机 Android SDK。
- 构建必须编译 Android 目标所需的 daemon library，并由 Tauri Android 入口装配为可启停的进程内监听；不能只打包一个没有本地内核的 WebView。
- 将 APK、构建日志、版本、commit SHA、ABI 和 SHA-256 上传为 workflow artifact；不自动发布为稳定桌面 Release 资产。
- 首阶段不要求签名密钥；接入签名、公钥校验和 Play 发布前必须另设安全方案。

### Phase B：移动 WebView 验收

- 用 Android emulator 或维护者设备验证启动、首次引导、网关默认关闭、APP 内启动/停止、首次配置、云端/局域网地址配置、登录、主题和语言跟随系统。
- 网关启动后切换到后台、锁屏、旋转屏幕、弹出/收起软键盘、连接/断开外接键盘，再回到 APP；监听地址、SQLite、配置和请求历史必须保持，不能因 Activity 重建而重启或丢失 daemon。
- 使用 Android 前台服务/等价系统生命周期边界维持用户明确启动的本机网关，并显示默认开启的常驻保活通知；通知和 Quick Settings tile 的 `START`/`STOP`/`TOGGLE` 都必须经过同一生命周期状态机。停止操作必须释放端口、结束服务任务且不删除配置。Android 系统真正杀死进程时，按持久化的“期望运行状态”执行可解释的恢复或保持关闭，不伪造在线。
- 验证触屏最小命中区、窄屏横向滚动、Routing 画布平移/缩放/右键替代手势和 Playground 结果卡片。
- 明确网络权限、明文局域网地址、HTTPS、后台限制、功耗和 Android 版本差异；断网时显示可恢复状态。

### Phase C：实验性发布

- 只有 Phase A/B 有可复核证据后，才在 GitHub Release 中附加标注 `Experimental Android APK` 的资产或独立实验性 Release。
- Release 文案必须写明 ABI、最低 Android API、是否签名、数据存储位置、已知限制和“无真机/仅模拟器”边界。
- 不把实验性 APK 写进 Windows 桌面稳定支持矩阵，不承诺本地模型推理、桌面托盘、多实例或推送通知。允许承诺的是“实验性本机网关”：必须附上已验证的 Android 版本、ABI、后台时长和已知功耗边界。

## 与现有架构的边界

- Android 本机模式复用 daemon 的 `build_state`、`build_app` 和监听层，在 Tauri Android 进程或专门的 Android service 边界内装配；HTTP 契约仍是 `/health`、`/v1/models`、`/v1/systemone` 和管理 API，provider key 仍只由 daemon 持有。
- Android 首次安装默认关闭本机网关。用户在 Dashboard 或命令面板明确启动后，才绑定端口；停止后端口释放，配置/数据库保留。远端/局域网地址是单独的连接目标，不得覆盖本机网关的状态。
- Android 数据目录使用应用私有目录，保存 providers 配置、SQLite、期望运行状态和迁移标记；不写入 APK 目录，也不把密钥放入前端 localStorage。
- 平台差异集中在 Tauri 配置、Android 原生生命周期/前台服务、通知与 Quick Settings tile、网络权限和数据目录适配；业务路由、provider、统计与 Playground 不复制第二套核心。

### 跨平台运行边界

```text
React/Tauri UI（可重建）
        │ 结构化 start / stop / status 意图
        ▼
Android service bridge（不可因 Activity 重建而丢失）
        │ 绑定应用私有数据目录、前台服务与通知动作
        ▼
Jev daemon library + ListenSupervisor
        │
        ├─ 127.0.0.1:11435（默认本机模式，可配置端口）
        └─ SQLite / providers.toml / desired-running.state

系统控制面
  通知 START / STOP / TOGGLE  ─┐
  Quick Settings tile       ───┼─→ 同一 LifecycleStateMachine → daemon listener
  Dashboard / Command Palette ┘
```

首版实现顺序：先让进程内 daemon 在 Android target 编译并能由 Tauri command 启停；再把同一状态机接入前台服务、通知动作和 Quick Settings tile；然后接入配置变化恢复、通知权限与省电策略；最后才做远端模式、深度省电优化和发布签名。任何阶段如果只能打开 WebView、不能证明本机 `/health` 或系统级启停动作，都只能标记为“控制台预览”，不能称为完整 Android APP。

## 验收门槛与风险

| 项目 | 通过条件 | 当前状态 |
|---|---|---|
| GitHub CI 构建 | 指定 commit 可生成包含 Android daemon 的 APK，artifact 可下载并能复算 SHA-256 | **通过**；Actions run #36721589274 产出 APK+JSON，并已附入 v0.4.1 Release |
| ABI/版本元数据 | APK 名称、版本、ABI、commit、签名类型和实验性标签一致 | **通过自动核对**；arm64 / 0.4.1 / commit `3379586` / debug keystore |
| 真机启动 | 至少一台普通 Android 设备安装 APK，默认网关关闭，控制台可打开 | 未开始；尚待维护者人工安装和验收 |
| 本机网关 | APP 内启动/停止成功；`/health`、`/v1/models` 和本机调用可用 | 进程内启停原型已写入工作树；未有 Android APK/真机证据 |
| 系统级启停 | 通知动作和 Quick Settings tile 的 START/STOP/TOGGLE 与 Dashboard 状态一致 | 未开始；需 Android 原生 service/tile |
| 保活通知 | 默认开启；设置可关闭并提示后台回收风险；权限拒绝有降级说明 | 未开始；需 Android 13+ 通知权限验收 |
| 调试日志 | 默认关闭；设置临时开启后落盘生命周期/错误元数据，不含请求敏感信息 | 设置与 Tauri command 原型已写入工作树；尚无 Android APK/真机文件核对 |
| 生命周期稳定性 | 后台、锁屏、旋转、软键盘和外接键盘变化不重启已运行网关；进程恢复策略可解释 | 未开始；前台服务/原生生命周期桥仍待实现 |
| 网络访问 | HTTPS 云端和用户明确选择的局域网地址可连接；断网可恢复；不覆盖本机状态 | 未开始 |
| 触屏交互 | Dashboard、Routing、Playground 无关键遮挡，画布操作可完成 | 未开始 |
| 安全边界 | 不暴露 provider key，不静默扫描局域网，不把未授权服务写入配置 | 设计约束 |

## 用户叙事

没有 Termux 的普通用户从 GitHub Release 下载实验性 APK，安装后第一次打开看到的是“网关当前关闭”的清晰状态；他可以先配置提供商，再点一次“启动网关”，APP 在后台保持本机服务并显示实际地址。切到别的 APP、旋转屏幕或弹出键盘回来，入口、历史和监听状态仍然在。若他只想管理另一台机器，也可以切换到云端/局域网地址；两种模式的状态和数据边界会明确分开，而不会把一个远端地址伪装成本机网关。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
