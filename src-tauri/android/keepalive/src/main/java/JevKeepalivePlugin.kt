package io.github.arcj137442.jevswitch.keepalive

import android.Manifest
import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
class KeepaliveStateArgs {
    var enabled: Boolean = true
    var gatewayRunning: Boolean = false
}

@InvokeArg
class DebugLogExportArgs {
    var path: String = ""
}

@InvokeArg
class PrivateFileExportArgs {
    var path: String = ""
    var name: String = ""
}

@InvokeArg
class ClipboardTextArgs {
    var text: String = ""
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
            // Start the foreground service with the explicit gateway state change.
            // Repeating this from Activity.onStop can violate Android 12+ background-start rules.
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
    fun foregroundServiceStatus(invoke: Invoke) {
        invoke.resolve(JSObject().put("active", RuntimeKeepaliveState.foregroundServiceActive))
    }

    @Command
    fun takePendingGatewayToggle(invoke: Invoke) {
        invoke.resolve(JSObject().put("pending", RuntimeKeepaliveState.takePendingGatewayToggle(appContext)))
    }

    @Command
    fun exportDebugLog(invoke: Invoke) {
        try {
            val source = File(invoke.parseArgs(DebugLogExportArgs::class.java).path)
            if (!source.isFile) throw IllegalStateException("Android debug log is not available")
            val name = "jev-switch-android-debug-${System.currentTimeMillis()}.log"
            exportPrivateFile(source, name, "text/plain", "shared_debug_logs", invoke)
        } catch (error: Exception) {
            invoke.reject(error.message, error)
        }
    }

    @Command
    fun exportJsonFile(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(PrivateFileExportArgs::class.java)
            if (!Regex("[A-Za-z0-9._-]{1,128}\\.json").matches(args.name)) {
                throw IllegalArgumentException("Invalid JSON export filename")
            }
            val source = File(args.path)
            if (!source.isFile) throw IllegalStateException("Android JSON export is not available")
            exportPrivateFile(source, args.name, "application/json", "shared_exports", invoke)
        } catch (error: Exception) {
            invoke.reject(error.message, error)
        }
    }

    @Command
    fun writeClipboard(invoke: Invoke) {
        try {
            val text = invoke.parseArgs(ClipboardTextArgs::class.java).text
            val clipboard = appContext.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            clipboard.setPrimaryClip(ClipData.newPlainText("Jev Switch", text))
            invoke.resolve(JSObject())
        } catch (error: Exception) {
            invoke.reject(error.message, error)
        }
    }

    private fun exportPrivateFile(source: File, name: String, mimeType: String, cacheFolder: String, invoke: Invoke) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                val resolver = appContext.contentResolver
                val values = android.content.ContentValues().apply {
                    put(MediaStore.MediaColumns.DISPLAY_NAME, name)
                    put(MediaStore.MediaColumns.MIME_TYPE, mimeType)
                    put(MediaStore.MediaColumns.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS)
                    put(MediaStore.MediaColumns.IS_PENDING, 1)
                }
                val uri = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
                    ?: throw IllegalStateException("Android Downloads did not create a log file")
                try {
                    val output = resolver.openOutputStream(uri)
                        ?: throw IllegalStateException("Android Downloads did not open the log file")
                    output.use { stream -> source.inputStream().use { it.copyTo(stream) } }
                    resolver.update(uri, android.content.ContentValues().apply {
                        put(MediaStore.MediaColumns.IS_PENDING, 0)
                    }, null, null)
                } catch (error: Exception) {
                    resolver.delete(uri, null, null)
                    throw error
                }
                invoke.resolve(JSObject().put("result", "downloads"))
            } else {
                // Android 7–9 cannot write public Downloads without storage
                // permission. Share a cache copy through FileProvider instead.
                val sharedDirectory = File(appContext.cacheDir, cacheFolder)
                if (!sharedDirectory.isDirectory && !sharedDirectory.mkdirs()) {
                    throw IllegalStateException("Android share cache is unavailable")
                }
                val shared = File(sharedDirectory, name)
                source.copyTo(shared, overwrite = true)
                val uri = FileProvider.getUriForFile(appContext, "${appContext.packageName}.fileprovider", shared)
                val intent = Intent(Intent.ACTION_SEND).apply {
                    type = mimeType
                    putExtra(Intent.EXTRA_STREAM, uri)
                    clipData = ClipData.newUri(appContext.contentResolver, name, uri)
                    addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                }
                activity.runOnUiThread {
                    try {
                        activity.startActivity(Intent.createChooser(intent, null).apply {
                            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                        })
                        invoke.resolve(JSObject().put("result", "shared"))
                    } catch (error: Exception) {
                        invoke.reject(error.message, error)
                    }
                }
            }
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

    private fun permissionState(): String {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return "granted"
        return if (activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED) {
            "granted"
        } else {
            "denied"
        }
    }
}
