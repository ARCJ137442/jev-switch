//! Android 本机网关生命周期。
//!
//! Android 不使用 Windows sidecar：Tauri 进程直接持有 daemon library
//! 与 ListenSupervisor。UI 可以重建，运行时句柄留在应用状态里；首次打开默认启动，
//! 只有用户明确停止后才在下一次打开时保持关闭。

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

#[derive(Debug, Clone, Serialize)]
pub struct KeepaliveNotificationStatus {
    pub enabled: bool,
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
    keepalive_notification_state_path: PathBuf,
    keepalive_notification_enabled: AtomicBool,
    debug_state_path: PathBuf,
    debug_log_path: PathBuf,
    debug_logging: AtomicBool,
    running: Mutex<Option<RunningGateway>>,
}

impl AndroidGatewayState {
    pub fn new(config_path: PathBuf, desired_state_path: PathBuf, keepalive_notification_state_path: PathBuf) -> Self {
        // Android is a self-contained gateway app: a fresh install starts its
        // embedded backend by default. An explicit stopped marker is the only
        // state that suppresses restoration on the next launch.
        let desired_running = std::fs::read_to_string(&desired_state_path)
            .map(|value| value.trim() != "stopped")
            .unwrap_or(true);
        let debug_state_path = desired_state_path.with_file_name("debug-logging.state");
        let debug_log_path = desired_state_path.with_file_name("jev-switch-debug.log");
        let debug_logging = std::fs::read_to_string(&debug_state_path)
            .ok()
            .is_some_and(|value| value.trim() == "enabled");
        let keepalive_notification_enabled = std::fs::read_to_string(&keepalive_notification_state_path)
            .ok()
            .map(|value| value.trim() != "disabled")
            .unwrap_or(true);
        Self {
            config_path,
            desired_state_path,
            desired_running: AtomicBool::new(desired_running),
            keepalive_notification_state_path,
            keepalive_notification_enabled: AtomicBool::new(keepalive_notification_enabled),
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

    pub fn keepalive_notification_status(&self) -> KeepaliveNotificationStatus {
        KeepaliveNotificationStatus {
            enabled: self.keepalive_notification_enabled.load(Ordering::SeqCst),
        }
    }

    fn set_keepalive_notification(&self, enabled: bool) -> Result<KeepaliveNotificationStatus, String> {
        std::fs::write(
            &self.keepalive_notification_state_path,
            if enabled { "enabled\n" } else { "disabled\n" },
        )
        .map_err(|error| format!("persist Android keepalive notification preference failed: {error}"))?;
        self.keepalive_notification_enabled.store(enabled, Ordering::SeqCst);
        Ok(self.keepalive_notification_status())
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
        let value = if running { "running\n" } else { "stopped\n" };
        std::fs::write(&self.desired_state_path, value)
            .map_err(|error| format!("persist Android gateway state failed: {error}"))?;
        self.desired_running.store(running, Ordering::SeqCst);
        Ok(())
    }

    pub fn start(&self) -> Result<GatewayStatus, String> {
        let mut guard = self
            .running
            .lock()
            .map_err(|_| "Android gateway state lock poisoned".to_string())?;
        if let Some(gateway) = guard.as_ref() {
            return Ok(self.status(Some(gateway.bound)));
        }

        let mut config = Config::load(&self.config_path)
            .map_err(|error| format!("load Android gateway config failed: {error}"))?;
        // A user-initiated or restored service start activates model calls for this process.
        config.gateway_enabled = Some(true);
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
        if let Err(error) = self.persist_desired_running(true) {
            let _ = runtime.block_on(handle.shutdown());
            return Err(error);
        }
        *guard = Some(RunningGateway {
            runtime,
            handle,
            bound,
        });
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

pub fn prepare_paths(app: &tauri::AppHandle) -> Result<(PathBuf, PathBuf, PathBuf), String> {
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
    Ok((
        config_path,
        data_dir.join("desired-running.state"),
        data_dir.join("keepalive-notification.state"),
    ))
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
pub fn android_keepalive_notification_status(
    state: State<'_, AndroidGatewayState>,
) -> KeepaliveNotificationStatus {
    state.keepalive_notification_status()
}

#[tauri::command]
pub fn android_set_keepalive_notification(
    app: tauri::AppHandle,
    state: State<'_, AndroidGatewayState>,
    enabled: bool,
) -> Result<KeepaliveNotificationStatus, String> {
    let previous = state.keepalive_notification_status().enabled;
    let status = state.set_keepalive_notification(enabled)?;
    let running = state
        .running
        .lock()
        .map_err(|_| "Android gateway state lock poisoned".to_string())?
        .is_some();
    if let Err(error) = super::android_keepalive::set_state(&app, enabled, running) {
        let _ = state.set_keepalive_notification(previous);
        let _ = super::android_keepalive::set_state(&app, previous, running);
        return Err(error);
    }
    Ok(status)
}

#[tauri::command]
pub fn android_notification_permission_state(app: tauri::AppHandle) -> Result<String, String> {
    super::android_keepalive::notification_permission_state(&app)
}

#[tauri::command]
pub fn android_request_notification_permission(app: tauri::AppHandle) -> Result<String, String> {
    super::android_keepalive::request_notification_permission(&app)
}

#[tauri::command]
pub fn android_take_pending_gateway_toggle(app: tauri::AppHandle) -> Result<bool, String> {
    super::android_keepalive::take_pending_gateway_toggle(&app)
}

#[tauri::command]
pub fn start_gateway(app: tauri::AppHandle, state: State<'_, AndroidGatewayState>) -> Result<GatewayStatus, String> {
    let status = match state.start() {
        Ok(status) => status,
        Err(error) => { state.debug_event("gateway_start_failed"); return Err(error); }
    };
    if let Err(error) = super::android_keepalive::set_state(
        &app,
        state.keepalive_notification_status().enabled,
        status.running,
    ) {
        // The daemon is the application's primary service. Notification/foreground
        // service support is a best-effort Android integration and must not roll
        // back a successfully bound gateway when permission or OEM policy rejects it.
        state.debug_event(&format!("keepalive_service_start_failed: {error}"));
    }
    Ok(status)
}

#[tauri::command]
pub fn stop_gateway(app: tauri::AppHandle, state: State<'_, AndroidGatewayState>) -> Result<GatewayStatus, String> {
    let status = match state.stop() {
        Ok(status) => status,
        Err(error) => { state.debug_event("gateway_stop_failed"); return Err(error); }
    };
    if let Err(error) = super::android_keepalive::set_state(
        &app,
        state.keepalive_notification_status().enabled,
        false,
    ) {
        state.debug_event(&format!("keepalive_service_stop_failed: {error}"));
    }
    Ok(status)
}

#[tauri::command]
pub fn toggle_gateway(app: tauri::AppHandle, state: State<'_, AndroidGatewayState>) -> Result<GatewayStatus, String> {
    let running = state
        .running
        .lock()
        .map_err(|_| "Android gateway state lock poisoned".to_string())?
        .is_some();
    if running {
        stop_gateway(app, state)
    } else {
        start_gateway(app, state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desired_state_defaults_to_running_and_round_trips() {
        let root = std::env::temp_dir().join(format!("jev-android-state-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&root);
        let config = root.join("providers.toml");
        let desired = root.join("desired-running.state");
        std::fs::write(&config, "").unwrap();
        let keepalive = root.join("keepalive-notification.state");
        let state = AndroidGatewayState::new(config, desired.clone(), keepalive.clone());
        assert!(state.desired_running.load(Ordering::SeqCst));
        assert!(state.keepalive_notification_status().enabled, "notifications default on");
        state.set_keepalive_notification(false).unwrap();
        assert!(!AndroidGatewayState::new(root.join("providers.toml"), desired.clone(), keepalive.clone()).keepalive_notification_status().enabled);
        state.set_keepalive_notification(true).unwrap();
        state.persist_desired_running(true).unwrap();
        assert_eq!(std::fs::read_to_string(desired).unwrap().trim(), "running");
        state.persist_desired_running(false).unwrap();
        assert_eq!(std::fs::read_to_string(desired).unwrap().trim(), "stopped");
        let _ = std::fs::remove_dir_all(root);
    }
}
