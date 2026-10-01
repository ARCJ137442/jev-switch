use serde_json::Value;
use tauri::{
    plugin::{Builder, PluginHandle, TauriPlugin},
    AppHandle, Manager, Runtime, Wry,
};

const PLUGIN_IDENTIFIER: &str = "io.github.arcj137442.jevswitch.keepalive";

struct NativeKeepalive(PluginHandle<Wry>);

pub fn init() -> TauriPlugin<Wry> {
    Builder::new("jev-android-keepalive")
        .setup(|app, api| {
            let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "JevKeepalivePlugin")?;
            app.manage(NativeKeepalive(handle));
            Ok(())
        })
        .build()
}

pub fn set_state<R: Runtime>(
    app: &AppHandle<R>,
    enabled: bool,
    gateway_running: bool,
) -> Result<(), String> {
    let plugin = app.state::<NativeKeepalive>();
    let _: Value = plugin
        .0
        .run_mobile_plugin(
            "setState",
            serde_json::json!({
                "enabled": enabled,
                "gatewayRunning": gateway_running,
            }),
        )
        .map_err(|error| format!("update Android keepalive service failed: {error}"))?;
    Ok(())
}

pub fn notification_permission_state<R: Runtime>(app: &AppHandle<R>) -> Result<String, String> {
    let plugin = app.state::<NativeKeepalive>();
    let result: Value = plugin
        .0
        .run_mobile_plugin("notificationPermissionState", serde_json::json!({}))
        .map_err(|error| format!("read Android notification permission failed: {error}"))?;
    Ok(result
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string())
}
