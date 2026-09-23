//! Jev-Switch 可执行入口（薄壳：装配 → ListenSupervisor 监听 → 等停机信号）。
//!
//! 逻辑全在 lib（`build_state` / `build_app` / [`listen`]）—— 拆出供 `tests/`
//! 进程内 `oneshot` 集成基座复用（A7/A8）。
//!
//! #43 双态 + ListenSupervisor 修订裁决（mode 热切 / bind 解耦）：
//! - **bind 解析**：env `JEV_BIND` ← 文件 `bind` ← **成对默认**
//!   （mode=local → `127.0.0.1:11435`；mode=cloud → `0.0.0.0:11435`）。
//!   显式 bind 恒绑不动，mode 翻转不 Rebind（peer 校验兜底）；
//!   非显式时 `PUT /v1/admin/mode` 翻转联动热 Rebind（零进程重启）。
//! - **监听层**：[`listen::start`] spawn ListenSupervisor（任务级小网关）——
//!   内核（Router/auth/handlers）`Arc` 共享永不因换地址而亡。
//!
//! 启动时做配置加载（含 DAG 检环）、`JEV_SWITCH_MODE`/`JEV_BIND` 非法值硬拒、
//! 0600 权限告警（contracts/04 §7）。

use anyhow::Context;
use jev_switch_daemon::{
    build_app, build_state,
    config::Config,
    listen::{self, ListenPlan, PairDefaults},
};
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
    // bind 解析（JEV_BIND ← 文件 bind ← 成对默认；非法 → 硬拒）
    let (bind_addr, explicit_bind) = config.effective_bind(mode).context("resolve bind")?;
    tracing::info!(
        providers = config.providers.len(),
        routes = config.route_edges().len(),
        path = %config_path.display(),
        ?mode,
        bind = %bind_addr,
        explicit_bind,
        "config loaded"
    );
    // contracts/04 §7：0600 权限启动检查 + 告警（Unix 查 mode；Windows 降级 warning）
    jev_switch_daemon::config::check_config_perms_warn(&config_path);

    // 2. 装配（Registry + admin state + auth 态）→ 3. axum app
    let state = build_state(config, config_path);
    let app = build_app(state.clone());

    // 4. ListenSupervisor（先入 AppState.listen slot 再 accept —— 无空窗）
    let plan = ListenPlan {
        addr: bind_addr,
        explicit: explicit_bind,
        defaults: PairDefaults::standard(),
    };
    let handle = listen::start(app, plan, &state.listen)
        .await
        .context("initial listen")?;
    tracing::info!(
        addr = %handle.bound(),
        ?mode,
        explicit_bind,
        "jev-switch listening (listen layer = supervisor task; rebind without process restart)"
    );

    // 5. 等 Ctrl-C → 优雅停机（停 accept、在途请求跑完、超时兜底强杀）
    tokio::signal::ctrl_c()
        .await
        .context("wait for ctrl-c")?;
    tracing::info!("ctrl-c — graceful shutdown");
    handle.shutdown().await.context("shutdown listen layer")?;
    Ok(())
}
