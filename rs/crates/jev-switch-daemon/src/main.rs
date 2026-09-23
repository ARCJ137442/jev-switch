//! Jev-Switch 可执行入口（薄壳：装配 → 绑 loopback → serve）。
//!
//! 逻辑全在 lib（`build_state` / `build_app`）—— 拆出供 `tests/`
//! 进程内 `oneshot` 集成基座复用（A7/A8）。
//!
//! 默认监听 `127.0.0.1:11435`（loopback 天然仅本机 —— contracts/05 §1 Admin 仅绑
//! 127.0.0.1）。启动时做配置加载（含 DAG 检环）与 0600 权限告警（contracts/04 §7）。

use anyhow::Context;
use jev_switch_daemon::{build_app, build_state, config::Config};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // tracing init
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // 1. load config（load 内含 DAG 检环：含环配置直接拒绝启动）
    let config_path = Config::resolve_path();
    let config = Config::load(&config_path).context("load config")?;
    tracing::info!(
        providers = config.providers.len(),
        routes = config.route_edges().len(),
        path = %config_path.display(),
        "config loaded"
    );
    // contracts/04 §7：0600 权限启动检查 + 告警（Unix 查 mode；Windows 降级 warning）
    jev_switch_daemon::config::check_config_perms_warn(&config_path);

    // 2. 装配（Registry + admin state）→ 3. axum app
    let state = build_state(config, config_path);
    let app = build_app(state);

    // 默认 11435 — 对齐用户叙事基址期望（docs/「Jev-Switch」用户叙事探索）
    let addr = SocketAddr::from(([127, 0, 0, 1], 11435));
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    tracing::info!(%addr, "jev-switch listening");

    axum::serve(listener, app).await?;
    Ok(())
}
