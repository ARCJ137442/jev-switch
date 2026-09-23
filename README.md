# jev-switch

本地 **Jev 协议多上游路由器** —— Jev 原生入口 × 多上游切换 × 桥接模式。Rust（axum）后端 + React 控制台，目标形态含 Tauri 桌面。

## 快速开始

```bash
# 1. 后端（监听 127.0.0.1:11435）
$env:JEV_SWITCH_CONFIG="rs\providers.example.toml"; cargo run --manifest-path rs/Cargo.toml

# 2. 前端
npm run dev --prefix ui    # http://127.0.0.1:5173

# 3. 可选：本地上游 Laya（18765）
E:\venvs\laya\Scripts\python.exe E:\tmp\jev_laya_server.py --port 18765
```

## 文档

完整索引见 **[docs/README.md](docs/README.md)**。
阅读顺序：`08-CONTRACT`（定稿） → `contracts/01–06` → `07-REVIEW`（意见清单） → 历史 `01–06`。

当前状态：`v0.1.0-mvp` 已封存 + P0 三修（`6af3a47`）；Phase 0 契约与前端设计稿齐备，待裁决并行施工 A/B。

License: MIT OR Apache-2.0（LICENSE 文件待补，见 `docs/07-REVIEW` P2）。
