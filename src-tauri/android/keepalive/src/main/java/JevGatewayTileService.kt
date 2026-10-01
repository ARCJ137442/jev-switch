package io.github.arcj137442.jevswitch.keepalive

import android.app.PendingIntent
import android.content.Intent
import android.graphics.drawable.Icon
import android.os.Build
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import io.github.arcj137442.jevswitch.R

class JevGatewayTileService : TileService() {
    override fun onStartListening() {
        super.onStartListening()
        updateTileState()
    }

    override fun onClick() {
        super.onClick()
        val launch = RuntimeKeepaliveState.launchIntent(this, true) ?: return
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            val pending = PendingIntent.getActivity(
                this,
                14352,
                launch,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
            startActivityAndCollapse(pending)
        } else {
            @Suppress("DEPRECATION")
            startActivityAndCollapse(launch)
        }
    }

    private fun updateTileState() {
        val active = RuntimeKeepaliveState.gatewayRunning(this)
        val tile = qsTile ?: return
        tile.state = if (active) Tile.STATE_ACTIVE else Tile.STATE_INACTIVE
        tile.label = getString(R.string.keepalive_tile_name)
        tile.subtitle = getString(if (active) R.string.keepalive_tile_running else R.string.keepalive_tile_stopped)
        tile.icon = Icon.createWithResource(this, R.drawable.ic_stat_jev_switch)
        tile.updateTile()
    }
}
