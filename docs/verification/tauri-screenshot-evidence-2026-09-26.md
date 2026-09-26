# Tauri 正式窗口截图证据（2026-09-26）

用户提供的原图归档在 `tauri-user-evidence-2026-09-26/`。该目录由仓库 `.gitignore` 排除。按用户后续要求，本轮复核后将五张不含凭据的产品界面图整理到公开图册；Providers 原图仍留在本机，因为画面保留了脱敏 key 前后缀和真实接入地址。

以下文件名对应本机忽略目录中的原图；Dashboard、活动摘要、Routing 入口/DAG 与 Playground 图以原始字节复制到 `docs/images/` 并列入 [公开图册](../screenshots.md)。截图用于展示产品界面，不替代 Release build manifest、窗口逻辑像素或 DPI 验收证据。

## 证据范围

截图均显示 Jev Switch `v0.1.0` 深色界面。Dashboard 图显示 Local `127.0.0.1:11435`、2/2 提供商可达、9 条路由；调用历史图显示结构化摘要和折叠的原始 JSON。Providers 图显示两份接入配置和模型；Routing 图分别展示对外入口卡片与左到右的入口→路由 DAG→提供商模型端口；Playground 图显示两个对外入口以共享输入并排比较。

Providers 页的“尚未探测”反映本页的手动 Probe 记录；Dashboard 每 30 秒独立自动探测。两种状态口径不同，截图并不证明探测失败，但文案可能使用户误认为数据不一致。该问题记为后续 UX 澄清项，不在取得这批最终截图后再改 UI 以免截图失去版本对应性。

用户确认托盘操作“关窗留托盘，点击托盘菜单后窗口重新出现”人工实测通过。此证据只覆盖关闭窗口隐藏和托盘恢复窗口；不扩写为完整进程退出、冷启动或退出后数据库恢复测试。

## 截图目录

| 本机文件 | 证明内容 | 公开安全性 |
|---|---|---|
| `01-dashboard.png` | Dashboard、首窗紧凑布局、服务与路由概况 | 已公开于 README 与截图图册 |
| `02-dashboard-activity.png` | 请求 ID、入口/路径、状态、耗时、tokens；原始 JSON 折叠 | 已公开于截图图册；只展示摘要，不包含请求正文或密钥 |
| `03-providers-internal.png` | Laya/Vercel 接入卡、模型和状态 | **不可公开**：虽已遮蔽 key 主体，仍露出前缀/后缀片段及真实上游地址 |
| `04-routing-endpoints.png` | Routing 入口页及启用入口卡片 | 已公开于截图图册；展示项目示例 ID 与调用计数 |
| `05-routing-dag.png` | 对外入口到路由节点再到提供商模型端口的 DAG | 已公开于截图图册；无 API key，URL 仅显示本地回环及公开 Vercel 服务 |
| `06-playground-comparison.png` | 两个对外入口共享输入、并排显示结构化结果与用量 | 已公开于截图图册；使用演示场景，原始响应仍折叠 |

SHA-256（用于确认本机归档未被替换）：

```text
01-dashboard.png              02B95DCCFA709ECDA68EA3BDAA48CB5F7A11C542B93EFB652067A459C63D2A12
02-dashboard-activity.png     7B3605469ABA56A578E3238269171952406B484FF97E403C74963A4A0E4F9658
03-providers-internal.png     453B844CBDA8BA02E82C7465DAD9B6C15F2AFD42F1892B94CD39DCE21DECE540
04-routing-endpoints.png      AC1200763637272AF4265D2E107C6A2E2E23D89349EB31FA7811CBB22E3C57F5
05-routing-dag.png            1305FB0CACB6A514DB7EE48486E2127F5AC3EA54D9073B53B0080BEA0BCE7622
06-playground-comparison.png  BF4F6CB62C538B31676C1E8F10B67FB5331F49506C0C669B0C840E6B8526F214
```

截图尺寸为 902×592（Dashboard 两张）、1600×740（Providers）、1602×861（Routing 入口）、1020×845（DAG 裁图）、1600×1400（Playground）。这些是图片像素尺寸，不等于 Windows 逻辑视口或 DPI 读数。响应式逻辑视口与多显示器几何分别由计划中记录的自动验收提供，不由截图推断。
