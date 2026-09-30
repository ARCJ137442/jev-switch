# Android APP 实验性计划

**作者：** GPT-6 Luna xhigh（OpenAI Codex）  
**AI 披露：** 本文由 AI 协助整理；Android 构建、安装和真机结论必须由后续 CI 与人工设备验收产生。

## 定位

Android APP 是面向没有 Termux 环境的普通移动端用户的实验性 Tauri 2 交付形态。它的首要价值是让用户在手机上直接打开 Jev-Switch 控制台，查看状态、管理入口/提供商、运行 Playground，并连接本机或云端已有的 Jev 服务；它不是把 Windows sidecar 原样搬进手机，也不替代 Termux + 浏览器 Web UI。

```text
普通 Android 用户
  └─ 实验性 Jev-Switch APK
       ├─ 触屏控制台：Dashboard / Providers / Entries / Routing / Playground
       ├─ 连接：本机局域网或云端 Jev API
       └─ 不承诺：Android 本地 daemon、桌面托盘、Windows sidecar、后台常驻
```

## 分阶段交付

### Phase A：CI 可构建

- 在 GitHub Actions Windows runner 上安装 Java 17、Android SDK/NDK 和 Rust Android targets。
- 执行 Tauri Android 初始化，生成受版本控制边界约束的 Android 工程。
- 构建 unsigned/debug 或明确标注实验性 release APK，优先 `aarch64`，可选 `x86_64` 模拟器 APK；不依赖本机 Android SDK。
- 将 APK、构建日志、版本、commit SHA、ABI 和 SHA-256 上传为 workflow artifact；不自动发布为稳定桌面 Release 资产。
- 首阶段不要求签名密钥；接入签名、公钥校验和 Play 发布前必须另设安全方案。

### Phase B：移动 WebView 验收

- 用 Android emulator 或维护者设备验证启动、首次连接、云端/局域网地址配置、登录、主题和语言跟随系统。
- 验证触屏最小命中区、软键盘遮挡、窄屏横向滚动、Routing 画布平移/缩放/右键替代手势和 Playground 结果卡片。
- 明确网络权限、明文局域网地址、HTTPS、后台切换和进程回收行为；断网时显示可恢复状态，不伪造 daemon 在线。

### Phase C：实验性发布

- 只有 Phase A/B 有可复核证据后，才在 GitHub Release 中附加标注 `Experimental Android APK` 的资产或独立实验性 Release。
- Release 文案必须写明 ABI、最低 Android API、是否签名、数据存储位置、已知限制和“无真机/仅模拟器”边界。
- 不把实验性 APK 写进 Windows 桌面稳定支持矩阵，不承诺后台 daemon、本地推理、托盘、多实例或推送通知。

## 与现有架构的边界

- Android 首版优先作为远程/局域网控制台客户端，调用既有 `/health`、`/v1/models`、`/v1/systemone` 和管理 API；provider key 仍由 daemon 持有。
- Android 不默认启动或打包 Windows sidecar；若未来需要手机本地 daemon，必须另做 Android 进程生命周期、数据目录和功耗专项。
- 复用现有 React UI 和 Tauri 入口，平台差异集中在 Tauri 配置、Android 权限、网络地址输入和生命周期适配，不复制一套移动业务核心。

## 验收门槛与风险

| 项目 | 通过条件 | 当前状态 |
|---|---|---|
| GitHub CI 构建 | 指定 commit 可生成 APK，artifact 可下载并能复算 SHA-256 | 待实施 |
| ABI/版本元数据 | APK 名称、版本、ABI、commit 和实验性标签一致 | 待实施 |
| 真机启动 | 至少一台普通 Android 设备安装并打开控制台 | 未开始 |
| 网络访问 | HTTPS 云端和用户明确选择的局域网地址可连接；断网可恢复 | 未开始 |
| 触屏交互 | Dashboard、Routing、Playground 无关键遮挡，画布操作可完成 | 未开始 |
| 安全边界 | 不暴露 provider key，不静默扫描局域网，不把未授权服务写入配置 | 设计约束 |

## 用户叙事

没有 Termux 的普通用户从 GitHub Release 下载实验性 APK，打开后输入自己的云端或局域网 Jev-Switch 地址，直接查看网关状态、测试入口和 Playground 结果；他不需要理解 Rust、daemon、端口转发或 Android 开发环境。遇到不支持的后台行为，APP 明确显示“需要保持远端服务运行”，而不是假装手机本地已经提供完整网关。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
