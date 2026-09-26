//! 首窗使用逻辑像素，屏幕工作区和边框使用物理像素；只在首次创建时调整。
//! 用户之后的缩放、最大化与拖动由系统管理，网页按实际 CSS 视口重排。

use tauri::{LogicalSize, PhysicalPosition, WebviewWindow};

const INITIAL_MAX_WIDTH: f64 = 900.0;
const INITIAL_MAX_HEIGHT: f64 = 560.0;

#[derive(Debug)]
struct InitialSize {
    width: f64,
    height: f64,
    min_width: f64,
    min_height: f64,
}

fn initial_size(
    work_width: u32,
    work_height: u32,
    scale: f64,
    frame_width: u32,
    frame_height: u32,
) -> Option<InitialSize> {
    if !scale.is_finite() || scale <= 0.0 || work_width == 0 || work_height == 0 {
        return None;
    }
    // 四周保留空间，同时计入标题栏/边框。高 DPI 或小工作区下，最小尺寸也必须让步。
    let width_budget = ((f64::from(work_width) * 0.9 - f64::from(frame_width)) / scale).max(1.0);
    let height_budget = ((f64::from(work_height) * 0.9 - f64::from(frame_height)) / scale).max(1.0);
    Some(InitialSize {
        width: INITIAL_MAX_WIDTH.min(width_budget).floor(),
        height: INITIAL_MAX_HEIGHT.min(height_budget).floor(),
        min_width: 760.0_f64.min(width_budget).floor(),
        min_height: 480.0_f64.min(height_budget).floor(),
    })
}

pub fn fit_initial_window(window: &WebviewWindow) -> tauri::Result<()> {
    let monitor = match window.current_monitor()? {
        Some(monitor) => Some(monitor),
        None => window.primary_monitor()?,
    };
    let Some(monitor) = monitor else {
        return Ok(());
    };
    let area = monitor.work_area();
    let scale = monitor.scale_factor();
    let inner = window.inner_size()?;
    let outer = window.outer_size()?;
    let frame_width = outer.width.saturating_sub(inner.width);
    let frame_height = outer.height.saturating_sub(inner.height);
    let Some(size) = initial_size(
        area.size.width,
        area.size.height,
        scale,
        frame_width,
        frame_height,
    ) else {
        return Ok(());
    };
    window.set_min_size(Some(LogicalSize::new(size.min_width, size.min_height)))?;
    window.set_size(LogicalSize::new(size.width, size.height))?;
    // 在工作区居中，保留非主屏的负坐标；不用整块显示器尺寸覆盖任务栏区域。
    let outer_width = size.width * scale + f64::from(frame_width);
    let outer_height = size.height * scale + f64::from(frame_height);
    window.set_position(PhysicalPosition::new(
        area.position.x + ((f64::from(area.size.width) - outer_width) / 2.0).round() as i32,
        area.position.y + ((f64::from(area.size.height) - outer_height) / 2.0).round() as i32,
    ))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_size_fits_work_area_including_dpi_and_decorations() {
        // 横屏、竖屏、超宽、高缩放，以及小于常规最小窗口的工作区。
        for (width, height, scale) in [
            (1920, 1040, 1.0),
            (1366, 728, 1.25),
            (1366, 728, 1.5),
            (1920, 1040, 2.0),
            (1080, 1880, 1.5),
            (3440, 1400, 1.0),
            (800, 560, 2.0),
        ] {
            let frame_width = (16.0_f64 * scale).round() as u32;
            let frame_height = (39.0_f64 * scale).round() as u32;
            let size = initial_size(width, height, scale, frame_width, frame_height).unwrap();
            assert!(size.width * scale + f64::from(frame_width) <= f64::from(width) * 0.9);
            assert!(size.height * scale + f64::from(frame_height) <= f64::from(height) * 0.9);
            assert!(size.min_width <= size.width && size.min_height <= size.height);
            assert!(size.width <= INITIAL_MAX_WIDTH && size.height <= INITIAL_MAX_HEIGHT);
        }
    }

    #[test]
    fn ordinary_work_area_uses_the_compact_default_size() {
        let size = initial_size(1920, 1040, 1.0, 16, 39).unwrap();
        assert_eq!(size.width, INITIAL_MAX_WIDTH);
        assert_eq!(size.height, INITIAL_MAX_HEIGHT);
        assert_eq!(size.min_width, 760.0);
        assert_eq!(size.min_height, 480.0);
    }

    #[test]
    fn invalid_monitor_data_does_not_produce_invalid_window_geometry() {
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(initial_size(1920, 1040, scale, 16, 39).is_none());
        }
        assert!(initial_size(0, 1040, 1.0, 16, 39).is_none());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Creates a hidden native WebView; run explicitly in an interactive Windows session"]
    fn native_initial_window_stays_inside_current_work_area() {
        use std::{
            fs,
            path::PathBuf,
            time::{SystemTime, UNIX_EPOCH},
        };
        use tauri::{Manager, RunEvent};

        // Keep the native geometry test isolated from the user's normal WebView2 profile.
        let data_dir = std::env::var_os("JEV_WINDOW_TEST_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system clock should be after Unix epoch")
                    .as_millis();
                std::env::temp_dir().join(format!(
                    "jev-window-initial-{}-{timestamp}",
                    std::process::id()
                ))
            });
        fs::create_dir_all(&data_dir).expect("create isolated test data directory");
        let profile = data_dir.join("webview-profile");
        fs::create_dir_all(&profile).expect("create isolated WebView profile");
        let mut context = tauri::generate_context!();
        context.config_mut().app.windows[0].create = false;
        let window_profile = profile.clone();

        // 沿用真实窗口配置及生产尺寸函数，不注册单实例、托盘或启动 daemon。
        let app = tauri::Builder::default()
            .any_thread()
            .setup(move |app| {
                let window = tauri::WebviewWindowBuilder::from_config(
                    app.handle(),
                    &app.config().app.windows[0],
                )?
                .data_directory(window_profile.clone())
                .build()?;
                fit_initial_window(&window)?;
                Ok(())
            })
            .build(context)
            .unwrap();
        let exit_code = app.run_return(|app, event| {
            if matches!(event, RunEvent::Ready) {
                let window = app.get_webview_window("main").unwrap();
                let monitor = window.current_monitor().unwrap().unwrap();
                let area = monitor.work_area();
                let position = window.outer_position().unwrap();
                let outer = window.outer_size().unwrap();
                let inner = window.inner_size().unwrap();
                let scale = window.scale_factor().unwrap();
                eprintln!("native-window: inner={}x{} physical, scale={}, logical={}x{}, outer={}x{} at {},{}; work={}x{} at {},{}",
                    inner.width, inner.height, scale, f64::from(inner.width) / scale,
                    f64::from(inner.height) / scale, outer.width, outer.height,
                    position.x, position.y, area.size.width, area.size.height,
                    area.position.x, area.position.y);
                assert!(position.x >= area.position.x && position.y >= area.position.y);
                assert!(i64::from(position.x) + i64::from(outer.width) <= i64::from(area.position.x) + i64::from(area.size.width));
                assert!(i64::from(position.y) + i64::from(outer.height) <= i64::from(area.position.y) + i64::from(area.size.height));
                assert!(f64::from(inner.width) / scale <= INITIAL_MAX_WIDTH + 1.0);
                assert!(f64::from(inner.height) / scale <= INITIAL_MAX_HEIGHT + 1.0);
                app.exit(0);
            }
        });
        assert_eq!(exit_code, 0);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Moves an isolated hidden native WebView across attached monitors; run explicitly on Windows"]
    fn native_initial_window_fits_each_attached_monitor() {
        use std::{
            fs,
            panic::{catch_unwind, AssertUnwindSafe},
            sync::{
                atomic::{AtomicBool, Ordering},
                mpsc, Arc,
            },
            thread,
            time::{Duration, Instant, SystemTime, UNIX_EPOCH},
        };
        use tauri::{Manager, PhysicalPosition, RunEvent};

        // Keep this WebView profile out of the user's normal application data.
        let data_dir = std::env::var_os("JEV_WINDOW_TEST_DATA_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system clock should be after Unix epoch")
                    .as_millis();
                std::env::temp_dir().join(format!(
                    "jev-window-multimonitor-{}-{timestamp}",
                    std::process::id()
                ))
            });
        fs::create_dir_all(&data_dir).expect("create isolated test data directory");
        let profile = data_dir.join("webview-profile");
        fs::create_dir_all(&profile).expect("create isolated WebView profile");

        // Load the real app config, but create its configured hidden window manually so
        // the test can inject an absolute, isolated WebView2 data directory.
        let mut context = tauri::generate_context!();
        context.config_mut().app.windows[0].create = false;
        let window_profile = profile.clone();
        let app = tauri::Builder::default()
            .any_thread()
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
            .unwrap();
        let started = Arc::new(AtomicBool::new(false));
        let worker_finished = Arc::new(AtomicBool::new(false));
        let (result_tx, result_rx) = mpsc::channel::<Result<(), String>>();
        let started_on_loop = Arc::clone(&started);
        let finished_on_loop = Arc::clone(&worker_finished);
        let result_tx_on_loop = result_tx.clone();
        let exit_code = app.run_return(move |app, event| {
            if matches!(event, RunEvent::Ready)
                && !started_on_loop.swap(true, Ordering::SeqCst)
            {
                let window = app.get_webview_window("main").unwrap();
                let app_handle = app.clone();
                let result_tx = result_tx_on_loop.clone();
                let worker_finished = Arc::clone(&finished_on_loop);
                let watchdog_handle = app_handle.clone();
                let watchdog_finished = Arc::clone(&worker_finished);
                thread::spawn(move || {
                    let deadline = Instant::now() + Duration::from_secs(45);
                    while Instant::now() < deadline {
                        if watchdog_finished.load(Ordering::SeqCst) {
                            return;
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
                    if !watchdog_finished.load(Ordering::SeqCst) {
                        eprintln!("multimonitor-test: watchdog timed out after 45 seconds");
                        watchdog_handle.exit(2);
                    }
                });
                thread::spawn(move || {
                    let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
                        let monitors = window.available_monitors().map_err(|e| e.to_string())?;
                        if monitors.is_empty() {
                            return Err("Tauri reported no attached monitors".into());
                        }
                        eprintln!("multimonitor-test: attached monitors={}", monitors.len());

                        for (index, expected) in monitors.iter().enumerate() {
                            let area = expected.work_area();
                            // Place the hidden window well inside the target work area. Moving
                            // and polling happen off the event loop so Windows can deliver DPI
                            // and monitor-change events while this test waits.
                            window
                                .set_position(PhysicalPosition::new(
                                    area.position.x + 32,
                                    area.position.y + 32,
                                ))
                                .map_err(|e| format!("monitor {index}: move failed: {e}"))?;

                            let settle_deadline = Instant::now() + Duration::from_secs(4);
                            let mut stable_reads = 0;
                            while Instant::now() < settle_deadline {
                                let current = window
                                    .current_monitor()
                                    .map_err(|e| format!("monitor {index}: current_monitor: {e}"))?;
                                let scale = window
                                    .scale_factor()
                                    .map_err(|e| format!("monitor {index}: scale_factor: {e}"))?;
                                let matches = current.as_ref().is_some_and(|current| {
                                    let current_area = current.work_area();
                                    current_area.position == area.position
                                        && current_area.size == area.size
                                        && (current.scale_factor() - expected.scale_factor()).abs() < 0.01
                                        && (scale - expected.scale_factor()).abs() < 0.01
                                });
                                stable_reads = if matches { stable_reads + 1 } else { 0 };
                                if stable_reads >= 3 {
                                    break;
                                }
                                thread::sleep(Duration::from_millis(80));
                            }
                            if stable_reads < 3 {
                                return Err(format!(
                                    "monitor {index}: target monitor/scale did not stabilize within 4s"
                                ));
                            }

                            fit_initial_window(&window)
                                .map_err(|e| format!("monitor {index}: fit failed: {e}"))?;
                            let mut previous = None;
                            let mut stable_geometry = 0;
                            let geometry_deadline = Instant::now() + Duration::from_secs(4);
                            let (position, outer, inner, scale) = loop {
                                if Instant::now() >= geometry_deadline {
                                    return Err(format!(
                                        "monitor {index}: fitted geometry did not stabilize within 4s"
                                    ));
                                }
                                let current = window
                                    .current_monitor()
                                    .map_err(|e| format!("monitor {index}: current_monitor: {e}"))?;
                                let Some(current) = current else {
                                    thread::sleep(Duration::from_millis(60));
                                    continue;
                                };
                                let current_area = current.work_area();
                                let scale = window
                                    .scale_factor()
                                    .map_err(|e| format!("monitor {index}: scale_factor: {e}"))?;
                                if current_area.position != area.position
                                    || current_area.size != area.size
                                    || (scale - expected.scale_factor()).abs() >= 0.01
                                    || (current.scale_factor() - expected.scale_factor()).abs() >= 0.01
                                {
                                    stable_geometry = 0;
                                    previous = None;
                                    thread::sleep(Duration::from_millis(60));
                                    continue;
                                }
                                let position = window
                                    .outer_position()
                                    .map_err(|e| format!("monitor {index}: outer_position: {e}"))?;
                                let outer = window
                                    .outer_size()
                                    .map_err(|e| format!("monitor {index}: outer_size: {e}"))?;
                                let inner = window
                                    .inner_size()
                                    .map_err(|e| format!("monitor {index}: inner_size: {e}"))?;
                                let geometry = (position.x, position.y, outer.width, outer.height, inner.width, inner.height);
                                stable_geometry = if previous == Some(geometry) { stable_geometry + 1 } else { 0 };
                                previous = Some(geometry);
                                if stable_geometry >= 2 {
                                    break (position, outer, inner, scale);
                                }
                                thread::sleep(Duration::from_millis(60));
                            };

                            let work_right = i64::from(area.position.x) + i64::from(area.size.width);
                            let work_bottom = i64::from(area.position.y) + i64::from(area.size.height);
                            let outer_right = i64::from(position.x) + i64::from(outer.width);
                            let outer_bottom = i64::from(position.y) + i64::from(outer.height);
                            let logical_width = f64::from(inner.width) / scale;
                            let logical_height = f64::from(inner.height) / scale;
                            let centered_x = area.position.x
                                + ((f64::from(area.size.width) - f64::from(outer.width)) / 2.0).round() as i32;
                            let centered_y = area.position.y
                                + ((f64::from(area.size.height) - f64::from(outer.height)) / 2.0).round() as i32;
                            eprintln!(
                                "monitor[{index}]: name={:?} scale={scale:.3} work={}x{} at {},{} inner={}x{} physical / {:.2}x{:.2} logical outer={}x{} at {},{}",
                                expected.name(), area.size.width, area.size.height,
                                area.position.x, area.position.y, inner.width, inner.height,
                                logical_width, logical_height, outer.width, outer.height,
                                position.x, position.y,
                            );
                            if position.x < area.position.x || position.y < area.position.y
                                || outer_right > work_right || outer_bottom > work_bottom
                            {
                                return Err(format!("monitor {index}: outer window escaped work area"));
                            }
                            if f64::from(outer.width) > f64::from(area.size.width) * 0.9 + 2.0
                                || f64::from(outer.height) > f64::from(area.size.height) * 0.9 + 2.0
                            {
                                return Err(format!("monitor {index}: outer window exceeded 90% work-area budget"));
                            }
                            if logical_width > INITIAL_MAX_WIDTH + 1.0
                                || logical_height > INITIAL_MAX_HEIGHT + 1.0
                            {
                                return Err(format!("monitor {index}: logical content exceeded default size cap"));
                            }
                            if (position.x - centered_x).abs() > 2 || (position.y - centered_y).abs() > 2 {
                                return Err(format!("monitor {index}: outer window was not centered in work area"));
                            }
                            if (area.position.x < 0 && position.x >= 0)
                                || (area.position.y < 0 && position.y >= 0)
                            {
                                return Err(format!("monitor {index}: negative monitor coordinates were lost"));
                            }
                        }
                        Ok(())
                    }))
                    .unwrap_or_else(|_| Err("multi-monitor worker panicked".into()));
                    let exit = if result.is_ok() { 0 } else { 1 };
                    if let Err(error) = &result {
                        eprintln!("multimonitor-test: FAILED: {error}");
                    }
                    let _ = result_tx.send(result);
                    worker_finished.store(true, Ordering::SeqCst);
                    app_handle.exit(exit);
                });
            }
        });
        drop(result_tx);
        let result = result_rx.recv_timeout(Duration::from_secs(2));
        assert_eq!(exit_code, 0, "hidden test app should exit cleanly");
        result
            .expect("multi-monitor worker must report before the watchdog exits")
            .expect("all attached monitors should satisfy initial-window geometry");
    }
}
