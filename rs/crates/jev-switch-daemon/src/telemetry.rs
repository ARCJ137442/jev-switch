//! Process-local runtime telemetry for the Dashboard.
//!
//! The counters intentionally describe the current daemon session only. They
//! are not a replacement for durable call history, token statistics, or
//! upstream billing data.

use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{atomic::{AtomicU64, Ordering}, Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, System};

const SAMPLE_CAPACITY: usize = 600;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct ResourceSnapshot {
    pub available: bool,
    #[cfg_attr(feature = "ts-rs", ts(type = "number | null"))]
    pub memory_bytes: Option<u64>,
    pub cpu_percent: Option<f64>,
    pub source: String,
    pub precision: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct TelemetrySample {
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub sample_at_ms: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub ingress_bytes_total: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub egress_bytes_total: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub ingress_bps: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub egress_bps: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub active_requests: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub total_requests: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub success_requests: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub failed_requests: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub failover_requests: u64,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct TelemetrySnapshot {
    pub schema_version: u32,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub session_started_at_ms: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub sampled_at_ms: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub ingress_bps: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub egress_bps: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub ingress_bytes_total: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub egress_bytes_total: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub active_requests: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub total_requests: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub success_requests: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub failed_requests: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub failover_requests: u64,
    pub avg_gateway_latency_ms: Option<f64>,
    pub upstream_latency_ms: Option<f64>,
    pub daemon: ResourceSnapshot,
    pub shell: ResourceSnapshot,
    pub samples: Vec<TelemetrySample>,
}

#[derive(Clone)]
pub struct Telemetry {
    inner: Arc<TelemetryInner>,
}

struct TelemetryInner {
    started_at_ms: u64,
    ingress_bytes: AtomicU64,
    egress_bytes: AtomicU64,
    active_requests: AtomicU64,
    total_requests: AtomicU64,
    success_requests: AtomicU64,
    failed_requests: AtomicU64,
    failover_requests: AtomicU64,
    gateway_latency_ms: AtomicU64,
    samples: Mutex<VecDeque<TelemetrySample>>,
}

impl Telemetry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(TelemetryInner {
                started_at_ms: unix_ms(),
                ingress_bytes: AtomicU64::new(0),
                egress_bytes: AtomicU64::new(0),
                active_requests: AtomicU64::new(0),
                total_requests: AtomicU64::new(0),
                success_requests: AtomicU64::new(0),
                failed_requests: AtomicU64::new(0),
                failover_requests: AtomicU64::new(0),
                gateway_latency_ms: AtomicU64::new(0),
                samples: Mutex::new(VecDeque::with_capacity(SAMPLE_CAPACITY)),
            }),
        }
    }

    pub fn begin_request(&self, ingress_bytes: u64) {
        self.inner.ingress_bytes.fetch_add(ingress_bytes, Ordering::Relaxed);
        self.inner.active_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn finish_request(
        &self,
        egress_bytes: Option<u64>,
        success: bool,
        gateway_latency_ms: u64,
        route_trace: Option<&serde_json::Value>,
    ) {
        if let Some(bytes) = egress_bytes {
            self.inner.egress_bytes.fetch_add(bytes, Ordering::Relaxed);
        }
        self.inner.active_requests.fetch_sub(1, Ordering::Relaxed);
        self.inner.total_requests.fetch_add(1, Ordering::Relaxed);
        if success {
            self.inner.success_requests.fetch_add(1, Ordering::Relaxed);
        } else {
            self.inner.failed_requests.fetch_add(1, Ordering::Relaxed);
        }
        if route_trace.map(trace_has_failover).unwrap_or(false) {
            self.inner.failover_requests.fetch_add(1, Ordering::Relaxed);
        }
        self.inner.gateway_latency_ms.fetch_add(gateway_latency_ms, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> TelemetrySnapshot {
        let sampled_at_ms = unix_ms();
        let ingress_bytes_total = self.inner.ingress_bytes.load(Ordering::Relaxed);
        let egress_bytes_total = self.inner.egress_bytes.load(Ordering::Relaxed);
        let active_requests = self.inner.active_requests.load(Ordering::Relaxed);
        let total_requests = self.inner.total_requests.load(Ordering::Relaxed);
        let success_requests = self.inner.success_requests.load(Ordering::Relaxed);
        let failed_requests = self.inner.failed_requests.load(Ordering::Relaxed);
        let failover_requests = self.inner.failover_requests.load(Ordering::Relaxed);
        let previous = self.inner.samples.lock().expect("telemetry samples lock").back().cloned();
        let (ingress_bps, egress_bps) = previous
            .as_ref()
            .map(|sample| {
                let elapsed_ms = sampled_at_ms.saturating_sub(sample.sample_at_ms).max(1);
                (
                    rate_per_second(ingress_bytes_total.saturating_sub(sample.ingress_bytes_total), elapsed_ms),
                    rate_per_second(egress_bytes_total.saturating_sub(sample.egress_bytes_total), elapsed_ms),
                )
            })
            .unwrap_or((0, 0));
        let sample = TelemetrySample {
            sample_at_ms: sampled_at_ms,
            ingress_bytes_total,
            egress_bytes_total,
            ingress_bps,
            egress_bps,
            active_requests,
            total_requests,
            success_requests,
            failed_requests,
            failover_requests,
        };
        let samples = {
            let mut samples = self.inner.samples.lock().expect("telemetry samples lock");
            samples.push_back(sample);
            while samples.len() > SAMPLE_CAPACITY {
                samples.pop_front();
            }
            samples.iter().cloned().collect()
        };
        let avg_gateway_latency_ms = if total_requests == 0 {
            None
        } else {
            Some(self.inner.gateway_latency_ms.load(Ordering::Relaxed) as f64 / total_requests as f64)
        };
        TelemetrySnapshot {
            schema_version: 1,
            session_started_at_ms: self.inner.started_at_ms,
            sampled_at_ms,
            ingress_bps,
            egress_bps,
            ingress_bytes_total,
            egress_bytes_total,
            active_requests,
            total_requests,
            success_requests,
            failed_requests,
            failover_requests,
            avg_gateway_latency_ms,
            upstream_latency_ms: None,
            daemon: daemon_resources(),
            shell: ResourceSnapshot {
                available: false,
                memory_bytes: None,
                cpu_percent: None,
                source: "tauri-shell-not-exposed".into(),
                precision: "unavailable".into(),
            },
            samples,
        }
    }

}

fn daemon_resources() -> ResourceSnapshot {
    let mut system = System::new();
    let pid = Pid::from_u32(std::process::id());
    if system.refresh_process(pid) {
        if let Some(process) = system.process(pid) {
            return ResourceSnapshot {
                available: true,
                memory_bytes: Some(process.memory()),
                cpu_percent: Some(f64::from(process.cpu_usage())),
                source: "daemon-process".into(),
                precision: "process-snapshot".into(),
            };
        }
    }
    ResourceSnapshot {
        available: false,
        memory_bytes: None,
        cpu_percent: None,
        source: "daemon-process".into(),
        precision: "unavailable".into(),
    }
}

fn trace_has_failover(trace: &serde_json::Value) -> bool {
    let Some(attempts) = trace.get("attempts").and_then(serde_json::Value::as_array) else {
        return false;
    };
    let providers: std::collections::HashSet<&str> = attempts
        .iter()
        .filter_map(|attempt| attempt.get("provider_id").and_then(serde_json::Value::as_str))
        .collect();
    providers.len() > 1
        || attempts.iter().any(|attempt| {
            attempt.get("retry_decision").and_then(serde_json::Value::as_str)
                == Some("next_candidate")
        })
}

fn rate_per_second(bytes: u64, elapsed_ms: u64) -> u64 {
    bytes.saturating_mul(1000).saturating_div(elapsed_ms.max(1))
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_counters_and_samples_are_process_local() {
        let telemetry = Telemetry::new();
        telemetry.begin_request(100);
        telemetry.finish_request(
            Some(250),
            true,
            12,
            Some(&serde_json::json!({
                "attempts": [
                    {"provider_id": "a", "retry_decision": "next_candidate"},
                    {"provider_id": "b"}
                ]
            })),
        );
        let snapshot = telemetry.snapshot();
        assert_eq!(snapshot.ingress_bytes_total, 100);
        assert_eq!(snapshot.egress_bytes_total, 250);
        assert_eq!(snapshot.total_requests, 1);
        assert_eq!(snapshot.success_requests, 1);
        assert_eq!(snapshot.failover_requests, 1);
        assert_eq!(snapshot.avg_gateway_latency_ms, Some(12.0));
        assert_eq!(snapshot.samples.len(), 1);
    }

    #[test]
    fn same_provider_retry_is_not_counted_as_failover() {
        let telemetry = Telemetry::new();
        telemetry.begin_request(1);
        telemetry.finish_request(
            None,
            false,
            2,
            Some(&serde_json::json!({
                "attempts": [
                    {"provider_id": "a", "retry_decision": "retry_same_candidate"},
                    {"provider_id": "a"}
                ]
            })),
        );
        assert_eq!(telemetry.snapshot().failover_requests, 0);
    }
}
