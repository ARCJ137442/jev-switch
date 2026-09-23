//! Jev-Switch 桌面壳（Tauri 2 · Windows 优先 —— docs/12 §二.8 裁决、§四-线3 / 任务 #44）。
//!
//! 形态对标 LM Studio「双击开箱」：
//! - 主窗口 1440×900（min 1024×640），先载壳内等待页，`/health` 200 后切入
//!   `http://127.0.0.1:11435`（sidecar daemon ServeDir 同源托管 ui/dist，local 态免 token）
//! - sidecar = 打包的 `jev-switch.exe`（externalBin）；`JEV_SWITCH_CONFIG` 指向
//!   `%APPDATA%\jev-switch\providers.toml`（首启播种、已有不覆盖）；`JEV_SWITCH_MODE` 不设 = local
//! - 单实例（tauri-plugin-single-instance）+ 系统托盘（显示 / 打开配置目录 / 退出）
//! - 关窗 = 隐藏到托盘；退出时优雅终止 sidecar

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod sidecar;

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

            // 3. sidecar 拉起 + 就绪轮询（200 → 切入控制台 UI）
            if let Err(e) = sidecar::start(app_handle.clone(), &config_path) {
                sidecar::set_status(&app_handle, &e);
                // 拉不起来也继续轮询：也许用户本机已有 daemon 在跑
                std::thread::spawn(move || loop {
                    if sidecar::health_ok() {
                        if let Some(w) = app_handle.get_webview_window("main") {
                            let _ = w.eval(&format!("location.replace('{}/')", sidecar::UI_ORIGIN));
                        }
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(500));
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // 关窗 → 最小化到托盘（退出标记置位时才真退）
            if let WindowEvent::CloseRequested { api, .. } = event {
                let quitting = window
                    .state::<ShellState>()
                    .quitting
                    .load(Ordering::SeqCst);
                if !quitting {
                    api.prevent_close();
                    let _ = window.hide();
                }
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

fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let config_item =
        MenuItem::with_id(app, "open_config", "打开配置目录", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &config_item, &quit_item])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("Jev-Switch")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "open_config" => {
                let dir = app.state::<ShellState>().config_dir.clone();
                if let Err(e) = sidecar::open_config_dir(&dir) {
                    sidecar::set_status(app, &e);
                }
            }
            "quit" => {
                app.state::<ShellState>().quitting.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        })
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
