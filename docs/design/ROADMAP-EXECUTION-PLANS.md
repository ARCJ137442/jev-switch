# Roadmap Execution Plans

> **TL;DR**：这些是基于当前 `abc793f` 主线的执行草案，不是版本承诺。近期先完成已有 Standalone 的精确验收和性能基线，再做主题跟随系统、命令面板、路由动效与 GitHub Pages；OpenRouter 仍是最高优先级的范围扩展，但需要独立的协议核对与适配器设计。
>
> **作者**：GPT-6 Luna xhigh（OpenAI Codex）  
> **AI 披露**：本文由 AI 协助整理，执行顺序和最终范围由项目维护者确认。

## 现状判断

当前代码已经提供 TypeSafe 官方/兼容服务、Provider 模型发现、Standalone 资源嵌入与校验、五页控制台，以及 Dashboard Phase 1 运行遥测。当前正式 Release 仍是 v0.1.0；新源码能力必须经过对应的候选包验收后才能进入发布。

`abc793f` 已把 upstream attempt timing 合入主线：`TimedUpstream` 在 daemon 组合边界记录每次真实 adapter attempt（包括失败与重试），并提供会话级 `avg_upstream_latency_ms` 与 `upstream_attempts`。它是 transport + parsing 的 adapter processing time，不是纯网络耗时；不写 SQLite，也不与 gateway latency 相加。性能计划应直接复用这组数据，再补固定场景的 p50/p95/p99。

路线图候选按实施形态分为三组：

1. **验收收口**：Standalone 精确候选、发布材料和桌面边界。
2. **已有数据源上的增量**：性能基线、跟随系统主题、命令面板、路由实时动效。
3. **独立工程**：GitHub Pages 首页、OpenRouter adapter。

## 计划矩阵

| 计划 | 当前基础 | 预计范围 | ICE（影响/信心/容易度） | 建议顺序 |
|---|---|---:|---:|---:|
| [Standalone 精确候选验收](ROADMAP-PLAN-STANDALONE-ACCEPTANCE.md) | `src-tauri/src/standalone.rs`、发布脚本、旧候选 E2E | 1 个验收轮次 | 9/8/7 = 50.4 | 1 |
| [网关性能基线](ROADMAP-PLAN-PERFORMANCE-BASELINE.md) | `Telemetry`、route trace、SQLite history | 1 个基线迭代 | 8/8/6 = 38.4 | 2 |
| [跟随系统主题](ROADMAP-PLAN-SYSTEM-THEME.md) | `main.tsx` 已读取 `prefers-color-scheme`，`ThemeToggle` 只有手动切换 | 小型 UI 功能 | 5/9/9 = 40.5 | 3 |
| [GitHub Pages 首页](ROADMAP-PLAN-GITHUB-PAGES.md) | README、公开截图、Release 链接 | 独立静态站 | 7/8/6 = 33.6 | 4 |
| [命令面板 Phase 1](ROADMAP-PLAN-COMMAND-PALETTE.md) | 顶栏导航、页面 hash 路由、现有动作入口 | 中型前端功能 | 8/8/6 = 38.4 | 5 |
| [路由图实时动效](ROADMAP-PLAN-ROUTING-LIVE-ANIMATION.md) | route trace、事件/SSE、`DagCanvas` | 后端事件关联 + UI | 8/7/5 = 28.0 | 6 |
| [OpenRouter adapter](ROADMAP-PLAN-OPENROUTER.md) | adapter trait、Vercel/TypeSafe 参考、Provider kind 注册 | 独立协议适配器 | 10/6/4 = 24.0 | 先做契约核对，再排实现 |

## 共用交付门槛

- 每项计划先补契约/测试，再改 UI；不把 mock fixture 当作真实能力证明。
- 真实 key 只存在于 daemon 或本机受保护文件，不能进入日志、截图、Git 或公开文档。
- 桌面功能分为源码测试、候选包测试和真实 OS 人工验收三层，三者不互相冒充。
- 交付时更新对应验证记录，并写明未验证边界；未达到门槛的功能不得写入正式 Release 能力列表。
- 计划执行完后运行 Rust 默认/`ts-rs`、UI test/lint/build 和与职责相称的 Tauri 测试。

— GPT-6 Luna xhigh（OpenAI Codex），AI 辅助整理，2026-09-30
