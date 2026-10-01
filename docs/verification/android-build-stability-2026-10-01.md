# Android APK 图标与签名稳定性核验

**作者：** GPT-6 Luna xhigh（OpenAI Codex）
**AI 披露：** 本文由 AI 协助整理；签名密钥由项目维护者独立保管。
**状态：** 本地源码、图标生成、Android Rust library、Kotlin keepalive/tile 编译和 arm64 debug APK 已验证；GitHub Secrets、Release CI 正式签名包与真机生命周期尚未验证
**决策：** Android APK 作为正式下载渠道发布，维护优先级次于 Windows；不再添加“实验性”发行标签。

## 问题与原因

- 安装截图中的彩色双轨标志与本机旧 `src-tauri/gen/android` launcher 资源一致；该生成目录被 Git 忽略。仓库原跟踪的 `src-tauri/icons/icon.png` 却是黑底白色 `J`，干净 CI checkout 无法复用本机的图标状态，桌面和 Android 也没有同一受控来源。
- Android workflow 原先构建 `assembleArm64Debug`。Windows runner 每次都是新机器，Gradle 生成的 debug keystore 不同，因此新 APK 可能无法覆盖安装到旧 APK 上。截图中的 `INSTALL_FAILED_UPDATE_INCOMPATIBLE (-7)` 与签名证书不匹配一致。
- Tauri Android config 版本字段落后于 UI 主版本，Android Gradle properties 缺省时使用 `1.0`，无法表达真实版本和升级顺序。

## 本轮修复

- 把用户可见的 Jev 彩色轨道标志提升为 `src-tauri/app-icon.png` 唯一图标源，并由 `cargo tauri icon` 生成桌面与各平台图标资源。
- Android 初始化后重新从图标源生成并同步所有 launcher mipmap 到 `gen/android`，不依赖被 Git 忽略的本机旧目录。
- 将 Android workflow 改为可复用的 Release 构建，主 Release 在 Android 构建与签名成功后才发布；手动构建也使用相同的正式签名链。
- APK 经 `zipalign`、固定 keystore 的 `apksigner sign` 和 `apksigner verify`。manifest 记录签名证书 SHA-256、APK SHA-256、图标源 SHA-256、commit、ABI、版本名和版本码。
- Android `versionName` 与 SemVer 同步；`versionCode = major * 1,000,000 + minor * 1,000 + patch`，保证版本升级时单调增加。
- 主 Release 版本门禁现同时检查 Android Tauri config 版本。

## 本地验证

- `cargo tauri icon src-tauri/app-icon.png --output src-tauri/icons` 成功生成桌面与 Android 图标资源。
- `pwsh -NoProfile -File scripts/android/prepare-android-project.ps1` 成功；生成资源写入本机 `gen/android`，共同步 17 个图标文件；Gradle properties 为 `versionName=0.5.0`、`versionCode=5000`。
- 使用本机 Android SDK `C:\Users\56506\AppData\Local\Android\Sdk` 与 NDK `27.3.13750724`，`cargo build --target aarch64-linux-android --features tauri/custom-protocol --lib --release` 通过；Tauri Android library 生成 `tauri.settings.gradle` 与 Kotlin bridge 文件。
- `:app:compileArm64DebugKotlin -Pkotlin.incremental=false --offline` 与 `:app:assembleArm64Debug` 通过，包含前台保活 service、通知权限插件、Quick Settings tile 和统一网关 toggle 事件桥；`app-arm64-debug.apk`（0.6.0）SHA-256：`84271D7ED2E63D924B819FEA1F6E4FEE31270F36C1FC1475186FD2D3B92210C4`。
- 生成的 `icon.png`、Android launcher foreground 与用户截图中的 Jev 彩色标志一致。资源源文件已跟踪，不再以 ignored build tree 作为真相。

## 尚未验证

- Android release APK 尚未在本机或 GitHub Actions 以稳定 keystore 签名；新 Release CI 需要仓库配置 `ANDROID_KEYSTORE_BASE64`、`ANDROID_KEYSTORE_PASSWORD`、`ANDROID_KEY_ALIAS`、`ANDROID_KEY_PASSWORD` 四项 Actions Secrets。当前本机 APK 是 debug 签名，仅证明构建链和资源可打包。
- Secrets 内容不会写入仓库或打印。必须为 Jev-Switch 单独创建 keystore，不复用 ExoMind 或其他应用密钥，并在离线位置备份；丢失私钥将阻止对已安装版本做覆盖更新。
- 当前发布的 v0.5.0 APK 是 debug-signed 历史构建。迁移到第一个稳定签名版本时，Android 会拒绝覆盖安装；需先卸载旧版，再安装新版本。后续同一签名身份的版本才可正常覆盖升级。
- Android service、通知、Quick Settings、LAN、safe-area、旋转/键盘和后台生命周期仍按[Android 构建与验收计划](../design/ROADMAP-PLAN-ANDROID-EXPERIMENTAL.md)逐项做设备验收；移除发行标签不等于这些能力已经全部验证。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
