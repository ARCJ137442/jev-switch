# Windows 便携版冷启动与 Laya 路由验收（2026-09-26）

## 目的与构建来源

验证 Release workflow-dispatch `36238959899` 上传的 Windows 便携 ZIP，而不是旧 5173 开发服务或先前驻留托盘的实例。包解压在 `E:\\tmp\\jev-switch-ci-portable-36238959899`；ZIP SHA-256：

```text
B7216FAF8447172E6DB77356709CA92025933610D27AE9FE9C99BEA1BB984EC4
```

本次启动前已确认 Jev-Switch 进程为空、11435 未监听。直接运行包内 `jev-switch.exe` 后，shell PID 47168 和 daemon PID 59096 的可执行文件路径都在该目录。daemon 监听 `127.0.0.1:11435`；`GET /health` 返回 `status=ok`、version `0.1.0`、build revision `f850d3f8245217ae21e5a617d424ed27e45b4e1a`。

## 文件与运行服务身份

manifest 中文件与实际解包文件复算相同；当前 daemon HTTP 服务直接提供的 index、JS 与 CSS 也与同一份 manifest 相同：

| 文件 | SHA-256 |
|---|---|
| `jev-switch.exe` | `04287DD9B4998CA9FE67112CDEA1E41AE357FBCC6D4357DD5FA50BB187C7A2CB` |
| `jev-switch-daemon.exe` / `resources/jev-switch-daemon.exe` | `7CA8D67880183E28DCD974D9E2B6601EBB648F6B977501A69784A030353DD443` |
| `ui/dist/index.html`（与 HTTP `/` 相同） | `BB3E1640E84FF76ADE710E95D0BE98FE28A704251B9217A94EC5248418C0CFB0` |
| `ui_assets /assets/index-rsyine2x.js`（HTTP 相同） | `E83B5A088D0A2BBFB7FAD5C3193029389BE377100682738B1090C304C6EE85B8` |
| `ui_assets /assets/index-Cp7fxIgV.css`（HTTP 相同） | `3F7DE44906B71EC067255E1F0DCA046865B6E0D13A9BC25F90A5D78619BCD1A6` |
| 同批 MSI | `0523527A13057119E3BC5CE9F9FB743E26253B92B15B2AD9FF57990EC666A83C` |
| 同批 NSIS | `9A7A90622683B3330149C3D30F9F8602E5C151BE4B20770426E6CEED5B6EC0F9` |

该 workflow-dispatch 是发布 dry-run：CI 和 artifact 上传已通过，但跳过 GHCR 推送与公开 GitHub Release；本次没有安装 MSI/NSIS，也没有触发 UAC。

## 原配置和真实路由请求

程序复用了当前用户原有 `%APPDATA%\\jev-switch`，没有覆盖配置或改动路由。管理 API 显示 Laya/Vercel 两份上游配置均启用，Vercel key 仍由 daemon 保管且没有向日志、文档或工具输出明文；7 个入口启用、共 9 条路由。SQLite `call_logs` 中保留原有记录并在此次测试后有 27 条。

向本地 daemon 的 `POST /v1/systemone` 发送了合成请求，公开入口为 `laya-english`。结果为 HTTP 200、一个 answer、一个上游调用，usage 为 49 input tokens / 0 output tokens。响应和持久化历史均关联 `request_id=jev-27`：

```text
endpoint:       laya-english
route/provider: laya
upstream model: laya-english
strategy:       failover
selected hops:  laya
attempts:       1
gateway time:   198 ms
history row:    id 27, HTTP 200, route trace present
```

没有把原始模型回答、调用体、凭据值或含回答的完整 JSON复制到该记录。当前运行版 Dashboard 已显示 `jev-27`，以可读摘要先展示入口/路由、成功状态、耗时、token 和上游次数；“展开原始 JSON”保持折叠。

## UI 与验收边界

通过 Codex In-app Browser 查看当前 11435 服务：Dashboard 显示 Local 模式、2/2 上游可达、9 条路由和上述新历史；Routing 的入口页显示 7 个入口，DAG 页显示已同步及 9 条边；Providers 展示现有 Laya/Vercel 配置与模型；Playground 页面展示多目标比较布局。浏览器捕获为 405×655 像素，Dashboard 概览卡片按窄布局纵向堆叠。这只验证 daemon 所服务的 UI 资源与浏览器响应式排版，不是 Tauri 原生 WebView 的屏幕尺寸或 DPI 验收。

当前可用 Computer Use 原生 inventory 返回 `apps=[]`，`listWindows` 不可用；当前工具集也没有 Tauri MCP。故本轮没有取得这批便携 Tauri 壳的原生窗口截图，亦没有操作其托盘菜单。用户对先前安装版“关窗留托盘、点击托盘恢复”的人工验证，依然只证明那次安装版的 Hide/Restore。该证据不外推为本批便携壳的 Hide/Restore/Exit 或退出后重启；应用本轮测试后保持运行。

此记录仅包含脱敏的运行元数据，不包含 API key 或原始调用内容。

## 21:30 同一 Release dry-run 便携包的三种入口比较

在上面同一个仍运行的 Release dry-run 便携实例和 `%APPDATA%\\jev-switch` 中，分别通过 Playground 用同一份内置 Support Routing 输入完成三种比较。所有请求均只调用本地 Laya，没有使用 Vercel 或产生其调用费用；UI、request ID 与只读 SQLite `call_logs` / `route_trace_json` 逐项一致。数据库历史从 27 条增至 33 条，原有记录均保留。

| 比较类型 | 两列结果与 request ID | 结果/追踪核验 |
|---|---|---|
| 公开入口 ↔ 公开入口 | `laya-english` (`jev-28`) ↔ `laya-multilingual` (`jev-29`) | 两列 HTTP 200，各调用一次 Laya；`failover` 策略分别解析到同名模型；109/106 输入 tokens，trace 均存在 |
| 直连上游 ↔ 直连上游 | `laya/laya-english` (`jev-30`) ↔ `laya/laya-multilingual` (`jev-31`) | 两列 HTTP 200，各调用一次 Laya；`direct` 路径明确为 `laya → laya-english` / `laya → laya-multilingual`；109/106 输入 tokens，trace 均存在 |
| 公开入口 ↔ 直连上游 | `laya-english` (`jev-32`) ↔ `laya/laya-multilingual` (`jev-33`) | 两列 HTTP 200，各调用一次 Laya；左列走 `failover` 入口策略，右列走 `direct`；109/106 输入 tokens，trace 均存在 |

Playground 对以上结果都先展示目标类型、模型、request ID、实际路径、策略、回答摘要、耗时、usage 和上游次数，原始响应保持折叠。1280×720 浏览器画面中，直连与混合比较的两列卡片并排占用内容宽度；这是当前打包 daemon 所服务页面的浏览器 UI 实测，不扩写成原生 Tauri WebView 截图或 DPI 验收。用户此前的 Tauri 截图仍对应 §19:48 中说明的当时实例。

## 21:48 同一 Release 包 Vercel 入口补测

为补足同一运行版本的两家上游证据，在当前便携实例的 Playground 仅运行一次既有公开入口 `jev-vercel`。该入口返回 HTTP 200、响应模型 `typesafe-ai/jev`、1 次上游调用、420 input / 45 output tokens；UI request ID `jev-34` 与 SQLite 行 `id=34` 一致。只读 route trace 解析为 `failover`，selected provider/model 为 `vercel / typesafe-ai/jev`，hops 为 `typesafe-ai/jev → vercel`，成本记录为 0.000018 USD。Dashboard 刷新后把这条成功摘要置顶，原始 JSON仍折叠，并显示 Laya/Vercel 两家当前 2/2 可达。

SQLite `call_logs` 总数为 34；Vercel 凭据始终由 daemon 保管，没有读取、输出或写入本记录。该检查使用用户先前明确授权的真实 Vercel 配置；没有额外重试或调用。
