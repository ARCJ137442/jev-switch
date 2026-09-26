#![cfg(unix)]

//! 子进程回归：Unix SIGTERM 必须由 daemon 捕获并在短预算内正常退出。

use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct TempConfig(PathBuf);

impl Drop for TempConfig {
    fn drop(&mut self) {
        if let Some(parent) = self.0.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}

struct ChildGuard(Option<Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

fn temp_config(port: u16) -> TempConfig {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "jev-shutdown-signal-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create isolated config directory");
    let path = dir.join("providers.toml");
    std::fs::write(
        &path,
        format!("mode = \"local\"\nbind = \"127.0.0.1:{port}\"\n"),
    )
    .expect("write isolated config");
    TempConfig(path)
}

fn ephemeral_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read ephemeral port")
        .port()
}

async fn wait_for_health(child: &mut Child, base: &str) {
    let client = reqwest::Client::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().expect("check daemon process") {
            panic!("daemon exited before health was ready: {status}");
        }
        if let Ok(Ok(response)) = tokio::time::timeout(
            Duration::from_millis(300),
            client.get(format!("{base}/health")).send(),
        )
        .await
        {
            if response.status().is_success() {
                return;
            }
        }
        assert!(
            Instant::now() < deadline,
            "daemon health did not become ready"
        );
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
}

fn signal_term(pid: u32) {
    let pid = pid.to_string();
    let output = Command::new("kill")
        .args(["-TERM", pid.as_str()])
        .output()
        .expect("run Unix kill utility");
    assert!(
        output.status.success(),
        "kill -TERM failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn wait_for_exit(guard: &mut ChildGuard, budget: Duration) -> Output {
    let deadline = Instant::now() + budget;
    loop {
        if let Some(status) = guard
            .0
            .as_mut()
            .expect("guarded child")
            .try_wait()
            .expect("poll daemon exit")
        {
            assert!(
                status.success(),
                "daemon did not exit successfully: {status}"
            );
            let child = guard.0.take().expect("take completed child");
            return child.wait_with_output().expect("collect daemon output");
        }
        assert!(
            Instant::now() < deadline,
            "daemon ignored SIGTERM past shutdown budget"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

#[tokio::test]
async fn sigterm_gracefully_stops_the_real_daemon_process() {
    let port = ephemeral_port();
    let config = temp_config(port);
    let mut guard = ChildGuard(Some(
        Command::new(env!("CARGO_BIN_EXE_jev-switch"))
            .env("JEV_SWITCH_CONFIG", &config.0)
            .env(
                "JEV_SWITCH_DATA_DIR",
                config.0.parent().expect("config parent directory"),
            )
            .env_remove("JEV_BIND")
            .env_remove("JEV_SWITCH_MODE")
            .env("RUST_LOG", "info")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn daemon binary"),
    ));
    let child = guard.0.as_mut().expect("guarded child");
    let pid = child.id();
    let base = format!("http://127.0.0.1:{port}");

    wait_for_health(child, &base).await;
    signal_term(pid);
    let output = wait_for_exit(&mut guard, Duration::from_secs(5));
    let logs = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        logs.contains("SIGTERM"),
        "daemon logs must identify the actual signal; output was: {logs}"
    );
}
