# Jev-Switch — 项目规则（给后续开发 Agent）

**一句话**：本地 Jev 协议多上游路由器 —— Jev 原生入口 × 多上游切换 × 桥接（Rust axum 后端 + React 控制台，目标形态含 Tauri）。

## 命令速查

| 事 | 命令 |
|---|---|
| 后端测试 | `cargo test --manifest-path rs/Cargo.toml` |
| 启动后端 | `$env:JEV_SWITCH_CONFIG="rs\providers.example.toml"; cargo run --manifest-path rs/Cargo.toml` → 监听 `127.0.0.1:11435` |
| 前端检查 / 构建 | `npm run lint --prefix ui` / `npm run build --prefix ui` |
| 健康检查 | `curl http://127.0.0.1:11435/health` |
| Laya 本地上游 | `E:\venvs\laya\Scripts\python.exe E:\tmp\jev_laya_server.py --port 18765` |

## 硬红线（违者打回）

1. 未读完 `docs/contracts/` 六份契约 → 不改 `rs/` / `ui/`
2. **P0（`docs/07-REVIEW` §7）未完成前不加第 5 个上游**
3. `git push` / 提交推送必须用户下令；密钥真实值不进上下文 / 日志 / UI 明文 / 提交信息
4. 只写本仓库；不碰 `~/Rime/`、`jev-life/`、`jev-rime/`
5. **`/openai-compat` 不做**（用户裁决 2026-09-23；01 §四.3 与 09 §二.1 同）——不实现 OpenAI/Anthropic 格式入口

## 深入文档指针表

| 要知道什么 | 去哪 |
|---|---|
| 对齐定稿 Q1–Q6、施工顺序 | `docs/08-CONTRACT`（冲突以此为准） |
| P0/P1/P2 意见清单 + 验收 | `docs/07-REVIEW` §7 |
| 协议 / 扩展 / 路由 / 密钥 / HTTP / 前端细则 | `docs/contracts/00-INDEX` → 01–06 |
| 前端三页设计稿 | `docs/design/01` |
| 索引 / 快照 / 叙事匹配 | `docs/README`、`docs/PROGRESS`、`docs/09` |

## 踩坑警示

- UI 曾因 `/v1/models` 形状漂移整体崩溃（`37b4242` 止血）——形状以 `contracts/05` 为准，TS 类型最终从 Rust 生成
- Vercel 概率曾被硬编码 0.95/0.05（已修 `6af3a47`）——动 `translate.rs` 必跑 `cargo test`
- 历史文档 01–06 中的端口 8765、旧 M1+ 排序等已过时——一律以 07 / 08 / contracts 为准
