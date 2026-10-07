//! Provider lifecycle command execution.
//!
//! Lifecycle control is deliberately narrower than a shell runner. A provider
//! must opt into control, the instance must enable host commands, and every
//! executable is launched as structured argv without a shell. Readiness is a
//! separate state from command acceptance so a successful spawn cannot be
//! mistaken for a usable model service.

use crate::config::{Config, ProcessPolicy, ProviderConfig};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, System};
use tokio::process::{Child, Command};
use tokio::time::sleep;

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const MAX_COMMAND_TIMEOUT_MS: u64 = 120_000;
const MAX_READINESS_TIMEOUT_MS: u64 = 300_000;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct LifecycleStatus {
    pub provider_id: String,
    pub state: String,
    pub readiness: String,
    pub controllable: bool,
    pub configured: bool,
    pub managed_by_jev: bool,
    #[cfg_attr(feature = "ts-rs", ts(type = "number | null"))]
    pub pid: Option<u32>,
    pub execution_id: Option<String>,
    pub message: Option<String>,
}

struct ManagedProcess {
    child: Child,
    execution_id: String,
    pid: Option<u32>,
    command_hash: String,
}

#[derive(Debug, Clone)]
struct PersistedProcess {
    execution_id: String,
    pid: Option<u32>,
    program: String,
    command_hash: String,
    process_policy: String,
    state: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeState {
    Ready,
    Starting,
    Stopped,
    Unknown,
}

enum AttachedProcessSnapshot {
    Running { pid: Option<u32>, execution_id: String },
    Exited(Option<String>),
    Error { pid: Option<u32>, execution_id: String },
}

static EXECUTION_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Debug, Clone)]
struct OperationInfo {
    action: String,
    execution_id: Option<String>,
}

#[derive(Clone, Default)]
pub struct LifecycleManager {
    processes: Arc<Mutex<HashMap<String, ManagedProcess>>>,
    operations: Arc<Mutex<HashMap<String, OperationInfo>>>,
    last_errors: Arc<Mutex<HashMap<String, String>>>,
    database: Arc<Mutex<Option<Arc<Mutex<Connection>>>>>,
    runtime_config: Arc<RwLock<Option<Config>>>,
}

struct OperationLease {
    provider_id: String,
    execution_id: Option<String>,
    operations: Arc<Mutex<HashMap<String, OperationInfo>>>,
}

impl Drop for OperationLease {
    fn drop(&mut self) {
        if let Ok(mut operations) = self.operations.lock() {
            let should_remove = operations
                .get(&self.provider_id)
                .is_some_and(|operation| operation.execution_id == self.execution_id);
            if should_remove {
                operations.remove(&self.provider_id);
            }
        }
    }
}

impl LifecycleManager {
    /// Attach the daemon's SQLite connection after the state aggregate is built.
    /// Test-only states may omit it; execution still works but has no durable
    /// lifecycle audit, which keeps pure manager tests independent of SQLite.
    pub fn attach_database(&self, database: Arc<Mutex<Connection>>) {
        *self.database.lock().expect("lifecycle database lock") = Some(database);
    }

    pub fn configure(&self, config: &Config) {
        *self.runtime_config.write().expect("lifecycle config lock") = Some(config.clone());
    }

    pub fn config_snapshot(&self) -> Option<Config> {
        self.runtime_config
            .read()
            .expect("lifecycle config lock")
            .clone()
    }

    pub async fn status(
        &self,
        provider_id: &str,
        provider: &ProviderConfig,
        config: &Config,
    ) -> LifecycleStatus {
        let configured = lifecycle_configured(provider);
        if !provider.lifecycle.controllable {
            return status_value(
                provider_id,
                "uncontrolled",
                "unknown",
                false,
                configured,
                false,
                None,
                None,
                None,
            );
        }
        if !config.allow_host_commands {
            return status_value(
                provider_id,
                "disabled",
                "unknown",
                false,
                configured,
                false,
                None,
                None,
                Some("host_commands_disabled"),
            );
        }

        if let Some(operation) = self
            .operations
            .lock()
            .expect("lifecycle operation lock")
            .get(provider_id)
            .cloned()
        {
            let (state, message) = if operation.action == "stop" {
                ("stopping", "lifecycle_stop_in_progress")
            } else {
                ("starting", "lifecycle_start_in_progress")
            };
            return status_value(
                provider_id,
                state,
                "pending",
                true,
                configured,
                false,
                None,
                operation.execution_id,
                Some(message),
            );
        }

        if let Some(status) = self
            .attached_status(provider_id, provider, configured, config)
            .await
        {
            return status;
        }

        if let Some(record) = self.load_process_record(provider_id) {
            if record.state == "running" && record.process_policy == "persistent" {
                if process_identity_matches(&record) {
                    let (probe, message) = self.probe(provider_id, provider, config).await;
                    if probe == ProbeState::Ready {
                        self.clear_last_error(provider_id);
                        return status_value(
                            provider_id,
                            "running",
                            "ready",
                            true,
                            configured,
                            true,
                            record.pid,
                            Some(record.execution_id),
                            message.as_deref(),
                        );
                    }
                    return status_value(
                        provider_id,
                        if self.last_error(provider_id).is_some() {
                            "failed"
                        } else {
                            "starting"
                        },
                        "pending",
                        true,
                        configured,
                        true,
                        record.pid,
                        Some(record.execution_id),
                        self.last_error(provider_id)
                            .as_deref()
                            .or(message.as_deref())
                            .or(Some("readiness_pending")),
                    );
                }
                // A stale PID must never be killed or treated as owned. Keep the
                // record for diagnostics but make the ownership loss explicit.
                // A healthy provider is still actually running even when its
                // detached launcher PID can no longer be verified.
                let (probe, message) = self.probe(provider_id, provider, config).await;
                if probe == ProbeState::Ready {
                    return status_value(
                        provider_id,
                        "running",
                        "ready",
                        true,
                        configured,
                        false,
                        None,
                        Some(record.execution_id),
                        Some("ownership_unverified"),
                    );
                }
                return status_value(
                    provider_id,
                    "unknown",
                    "unknown",
                    true,
                    configured,
                    false,
                    record.pid,
                    Some(record.execution_id),
                    message.as_deref().or(Some("ownership_unverified")),
                );
            }
        }

        match self.probe(provider_id, provider, config).await {
            (ProbeState::Ready, None) => status_value(
                provider_id,
                "running",
                "ready",
                true,
                configured,
                false,
                None,
                None,
                None,
            ),
            (ProbeState::Ready, Some(message)) => status_value(
                provider_id,
                "running",
                "ready",
                true,
                configured,
                false,
                None,
                None,
                Some(message.as_str()),
            ),
            (ProbeState::Starting, message) => status_value(
                provider_id,
                "starting",
                "pending",
                true,
                configured,
                false,
                None,
                None,
                message.as_deref(),
            ),
            (ProbeState::Stopped, Some(message)) if message == "readiness_not_configured" => {
                status_value(
                    provider_id,
                    "unknown",
                    "unknown",
                    true,
                    configured,
                    false,
                    None,
                    None,
                    Some("readiness_not_configured"),
                )
            }
            (ProbeState::Stopped, message) => {
                let error = self.last_error(provider_id);
                status_value(
                    provider_id,
                    if error.is_some() { "failed" } else { "stopped" },
                    "not_ready",
                    true,
                    configured,
                    false,
                    None,
                    None,
                    error.as_deref().or(message.as_deref()),
                )
            }
            (ProbeState::Unknown, message) => {
                let error = self.last_error(provider_id);
                status_value(
                    provider_id,
                    if error.is_some() { "failed" } else { "unknown" },
                    "unknown",
                    true,
                    configured,
                    false,
                    None,
                    None,
                    error
                        .as_deref()
                        .or(message.as_deref())
                        .or(Some("readiness_unknown")),
                )
            }
        }
    }

    pub async fn start(
        &self,
        provider_id: &str,
        provider: &ProviderConfig,
        config: &Config,
    ) -> Result<LifecycleStatus, String> {
        ensure_allowed(provider_id, provider, config)?;
        let current = self.status(provider_id, provider, config).await;
        if matches!(current.state.as_str(), "running" | "starting") {
            return Ok(current);
        }
        let lifecycle = &provider.lifecycle;
        let program = lifecycle
            .program
            .as_deref()
            .ok_or_else(|| "command_not_configured".to_string())?;
        let execution_id = format!("lifecycle-{}-{}", provider_id, uuid_like());
        let _lease = self.acquire(provider_id, "start", Some(execution_id.clone()))?;
        self.clear_last_error(provider_id);
        let expanded_args = expand_args(&lifecycle.args, provider_id, provider)?;
        let command_hash = command_hash(program, &expanded_args);
        let mut command = build_command(program, &lifecycle.args, provider_id, provider, config)?;
        if let Some(dir) = &lifecycle.working_dir {
            command.current_dir(dir);
        }
        let started_at = unix_ms();
        let child = command
            .spawn()
            .map_err(|error| format!("command_failed: {error}"))?;
        let pid = child.id();
        let record = PersistedProcess {
            execution_id: execution_id.clone(),
            pid,
            program: program.to_string(),
            command_hash: command_hash.clone(),
            process_policy: process_policy_name(&lifecycle.process_policy).to_string(),
            state: "starting".to_string(),
        };
        self.processes
            .lock()
            .expect("lifecycle process lock")
            .insert(
                provider_id.to_string(),
                ManagedProcess {
                    child,
                    execution_id: execution_id.clone(),
                    pid,
                    command_hash: command_hash.clone(),
                },
            );
        self.save_process_record(provider_id, &record);

        match self.wait_until_ready(provider_id, provider, config).await {
            Ok(()) => {
                self.update_process_state(provider_id, "running");
                self.clear_last_error(provider_id);
                self.record_event(
                    provider_id,
                    Some(&execution_id),
                    "start",
                    "running",
                    "success",
                    started_at,
                    pid,
                    Some(&command_hash),
                    None,
                );
                Ok(status_value(
                    provider_id,
                    "running",
                    "ready",
                    true,
                    true,
                    true,
                    pid,
                    Some(execution_id),
                    None,
                ))
            }
            Err(reason) => {
                self.kill_attached(provider_id);
                self.remove_process_record(provider_id);
                self.set_last_error(provider_id, &reason);
                self.record_event(
                    provider_id,
                    Some(&execution_id),
                    "start",
                    "failed",
                    "failure",
                    started_at,
                    pid,
                    Some(&command_hash),
                    Some(&reason),
                );
                Err(reason)
            }
        }
    }

    pub async fn stop(
        &self,
        provider_id: &str,
        provider: &ProviderConfig,
        config: &Config,
    ) -> Result<LifecycleStatus, String> {
        ensure_allowed(provider_id, provider, config)?;
        let started_at = unix_ms();
        let execution_id = format!("lifecycle-{}-{}", provider_id, uuid_like());
        let _lease = self.acquire(provider_id, "stop", Some(execution_id.clone()))?;
        self.clear_last_error(provider_id);
        // An explicit stop command is authoritative. This matters for shell
        // launchers that own a child server process; killing only the wrapper
        // would leave the actual model server running.
        if let Some(program) = provider.lifecycle.stop_program.as_deref() {
            let mut command =
                build_command(program, &provider.lifecycle.stop_args, provider_id, provider, config)?;
            if let Some(dir) = &provider.lifecycle.working_dir {
                command.current_dir(dir);
            }
            let command_hash = command_hash(program, &provider.lifecycle.stop_args);
            let status = tokio::time::timeout(
                command_timeout(provider.lifecycle.timeout_ms),
                command.status(),
            )
            .await
            .map_err(|_| "command_timeout".to_string())?
            .map_err(|error| format!("command_failed: {error}"))?;
            if !status.success() {
                let message = format!("command_failed: exit {}", status.code().unwrap_or(-1));
                self.set_last_error(provider_id, &message);
                self.record_event(
                    provider_id,
                    Some(&execution_id),
                    "stop",
                    "failed",
                    "failure",
                    started_at,
                    None,
                    Some(&command_hash),
                    Some(&message),
                );
                return Err(message);
            }
            self.kill_attached(provider_id);
            self.remove_process_record(provider_id);
            self.record_event(
                provider_id,
                Some(&execution_id),
                "stop",
                "stopped",
                "success",
                started_at,
                None,
                Some(&command_hash),
                None,
            );
            return Ok(status_value(
                provider_id,
                "stopped",
                "not_ready",
                true,
                true,
                false,
                None,
                None,
                None,
            ));
        }
        if provider.lifecycle.process_policy != ProcessPolicy::External {
            let managed = self
                .processes
                .lock()
                .expect("lifecycle process lock")
                .remove(provider_id);
            if let Some(mut process) = managed {
                if let Err(error) = process.child.start_kill() {
                    self.processes
                        .lock()
                        .expect("lifecycle process lock")
                        .insert(provider_id.to_string(), process);
                    let message = format!("command_failed: {error}");
                    self.set_last_error(provider_id, &message);
                    return Err(message);
                }
                self.remove_process_record(provider_id);
                self.record_event(
                    provider_id,
                    Some(&process.execution_id),
                    "stop",
                    "stopped",
                    "success",
                    started_at,
                    process.pid,
                    Some(&process.command_hash),
                    None,
                );
                return Ok(status_value(
                    provider_id,
                    "stopped",
                    "not_ready",
                    true,
                    true,
                    true,
                    process.pid,
                    Some(process.execution_id),
                    None,
                ));
            }
        }

        let lifecycle = &provider.lifecycle;
        let program = lifecycle
            .stop_program
            .as_deref()
            .ok_or_else(|| "stop_not_configured".to_string())?;
        let mut command =
            build_command(program, &lifecycle.stop_args, provider_id, provider, config)?;
        if let Some(dir) = &lifecycle.working_dir {
            command.current_dir(dir);
        }
        let status = tokio::time::timeout(command_timeout(lifecycle.timeout_ms), command.status())
            .await
            .map_err(|_| "command_timeout".to_string())?
            .map_err(|error| format!("command_failed: {error}"))?;
        if !status.success() {
            let message = format!("command_failed: exit {}", status.code().unwrap_or(-1));
            self.set_last_error(provider_id, &message);
            self.record_event(
                provider_id,
                Some(&execution_id),
                "stop",
                "failed",
                "failure",
                started_at,
                None,
                Some(&command_hash(program, &lifecycle.stop_args)),
                Some(&message),
            );
            return Err(message);
        }
        self.remove_process_record(provider_id);
        self.record_event(
            provider_id,
            Some(&execution_id),
            "stop",
            "stopped",
            "success",
            started_at,
            None,
            Some(&command_hash(program, &lifecycle.stop_args)),
            None,
        );
        Ok(status_value(
            provider_id,
            "stopped",
            "not_ready",
            true,
            true,
            false,
            None,
            None,
            None,
        ))
    }

    /// Called once after the listener is available. Only an explicit `stopped`
    /// result can trigger startup; unknown and permission failures never do.
    pub async fn startup_check(&self, config: &Config) {
        for (provider_id, provider) in &config.providers {
            if provider.lifecycle.mode != "startup_check" || !provider.lifecycle.controllable {
                continue;
            }
            let current = self.status(provider_id, provider, config).await;
            if current.state == "stopped" {
                if let Err(error) = self.start(provider_id, provider, config).await {
                    tracing::warn!(provider = %provider_id, error = %error, "provider startup_check failed");
                }
            }
        }
    }

    /// Stop only services owned by this daemon and configured with `session`.
    /// Persistent and external services intentionally survive normal shutdown.
    pub async fn shutdown_session_services(&self, providers: &HashMap<String, ProviderConfig>) {
        let ids: Vec<String> = self
            .processes
            .lock()
            .expect("lifecycle process lock")
            .keys()
            .cloned()
            .collect();
        for provider_id in ids {
            let Some(provider) = providers.get(&provider_id) else {
                continue;
            };
            if provider.lifecycle.process_policy == ProcessPolicy::Session {
                self.kill_attached(&provider_id);
                self.remove_process_record(&provider_id);
            }
        }
    }

    fn acquire(
        &self,
        provider_id: &str,
        action: &str,
        execution_id: Option<String>,
    ) -> Result<OperationLease, String> {
        let mut operations = self.operations.lock().expect("lifecycle operation lock");
        if operations.contains_key(provider_id) {
            return Err("lifecycle_operation_in_progress".into());
        }
        operations.insert(
            provider_id.to_string(),
            OperationInfo {
                action: action.to_string(),
                execution_id: execution_id.clone(),
            },
        );
        Ok(OperationLease {
            provider_id: provider_id.to_string(),
            execution_id,
            operations: self.operations.clone(),
        })
    }

    async fn attached_status(
        &self,
        provider_id: &str,
        provider: &ProviderConfig,
        configured: bool,
        config: &Config,
    ) -> Option<LifecycleStatus> {
        let snapshot = {
            let mut processes = self.processes.lock().expect("lifecycle process lock");
            let process = processes.get_mut(provider_id)?;
            match process.child.try_wait() {
                Ok(None) => AttachedProcessSnapshot::Running {
                    pid: process.pid,
                    execution_id: process.execution_id.clone(),
                },
                Ok(Some(status)) => {
                    let code = status.code().map(|code| code.to_string());
                    processes.remove(provider_id);
                    AttachedProcessSnapshot::Exited(code)
                }
                Err(_) => AttachedProcessSnapshot::Error {
                    pid: process.pid,
                    execution_id: process.execution_id.clone(),
                },
            }
        };

        match snapshot {
            AttachedProcessSnapshot::Running { pid, execution_id } => {
                let (probe, message) = self.probe(provider_id, provider, config).await;
                Some(status_value(
                    provider_id,
                    if probe == ProbeState::Ready { "running" } else { "starting" },
                    if probe == ProbeState::Ready { "ready" } else { "pending" },
                    true,
                    configured,
                    true,
                    pid,
                    Some(execution_id),
                    message.as_deref().or_else(|| {
                        (probe != ProbeState::Ready).then_some("readiness_pending")
                    }),
                ))
            }
            AttachedProcessSnapshot::Exited(code) => {
                self.remove_process_record(provider_id);
                let (probe, message) = self.probe(provider_id, provider, config).await;
                if probe == ProbeState::Ready {
                    Some(status_value(
                        provider_id,
                        "running",
                        "ready",
                        provider.lifecycle.controllable,
                        configured,
                        false,
                        None,
                        None,
                        message.as_deref(),
                    ))
                } else {
                    Some(status_value(
                        provider_id,
                        "stopped",
                        "not_ready",
                        provider.lifecycle.controllable,
                        configured,
                        false,
                        None,
                        None,
                        message.as_deref().or(code.as_deref()),
                    ))
                }
            }
            AttachedProcessSnapshot::Error { pid, execution_id } => Some(status_value(
                provider_id,
                "unknown",
                "unknown",
                true,
                configured,
                true,
                pid,
                Some(execution_id),
                Some("process_status_failed"),
            )),
        }
    }

    async fn wait_until_ready(
        &self,
        provider_id: &str,
        provider: &ProviderConfig,
        config: &Config,
    ) -> Result<(), String> {
        let deadline = tokio::time::Instant::now() + readiness_timeout(provider);
        loop {
            let process_exit = {
                let mut processes = self.processes.lock().expect("lifecycle process lock");
                let Some(process) = processes.get_mut(provider_id) else {
                    return Err("process_lost".into());
                };
                match process.child.try_wait() {
                    Ok(None) => None,
                    Ok(Some(status)) => Some(status.code().unwrap_or(-1)),
                    Err(_) => return Err("process_status_failed".into()),
                }
            };
            if let Some(exit_code) = process_exit {
                // A registered launcher may intentionally detach the real
                // model service. Its exit is only successful if the provider
                // readiness check confirms that the detached service is up.
                return match self.probe(provider_id, provider, config).await {
                    (ProbeState::Ready, _) => Ok(()),
                    _ => Err(format!("readiness_process_exited:exit={exit_code}")),
                };
            }
            match self.probe(provider_id, provider, config).await {
                (ProbeState::Ready, _) => return Ok(()),
                (ProbeState::Stopped, Some(message)) if message == "readiness_not_configured" => {
                    return Err(message)
                }
                _ if tokio::time::Instant::now() >= deadline => {
                    return Err("readiness_timeout".into())
                }
                _ => sleep(POLL_INTERVAL).await,
            }
        }
    }

    async fn probe(
        &self,
        provider_id: &str,
        provider: &ProviderConfig,
        config: &Config,
    ) -> (ProbeState, Option<String>) {
        if let Some(program) = provider.lifecycle.status_program.as_deref() {
            return match run_status_command(
                program,
                &provider.lifecycle.status_args,
                provider_id,
                provider,
                config,
            )
            .await
            {
                Ok(code) => match code {
                    0 => (ProbeState::Ready, None),
                    3 => (ProbeState::Stopped, Some("status_stopped".into())),
                    4 => (ProbeState::Starting, Some("status_starting".into())),
                    _ => (ProbeState::Unknown, Some("status_command_failed".into())),
                },
                Err(error) => (ProbeState::Unknown, Some(error)),
            };
        }
        let Some(url) = health_url(&provider.base) else {
            return (ProbeState::Stopped, Some("readiness_not_configured".into()));
        };
        let timeout = command_timeout(provider.lifecycle.timeout_ms.min(5_000));
        let client = match reqwest::Client::builder().timeout(timeout).build() {
            Ok(client) => client,
            Err(_) => return (ProbeState::Unknown, Some("health_client_failed".into())),
        };
        match client.get(url).send().await {
            Ok(response) if response.status().is_success() => (ProbeState::Ready, None),
            Ok(_) => (ProbeState::Unknown, Some("health_not_ready".into())),
            Err(_) => (ProbeState::Unknown, Some("health_unreachable".into())),
        }
    }

    fn kill_attached(&self, provider_id: &str) {
        if let Some(mut process) = self
            .processes
            .lock()
            .expect("lifecycle process lock")
            .remove(provider_id)
        {
            let _ = process.child.start_kill();
        }
    }

    fn load_process_record(&self, provider_id: &str) -> Option<PersistedProcess> {
        let database = self.database.lock().ok()?.clone()?;
        let conn = database.lock().ok()?;
        conn.query_row(
            "SELECT execution_id, pid, program, command_hash, process_policy, state
             FROM provider_lifecycle_processes WHERE provider_id = ?1",
            [provider_id],
            |row| {
                Ok(PersistedProcess {
                    execution_id: row.get(0)?,
                    pid: row.get::<_, Option<i64>>(1)?.map(|pid| pid as u32),
                    program: row.get(2)?,
                    command_hash: row.get(3)?,
                    process_policy: row.get(4)?,
                    state: row.get(5)?,
                })
            },
        )
        .optional()
        .ok()
        .flatten()
    }

    fn save_process_record(&self, provider_id: &str, record: &PersistedProcess) {
        let Some(database) = self.database.lock().ok().and_then(|guard| guard.clone()) else {
            return;
        };
        let Ok(conn) = database.lock() else { return };
        let _ = conn.execute(
            "INSERT INTO provider_lifecycle_processes
             (provider_id, execution_id, pid, program, command_hash, process_policy, started_at, state, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(provider_id) DO UPDATE SET execution_id = excluded.execution_id,
             pid = excluded.pid, program = excluded.program, command_hash = excluded.command_hash,
             process_policy = excluded.process_policy, started_at = excluded.started_at,
             state = excluded.state, updated_at = excluded.updated_at",
            params![
                provider_id,
                record.execution_id,
                record.pid.map(i64::from),
                record.program,
                record.command_hash,
                record.process_policy,
                unix_ms(),
                record.state,
                unix_ms(),
            ],
        );
    }

    fn update_process_state(&self, provider_id: &str, state: &str) {
        let Some(database) = self.database.lock().ok().and_then(|guard| guard.clone()) else {
            return;
        };
        let Ok(conn) = database.lock() else { return };
        let _ = conn.execute(
            "UPDATE provider_lifecycle_processes SET state = ?1, updated_at = ?2 WHERE provider_id = ?3",
            params![state, unix_ms(), provider_id],
        );
    }

    fn remove_process_record(&self, provider_id: &str) {
        let Some(database) = self.database.lock().ok().and_then(|guard| guard.clone()) else {
            return;
        };
        if let Ok(conn) = database.lock() {
            let _ = conn.execute(
                "DELETE FROM provider_lifecycle_processes WHERE provider_id = ?1",
                [provider_id],
            );
        };
    }

    fn set_last_error(&self, provider_id: &str, message: &str) {
        if let Ok(mut errors) = self.last_errors.lock() {
            errors.insert(provider_id.to_string(), stable_message(message));
        }
    }

    fn clear_last_error(&self, provider_id: &str) {
        if let Ok(mut errors) = self.last_errors.lock() {
            errors.remove(provider_id);
        }
    }

    fn last_error(&self, provider_id: &str) -> Option<String> {
        self.last_errors
            .lock()
            .ok()
            .and_then(|errors| errors.get(provider_id).cloned())
    }

    #[allow(clippy::too_many_arguments)]
    fn record_event(
        &self,
        provider_id: &str,
        execution_id: Option<&str>,
        action: &str,
        state: &str,
        outcome: &str,
        started_at: i64,
        pid: Option<u32>,
        command_hash: Option<&str>,
        message: Option<&str>,
    ) {
        let Some(database) = self.database.lock().ok().and_then(|guard| guard.clone()) else {
            return;
        };
        if let Ok(conn) = database.lock() {
            let _ = conn.execute(
                "INSERT INTO provider_lifecycle_events
                 (provider_id, execution_id, action, state, outcome, started_at, finished_at, pid, command_hash, message)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    provider_id,
                    execution_id,
                    action,
                    state,
                    outcome,
                    started_at,
                    unix_ms(),
                    pid.map(i64::from),
                    command_hash,
                    message.map(stable_message),
                ],
            );
        };
    }
}

fn status_value(
    provider_id: &str,
    state: &str,
    readiness: &str,
    controllable: bool,
    configured: bool,
    managed_by_jev: bool,
    pid: Option<u32>,
    execution_id: Option<String>,
    message: Option<&str>,
) -> LifecycleStatus {
    LifecycleStatus {
        provider_id: provider_id.into(),
        state: state.into(),
        readiness: readiness.into(),
        controllable,
        configured,
        managed_by_jev,
        pid,
        execution_id,
        message: message.map(str::to_owned),
    }
}

fn lifecycle_configured(provider: &ProviderConfig) -> bool {
    provider.lifecycle.program.is_some()
        || provider.lifecycle.stop_program.is_some()
        || provider.lifecycle.status_program.is_some()
}

fn process_policy_name(policy: &ProcessPolicy) -> &'static str {
    match policy {
        ProcessPolicy::Persistent => "persistent",
        ProcessPolicy::Session => "session",
        ProcessPolicy::External => "external",
    }
}

fn command_timeout(value: u64) -> Duration {
    Duration::from_millis(value.clamp(1, MAX_COMMAND_TIMEOUT_MS))
}

fn readiness_timeout(provider: &ProviderConfig) -> Duration {
    Duration::from_millis(
        provider
            .lifecycle
            .readiness_timeout_ms
            .clamp(1, MAX_READINESS_TIMEOUT_MS),
    )
}

fn health_url(base: &str) -> Option<reqwest::Url> {
    let mut url = reqwest::Url::parse(base).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return None;
    }
    url.set_path("/health");
    url.set_query(None);
    url.set_fragment(None);
    Some(url)
}

async fn run_status_command(
    program: &str,
    args: &[String],
    provider_id: &str,
    provider: &ProviderConfig,
    config: &Config,
) -> Result<i32, String> {
    let mut command = build_command(program, args, provider_id, provider, config)?;
    if let Some(dir) = &provider.lifecycle.working_dir {
        command.current_dir(dir);
    }
    let status = tokio::time::timeout(
        command_timeout(provider.lifecycle.timeout_ms),
        command.status(),
    )
    .await
    .map_err(|_| "status_command_timeout".to_string())?
    .map_err(|error| format!("status_command_failed: {error}"))?;
    Ok(status.code().unwrap_or(-1))
}

fn process_identity_matches(record: &PersistedProcess) -> bool {
    let Some(pid) = record.pid else { return false };
    let mut system = System::new();
    let process_id = Pid::from_u32(pid);
    if !system.refresh_process(process_id) {
        return false;
    }
    let Some(process) = system.process(process_id) else {
        return false;
    };
    let Some(exe) = process.exe() else {
        return false;
    };
    if !same_executable(exe, std::path::Path::new(&record.program)) {
        return false;
    }
    let command = process.cmd();
    if command.is_empty() {
        return false;
    }
    let args: Vec<String> = command.iter().skip(1).cloned().collect();
    command_hash(&record.program, &args) == record.command_hash
}

fn same_executable(actual: &std::path::Path, expected: &std::path::Path) -> bool {
    let actual = std::fs::canonicalize(actual).unwrap_or_else(|_| actual.to_path_buf());
    let expected = std::fs::canonicalize(expected).unwrap_or_else(|_| expected.to_path_buf());
    if cfg!(windows) {
        actual
            .to_string_lossy()
            .eq_ignore_ascii_case(&expected.to_string_lossy())
    } else {
        actual == expected
    }
}

fn ensure_allowed(
    provider_id: &str,
    provider: &ProviderConfig,
    config: &Config,
) -> Result<(), String> {
    if !config.allow_host_commands {
        return Err("host_commands_disabled".into());
    }
    if !provider.lifecycle.controllable {
        return Err(format!("provider_not_controllable: {provider_id}"));
    }
    Ok(())
}

fn build_command(
    program: &str,
    args: &[String],
    provider_id: &str,
    provider: &ProviderConfig,
    config: &Config,
) -> Result<Command, String> {
    if program.trim().is_empty() {
        return Err("command_not_configured".into());
    }
    let executable = std::path::Path::new(program);
    if !executable.is_absolute() {
        return Err("program_path_must_be_absolute".into());
    }
    let is_shell = executable
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name.to_ascii_lowercase().as_str(),
                "cmd.exe" | "powershell.exe" | "pwsh.exe" | "sh" | "bash" | "zsh"
            )
        });
    if is_shell && !config.allow_shell_commands {
        return Err("shell_execution_not_allowed".into());
    }
    let expanded_args = expand_args(args, provider_id, provider)?;
    let mut command = Command::new(program);
    command.args(expanded_args);
    command.env("JEV_PROVIDER_ID", provider_id);
    command.env("JEV_PROVIDER_KIND", &provider.kind);
    command.env("JEV_PROVIDER_BASE_URL", &provider.base);
    command.env(
        "JEV_PROVIDER_MODELS_JSON",
        serde_json::to_string(&provider.models).unwrap_or_else(|_| "[]".into()),
    );
    command.env("JEV_PROVIDER_MODELS", provider.models.join(","));
    if let Some(dir) = &provider.lifecycle.working_dir {
        command.env("JEV_PROVIDER_WORKING_DIR", dir);
    }
    if provider.lifecycle.inject_api_key {
        let key = config
            .effective_api_key(provider_id)
            .ok_or_else(|| "secrets_unavailable".to_string())?;
        command.env("JEV_PROVIDER_API_KEY", key);
    }
    Ok(command)
}

fn expand_args(
    args: &[String],
    provider_id: &str,
    provider: &ProviderConfig,
) -> Result<Vec<String>, String> {
    args.iter()
        .map(|arg| expand_arg(arg, provider_id, provider))
        .collect()
}

fn expand_arg(arg: &str, provider_id: &str, provider: &ProviderConfig) -> Result<String, String> {
    let mut value = arg.to_string();
    let replacements = [
        ("${JEV_PROVIDER_ID}", provider_id.to_string()),
        ("${JEV_PROVIDER_KIND}", provider.kind.clone()),
        ("${JEV_PROVIDER_BASE_URL}", provider.base.clone()),
        ("${JEV_PROVIDER_MODELS}", provider.models.join(",")),
        (
            "${JEV_PROVIDER_MODELS_JSON}",
            serde_json::to_string(&provider.models).unwrap_or_else(|_| "[]".into()),
        ),
        (
            "${JEV_PROVIDER_API_KEY_ENV}",
            provider.api_key_env.clone().unwrap_or_default(),
        ),
    ];
    for (needle, replacement) in replacements {
        value = value.replace(needle, &replacement);
    }
    if value.contains("${JEV_PROVIDER_API_KEY}") {
        return Err("secret_placeholder_requires_environment".into());
    }
    if value.contains("${") {
        return Err("unknown_lifecycle_placeholder".into());
    }
    Ok(value)
}

fn command_hash(program: &str, args: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(program.as_bytes());
    hasher.update([0]);
    for arg in args {
        hasher.update(arg.as_bytes());
        hasher.update([0]);
    }
    format!("{:x}", hasher.finalize())
}

fn stable_message(message: &str) -> String {
    // Only stable error codes are stored. Child/HTTP output is never captured,
    // so API keys cannot reach the audit table through process output.
    message
        .split(':')
        .next()
        .unwrap_or(message)
        .trim()
        .chars()
        .take(120)
        .collect()
}

fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

fn uuid_like() -> String {
    EXECUTION_SEQUENCE
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        .saturating_add(1)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> ProviderConfig {
        ProviderConfig {
            kind: "typesafe".into(),
            base: "http://127.0.0.1:11436/v1/systemone".into(),
            name: None,
            account: None,
            models: vec!["jev-local".into()],
            api_key: None,
            api_key_env: None,
            enabled: true,
            forward_extensions: false,
            lifecycle: crate::config::ProviderLifecycleConfig {
                controllable: true,
                program: Some("C:/Tools/jev-test-service.exe".into()),
                args: vec!["${JEV_PROVIDER_BASE_URL}".into()],
                ..Default::default()
            },
        }
    }

    #[tokio::test]
    async fn host_command_gate_blocks_start_before_spawning() {
        let manager = LifecycleManager::default();
        let config = Config::default();
        let error = manager
            .start("local", &provider(), &config)
            .await
            .expect_err("disabled host commands must block start");
        assert_eq!(error, "host_commands_disabled");
    }

    #[tokio::test]
    async fn status_reports_in_flight_operation_after_page_reload() {
        let manager = LifecycleManager::default();
        let mut config = Config::default();
        config.allow_host_commands = true;
        let execution_id = "lifecycle-laya-test".to_string();
        let lease = manager
            .acquire("laya", "start", Some(execution_id.clone()))
            .expect("test operation should be acquired");

        let status = manager.status("laya", &provider(), &config).await;
        assert_eq!(status.state, "starting");
        assert_eq!(status.readiness, "pending");
        assert_eq!(status.execution_id.as_deref(), Some(execution_id.as_str()));

        drop(lease);
        let status = manager.status("laya", &provider(), &config).await;
        assert_ne!(status.state, "starting");
    }

    #[test]
    fn unknown_placeholder_is_rejected_without_shell_parsing() {
        let provider = provider();
        let error = expand_arg("--unknown=${JEV_UNREGISTERED}", "local", &provider)
            .expect_err("unknown placeholders must not reach a child process");
        assert_eq!(error, "unknown_lifecycle_placeholder");
    }

    #[tokio::test]
    async fn shell_programs_are_rejected_by_default_even_when_the_global_gate_is_open() {
        let manager = LifecycleManager::default();
        let mut provider = provider();
        provider.lifecycle.program = Some(if cfg!(windows) {
            "C:/Windows/System32/cmd.exe"
        } else {
            "/bin/sh"
        }
        .into());
        let mut config = Config::default();
        config.allow_host_commands = true;
        let error = manager
            .start("local", &provider, &config)
            .await
            .expect_err("shell entrypoints must never be used as lifecycle programs");
        assert_eq!(error, "shell_execution_not_allowed");
    }

    #[test]
    fn explicitly_allowed_shell_programs_use_structured_arguments() {
        let mut config = Config::default();
        config.allow_host_commands = true;
        config.allow_shell_commands = true;
        let provider = provider();
        let program = if cfg!(windows) {
            "C:/Windows/System32/cmd.exe"
        } else {
            "/bin/sh"
        };
        build_command(program, &["/C".into(), "echo Jev".into()], "local", &provider, &config)
            .expect("an explicit shell opt-in should allow the registered interpreter");
    }

    #[tokio::test]
    async fn configured_service_without_a_probe_is_unknown_not_stopped() {
        let manager = LifecycleManager::default();
        let mut config = Config::default();
        config.allow_host_commands = true;
        let mut provider = provider();
        provider.base = "file:///tmp/laya".into();
        provider.lifecycle.program = Some("C:/Tools/jev-test-service.exe".into());
        let status = manager.status("laya", &provider, &config).await;
        assert_eq!(status.state, "unknown");
        assert_eq!(status.readiness, "unknown");
        assert_eq!(status.message.as_deref(), Some("readiness_not_configured"));
    }

    #[test]
    fn health_url_discards_provider_path_and_query() {
        assert_eq!(
            health_url("http://127.0.0.1:11436/v1/systemone?x=1")
                .unwrap()
                .as_str(),
            "http://127.0.0.1:11436/health"
        );
    }
}
