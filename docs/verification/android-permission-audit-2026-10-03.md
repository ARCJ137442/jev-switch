# Android 权限与平台能力审查（2026-10-03）

> 范围：`v0.6.3` 的设备证据、`v0.7.0` 已发布的 Tauri/Android 生成脚本、Kotlin 桥、React 调用点和官方 Android 平台规则。维护者已在设备上确认 v0.6.3 的本机网关启停与 UI 连接正常；下列新能力不能由这一项验收外推。

## 已证实与本轮修复

| 能力 | 权限/平台边界 | 证据与状态 |
|---|---|---|
| 本机 HTTP 网关 | `INTERNET`、release `usesCleartextTraffic=true`、daemon 对 Tauri Android origin 的 CORS | `v0.6.3` CI 已检查签名包清单；维护者确认 Android 本机启停和 UI 连接均正常。`/` 返回 404 不代表监听失败，应检查 `/health`。 |
| 安装包权限清单 | 打包 APK 仅申请 `INTERNET`、两项前台服务权限和 `POST_NOTIFICATIONS`；依赖另有应用私有的动态广播接收器权限 | `aapt dump permissions` 已核对；没有全盘存储、查询全部应用等宽泛权限。本轮门禁还会防止这些权限意外进入清单。 |
| 通知与前台服务 | 清单声明 `FOREGROUND_SERVICE`、`FOREGROUND_SERVICE_SPECIAL_USE`、`POST_NOTIFICATIONS`，service 的 `specialUse` 类型及用途说明 | 生成/打包清单门禁检查；Android 13+ 拒绝通知授权不能阻止网关本体启动。原生 service 是否在锁屏、后台和 OEM 省电策略下稳定仍须设备验收。 |
| Quick Settings | tile service 以 `BIND_QUICK_SETTINGS_TILE` 保护，导出的组件只供系统绑定 | 清单门禁检查；点击 tile 当前先打开 Activity 再处理切换，不是后台直接启动前台服务。实际系统面板添加、点击、锁屏和状态同步尚未设备验收。 |
| Android 10+ 日志导出 | 应用自己创建的 `MediaStore.Downloads` 项不需申请广泛存储权限 | 源码使用 `IS_PENDING` 写入后公开；Kotlin 编译通过，导出后能否在不同文件管理器中打开仍须真机验收。 |
| Android 7–9 日志导出 | 私有缓存文件只能通过受限 `FileProvider` URI 与临时读取授权分享 | **发现已发布源码缺少 provider 注册**，`getUriForFile` 会失败。本轮增加专用 `shared_debug_logs/` 路径、非导出 provider、分享授权与失败回传；本机生成工程、合并清单和 Kotlin/Gradle 构建已通过，API 24–28 设备行为待测。 |
| JSON 导出与剪贴板 | Android 10+ 将应用生成的 JSON 写入 Downloads；Android 7–9 经受限 `FileProvider` 分享；文本复制走原生 `ClipboardManager` | `v0.7.0` 已发布并接入设置备份、演练场样例及常用复制按钮，并限制文件名与大小；CI 构建/签名通过，含 API key 的导出和系统文件/剪贴板结果仍须真机验收。 |
| 网关与前台服务区分 | Rust listener 状态不能由通知授权或 service 启动结果代替 | 设置页增加独立的前台服务实际状态观测；同一进程不保证系统回收后自动恢复，仍须真机比对首页监听和通知。 |
| Activity 退后台 | Android 12+ 禁止多数后台前台服务启动 | 移除 `Activity.onStop` 中重复的 `startForegroundService`；显式网关状态变更仍是 service 启停入口。该调整已通过编译，后台场景待设备验收。 |

Android 官方依据：[前台服务类型和权限](https://developer.android.com/about/versions/14/changes/fgs-types-required)、[后台启动限制](https://developer.android.com/develop/background-work/services/fgs/restrictions-bg-start)、[通知权限与前台服务](https://developer.android.com/develop/ui/compose/notifications/notification-permission)、[MediaStore 自有文件](https://developer.android.com/training/data-storage/shared/media)、[FileProvider 路径和临时授权](https://developer.android.com/reference/androidx/core/content/FileProvider)。

## 仍需逐项设备验收的边界

1. **前台服务不是进程永不被杀的保证。** 当前 Kotlin service 使用 `START_NOT_STICKY`，Rust listener 仍由同一应用进程持有；系统杀死整个进程后，需要重新打开应用才会读取期望运行状态。不能宣称已实现独立进程恢复。分别测通知授权允许/拒绝、保活开/关、锁屏、切后台、滑掉任务、系统回收与厂商省电策略；诊断结果应区分 listener、service 和通知三种状态。
2. **普通 JSON 导出与剪贴板。** 已从 WebView `blob:`/`navigator.clipboard` 切到受控原生桥，设置备份可能包含 API key，原有导出警示仍须确认。分别在 Android 7–9 和 10+ 真机确认分享/Downloads、文件内容与剪贴板；不增加宽泛存储权限。
3. **文件导入。** 设置 JSON 与演练场示例使用用户主动选择的文件输入。需确认 Android 文件选择器返回内容可读、取消时不改状态、导入失败不污染原配置；不应为此请求整个存储空间权限。
4. **LAN 与 HTTPS。** 已发布 APK 能连接本机 loopback，但手机热点让平板访问 `0.0.0.0` 的设备对设备链路仍须测防火墙、IP 变化、用户确认和鉴权；自签名 HTTPS 也不应靠关闭证书校验解决。生成项目当前 `targetSdk=36`；[Android 17 本地网络权限](https://developer.android.com/privacy-and-security/local-network-permission)只在升级到 SDK 37+ 时才必须声明并运行时请求，**现在不应预先加权限**。升级目标 SDK 前须做专门迁移与拒绝授权测试。
5. **权限拒绝与状态一致性。** 网关成功绑定后，通知或 OEM 前台服务失败须显示独立状态，不可把它报成“网关启动失败”；tile、通知和首页应读取真实 listener 状态。当前启动路径已把 keepalive 失败记日志并保持 listener，设置页单独显示 service 实际状态；其真机一致性待测。
6. **通知权限初始状态。** Android 13+ 的 Kotlin 桥目前把尚未授权与明确拒绝都回报为 `denied`，虽可点击请求权限，但 UI 不能据此分辨“首次询问”与“用户已拒绝”。需要设备测试授权、拒绝、再次请求和系统设置中手动撤销后的状态文案；不要让通知状态覆盖 listener 的真实状态。

## 可复现验收矩阵

| Android/API | 建议操作 | 观察点 |
|---|---|---|
| 7–9 / 24–28 | 开日志 → 启停 → 分享日志 | 选择器出现、接收应用能读取 URI；不申请全盘存储权限。 |
| 10–12 / 29–32 | 导出日志、JSON 备份和样例；切后台/锁屏 | Downloads 中可打开各文件；原生剪贴板可粘贴；listener 不中断。 |
| 13 / 33 | 分别允许/拒绝通知授权再启停 | 拒绝通知后网关 `/health` 仍正常；权限状态与通知显示一致。 |
| 14–16 / 34–36 | service/tile、旋转、软键盘、热点 LAN、系统回收 | 无前台服务权限异常；入口状态、端口、tile 与通知不矛盾。 |
| 17 / 37（未来升级） | 拒绝/授予本地网络权限 | 本机、局域网入站/出站和 WebView 权限路径分别可解释。 |

## 对路线图的近期排序建议

1. **Android 平台收口**：先做上述真机矩阵，验证已接入的原生 JSON/剪贴板、service 状态，再考虑服务进程恢复；它直接影响已发布 APK 的可信度。
2. **路由策略矩阵补足**：父请求的一条持久记录已包含每次 provider attempt 的结果/耗时，入口与出口视角可从中展开。继续补 sticky、能力跳过、网络/超时、在途配置变更与游标边界测试；这是核心差异能力。
3. **受限服务发现**：已有 provider 模型发现与健康探测，可从用户明确填写的本机/局域网地址做非破坏性探查，再“测试/快速添加”；先不做自动扫网段或推理调用。
4. **固定场景性能基线**：复用现有遥测和 adapter attempt timing，补网关、SQLite、UI 冷暖启动的 p50/p95/p99，先解决真实瓶颈，再扩展动效。
5. **数据目录选择/迁移**：有价值但涉及 SQLite、配置和密钥的原子迁移，应独立施工。TUI 和路由顾问保持后置，等待对应使用反馈与评估数据。

此排序是审查建议，不是 Release 范围承诺；以 `ROADMAP.md` 与专项计划中的原始边界为准。
