//! Sidecar：打包进资源的 `jev-switch.exe`（daemon release 产物）启停与就绪探测。
//!
//! 契约对齐：
//! - `JEV_SWITCH_CONFIG` → `%APPDATA%\jev-switch\providers.toml`（首启播种模板、已有不覆盖）
//! - `JEV_SWITCH_MODE` **不设** = local 态（docs/deployment.md §三）
//! - `JEV_UI_DIST` → 打包的 `ui/dist`（daemon ServeDir 同源托管，webview 直载同源 UI）
//! - 就绪 = `/health` 的服务身份、接口修订和版本均兼容，随后切入控制台

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use tauri::{AppHandle, Manager, WebviewWindow};

use crate::runtime_probe::{self, RuntimeProbe};
use crate::ShellState;

/// webview 目标 origin（local 态绑 127.0.0.1:11435 —— contracts/05 §1）。
pub const UI_ORIGIN: &str = "http://127.0.0.1:11435";

/// 首启播种的默认配置（示例值 only：`api_key_env` 形式，无真实密钥）。
const DEFAULT_CONFIG: &str = include_str!("default_providers.toml");

/// 配置目录：`%APPDATA%\jev-switch`；无 APPDATA（异常环境）回退 `~/.jev-switch`。
pub fn config_dir() -> PathBuf {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        if !appdata.is_empty() {
            return PathBuf::from(appdata).join("jev-switch");
        }
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".jev-switch")
}

/// 首次启动落地导入模板；已有文件不覆盖，首次导入后以 SQLite 配置为准。
pub fn seed_config(dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join("providers.toml");
    if !path.exists() {
        std::fs::write(&path, DEFAULT_CONFIG)?;
    }
    Ok(path)
}

/// 解析 sidecar 可执行文件：打包布局 + dev 布局多候选兜底。
///
/// 打包态：externalBin 落资源目录（具体子路径随平台，逐一探测）。
/// dev 态：`src-tauri/binaries/jev-switch-<triple>.exe` 或仓库 `rs/target/release/`。
pub fn resolve_sidecar_exe(app: &AppHandle) -> Option<PathBuf> {
    // 打包名 = jev-switch-daemon.exe（**不能**与壳主二进制 jev-switch.exe 同名：
    // WiX ICE30 两组件同装一个文件名 → MSI light 失败；externalBin 落地时剥 triple）
    let daemon_plain = "jev-switch-daemon.exe";
    let daemon_triple = format!(
        "jev-switch-daemon-{}-pc-windows-msvc.exe",
        arch_triple_prefix()
    );
    // dev 兜底：rs workspace 的 bin 名仍是 jev-switch.exe
    #[cfg(debug_assertions)]
    let dev_plain = "jev-switch.exe";
    #[cfg(debug_assertions)]
    let dev_triple = format!("jev-switch-{}-pc-windows-msvc.exe", arch_triple_prefix());
    let names: [&str; 2] = [daemon_triple.as_str(), daemon_plain];

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(res) = app.path().resource_dir() {
        for name in names {
            candidates.push(res.join("binaries").join(name));
            candidates.push(res.join("bin").join(name));
            candidates.push(res.join(name));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in names {
                candidates.push(dir.join("binaries").join(name));
                candidates.push(dir.join("bin").join(name));
                candidates.push(dir.join(name));
            }
            // 仓库路径仅供 debug 开发；安装包不能回退到工作目录中的旧内核。
            #[cfg(debug_assertions)]
            {
                // dev：exe 在 src-tauri/target/debug/ → ../../binaries/
                candidates.push(dir.join("../../binaries").join(&daemon_triple));
                candidates.push(dir.join("../../binaries").join(&daemon_plain));
                candidates.push(dir.join("../../binaries").join(&dev_triple));
                candidates.push(dir.join("../../binaries").join(dev_plain));
                // dev：仓库根 rs/target/release（免 copy 直跑）
                candidates.push(dir.join("../../../rs/target/release").join(dev_plain));
            }
        }
    }
    // cwd 兜底（`tauri dev` 常以 src-tauri 为 cwd）
    #[cfg(debug_assertions)]
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("binaries").join(&daemon_triple));
        candidates.push(cwd.join("binaries").join(&daemon_plain));
        candidates.push(cwd.join("binaries").join(dev_plain));
        candidates.push(cwd.join("rs/target/release").join(dev_plain));
        candidates.push(cwd.join("../rs/target/release").join(dev_plain));
    }

    select_sidecar(candidates, std::env::current_exe().ok().as_deref())
}

fn select_sidecar(candidates: Vec<PathBuf>, current_exe: Option<&Path>) -> Option<PathBuf> {
    let current = current_exe.and_then(|path| path.canonicalize().ok());
    candidates.into_iter().find(|path| {
        path.is_file()
            && path
                .canonicalize()
                .ok()
                .is_some_and(|resolved| Some(resolved) != current)
    })
}

/// `x86_64-pc-windows-msvc` 的 arch 段（当前仅 Windows 线；mac/Linux 后置）。
fn arch_triple_prefix() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        _ => "x86_64",
    }
}

/// 解析 ui/dist：打包资源 → dev 仓库路径。
pub fn resolve_ui_dist(app: &AppHandle) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(res) = app.path().resource_dir() {
        candidates.push(res.join("ui/dist"));
        candidates.push(res.join("resources/ui/dist"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("ui/dist"));
            // 发行包只使用随包资源，避免静默加载工作区旧页面。
            #[cfg(debug_assertions)]
            candidates.push(dir.join("../../../ui/dist"));
        }
    }
    #[cfg(debug_assertions)]
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("ui/dist"));
        candidates.push(cwd.join("../ui/dist"));
    }
    candidates
        .into_iter()
        .find(|p| p.join("index.html").is_file())
}

fn probe_daemon() -> RuntimeProbe {
    runtime_probe::probe("127.0.0.1:11435".parse().unwrap())
}

/// spawn sidecar 并启动就绪轮询线程。
pub fn start(app: AppHandle, config_path: &Path) -> Result<(), String> {
    start_with_probe(app, config_path, probe_daemon(), spawn_sidecar)
}

fn start_with_probe(
    app: AppHandle,
    config_path: &Path,
    probe: RuntimeProbe,
    spawn: impl FnOnce(&AppHandle, &Path) -> Result<(), String>,
) -> Result<(), String> {
    let reuse_app = app.clone();
    dispatch_startup_probe(
        probe,
        || {
            set_status(
                &reuse_app,
                "检测到已运行的 daemon（127.0.0.1:11435），直接接入…",
            );
            spawn_watch(reuse_app);
        },
        || spawn(&app, config_path),
    )
}

/// Route startup through one tested decision point: reuse a compatible daemon,
/// reject an incompatible listener, or spawn only when the port is unavailable.
fn dispatch_startup_probe(
    probe: RuntimeProbe,
    on_reuse: impl FnOnce(),
    on_spawn: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    match probe {
        RuntimeProbe::Ready => {
            on_reuse();
            Ok(())
        }
        RuntimeProbe::Occupied(reason) => Err(format!("127.0.0.1:11435：{reason}")),
        RuntimeProbe::Unavailable => on_spawn(),
    }
}

fn spawn_sidecar(app: &AppHandle, config_path: &Path) -> Result<(), String> {
    let exe = resolve_sidecar_exe(app).ok_or_else(|| {
        "找不到随包内核（jev-switch-daemon.exe）。请按 docs/deployment.md「Tauri 桌面」节重新构建。".to_string()
    })?;
    let ui_dist =
        resolve_ui_dist(app).ok_or("缺少随包控制台 ui/dist/index.html，请重新构建桌面包")?;
    let data_dir = config_path.parent().unwrap_or(Path::new("."));
    let log_path = data_dir.join("daemon.log");
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("无法打开内核日志：{e}（{}）", log_path.display()))?;
    let error_log = log
        .try_clone()
        .map_err(|e| format!("无法初始化内核日志：{e}"))?;

    let mut cmd = Command::new(&exe);
    cmd.env("JEV_SWITCH_CONFIG", config_path)
        .env("JEV_UI_DIST", &ui_dist)
        .env("JEV_SWITCH_DATA_DIR", data_dir)
        // JEV_SWITCH_MODE 不设 = local（docs/deployment.md §三）
        .current_dir(config_path.parent().unwrap_or(Path::new(".")))
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(error_log));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // GUI 壳派生控制台 daemon：不闪黑窗
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("spawn sidecar 失败：{e}（{}）", exe.display()))?;
    *app.state::<ShellState>().child.lock().unwrap() = Some(child);

    set_status(app, "守护进程已拉起，等待 /health 就绪…");
    spawn_watch(app.clone());
    Ok(())
}

pub fn wait_for_daemon(app: AppHandle) {
    spawn_watch(app);
}

/// 就绪轮询：验证身份后切入控制台；已退出的子进程不再作为启动中显示。
fn spawn_watch(app: AppHandle) {
    std::thread::spawn(move || {
        loop {
            if app
                .state::<ShellState>()
                .quitting
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                return;
            }
            match probe_daemon() {
                RuntimeProbe::Ready => {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.eval(&format!("location.replace('{}/')", UI_ORIGIN));
                    }
                    return;
                }
                RuntimeProbe::Occupied(reason) => {
                    set_status(&app, &format!("127.0.0.1:11435：{reason}"));
                    return;
                }
                RuntimeProbe::Unavailable => {}
            }
            // 子进程若已退出，保留明确失败和日志位置。
            {
                let dead_status = {
                    let state = app.state::<ShellState>();
                    let mut guard = state.child.lock().unwrap();
                    match guard.as_mut().map(|c| c.try_wait()) {
                        Some(Ok(Some(status))) => {
                            *guard = None;
                            Some(status)
                        }
                        _ => None,
                    }
                };
                if let Some(status) = dead_status {
                    set_status(
                        &app,
                        &format!(
                            "守护进程已退出（{status}）。请查看配置目录中的 daemon.log 后重新启动。"
                        ),
                    );
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    });
}

/// 等待页状态文案（Rust → webview eval；等待页无跨源 fetch 能力）。
pub fn set_status(app: &AppHandle, msg: &str) {
    if let Some(w) = app.get_webview_window("main") {
        set_status_window(&w, msg);
    }
}

pub fn set_status_window(w: &WebviewWindow, msg: &str) {
    let safe = msg
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\r', " ")
        .replace('\n', " ");
    let _ = w.eval(&format!(
        "(()=>{{const e=document.getElementById('status');if(e)e.textContent='{}';}})()",
        safe
    ));
}

/// 打开配置目录（explorer 自身单实例，无需自造）。
pub fn open_config_dir(dir: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(dir)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("打开资源管理器失败：{e}"))
    }
    #[cfg(not(windows))]
    {
        let _ = dir;
        Err("打开配置目录仅实现 Windows（mac/Linux 后置）".into())
    }
}

/// 退出时先尝试 `taskkill /T`，2s 后兜底 `kill`；不保证上游请求已完成。
pub fn shutdown(child: &mut Child) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill")
            .creation_flags(0x0800_0000)
            .args(["/PID", &child.id().to_string(), "/T"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        for _ in 0..20 {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(_) => break,
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn never_resolves_the_shell_itself_as_its_daemon() {
        let current = std::env::current_exe().unwrap();
        assert!(select_sidecar(vec![current.clone()], Some(&current)).is_none());
    }

    #[test]
    fn compatible_existing_daemon_is_reused_without_spawning() {
        let reused = Cell::new(false);
        let spawned = Cell::new(false);

        let result = dispatch_startup_probe(
            RuntimeProbe::Ready,
            || reused.set(true),
            || {
                spawned.set(true);
                Ok(())
            },
        );

        assert!(result.is_ok());
        assert!(reused.get());
        assert!(!spawned.get());
    }

    #[test]
    fn incompatible_listener_is_not_replaced_or_reused() {
        let reused = Cell::new(false);
        let spawned = Cell::new(false);

        let result = dispatch_startup_probe(
            RuntimeProbe::Occupied("non-Jev service".into()),
            || reused.set(true),
            || {
                spawned.set(true);
                Ok(())
            },
        );

        let error = result.expect_err("an incompatible service must stop startup");
        assert!(error.contains("11435"));
        assert!(error.contains("non-Jev service"));
        assert!(!reused.get());
        assert!(!spawned.get());
    }

    #[test]
    fn unavailable_port_runs_the_spawn_path_and_returns_its_result() {
        let reused = Cell::new(false);
        let spawned = Cell::new(false);

        let result = dispatch_startup_probe(
            RuntimeProbe::Unavailable,
            || reused.set(true),
            || {
                spawned.set(true);
                Err("sidecar unavailable".into())
            },
        );

        assert_eq!(result, Err("sidecar unavailable".into()));
        assert!(!reused.get());
        assert!(spawned.get());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Requires a compatible live Jev daemon on 127.0.0.1:11435; this test never spawns or stops it"]
    fn isolated_tauri_shell_reuses_live_daemon_and_leaves_it_running() {
        use std::{
            fs,
            path::PathBuf,
            sync::{
                atomic::{AtomicBool, Ordering},
                mpsc, Arc,
            },
            thread,
            time::{Duration, Instant, SystemTime, UNIX_EPOCH},
        };
        use tauri::{Manager, RunEvent};

        let probe = probe_daemon();
        assert_eq!(
            probe,
            RuntimeProbe::Ready,
            "a compatible live daemon is required"
        );

        let data_dir = std::env::var_os("JEV_TAURI_REUSE_TEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system clock should be after Unix epoch")
                    .as_millis();
                std::env::temp_dir().join(format!(
                    "jev-tauri-reuse-{}-{timestamp}",
                    std::process::id()
                ))
            });
        fs::create_dir_all(&data_dir).expect("create isolated test data directory");
        let profile = data_dir.join("webview-profile");
        fs::create_dir_all(&profile).expect("create isolated WebView2 profile");
        let config_path = data_dir.join("providers.toml");

        let mut context = tauri::generate_context!();
        context.config_mut().app.windows[0].create = false;
        let window_profile = profile.clone();
        let setup_probe = probe;
        let spawn_called = Arc::new(AtomicBool::new(false));
        let spawn_called_on_setup = Arc::clone(&spawn_called);
        let app = tauri::Builder::default()
            .any_thread()
            .manage(ShellState {
                child: std::sync::Mutex::new(None),
                quitting: AtomicBool::new(false),
                config_dir: data_dir.clone(),
            })
            .setup(move |app| {
                tauri::WebviewWindowBuilder::from_config(
                    app.handle(),
                    &app.config().app.windows[0],
                )?
                .data_directory(window_profile.clone())
                .build()?;
                start_with_probe(
                    app.handle().clone(),
                    &config_path,
                    setup_probe,
                    move |_, _| {
                        spawn_called_on_setup.store(true, Ordering::SeqCst);
                        Err("sidecar spawn disabled in live reuse test".into())
                    },
                )
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
                Ok(())
            })
            .build(context)
            .expect("build isolated Tauri test shell");

        let finished = Arc::new(AtomicBool::new(false));
        let watchdog_finished = Arc::clone(&finished);
        let watchdog = app.handle().clone();
        let shell_state = app.handle().clone();
        let (watchdog_tx, watchdog_rx) = mpsc::channel();
        let watchdog_thread = thread::spawn(move || {
            if watchdog_rx.recv_timeout(Duration::from_secs(20)).is_err()
                && !watchdog_finished.swap(true, Ordering::SeqCst)
            {
                watchdog
                    .state::<ShellState>()
                    .quitting
                    .store(true, Ordering::SeqCst);
                watchdog.exit(3);
            }
        });

        let exit_code = app.run_return(move |app, event| {
            if matches!(event, RunEvent::Ready) {
                let window = app.get_webview_window("main").unwrap();
                let handle = app.clone();
                let worker_finished = Arc::clone(&finished);
                thread::spawn(move || {
                    let deadline = Instant::now() + Duration::from_secs(15);
                    while Instant::now() < deadline && !worker_finished.load(Ordering::SeqCst) {
                        let loaded_local_ui = window.url().is_ok_and(|url| {
                            url.scheme() == "http"
                                && url.host_str() == Some("127.0.0.1")
                                && url.port() == Some(11435)
                                && url.path() == "/"
                        });
                        if loaded_local_ui {
                            worker_finished.store(true, Ordering::SeqCst);
                            handle
                                .state::<ShellState>()
                                .quitting
                                .store(true, Ordering::SeqCst);
                            eprintln!(
                                "tauri-reuse: local UI loaded from the already-running daemon"
                            );
                            handle.exit(0);
                            return;
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
                    if !worker_finished.swap(true, Ordering::SeqCst) {
                        handle
                            .state::<ShellState>()
                            .quitting
                            .store(true, Ordering::SeqCst);
                        eprintln!("tauri-reuse: timed out waiting for local UI navigation");
                        handle.exit(2);
                    }
                });
            }
        });
        let _ = watchdog_tx.send(());
        let _ = watchdog_thread.join();

        assert_eq!(
            exit_code, 0,
            "the isolated Tauri shell should connect to the live UI"
        );
        assert!(
            !spawn_called.load(Ordering::SeqCst),
            "reuse must not invoke spawn"
        );
        assert!(shell_state
            .state::<ShellState>()
            .child
            .lock()
            .unwrap()
            .is_none());
        assert_eq!(
            probe_daemon(),
            RuntimeProbe::Ready,
            "the external daemon must remain healthy"
        );
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Starts an isolated sidecar and hidden Tauri WebView; run explicitly on Windows"]
    fn isolated_tauri_shell_spawns_and_stops_its_sidecar() {
        use std::{
            fs,
            io::{Read, Write},
            net::{SocketAddr, TcpStream},
            path::PathBuf,
            sync::{
                atomic::{AtomicBool, Ordering},
                Arc,
            },
            thread,
            time::{Duration, SystemTime, UNIX_EPOCH},
        };
        use tauri::{Manager, RunEvent};

        const ADDRESS: &str = "127.0.0.1:11437";
        let data_dir = std::env::var_os("JEV_TAURI_SPAWN_TEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system clock should be after Unix epoch")
                    .as_millis();
                std::env::temp_dir().join(format!(
                    "jev-tauri-spawn-{}-{timestamp}",
                    std::process::id()
                ))
            });
        assert!(
            !data_dir.exists(),
            "refusing to reuse existing isolated test directory"
        );
        fs::create_dir_all(&data_dir).expect("create isolated test directory");
        let profile = data_dir.join("webview-profile");
        fs::create_dir_all(&profile).expect("create isolated WebView2 profile");
        let config_path = data_dir.join("providers.toml");
        fs::write(
            &config_path,
            format!(
                "mode = \"local\"\nbind = \"{ADDRESS}\"\n\n[providers.test]\nkind = \"laya\"\nbase = \"http://127.0.0.1:18767/v1/systemone\"\nenabled = false\n"
            ),
        )
        .expect("write isolated provider configuration");

        let mut context = tauri::generate_context!();
        context.config_mut().app.windows[0].create = false;
        let window_profile = profile.clone();
        let setup_config = config_path.clone();
        let app = tauri::Builder::default()
            .any_thread()
            .manage(ShellState {
                child: std::sync::Mutex::new(None),
                quitting: AtomicBool::new(false),
                config_dir: data_dir.clone(),
            })
            .setup(move |app| {
                tauri::WebviewWindowBuilder::from_config(
                    app.handle(),
                    &app.config().app.windows[0],
                )?
                .data_directory(window_profile.clone())
                .build()?;
                start_with_probe(
                    app.handle().clone(),
                    &setup_config,
                    RuntimeProbe::Unavailable,
                    spawn_sidecar,
                )
                .map_err(std::io::Error::other)?;
                Ok(())
            })
            .build(context)
            .expect("build isolated Tauri shell");

        let worker_started = Arc::new(AtomicBool::new(false));
        let app_handle = app.handle().clone();
        let shell_state = app.handle().clone();
        let exit_code = app.run_return(move |app, event| {
            if matches!(event, RunEvent::Ready)
                && !worker_started.swap(true, Ordering::SeqCst)
            {
                let handle = app_handle.clone();
                thread::spawn(move || {
                    let address: SocketAddr = ADDRESS.parse().unwrap();
                    for _ in 0..120 {
                        if runtime_probe::probe(address) == RuntimeProbe::Ready {
                            let page_loaded = TcpStream::connect(address).is_ok_and(|mut stream| {
                                stream
                                    .set_read_timeout(Some(Duration::from_secs(2)))
                                    .is_ok()
                                    && write!(
                                        stream,
                                        "GET / HTTP/1.1\r\nHost: {ADDRESS}\r\nConnection: close\r\n\r\n"
                                    )
                                    .is_ok()
                                    && {
                                        let mut response = Vec::new();
                                        stream.read_to_end(&mut response).is_ok()
                                            && response.starts_with(b"HTTP/1.1 200")
                                            && String::from_utf8_lossy(&response)
                                                .contains("index-rsyine2x.js")
                                    }
                            });
                            eprintln!(
                                "tauri-spawn: isolated sidecar ready; current UI asset served={page_loaded}"
                            );
                            handle
                                .state::<ShellState>()
                                .quitting
                                .store(true, Ordering::SeqCst);
                            handle.exit(if page_loaded { 0 } else { 2 });
                            return;
                        }
                        thread::sleep(Duration::from_millis(150));
                    }
                    eprintln!("tauri-spawn: isolated sidecar readiness timed out");
                    handle
                        .state::<ShellState>()
                        .quitting
                        .store(true, Ordering::SeqCst);
                    handle.exit(3);
                });
            }
            if matches!(event, RunEvent::Exit) {
                let child = app.state::<ShellState>().child.lock().unwrap().take();
                if let Some(mut child) = child {
                    shutdown(&mut child);
                }
            }
        });

        assert_eq!(
            exit_code, 0,
            "the isolated Tauri sidecar should serve the UI"
        );
        assert!(shell_state
            .state::<ShellState>()
            .child
            .lock()
            .unwrap()
            .is_none());
        assert_eq!(
            runtime_probe::probe(ADDRESS.parse().unwrap()),
            RuntimeProbe::Unavailable,
            "the Tauri exit path should stop its owned sidecar"
        );
    }
}
