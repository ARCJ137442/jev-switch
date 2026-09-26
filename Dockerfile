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
# ts-rs 的 JevRequest/JevResponse 引用共享 JsonValue 生成类型。
# 保持仓库相对路径，独立 UI 构建阶段也需要这一小份类型源。
COPY rs/crates/jev-protocol/bindings/ /src/rs/crates/jev-protocol/bindings/
# npm run build = tsc --noEmit && vite build（契约级类型门禁随镜像构建生效）
RUN npm run build

# ─── stage 2 · Rust release ────────────────────────────────────────
FROM rust:1-bookworm AS rust-base
FROM rust-base AS rs
WORKDIR /src
ARG JEV_BUILD_REVISION
# 仅用于构建网络诊断；默认沿用 Cargo 行为，运行镜像不继承这些参数。
ARG CARGO_HTTP_MULTIPLEXING=true
ARG CARGO_HTTP_TIMEOUT=30
ARG CARGO_NET_RETRY=3
COPY rs/ ./rs/
# --locked：锁文件与 Cargo.toml 同步才放行（可复现构建）；rs/Cargo.lock 已入库
# 已有完整缓存时离线确认；缺失时才有界联网，避免每次重建都刷新索引。
# 包完整性仍由 Cargo 按锁文件校验，编译阶段离线进行。
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    (cargo fetch --locked --offline --manifest-path rs/Cargo.toml \
     || timeout 300 cargo fetch --locked --manifest-path rs/Cargo.toml)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/rs/target \
    cargo build --release --locked --offline --manifest-path rs/Cargo.toml \
    && cp /src/rs/target/release/jev-switch /usr/local/bin/jev-switch

# ─── stage 3 · runtime ─────────────────────────────────────────────
FROM debian:bookworm-slim
# HTTPS 上游信任根从相同 Debian 系列的 Rust 构建镜像复制；运行镜像无需 apt 网络访问。
COPY --from=rust-base /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

COPY --from=rs /usr/local/bin/jev-switch /usr/local/bin/jev-switch
COPY --from=ui /src/ui/dist /app/ui/dist
# 种子配置：容器首启时若 JEV_SWITCH_CONFIG 不存在 → 拷贝此文件初始化
COPY rs/providers.example.toml /app/providers.example.toml

# JEV_UI_DIST   — build_app 的 ServeDir 根（同源托管，CORS 消解）
# JEV_SWITCH_CONFIG — 首启导入与人工同步的 TOML（不覆盖已有 SQLite 快照）
# JEV_SWITCH_DATA_DIR — 持久配置、入口、Token 与调用记录所在目录
# 容器默认 cloud，发布端口经调用 token/管理权限访问。
ENV JEV_UI_DIST=/app/ui/dist \
    JEV_SWITCH_CONFIG=/data/providers.toml \
    JEV_SWITCH_DATA_DIR=/data \
    JEV_SWITCH_MODE=cloud

EXPOSE 11435
# /health 双态皆放行；daemon 内置探活命令也校验 product 与 API revision。
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s \
    CMD ["/usr/local/bin/jev-switch", "--healthcheck"]

# 首启播种（幂等）：目录保证存在 + 文件缺失才拷贝 → exec 交接 PID 1
CMD ["sh", "-ec", "mkdir -p \"$(dirname \"$JEV_SWITCH_CONFIG\")\" \"$JEV_SWITCH_DATA_DIR\"; if [ ! -f \"$JEV_SWITCH_CONFIG\" ]; then cp /app/providers.example.toml \"$JEV_SWITCH_CONFIG\"; chmod 600 \"$JEV_SWITCH_CONFIG\"; fi; exec /usr/local/bin/jev-switch"]
