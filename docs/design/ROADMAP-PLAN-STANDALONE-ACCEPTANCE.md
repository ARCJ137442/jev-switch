# Standalone 精确候选验收计划

**作者**：GPT-6 Luna xhigh（OpenAI Codex）  
**AI 披露**：本文由 AI 协助整理。

**当前状态（2026-09-30）**：Standalone 已随 v0.2.0 发布。精确 EXE 的冷启动、版本化缓存、health 和静态资源身份有 v0.2 候选证据；托盘 OS 菜单实际点击、退出恢复、干净账户和多硬件退出时间不由该记录覆盖。后续只补剩余生命周期证据，不重新开展已完成的冷启动/缓存验证。

**目标**：把 `abc793f` 之后的单文件候选从“构建成功”推进到“精确 EXE 运行证据”，决定是否具备进入下一版 Release 的条件。

## 现有入口

- `src-tauri/src/standalone.rs`：嵌入资源、版本化缓存、完整性检查、损坏恢复。
- `src-tauri/src/sidecar.rs`：Standalone runtime 路径、daemon 探活和 UI 复用。
- `scripts/build-standalone.ps1`、`scripts/build-windows-release.ps1`：同批构建与打包。
- `docs/verification/windows-release-candidate-2026-09-29.md`：当前候选 hash 和未完成边界。

## 分阶段工作

1. 用当前工作树重建 MSI、NSIS、Portable、Standalone，并记录四类 hash、manifest 和 UI 资源 hash。
2. 在没有 11435 监听的情况下启动精确 Standalone，核对 `/health` 的 `product/version/api_revision/build_revision` 和启动缓存路径。
3. 核对 Dashboard、Providers、Entries、Routing DAG、Playground 五页；确认模型发现按钮来自当前构建资源。
4. 用 WireMock 或已授权的 TypeSafe 测试配置点击“从上游获取模型”，确认 `jev-latest`/`jev-preview` 回填且 key 不出现在 UI/日志。
5. 做一次关闭留托盘、退出、重启恢复、第二次启动聚焦的真实人工验收；无法自动化的步骤附截图/操作记录。
6. 在干净目录/空缓存/损坏缓存/无写权限/路径含空格场景复跑缓存恢复测试。

## 主要文件

`src-tauri/src/standalone.rs`、`src-tauri/src/sidecar.rs`、`scripts/build-standalone.ps1`、`scripts/build-windows-release.ps1`、`docs/RELEASE.md`、`docs/verification/`。

## 完成证据

- 精确候选 hash 与 manifest 对齐。
- health、五页 WebView、模型发现、托盘/恢复和缓存恢复均有同一候选证据。
- 发布文档明确 Standalone 是否进入 Release；若未通过，继续标为本机候选而不是正式下载项。

## 不做

本计划不处理 macOS/Linux、代码签名、增量更新、数据目录迁移或单例开关的产品设计。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理
