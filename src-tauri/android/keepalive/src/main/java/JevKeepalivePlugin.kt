package io.github.arcj137442.jevswitch.keepalive

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class KeepaliveStateArgs {
    var enabled: Boolean = true
    var gatewayRunning: Boolean = false
}

@TauriPlugin(
    permissions = [
        Permission(strings = [Manifest.permission.POST_NOTIFICATIONS], alias = "notifications"),
    ],
)
class JevKeepalivePlugin(private val activity: Activity) : Plugin(activity) {
    private val appContext: Context
        get() = activity.applicationContext

    @Command
    fun setState(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(KeepaliveStateArgs::class.java)
            RuntimeKeepaliveState.setState(appContext, args.enabled, args.gatewayRunning)
            invoke.resolve(
                JSObject()
                    .put("enabled", args.enabled)
                    .put("gatewayRunning", args.gatewayRunning),
            )
        } catch (error: Exception) {
            invoke.reject(error.message, error)
        }
    }

    @Command
    fun notificationPermissionState(invoke: Invoke) {
        invoke.resolve(JSObject().put("state", permissionState()))
    }

    @Command
    fun takePendingGatewayToggle(invoke: Invoke) {
        invoke.resolve(JSObject().put("pending", RuntimeKeepaliveState.takePendingGatewayToggle(appContext)))
    }

    @Command
    fun requestNotificationPermission(invoke: Invoke) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU || permissionState() == "granted") {
            notificationPermissionState(invoke)
            return
        }
        requestPermissionForAlias("notifications", invoke, "onNotificationPermissionResult")
    }

    @PermissionCallback
    fun onNotificationPermissionResult(invoke: Invoke) {
        notificationPermissionState(invoke)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        if (intent.getBooleanExtra(RuntimeKeepaliveState.EXTRA_TOGGLE_GATEWAY, false)) {
            RuntimeKeepaliveState.markPendingGatewayToggle(appContext)
            trigger("jev-switch-gateway-toggle", JSObject())
        }
    }

    override fun onStop() {
        RuntimeKeepaliveState.reconcile(appContext)
    }

    private fun permissionState(): String {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return "granted"
        return if (activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED) {
            "granted"
        } else {
            "denied"
        }
    }
}
