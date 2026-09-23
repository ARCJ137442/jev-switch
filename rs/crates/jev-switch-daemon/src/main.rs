//! Jev-Switch 可执行入口（薄壳：装配 → 按 mode 绑地址 → serve）。
//!
//! 逻辑全在 lib（`build_state` / `build_app`）—— 拆出供 `tests/`
//! 进程内 `oneshot` 集成基座复用（A7/A8）。
//!
//! #43 双态绑定（docs/12 §一 · mode 只管鉴权，不涉及上游拓扑）：
//! - `mode = local`（默认）→ `127.0.0.1:11435`（loopback 天然仅本机 ——
//!   contracts/05 §1 Admin 仅绑 127.0.0.1）+ **完全不校验 token/密码**
//! - `mode = cloud` → `0.0.0.0:11435`（**部署形态偏差**：容器内必需；cloud 态由
//!   鉴权而非绑定位承担防护 —— 不改契约文件，偏差与理由备案 `docs/deployment.md`）
//!
//! 启动时做配置加载（含 DAG 检环）、`JEV_SWITCH_MODE` 非法值硬拒、0600 权限告警
//! （contracts/04 §7）。

use anyhow::Context;
use jev_switch_daemon::{build_app, build_state, config::Config};
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
    // #43：mode 解析（env JEV_SWITCH_MODE 覆盖文件；非法值 → 硬拒带病启动）
    let mode = config.effective_mode().context("resolve mode")?;
    tracing::info!(
        providers = config.providers.len(),
        routes = config.route_edges().len(),
        path = %config_path.display(),
        ?mode,
        "config loaded"
    );
    // contracts/04 §7：0600 权限启动检查 + 告警（Unix 查 mode；Windows 降级 warning）
    jev_switch_daemon::config::check_config_perms_warn(&config_path);

    // 2. 装配（Registry + admin state + auth 态）→ 3. axum app
    let state = build_state(config, config_path);
    let app = build_app(state);

    // #43：按 mode 选绑 —— local=loopback（现状），cloud=全接口（容器必需；偏差备案）
    let addr = mode.bind_addr();
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    tracing::info!(%addr, ?mode, "jev-switch listening");

    axum::serve(listener, app).await?;
    Ok(())
}
