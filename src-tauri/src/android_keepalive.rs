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

pub fn request_notification_permission<R: Runtime>(app: &AppHandle<R>) -> Result<String, String> {
    let plugin = app.state::<NativeKeepalive>();
    let result: Value = plugin
        .0
        .run_mobile_plugin("requestNotificationPermission", serde_json::json!({}))
        .map_err(|error| format!("request Android notification permission failed: {error}"))?;
    Ok(result
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string())
}

pub fn take_pending_gateway_toggle<R: Runtime>(app: &AppHandle<R>) -> Result<bool, String> {
    let plugin = app.state::<NativeKeepalive>();
    let result: Value = plugin
        .0
        .run_mobile_plugin("takePendingGatewayToggle", serde_json::json!({}))
        .map_err(|error| format!("read Android gateway toggle request failed: {error}"))?;
    Ok(result
        .get("pending")
        .and_then(Value::as_bool)
        .unwrap_or(false))
}
