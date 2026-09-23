# syntax=docker/dockerfile:1
# Jev-Switch 多阶段构建（任务 #43 · docs/12 §四线1）
#   stage ui   — npm ci + tsc + vite build → dist
#   stage rs   — cargo build --release（default-members = daemon bin）
#   runtime    — 精简 Debian + 二进制 + dist 静态托管（**ServeDir 同源**，
#                选型理由见 docs/deployment.md：单进程免 nginx、同源免 CORS）
#
# 静态托管与配置初始化走 Dockerfile 内联 CMD（仓库铁律：不新增 scripts/ 文件）。

# ─── stage 1 · UI ───────────────────────────────────────────────────
FROM node:22-alpine AS ui
WORKDIR /src/ui
COPY ui/package.json ui/package-lock.json ./
RUN npm ci
COPY ui/ ./
# npm run build = tsc --noEmit && vite build（契约级类型门禁随镜像构建生效）
RUN npm run build

# ─── stage 2 · Rust release ────────────────────────────────────────
FROM rust:1-bookworm AS rs
WORKDIR /src
COPY rs/ ./rs/
# --locked：锁文件与 Cargo.toml 同步才放行（可复现构建）；rs/Cargo.lock 已入库
RUN cargo build --release --locked --manifest-path rs/Cargo.toml

# ─── stage 3 · runtime ─────────────────────────────────────────────
FROM debian:bookworm-slim
# curl：HEALTHCHECK 探活；ca-certificates：HTTPS 上游兜底
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rs /src/rs/target/release/jev-switch /usr/local/bin/jev-switch
COPY --from=ui /src/ui/dist /app/ui/dist
# 种子配置：容器首启时若 JEV_SWITCH_CONFIG 不存在 → 拷贝此文件初始化
COPY rs/providers.example.toml /app/providers.example.toml

# JEV_UI_DIST   — build_app 的 ServeDir 根（同源托管，CORS 消解）
# JEV_SWITCH_CONFIG — 真值源（compose 挂 ./data:/data → ./data/providers.toml）
ENV JEV_UI_DIST=/app/ui/dist \
    JEV_SWITCH_CONFIG=/data/providers.toml

EXPOSE 11435
# /health 双态皆放行（auth.rs 公开门）→ 探活无需 token
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s \
    CMD curl -fsS http://127.0.0.1:11435/health || exit 1

# 首启播种（幂等）：目录保证存在 + 文件缺失才拷贝 → exec 交接 PID 1
CMD ["sh", "-c", "mkdir -p \"$(dirname \"$JEV_SWITCH_CONFIG\")\"; if [ ! -f \"$JEV_SWITCH_CONFIG\" ]; then cp /app/providers.example.toml \"$JEV_SWITCH_CONFIG\"; fi; exec /usr/local/bin/jev-switch"]
