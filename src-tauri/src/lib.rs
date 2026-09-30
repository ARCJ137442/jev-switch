// Tauri Android links a library target and calls this mobile entry point.
// The implementation remains in the existing binary module so desktop and
// mobile keep one lifecycle composition root while Android gains the required
// library artifact.
#[cfg(target_os = "android")]
#[allow(unused_attributes)]
#[path = "main.rs"]
mod app_main;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[cfg(target_os = "android")]
pub fn run() {
    app_main::main();
}

#[cfg(not(target_os = "android"))]
pub fn run() {}
