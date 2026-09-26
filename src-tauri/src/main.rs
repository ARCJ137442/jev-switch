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

mod runtime_probe;
mod sidecar;
mod window_layout;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, RunEvent, WindowEvent};

/// 壳层托管状态：sidecar 子进程 + 退出标记 + 配置目录。
pub struct ShellState {
    pub child: Mutex<Option<std::process::Child>>,
    pub quitting: AtomicBool,
    pub config_dir: std::path::PathBuf,
}

fn main() {
    tauri::Builder::default()
        // 单实例锁必须第一个注册：二次双击 → 聚焦已有窗口
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main_window(app);
        }))
        .setup(|app| {
            let app_handle = app.handle().clone();

            // 首次显示前调整，避免大窗口闪现；服务尚未就绪也要显示等待/错误页。
            if let Some(window) = app.get_webview_window("main") {
                if let Err(error) = window_layout::fit_initial_window(&window) {
                    eprintln!("initial window layout: {error}");
                }
                window.show()?;
            }

            // 1. 配置目录 + 首启播种（已有不覆盖）
            let config_dir = sidecar::config_dir();
            let config_path = match sidecar::seed_config(&config_dir) {
                Ok(p) => p,
                Err(e) => {
                    sidecar::set_status(
                        &app_handle,
                        &format!("配置播种失败：{e}（{}）", config_dir.display()),
                    );
                    config_dir.join("providers.toml")
                }
            };

            app.manage(ShellState {
                child: Mutex::new(None),
                quitting: AtomicBool::new(false),
                config_dir: config_dir.clone(),
            });

            // 2. 托盘
            build_tray(&app_handle)?;

            // 3. sidecar 拉起 + 就绪轮询（服务身份及版本通过 → 切入控制台 UI）
            if let Err(e) = sidecar::start(app_handle.clone(), &config_path) {
                sidecar::set_status(&app_handle, &e);
                // 可等待用户手动启动兼容内核；身份不符时保持错误提示。
                sidecar::wait_for_daemon(app_handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // 关窗 → 最小化到托盘（退出标记置位时才真退）
            if let WindowEvent::CloseRequested { api, .. } = event {
                let quitting = window.state::<ShellState>().quitting.load(Ordering::SeqCst);
                handle_window_close_request(window, api, quitting);
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            // 退出兜底：无论从托盘/关窗路径来，子进程统一在此收尸
            RunEvent::Exit => {
                let child = app.state::<ShellState>().child.lock().unwrap().take();
                if let Some(mut child) = child {
                    sidecar::shutdown(&mut child);
                }
            }
            _ => {}
        });
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

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

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
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
              viewport: { width: innerWidth, height: innerHeight },
              document: { width: root.scrollWidth, clientWidth: root.clientWidth },
              main: main ? {
                width: main.clientWidth,
                scrollWidth: main.scrollWidth,
                scrollHeight: main.scrollHeight,
                clientHeight: main.clientHeight
              } : null,
              heading: document.querySelector('main h1')?.textContent?.trim() || '',
              activeNav: active?.getAttribute('aria-current') === 'page',
              themeTogglePresent: Boolean(toggle)
            };
          };
          document.documentElement.dataset.theme = 'light';
          window.dispatchEvent(new Event('jev-theme-change'));
          await pause(100);
          const light = measure();
          if (toggle) toggle.click();
          await pause(100);
          const dark = measure();
          if (toggle) toggle.click();
          await pause(100);
          const restored = measure();
          await fetch('__REPORT_URL__', {
            method: 'POST',
            headers: { 'Content-Type': 'text/plain' },
            body: JSON.stringify({ route: expectedRoute, light, dark, restored })
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
    fn native_webview_routes_the_four_pages_and_measures_responsive_layout() {
        use tauri::{LogicalSize, Manager, RunEvent};

        assert_eq!(
            crate::runtime_probe::probe("127.0.0.1:11435".parse().unwrap()),
            crate::runtime_probe::RuntimeProbe::Ready,
            "a compatible local daemon is required; this test never starts or stops it"
        );

        const ROUTES: [&str; 4] = ["dashboard", "providers", "routing", "playground"];
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
                                sidecar::UI_ORIGIN
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
                                for theme in ["light", "dark", "restored"] {
                                    let measured = &snapshot[theme];
                                    assert_eq!(measured["themeTogglePresent"], true);
                                    assert!(!measured["heading"].as_str().unwrap_or_default().is_empty());
                                    assert_eq!(measured["activeNav"], true);
                                    let client_width = measured["document"]["clientWidth"].as_u64().unwrap_or(0);
                                    let scroll_width = measured["document"]["width"].as_u64().unwrap_or(u64::MAX);
                                    assert!(
                                        scroll_width <= client_width + 1,
                                        "horizontal document overflow on {route} {width}x{height} {theme}: {body}"
                                    );
                                }
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
