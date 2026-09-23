//! Sidecar：打包进资源的 `jev-switch.exe`（daemon release 产物）启停与就绪探测。
//!
//! 契约对齐：
//! - `JEV_SWITCH_CONFIG` → `%APPDATA%\jev-switch\providers.toml`（首启播种模板、已有不覆盖）
//! - `JEV_SWITCH_MODE` **不设** = local 态（docs/deployment.md §三）
//! - `JEV_UI_DIST` → 打包的 `ui/dist`（daemon ServeDir 同源托管，webview 直载同源 UI）
//! - 就绪 = `GET /health` → 200（contracts/05 §2），到达后壳层 navigate 进控制台

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use tauri::{AppHandle, Manager, WebviewWindow};

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

/// 首次启动自动落地模板；**已有文件不覆盖**（Q5=a 文件=真值源）。
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
    let dev_plain = "jev-switch.exe";
    let dev_triple = format!("jev-switch-{}-pc-windows-msvc.exe", arch_triple_prefix());
    let names: [&str; 4] = [
        daemon_triple.as_str(),
        daemon_plain,
        dev_triple.as_str(),
        dev_plain,
    ];

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
            // dev：exe 在 src-tauri/target/{debug,release}/ → ../../binaries/
            candidates.push(dir.join("../../binaries").join(&daemon_triple));
            candidates.push(dir.join("../../binaries").join(&daemon_plain));
            candidates.push(dir.join("../../binaries").join(&dev_triple));
            candidates.push(dir.join("../../binaries").join(dev_plain));
            // dev：仓库根 rs/target/release（免 copy 直跑）
            candidates.push(dir.join("../../../rs/target/release").join(dev_plain));
        }
    }
    // cwd 兜底（`tauri dev` 常以 src-tauri 为 cwd）
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("binaries").join(&daemon_triple));
        candidates.push(cwd.join("binaries").join(&daemon_plain));
        candidates.push(cwd.join("binaries").join(dev_plain));
        candidates.push(cwd.join("rs/target/release").join(dev_plain));
        candidates.push(cwd.join("../rs/target/release").join(dev_plain));
    }

    candidates.into_iter().find(|p| p.is_file())
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
pub fn resolve_ui_dist(app: &AppHandle) -> PathBuf {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(res) = app.path().resource_dir() {
        candidates.push(res.join("ui/dist"));
        candidates.push(res.join("resources/ui/dist"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("ui/dist"));
            // dev：src-tauri/target/{debug,release} → 仓库根 ui/dist
            candidates.push(dir.join("../../../ui/dist"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("ui/dist"));
        candidates.push(cwd.join("../ui/dist"));
    }
    candidates
        .into_iter()
        .find(|p| p.join("index.html").is_file())
        .unwrap_or_else(|| PathBuf::from("ui/dist"))
}

/// `GET /health` 是否 200（纯 std TCP，不引入 HTTP 客户端依赖）。
pub fn health_ok() -> bool {
    use std::io::{BufRead, BufReader, Write};
    let Ok(stream) = std::net::TcpStream::connect_timeout(
        &"127.0.0.1:11435".parse().unwrap(),
        Duration::from_millis(300),
    ) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(800)));
    let mut stream = stream;
    if stream
        .write_all(
            b"GET /health HTTP/1.1\r\nHost: 127.0.0.1:11435\r\nConnection: close\r\n\r\n",
        )
        .is_err()
    {
        return false;
    }
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return false;
    }
    // "HTTP/1.1 200 OK"
    line.split_whitespace().nth(1) == Some("200")
}

/// spawn sidecar 并启动就绪轮询线程。
pub fn start(app: AppHandle, config_path: &Path) -> Result<(), String> {
    // 已有健康 daemon（开发机手起 / 上次残留）→ 直接复用，不重复 spawn
    if health_ok() {
        set_status(&app, "检测到已运行的 daemon（127.0.0.1:11435），直接接入…");
        spawn_watch(app, None);
        return Ok(());
    }

    let exe = resolve_sidecar_exe(&app).ok_or_else(|| {
        "找不到 sidecar 可执行文件（jev-switch.exe）。请按 docs/deployment.md「Tauri 桌面」节重新构建。".to_string()
    })?;
    let ui_dist = resolve_ui_dist(&app);

    let mut cmd = Command::new(&exe);
    cmd.env("JEV_SWITCH_CONFIG", config_path)
        .env("JEV_UI_DIST", &ui_dist)
        // JEV_SWITCH_MODE 不设 = local（docs/deployment.md §三）
        .current_dir(config_path.parent().unwrap_or(Path::new(".")))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // GUI 壳派生控制台 daemon：不闪黑窗
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let child = cmd.spawn().map_err(|e| format!("spawn sidecar 失败：{e}（{}）", exe.display()))?;
    *app.state::<ShellState>().child.lock().unwrap() = Some(child);

    set_status(&app, "守护进程已拉起，等待 /health 就绪…");
    spawn_watch(app, None);
    Ok(())
}

/// 就绪轮询：200 → location 切入控制台；子进程退出 → 状态提示（继续轮询兜底）。
fn spawn_watch(app: AppHandle, mut _unused: Option<()>) {
    std::thread::spawn(move || {
        let mut announced_death = false;
        loop {
            if health_ok() {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.eval(&format!("location.replace('{}/')", UI_ORIGIN));
                }
                return;
            }
            // 子进程若已退出，提示但继续轮询（可能有别的 daemon 接管 / 用户手修）
            if !announced_death {
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
                    announced_death = true;
                    set_status(
                        &app,
                        &format!(
                            "守护进程已退出（{status}）—— 正在继续等待 127.0.0.1:11435/health…"
                        ),
                    );
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

/// 退出时优雅终止子进程：先 `taskkill /T`（请求级），2s 后兜底 `kill`。
pub fn shutdown(child: &mut Child) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
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
