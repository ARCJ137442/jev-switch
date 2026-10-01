package io.github.arcj137442.jevswitch

import android.app.ActivityManager
import android.graphics.BitmapFactory
import android.os.Bundle
import androidx.activity.enableEdgeToEdge

/** Tauri activity with an explicit task/recents icon. */
class MainActivity : TauriActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        updateTaskDescription()
    }

    override fun onNewIntent(intent: android.content.Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        updateTaskDescription()
    }

    private fun updateTaskDescription() {
        // Use a dedicated opaque raster so Android does not fall back to the
        // adaptive-icon template when rendering the recent-tasks card.
        val icon = BitmapFactory.decodeResource(resources, R.drawable.ic_task)
        if (icon != null) {
            setTaskDescription(ActivityManager.TaskDescription(getString(R.string.app_name), icon))
        }
    }
}
