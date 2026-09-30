# Android APK 构建核验

**作者：** GPT-6 Luna xhigh（OpenAI Codex）
**AI 披露：** 本记录由 AI 协助构建与整理；没有 Android 安装、模拟器或真机运行结论。

## TL;DR

Android `aarch64` APK 已由本机和 GitHub Actions 构建成功；CI 的 debug-keystore 签名 APK 与 metadata 已补入 v0.4.1 Release，可下载侧载。构建验证到 Gradle/AAPT/APK 产物为止；没有启动模拟器或安装测试，也没有把“能打包”写成“Android 支持已完成”。

## 本机 Release 构建

| 项目 | 结果 |
|---|---|
| ABI | `arm64-v8a` |
| 版本 | `0.4.1` |
| 产物 | `app-arm64-release-unsigned.apk` |
| 大小 | `13,778,362` bytes |
| SHA-256 | `6B3CE6F86B9B64EA0AB68842D0511EE9BC6EBDE9339634A1368E2D4F72BE4F01` |
| 签名 | unsigned；不能直接安装 |

## GitHub Actions 实验性构建

| 项目 | 结果 |
|---|---|
| Actions run | [#36721589274](https://github.com/ARCJ137442/jev-switch/actions/runs/36721589274) |
| Commit | `33795864567a6af3a8847133d5a9a832f16d0f7c` |
| ABI / variant | `arm64-v8a` / `arm64Debug` |
| 包名 / version | `io.github.arcj137442.jevswitch` / `0.4.1` |
| APK | [jev-switch-0.4.1-android-aarch64-experimental.apk](https://github.com/ARCJ137442/jev-switch/releases/download/v0.4.1/jev-switch-0.4.1-android-aarch64-experimental.apk) |
| Metadata | [jev-switch-0.4.1-android-aarch64-experimental.json](https://github.com/ARCJ137442/jev-switch/releases/download/v0.4.1/jev-switch-0.4.1-android-aarch64-experimental.json) |
| Size / SHA-256 | `18,420,598` bytes / `1F5567A4263B8EC550F8CEA24630F13FFCD1EE70E9A1E3891AF317A3290D1A9D` |
| Signing | Android debug keystore，已用 `apksigner verify --print-certs` 确认 |
| Human runtime acceptance | 未开始 |

GitHub Actions run #36721589274 全部步骤成功，APK 与 JSON metadata 已附入 v0.4.1 Release。该 job 的 workflow 于之后通过独立 CI 修复提交稳定下来；APK 内容对应 v0.4.1 提交 `3379586`。

该 debug-signed APK 可以侧载安装用于实验性验收；签名仅用于开发/测试，不能当作正式发布密钥或 Play Store 包。构建成功不证明安装后启动、本机网关、通知/tile 或后台生命周期正确。

## 构建路径

1. 用 Tauri Android 初始化生成工程，复用 React `ui/dist` 与 Android target 的 daemon library。
2. 用 Android NDK clang、`llvm-ar` 和 `llvm-ranlib` 编译 `libjev_switch_shell.so`。
3. 把 `.so` 放入 `arm64-v8a` JNI 目录，由 Gradle `assembleArm64Release` 生成本机 unsigned 核验包；GitHub Actions 同样编译 `.so` 后用 `assembleArm64Debug` 生成 debug-signed 实验包。
4. Windows 当前未启用开发者模式，Tauri CLI 的 JNI symlink 步骤不能直接使用；CI 采用“直接复制 `.so` + 跳过重复 Rust link task”的等价路径。

## 尚未验证

- APK 安装、首次启动、默认关闭网关与 APP 内启停。
- 前台 service、常驻通知、通知 action、Quick Settings tile 与后台恢复。
- 旋转、锁屏、外接键盘、软键盘、进程被系统回收后的恢复。
- Android 调试日志实际写盘、关闭后停止追加、512 KiB 滚动与敏感字段排除。

设置中的 Android 调试日志默认关闭；开启后只记录生命周期、监听地址、端口冲突、权限拒绝、恢复结果和错误类别，不记录请求体、答案、provider key、调用 Token 或完整 Authorization 头。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助构建与整理
