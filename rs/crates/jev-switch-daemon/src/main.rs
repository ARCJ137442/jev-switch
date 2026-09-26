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
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    time::Duration,
};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Container liveness check: use the same binary and verify the service identity,
    // so the runtime image does not need curl/wget or an apt package install.
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--healthcheck")) {
        return check_local_health();
    }

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

    // 5. 等平台停机信号 → 优雅停机（停 accept、在途请求跑完、超时兜底强杀）。
    let signal = shutdown_signal().await?;
    tracing::info!(signal, "shutdown signal received; graceful shutdown");
    handle.shutdown().await.context("shutdown listen layer")?;
    Ok(())
}

fn check_local_health() -> anyhow::Result<()> {
    let addr: SocketAddr = "127.0.0.1:11435".parse()?;
    check_health_at(addr)
}

fn check_health_at(addr: SocketAddr) -> anyhow::Result<()> {
    let timeout = Duration::from_secs(2);
    let mut stream = TcpStream::connect_timeout(&addr, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1:11435\r\nConnection: close\r\n\r\n")?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| anyhow::anyhow!("health response has no HTTP header terminator"))?;
    let headers = std::str::from_utf8(&response[..header_end])?;
    let status_line = headers.lines().next().unwrap_or_default();
    if !(status_line.starts_with("HTTP/1.1 200 ") || status_line.starts_with("HTTP/1.0 200 ")) {
        anyhow::bail!("health endpoint returned a non-success status");
    }

    let body: serde_json::Value = serde_json::from_slice(&response[header_end + 4..])?;
    if body.get("status").and_then(serde_json::Value::as_str) != Some("ok")
        || body.get("product").and_then(serde_json::Value::as_str) != Some("jev-switch")
        || body.get("api_revision").and_then(serde_json::Value::as_u64) != Some(1)
    {
        anyhow::bail!("health endpoint returned an incompatible service identity");
    }
    Ok(())
}

#[cfg(test)]
mod healthcheck_tests {
    use super::check_health_at;
    use std::{
        io::{Read, Write},
        net::{SocketAddr, TcpListener},
        thread,
    };

    fn check_mock(status: &str, body: &str) -> anyhow::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr: SocketAddr = listener.local_addr()?;
        let status = status.to_owned();
        let body = body.to_owned();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 256];
            let _ = stream.read(&mut request);
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let result = check_health_at(addr);
        server.join().unwrap();
        result
    }

    #[test]
    fn accepts_only_healthy_jev_switch_identity() {
        assert!(check_mock(
            "200 OK",
            r#"{"status":"ok","product":"jev-switch","api_revision":1}"#
        )
        .is_ok());
    }

    #[test]
    fn rejects_other_products_and_unhealthy_status() {
        assert!(check_mock(
            "200 OK",
            r#"{"status":"ok","product":"other","api_revision":1}"#
        )
        .is_err());
        assert!(check_mock(
            "200 OK",
            r#"{"status":"starting","product":"jev-switch","api_revision":1}"#
        )
        .is_err());
        assert!(check_mock(
            "503 Service Unavailable",
            r#"{"status":"ok","product":"jev-switch","api_revision":1}"#
        )
        .is_err());
    }
}

#[cfg(unix)]
async fn shutdown_signal() -> anyhow::Result<&'static str> {
    use tokio::signal::unix::{signal, SignalKind};

    let mut interrupt = signal(SignalKind::interrupt()).context("register SIGINT handler")?;
    let mut terminate = signal(SignalKind::terminate()).context("register SIGTERM handler")?;
    tokio::select! {
        _ = interrupt.recv() => Ok("SIGINT"),
        _ = terminate.recv() => Ok("SIGTERM"),
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() -> anyhow::Result<&'static str> {
    tokio::signal::ctrl_c().await.context("wait for Ctrl-C")?;
    Ok("Ctrl-C")
}
