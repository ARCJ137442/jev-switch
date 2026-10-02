//! Jev-Switch 桌面壳（Tauri 2 · Windows 优先 —— docs/12 §二.8 裁决、§四-线3 / 任务 #44）。
//!
//! 形态对标 LM Studio「双击开箱」：
//! - 主窗口默认 900×560 逻辑像素，按显示器工作区/DPI 缩小后显示；服务身份核对通过后切入
//!   `http://127.0.0.1:11435`（sidecar daemon ServeDir 同源托管 ui/dist，local 态免 token）
//! - sidecar = 打包的 `jev-switch.exe`（externalBin）；`JEV_SWITCH_CONFIG` 指向
//!   `%APPDATA%\jev-switch\providers.toml`（首启播种、已有不覆盖）；`JEV_SWITCH_MODE` 不设 = local
//! - 单实例（tauri-plugin-single-instance）+ 系统托盘（显示 / 打开配置目录 / 退出）
//! - 关窗 = 隐藏到托盘；退出时终止本壳启动的 sidecar

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "android")]
mod android_gateway;
#[cfg(target_os = "android")]
mod android_keepalive;
#[cfg(not(target_os = "android"))]
mod runtime_probe;
#[cfg(not(target_os = "android"))]
mod sidecar;
#[cfg(all(feature = "standalone", not(target_os = "android")))]
mod standalone;
#[cfg(not(target_os = "android"))]
mod window_layout;

#[cfg(not(target_os = "android"))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(target_os = "android"))]
use std::sync::Mutex;

#[cfg(not(target_os = "android"))]
use tauri::menu::{Menu, MenuItem};
#[cfg(not(target_os = "android"))]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
#[cfg(not(target_os = "android"))]
use tauri::{RunEvent, WindowEvent};

/// 壳层托管状态：sidecar 子进程 + 退出标记 + 配置目录。
#[cfg(not(target_os = "android"))]
pub struct ShellState {
    pub child: Mutex<Option<std::process::Child>>,
    pub quitting: AtomicBool,
    pub config_dir: std::path::PathBuf,
    pub runtime_dir: Option<std::path::PathBuf>,
}

pub fn main() {
    let mut builder = tauri::Builder::default();
    #[cfg(target_os = "android")]
    {
        builder = builder.plugin(android_keepalive::init());
        builder = builder.invoke_handler(tauri::generate_handler![
            android_gateway::gateway_status,
            android_gateway::start_gateway,
            android_gateway::stop_gateway,
            android_gateway::toggle_gateway,
            android_gateway::android_debug_log_status,
            android_gateway::android_set_debug_log,
            android_gateway::android_keepalive_notification_status,
            android_gateway::android_set_keepalive_notification,
            android_gateway::android_notification_permission_state,
            android_gateway::android_request_notification_permission,
            android_gateway::android_take_pending_gateway_toggle,
        ]);
    }
    #[cfg(not(target_os = "android"))]
    {
        // 单实例锁必须第一个注册：二次双击 → 聚焦已有窗口
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main_window(app);
        }));
    }
    builder
        .setup(|app| {
            #[cfg(target_os = "android")]
            {
                let (config_path, desired_state_path, keepalive_state_path) =
                    android_gateway::prepare_paths(app.handle())?;
                let android_state = android_gateway::AndroidGatewayState::new(
                    config_path,
                    desired_state_path,
                    keepalive_state_path,
                );
                // Android owns the embedded daemon. Fresh install and a previous
                // running state both restore the local backend before the UI is usable;
                // only an explicit stopped marker keeps it off.
                let running = if android_state.wants_running() {
                    match android_state.start() {
                        Ok(status) => status.running,
                        Err(error) => {
                            eprintln!("Android gateway restore failed: {error}");
                            false
                        }
                    }
                } else { false };
                if let Err(error) = android_keepalive::set_state(
                    app.handle(),
                    android_state.keepalive_notification_status().enabled,
                    running,
                ) {
                    eprintln!("Android keepalive service restore failed: {error}");
                }
                app.manage(android_state);
                return Ok(());
            }
            #[cfg(not(target_os = "android"))]
            {
                let app_handle = app.handle().clone();

                // 首次显示前调整，避免大窗口闪现；服务尚未就绪也要显示等待/错误页。
                if let Some(window) = app.get_webview_window("main") {
                    let icon = app.default_window_icon().ok_or_else(|| {
                        std::io::Error::other("the bundled Jev-Switch window icon is missing")
                    })?;
                    window.set_icon(icon.clone())?;
                    if let Err(error) = window_layout::fit_initial_window(&window) {
                        eprintln!("initial window layout: {error}");
                    }
                    window.show()?;
                }

                // Standalone builds release their embedded daemon/UI before config or spawn.
                #[cfg(feature = "standalone")]
                let runtime_result = standalone::prepare_runtime().map(Some);
                #[cfg(not(feature = "standalone"))]
                let runtime_result: Result<Option<std::path::PathBuf>, String> = Ok(None);

                let (runtime_dir, runtime_error) = match runtime_result {
                    Ok(runtime_dir) => (runtime_dir, None),
                    Err(error) => (None, Some(error)),
                };

                // 1. 配置目录（用户数据与可执行资源缓存分开存放）
                let config_dir = sidecar::config_dir();

                app.manage(ShellState {
                    child: Mutex::new(None),
                    quitting: AtomicBool::new(false),
                    config_dir: config_dir.clone(),
                    runtime_dir,
                });

                // 2. 托盘
                build_tray(&app_handle)?;

                if let Some(error) = runtime_error {
                    sidecar::set_status(&app_handle, &error);
                    return Ok(());
                }

                let config_path = match sidecar::seed_config(&config_dir) {
                    Ok(path) => path,
                    Err(error) => {
                        sidecar::set_status(
                            &app_handle,
                            &format!("配置目录不可用：{error}（{}）", config_dir.display()),
                        );
                        return Ok(());
                    }
                };

                // 3. sidecar 拉起 + 就绪轮询（服务身份及版本通过 → 切入控制台 UI）
                if let Err(e) = sidecar::start(app_handle.clone(), &config_path) {
                    sidecar::set_status(&app_handle, &e);
                    #[cfg(not(feature = "standalone"))]
                    sidecar::wait_for_daemon(app_handle);
                }
                Ok(())
            }
        })
        .on_window_event(|_window, _event| {
            #[cfg(not(target_os = "android"))]
            {
                // 关窗 → 最小化到托盘（退出标记置位时才真退）
                if let WindowEvent::CloseRequested { api, .. } = _event {
                    let quitting = _window
                        .state::<ShellState>()
                        .quitting
                        .load(Ordering::SeqCst);
                    handle_window_close_request(_window, api, quitting);
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| match event {
            // 退出兜底：无论从托盘/关窗路径来，子进程统一在此收尸
            #[cfg(not(target_os = "android"))]
            RunEvent::Exit => {
                let child = _app.state::<ShellState>().child.lock().unwrap().take();
                if let Some(mut child) = child {
                    sidecar::shutdown(&mut child);
                }
            }
            _ => {}
        });
}

#[cfg(not(target_os = "android"))]
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[cfg(not(target_os = "android"))]
fn handle_window_close_request<R: tauri::Runtime>(
    window: &tauri::Window<R>,
    api: &tauri::CloseRequestApi,
    quitting: bool,
) {
    if !quitting {
        api.prevent_close();
        let _ = window.hide();
    }
}

#[cfg(not(target_os = "android"))]
fn handle_tray_menu_event(app: &tauri::AppHandle, menu_id: &str) {
    match menu_id {
        "show" => show_main_window(app),
        "open_config" => {
            let dir = app.state::<ShellState>().config_dir.clone();
            if let Err(error) = sidecar::open_config_dir(&dir) {
                sidecar::set_status(app, &error);
            }
        }
        "quit" => {
            app.state::<ShellState>()
                .quitting
                .store(true, Ordering::SeqCst);
            app.exit(0);
        }
        _ => {}
    }
}

#[cfg(not(target_os = "android"))]
fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let config_item = MenuItem::with_id(app, "open_config", "打开配置目录", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &config_item, &quit_item])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("Jev-Switch")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_tray_menu_event(app, event.id.as_ref()))
        .on_tray_icon_event(|tray, event| {
            // 左键单击托盘 → 显示主窗口（右键出菜单）
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    let icon = app
        .default_window_icon()
        .expect("the bundled Jev-Switch tray icon must be present");
    builder = builder.icon(icon.clone());
    builder.build(app)?;
    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::{
        fs,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        path::PathBuf,
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Arc,
        },
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn single_instance_identity_isolated_for_each_patch_version() {
        let identity = tauri_plugin_single_instance::semver_instance_key(env!("CARGO_PKG_VERSION"));
        for other_version in ["0.6.0", "0.6.1", "0.6.2", "0.6.4"] {
            let other_identity =
                tauri_plugin_single_instance::semver_instance_key(other_version);
            assert_ne!(identity, other_identity, "{other_version} must not capture current build");
        }
    }
    use tauri::{Manager, RunEvent};

    fn wait_for_visibility(
        window: &tauri::WebviewWindow,
        expected: bool,
        description: &str,
    ) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if window.is_visible().map_err(|error| error.to_string())? == expected {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
        Err(format!(
            "window did not become {description} before timeout"
        ))
    }

    fn find_header_end(bytes: &[u8]) -> Option<usize> {
        bytes.windows(4).position(|window| window == b"\r\n\r\n")
    }

    fn read_http_request(stream: &mut TcpStream) -> Result<(String, Vec<u8>), String> {
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            if let Some(header_end) = find_header_end(&bytes) {
                let headers = String::from_utf8_lossy(&bytes[..header_end]);
                let method = headers
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().next())
                    .ok_or_else(|| "HTTP request has no method".to_string())?
                    .to_string();
                if method == "OPTIONS" {
                    return Ok((method, Vec::new()));
                }
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                let body_start = header_end + 4;
                if bytes.len() >= body_start + content_length {
                    return Ok((
                        method,
                        bytes[body_start..body_start + content_length].to_vec(),
                    ));
                }
            }
            if bytes.len() > 64 * 1024 {
                return Err("HTTP report exceeded the test limit".into());
            }
            let read = stream.read(&mut chunk).map_err(|error| error.to_string())?;
            if read == 0 {
                return Err("HTTP report connection ended early".into());
            }
            bytes.extend_from_slice(&chunk[..read]);
        }
    }

    fn start_report_server(
        expected_reports: usize,
    ) -> (
        std::net::SocketAddr,
        mpsc::Receiver<String>,
        thread::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind isolated report receiver");
        listener
            .set_nonblocking(true)
            .expect("set report receiver nonblocking");
        let address = listener.local_addr().expect("read report receiver address");
        let (reports_tx, reports_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(45);
            let mut received = 0;
            while received < expected_reports && Instant::now() < deadline {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                        if let Ok((method, body)) = read_http_request(&mut stream) {
                            let response = if method == "OPTIONS" {
                                "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nAccess-Control-Allow-Headers: content-type\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            } else {
                                "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Type: text/plain\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
                            };
                            let _ = stream.write_all(response.as_bytes());
                            if method == "POST" {
                                let report = String::from_utf8_lossy(&body).into_owned();
                                if reports_tx.send(report).is_ok() {
                                    received += 1;
                                } else {
                                    break;
                                }
                            }
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(20));
                    }
                    Err(_) => break,
                }
            }
        });
        (address, reports_rx, server)
    }

    fn webview_probe_script(report_url: &str) -> String {
        r##"(async () => {
          const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
          const expectedRoute = location.hash.replace(/^#\/?/, '').split(/[?#]/)[0];
          const deadline = Date.now() + 5000;
          const routeLink = () => document.querySelector(`nav[aria-label="primary"] a[href="#/${expectedRoute}"]`);
          while ((!document.querySelector('main h1') || !routeLink()?.hasAttribute('aria-current')) && Date.now() < deadline) await pause(50);
          const toggle = document.querySelector('button[aria-pressed][aria-label]');
          const measure = () => {
            const root = document.documentElement;
            const main = document.querySelector('main');
            const active = routeLink();
            return {
              theme: root.dataset.theme || null,
              themePreference: root.dataset.themePreference || null,
              viewport: { width: innerWidth, height: innerHeight },
              document: { width: root.scrollWidth, clientWidth: root.clientWidth },
              header: (() => {
                const inner = document.querySelector('.app-shell__header-inner');
                const brand = document.querySelector('.app-shell__brand');
                const actions = document.querySelector('.app-shell__actions');
                const nav = [...document.querySelectorAll('.app-shell__nav a')].map(link => ({
                  text: link.textContent.trim(),
                  href: link.getAttribute('href')
                }));
                return inner && brand && actions ? {
                  leftInset: inner.getBoundingClientRect().left,
                  rightInset: innerWidth - inner.getBoundingClientRect().right,
                  brandLeft: brand.getBoundingClientRect().left,
                  actionsRight: innerWidth - actions.getBoundingClientRect().right,
                  nav
                } : null;
              })(),
              main: main ? {
                width: main.clientWidth,
                scrollWidth: main.scrollWidth,
                scrollHeight: main.scrollHeight,
                clientHeight: main.clientHeight
              } : null,
              heading: document.querySelector('main h1')?.textContent?.trim() || '',
              routingTabsPresent: Boolean(document.querySelector('.routing-tabs')),
              graphToolsPresent: Boolean(document.querySelector('.dag-tools')),
              activeNav: active?.getAttribute('aria-current') === 'page',
              themeTogglePresent: Boolean(toggle)
            };
          };
          localStorage.setItem('jev_theme', 'light');
          document.documentElement.dataset.themePreference = 'light';
          document.documentElement.dataset.theme = 'light';
          window.dispatchEvent(new Event('jev-theme-change'));
          await pause(100);
          const light = measure();
          if (toggle) toggle.click();
          await pause(100);
          const dark = measure();
          if (toggle) toggle.click();
          await pause(100);
          const system = measure();
          if (toggle) toggle.click();
          await pause(100);
          const restored = measure();
          await fetch('__REPORT_URL__', {
            method: 'POST',
            headers: { 'Content-Type': 'text/plain' },
            body: JSON.stringify({ route: expectedRoute, light, dark, system, restored })
          });
        })().catch(() => {});"##
            .replace("__REPORT_URL__", report_url)
    }

    #[test]
    #[ignore = "Creates an isolated native Tauri window; run explicitly on Windows"]
    fn tray_dispatch_shows_hides_and_quits_an_isolated_window() {
        let data_dir = std::env::var_os("JEV_TAURI_TRAY_TEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system clock should be after Unix epoch")
                    .as_millis();
                std::env::temp_dir()
                    .join(format!("jev-tauri-tray-{}-{timestamp}", std::process::id()))
            });
        assert!(
            !data_dir.exists(),
            "refusing to reuse an existing isolated test directory"
        );
        fs::create_dir_all(&data_dir).expect("create isolated tray test directory");
        let profile = data_dir.join("webview-profile");
        fs::create_dir_all(&profile).expect("create isolated WebView2 profile");

        let mut context = tauri::generate_context!();
        context.config_mut().app.windows[0].create = false;
        let window_profile = profile.clone();
        let app = tauri::Builder::default()
            .any_thread()
            .manage(ShellState {
                child: Mutex::new(None),
                quitting: AtomicBool::new(false),
                config_dir: data_dir,
                runtime_dir: None,
            })
            .setup(move |app| {
                tauri::WebviewWindowBuilder::from_config(
                    app.handle(),
                    &app.config().app.windows[0],
                )?
                .data_directory(window_profile.clone())
                .build()?;
                Ok(())
            })
            .on_window_event(|window, event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    let quitting = window.state::<ShellState>().quitting.load(Ordering::SeqCst);
                    handle_window_close_request(window, api, quitting);
                }
            })
            .build(context)
            .expect("build isolated tray lifecycle test shell");

        let worker_started = Arc::new(AtomicBool::new(false));
        let (result_tx, result_rx) = mpsc::channel::<Result<(), String>>();
        let (watchdog_tx, watchdog_rx) = mpsc::channel();
        let watchdog = app.handle().clone();
        let watchdog_thread = thread::spawn(move || {
            if watchdog_rx.recv_timeout(Duration::from_secs(20)).is_err() {
                watchdog
                    .state::<ShellState>()
                    .quitting
                    .store(true, Ordering::SeqCst);
                watchdog.exit(3);
            }
        });

        let exit_code = app.run_return(move |app, event| {
            if matches!(event, RunEvent::Ready) && !worker_started.swap(true, Ordering::SeqCst) {
                let handle = app.clone();
                let result_tx = result_tx.clone();
                thread::spawn(move || {
                    let result = (|| {
                        let window = handle
                            .get_webview_window("main")
                            .ok_or_else(|| "isolated test window is missing".to_string())?;

                        handle_tray_menu_event(&handle, "show");
                        wait_for_visibility(&window, true, "visible")?;

                        window.close().map_err(|error| error.to_string())?;
                        wait_for_visibility(&window, false, "hidden after close")?;
                        if handle.state::<ShellState>().quitting.load(Ordering::SeqCst) {
                            return Err("closing the window unexpectedly requested app exit".into());
                        }

                        handle_tray_menu_event(&handle, "show");
                        wait_for_visibility(&window, true, "visible after tray show")?;
                        handle_tray_menu_event(&handle, "quit");
                        if !handle.state::<ShellState>().quitting.load(Ordering::SeqCst) {
                            return Err("tray quit did not set the quitting flag".into());
                        }
                        Ok(())
                    })();

                    let succeeded = result.is_ok();
                    let _ = result_tx.send(result);
                    if !succeeded {
                        handle
                            .state::<ShellState>()
                            .quitting
                            .store(true, Ordering::SeqCst);
                        handle.exit(2);
                    }
                });
            }
        });

        let _ = watchdog_tx.send(());
        let _ = watchdog_thread.join();
        assert_eq!(exit_code, 0, "tray lifecycle test app should exit cleanly");
        result_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("tray lifecycle worker should report its result")
            .expect("tray lifecycle transitions should succeed");
    }

    #[test]
    #[ignore = "Loads the live local UI in an isolated hidden Tauri WebView; run explicitly on Windows"]
    fn native_webview_routes_the_five_pages_and_measures_responsive_layout() {
        use tauri::{LogicalSize, Manager, RunEvent};

        assert_eq!(
            crate::runtime_probe::probe("127.0.0.1:11435".parse().unwrap()),
            crate::runtime_probe::RuntimeProbe::Ready,
            "a compatible local daemon is required; this test never starts or stops it"
        );

        const ROUTES: [&str; 5] = [
            "dashboard",
            "providers",
            "endpoints",
            "routing",
            "playground",
        ];
        const SIZES: [(f64, f64); 3] = [(760.0, 480.0), (900.0, 560.0), (1280.0, 720.0)];
        let expected_reports = ROUTES.len() * SIZES.len();
        let (report_address, report_rx, report_server) = start_report_server(expected_reports);

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_millis();
        let data_dir = std::env::temp_dir().join(format!(
            "jev-tauri-webview-{}-{timestamp}",
            std::process::id()
        ));
        assert!(!data_dir.exists(), "refusing to reuse a test profile");
        fs::create_dir_all(&data_dir).expect("create isolated test directory");
        let profile = data_dir.join("webview-profile");
        fs::create_dir_all(&profile).expect("create isolated WebView2 profile");

        let mut context = tauri::generate_context!();
        context.config_mut().app.windows[0].create = false;
        let window_profile = profile.clone();
        let app = tauri::Builder::default()
            .any_thread()
            .manage(ShellState {
                child: Mutex::new(None),
                quitting: AtomicBool::new(false),
                config_dir: data_dir.clone(),
                runtime_dir: None,
            })
            .setup(move |app| {
                tauri::WebviewWindowBuilder::from_config(
                    app.handle(),
                    &app.config().app.windows[0],
                )?
                .data_directory(window_profile.clone())
                .build()?;
                Ok(())
            })
            .build(context)
            .expect("build isolated native WebView test shell");

        let worker_started = Arc::new(AtomicBool::new(false));
        let (watchdog_tx, watchdog_rx) = mpsc::channel();
        let watchdog = app.handle().clone();
        let watchdog_thread = thread::spawn(move || {
            if watchdog_rx.recv_timeout(Duration::from_secs(60)).is_err() {
                watchdog
                    .state::<ShellState>()
                    .quitting
                    .store(true, Ordering::SeqCst);
                watchdog.exit(3);
            }
        });
        let report_rx = Mutex::new(Some(report_rx));
        let worker_thread = Arc::new(Mutex::new(None));
        let worker_thread_on_ready = Arc::clone(&worker_thread);
        let test_handle = app.handle().clone();
        let exit_code = app.run_return(move |app, event| {
            if matches!(event, RunEvent::Ready) && !worker_started.swap(true, Ordering::SeqCst) {
                let handle = app.clone();
                let report_rx = report_rx
                    .lock()
                    .expect("report receiver lock should not be poisoned")
                    .take()
                    .expect("report receiver should be moved into the worker only once");
                let worker = thread::spawn(move || {
                    let result = (|| -> Result<(), String> {
                        let window = handle
                            .get_webview_window("main")
                            .ok_or_else(|| "isolated WebView window is missing".to_string())?;
                        window
                            .eval(&format!(
                                "location.replace('{}#/dashboard')",
                                sidecar::ui_entry_url()
                            ))
                            .map_err(|error| error.to_string())?;

                        let load_deadline = Instant::now() + Duration::from_secs(10);
                        loop {
                            let loaded = window.url().is_ok_and(|url| {
                                url.scheme() == "http"
                                    && url.host_str() == Some("127.0.0.1")
                                    && url.port() == Some(11435)
                                    && url.path() == "/"
                            });
                            if loaded {
                                break;
                            }
                            if Instant::now() >= load_deadline {
                                return Err("native WebView did not load the local UI".into());
                            }
                            thread::sleep(Duration::from_millis(50));
                        }

                        let report_url = format!("http://{report_address}/");
                        for (width, height) in SIZES {
                            window
                                .set_size(LogicalSize::new(width, height))
                                .map_err(|error| error.to_string())?;
                            for route in ROUTES {
                                window
                                    .eval(&format!("location.hash = '#/{route}';"))
                                    .map_err(|error| error.to_string())?;
                                thread::sleep(Duration::from_millis(250));
                                let script = webview_probe_script(&report_url);
                                window
                                    .eval(&script)
                                    .map_err(|error| error.to_string())?;
                                let body = report_rx
                                    .recv_timeout(Duration::from_secs(8))
                                    .map_err(|error| format!("waiting for DOM snapshot: {error}"))?;
                                let snapshot: serde_json::Value = serde_json::from_str(&body)
                                    .map_err(|error| format!("invalid DOM snapshot: {error}; {body}"))?;

                                assert_eq!(snapshot["route"], route, "snapshot route mismatch");
                                for theme in ["light", "dark", "system", "restored"] {
                                    let measured = &snapshot[theme];
                                    assert_eq!(measured["themeTogglePresent"], true);
                                    assert!(!measured["heading"].as_str().unwrap_or_default().is_empty());
                                    assert_eq!(measured["activeNav"], true);
                                    let header = &measured["header"];
                                    assert!(header["leftInset"].as_f64().unwrap_or(f64::MAX) <= 1.0);
                                    assert!(header["rightInset"].as_f64().unwrap_or(f64::MAX) <= 1.0);
                                    assert!(header["brandLeft"].as_f64().unwrap_or(f64::MAX) <= 20.0);
                                    assert!(header["actionsRight"].as_f64().unwrap_or(f64::MAX) <= 20.0);
                                    let navigation: Vec<&str> = header["nav"]
                                        .as_array()
                                        .unwrap()
                                        .iter()
                                        .filter_map(|item| item["href"].as_str())
                                        .collect();
                                    assert_eq!(
                                        navigation.iter().position(|href| *href == "#/endpoints").unwrap() + 1,
                                        navigation.iter().position(|href| *href == "#/routing").unwrap()
                                    );
                                    assert_eq!(measured["routingTabsPresent"], false);
                                    assert_eq!(measured["graphToolsPresent"], route == "routing");
                                    let client_width = measured["document"]["clientWidth"].as_u64().unwrap_or(0);
                                    let scroll_width = measured["document"]["width"].as_u64().unwrap_or(u64::MAX);
                                    assert!(
                                        scroll_width <= client_width + 1,
                                        "horizontal document overflow on {route} {width}x{height} {theme}: {body}"
                                    );
                                }
                                assert_eq!(snapshot["light"]["themePreference"], "light");
                                assert_eq!(snapshot["dark"]["themePreference"], "dark");
                                assert_eq!(snapshot["system"]["themePreference"], "system");
                                assert!(matches!(
                                    snapshot["system"]["theme"].as_str(),
                                    Some("light" | "dark")
                                ));
                                assert_eq!(snapshot["restored"]["themePreference"], "light");
                                assert_eq!(snapshot["light"]["theme"], "light");
                                assert_eq!(snapshot["dark"]["theme"], "dark");
                                assert_eq!(snapshot["restored"]["theme"], "light");
                                println!("native-webview: {width}x{height} #{route} light/dark/restored pass");
                            }
                        }

                        handle
                            .state::<ShellState>()
                            .quitting
                            .store(true, Ordering::SeqCst);
                        handle.exit(0);
                        Ok(())
                    })();

                    if let Err(error) = result {
                        eprintln!("native-webview acceptance failed: {error}");
                        handle
                            .state::<ShellState>()
                            .quitting
                            .store(true, Ordering::SeqCst);
                        handle.exit(2);
                    }
                });
                *worker_thread_on_ready
                    .lock()
                    .expect("worker thread lock should not be poisoned") = Some(worker);
            }
        });

        let _ = watchdog_tx.send(());
        let _ = watchdog_thread.join();
        if let Some(worker) = worker_thread
            .lock()
            .expect("worker thread lock should not be poisoned")
            .take()
        {
            worker
                .join()
                .expect("native WebView worker should finish before cleanup");
        }
        assert_eq!(
            exit_code, 0,
            "native WebView acceptance shell should exit cleanly"
        );
        report_server
            .join()
            .expect("report server should receive all native UI snapshots");
        drop(test_handle);

        let temp_root = std::env::temp_dir()
            .canonicalize()
            .expect("canonicalize temporary root");
        let test_dir = data_dir.canonicalize().expect("canonicalize test profile");
        assert!(
            test_dir.starts_with(&temp_root),
            "test data must remain in temp"
        );
        let cleanup_deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match fs::remove_dir_all(&test_dir) {
                Ok(()) => break,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Err(_) if Instant::now() < cleanup_deadline => {
                    thread::sleep(Duration::from_millis(100));
                }
                Err(error) if error.raw_os_error() == Some(32) => {
                    eprintln!(
                        "WebView2 still owns the isolated profile; leaving it under temp for later cleanup: {}",
                        test_dir.display()
                    );
                    break;
                }
                Err(error) => panic!("remove successful isolated WebView profile: {error}"),
            }
        }
    }
}
