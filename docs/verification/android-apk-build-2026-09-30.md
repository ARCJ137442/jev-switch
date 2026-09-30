# Android APK 构建核验

**作者：** GPT-6 Luna xhigh（OpenAI Codex）
**AI 披露：** 本记录由 AI 协助构建与整理；没有 Android 安装、模拟器或真机运行结论。

## TL;DR

Android `aarch64` release APK 已在本机 SDK/NDK 环境中构建成功。构建验证到 Gradle/AAPT/APK 产物为止；没有启动模拟器，也没有把“能打包”写成“Android 支持已完成”。

## 产物

| 项目 | 结果 |
|---|---|
| ABI | `arm64-v8a` |
| 版本 | `0.4.0` |
| 产物 | `app-arm64-release-unsigned.apk` |
| 大小 | `13,778,362` bytes |
| SHA-256 | `6B3CE6F86B9B64EA0AB68842D0511EE9BC6EBDE9339634A1368E2D4F72BE4F01` |
| 签名 | unsigned；不能直接安装。后续 CI 安装验收改产 debug-signed experimental APK |
| 运行验收 | 未开始 |

## 构建路径

1. 用 Tauri Android 初始化生成工程，复用 React `ui/dist` 与 Android target 的 daemon library。
2. 用 Android NDK clang、`llvm-ar` 和 `llvm-ranlib` 编译 `libjev_switch_shell.so`。
3. 把 `.so` 放入 `arm64-v8a` JNI 目录，由 Gradle `assembleArm64Release` 完成资源、Kotlin、R8、AAPT 和 APK 打包。
4. Windows 当前未启用开发者模式，Tauri CLI 的 JNI symlink 步骤不能直接使用；CI 采用“直接复制 `.so` + 跳过重复 Rust link task”的等价路径。

## 尚未验证

- APK 安装、首次启动、默认关闭网关与 APP 内启停。
- 前台 service、常驻通知、通知 action、Quick Settings tile 与后台恢复。
- 旋转、锁屏、外接键盘、软键盘、进程被系统回收后的恢复。
- Android 调试日志实际写盘、关闭后停止追加、512 KiB 滚动与敏感字段排除。

设置中的 Android 调试日志默认关闭；开启后只记录生命周期、监听地址、端口冲突、权限拒绝、恢复结果和错误类别，不记录请求体、答案、provider key、调用 Token 或完整 Authorization 头。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助构建与整理
