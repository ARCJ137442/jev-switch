//! Android 本机网关生命周期。
//!
//! Android 不使用 Windows sidecar：Tauri 进程在用户明确启动后装配 daemon library
//! 与 ListenSupervisor。UI 可以重建，运行时句柄留在应用状态里；默认安装状态永远是关闭。

use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use jev_switch_daemon::{
    build_app, build_state,
    config::Config,
    listen::{self, ListenHandle, ListenPlan, PairDefaults},
};
use serde::Serialize;
use tauri::{Manager, State};

const DEFAULT_CONFIG: &str = include_str!("default_providers.toml");
const MAX_DEBUG_LOG_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct GatewayStatus {
    pub running: bool,
    pub bind: Option<String>,
    pub desired_running: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugLogStatus {
    pub enabled: bool,
    pub path: Option<String>,
}

struct RunningGateway {
    runtime: tokio::runtime::Runtime,
    handle: ListenHandle,
    bound: SocketAddr,
}

pub struct AndroidGatewayState {
    config_path: PathBuf,
    desired_state_path: PathBuf,
    desired_running: AtomicBool,
    debug_state_path: PathBuf,
    debug_log_path: PathBuf,
    debug_logging: AtomicBool,
    running: Mutex<Option<RunningGateway>>,
}

impl AndroidGatewayState {
    pub fn new(config_path: PathBuf, desired_state_path: PathBuf) -> Self {
        let desired_running = std::fs::read_to_string(&desired_state_path)
            .ok()
            .is_some_and(|value| value.trim() == "running");
        let debug_state_path = desired_state_path.with_file_name("debug-logging.state");
        let debug_log_path = desired_state_path.with_file_name("jev-switch-debug.log");
        let debug_logging = std::fs::read_to_string(&debug_state_path)
            .ok()
            .is_some_and(|value| value.trim() == "enabled");
        Self {
            config_path,
            desired_state_path,
            desired_running: AtomicBool::new(desired_running),
            debug_state_path,
            debug_log_path,
            debug_logging: AtomicBool::new(debug_logging),
            running: Mutex::new(None),
        }
    }

    fn status(&self, running: Option<SocketAddr>) -> GatewayStatus {
        GatewayStatus {
            running: running.is_some(),
            bind: running.map(|address| address.to_string()),
            desired_running: self.desired_running.load(Ordering::SeqCst),
        }
    }

    pub fn wants_running(&self) -> bool {
        self.desired_running.load(Ordering::SeqCst)
    }

    fn debug_status(&self) -> DebugLogStatus {
        DebugLogStatus {
            enabled: self.debug_logging.load(Ordering::SeqCst),
            path: self
                .debug_logging
                .load(Ordering::SeqCst)
                .then(|| self.debug_log_path.display().to_string()),
        }
    }

    fn debug_event(&self, event: &str) {
        if !self.debug_logging.load(Ordering::SeqCst) {
            return;
        }
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or_default();
        let line = format!("{timestamp} {event}\n");
        if std::fs::metadata(&self.debug_log_path).is_ok_and(|metadata| {
            metadata.len().saturating_add(line.len() as u64) > MAX_DEBUG_LOG_BYTES
        }) {
            if std::fs::write(&self.debug_log_path, "log rotated at size limit\n").is_err() {
                return;
            }
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.debug_log_path)
        {
            use std::io::Write;
            let _ = file.write_all(line.as_bytes());
        }
    }

    fn persist_desired_running(&self, running: bool) -> Result<(), String> {
        self.desired_running.store(running, Ordering::SeqCst);
        let value = if running { "running\n" } else { "stopped\n" };
        std::fs::write(&self.desired_state_path, value)
            .map_err(|error| format!("persist Android gateway state failed: {error}"))
    }

    pub fn start(&self) -> Result<GatewayStatus, String> {
        let mut guard = self
            .running
            .lock()
            .map_err(|_| "Android gateway state lock poisoned".to_string())?;
        if let Some(gateway) = guard.as_ref() {
            return Ok(self.status(Some(gateway.bound)));
        }

        let config = Config::load(&self.config_path)
            .map_err(|error| format!("load Android gateway config failed: {error}"))?;
        let mode = config
            .effective_mode()
            .map_err(|error| format!("resolve Android gateway mode failed: {error}"))?;
        let (bind_addr, explicit) = config
            .effective_bind(mode)
            .map_err(|error| format!("resolve Android gateway bind failed: {error}"))?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("create Android gateway runtime failed: {error}"))?;
        let state_for_daemon = build_state(config, self.config_path.clone());
        let app = build_app(state_for_daemon.clone());
        let plan = ListenPlan {
            addr: bind_addr,
            explicit,
            defaults: PairDefaults::standard(),
        };
        let handle = runtime
            .block_on(listen::start(app, plan, &state_for_daemon.listen))
            .map_err(|error| format!("start Android gateway listener failed: {error}"))?;
        let bound = handle.bound();
        *guard = Some(RunningGateway {
            runtime,
            handle,
            bound,
        });
        self.persist_desired_running(true)?;
        self.debug_event(&format!("gateway_start bind={bound}"));
        Ok(self.status(Some(bound)))
    }

    pub fn stop(&self) -> Result<GatewayStatus, String> {
        let gateway = self
            .running
            .lock()
            .map_err(|_| "Android gateway state lock poisoned".to_string())?
            .take();
        if let Some(gateway) = gateway {
            gateway
                .runtime
                .block_on(gateway.handle.shutdown())
                .map_err(|error| format!("stop Android gateway listener failed: {error}"))?;
        }
        self.persist_desired_running(false)?;
        self.debug_event("gateway_stop");
        Ok(self.status(None))
    }

    fn set_debug_logging(&self, enabled: bool) -> Result<DebugLogStatus, String> {
        self.debug_logging.store(enabled, Ordering::SeqCst);
        std::fs::write(
            &self.debug_state_path,
            if enabled { "enabled\n" } else { "disabled\n" },
        )
        .map_err(|error| format!("persist Android debug log preference failed: {error}"))?;
        if enabled {
            self.debug_event("debug_logging_enabled");
        }
        Ok(self.debug_status())
    }
}

pub fn prepare_paths(app: &tauri::AppHandle) -> Result<(PathBuf, PathBuf), String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("resolve Android app data directory failed: {error}"))?;
    std::fs::create_dir_all(&data_dir)
        .map_err(|error| format!("create Android app data directory failed: {error}"))?;
    let config_path = data_dir.join("providers.toml");
    if !config_path.exists() {
        std::fs::write(&config_path, DEFAULT_CONFIG)
            .map_err(|error| format!("seed Android provider config failed: {error}"))?;
    }
    Ok((config_path, data_dir.join("desired-running.state")))
}

#[tauri::command]
pub fn gateway_status(state: State<'_, AndroidGatewayState>) -> GatewayStatus {
    let running = state
        .running
        .lock()
        .expect("Android gateway state lock")
        .as_ref()
        .map(|gateway| gateway.bound);
    state.status(running)
}

#[tauri::command]
pub fn android_debug_log_status(state: State<'_, AndroidGatewayState>) -> DebugLogStatus {
    state.debug_status()
}

#[tauri::command]
pub fn android_set_debug_log(
    state: State<'_, AndroidGatewayState>,
    enabled: bool,
) -> Result<DebugLogStatus, String> {
    state.set_debug_logging(enabled)
}

#[tauri::command]
pub fn start_gateway(state: State<'_, AndroidGatewayState>) -> Result<GatewayStatus, String> {
    let result = state.start();
    if result.is_err() {
        state.debug_event("gateway_start_failed");
    }
    result
}

#[tauri::command]
pub fn stop_gateway(state: State<'_, AndroidGatewayState>) -> Result<GatewayStatus, String> {
    let result = state.stop();
    if result.is_err() {
        state.debug_event("gateway_stop_failed");
    }
    result
}

#[tauri::command]
pub fn toggle_gateway(state: State<'_, AndroidGatewayState>) -> Result<GatewayStatus, String> {
    let running = state
        .running
        .lock()
        .map_err(|_| "Android gateway state lock poisoned".to_string())?
        .is_some();
    if running {
        stop_gateway(state)
    } else {
        start_gateway(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desired_state_defaults_to_stopped_and_round_trips() {
        let root = std::env::temp_dir().join(format!("jev-android-state-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&root);
        let config = root.join("providers.toml");
        let desired = root.join("desired-running.state");
        std::fs::write(&config, "").unwrap();
        let state = AndroidGatewayState::new(config, desired.clone());
        assert!(!state.desired_running.load(Ordering::SeqCst));
        state.persist_desired_running(true).unwrap();
        assert_eq!(std::fs::read_to_string(desired).unwrap().trim(), "running");
        state.persist_desired_running(false).unwrap();
        assert_eq!(std::fs::read_to_string(desired).unwrap().trim(), "stopped");
        let _ = std::fs::remove_dir_all(root);
    }
}
