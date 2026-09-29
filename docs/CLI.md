# Jev-Switch CLI

**状态：** v0.3.0 CLI Phase 1<br>
**作者：** GPT-6 Luna xhigh（OpenAI Codex）<br>
**AI 披露：** 本文由 AI 协助整理；命令边界、鉴权和发布范围以项目维护者确认的 HTTP 契约为准。

`jev-switch-cli` 是 daemon HTTP API 的轻量客户端，适合 Termux、SSH、脚本和没有浏览器的调试环境。它不复制路由内核、SQLite 或 provider key 管理；Web 控制台和 Tauri 仍是完整交互主战场。TUI 尚未包含在本阶段。

## 安装

从源码构建：

```bash
cargo install --path rs/crates/jev-switch-cli --locked
```

Termux/Android arm64 使用设备本机 Rust 工具链执行同一命令。已有 [Termux 核验](verification/termux-build-deployment-2026-09-27.md)证明 daemon、`/health`、`/v1/models` 和同源 Web UI 烟测可行，但不把它外推成 Tauri Android 或预编译 Android CLI 发布支持。

Windows/Linux Release 会提供对应主机的 CLI 二进制；Termux 仍建议本机编译，以匹配 Android host 工具链。

## 鉴权与地址

```bash
export JEV_SWITCH_URL=http://127.0.0.1:11435
export JEV_SWITCH_TOKEN='网关调用 token'
export JEV_SWITCH_ADMIN_TOKEN='cloud 模式下的 admin managed token'
```

`JEV_SWITCH_TOKEN` 只用于 `/v1/models` 和 `/v1/systemone`；`JEV_SWITCH_ADMIN_TOKEN` 只用于 `status` 的 admin 状态、`routes` 和 `events`。local loopback 按 daemon 现有边界可不设置 token。不要把 token 放进命令行参数、shell history、请求文件或日志。

## 命令

```bash
jev-switch-cli status
jev-switch-cli --format json models
jev-switch-cli routes
jev-switch-cli events --since 0 --limit 50
jev-switch-cli invoke --file request.json
cat request.json | jev-switch-cli --format json invoke
```

`request.json` 必须是 Jev 原生 `{model,state,questions}` 请求。CLI 对 stdin/文件输入限制为 2 MiB；`events --limit` 最大为 500。`routes` 和 `events` 当前只读，写路由仍通过 Web 控制台/API 完成。

表格输出面向人阅读；`--format json` 输出稳定 JSON，适合脚本。`invoke` 的表格输出会显示 request ID 和结构化答案摘要，原始响应仍可用 JSON 模式取得。

## 替代方案

Android 用户若不需要 SSH/脚本/低带宽终端，优先在 Termux 启动 daemon 后用 Android 浏览器访问同源 Web UI；需要原生触控和前后台生命周期时，再评估 Tauri 2 Android。CLI 只承诺当前 API 的薄转发，不承诺复制完整 Web/Tauri 功能。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
