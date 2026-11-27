package ewe.todo

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import ewe.ui.runtime.EventBridge
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive

/** Connect the shared Todo countdown to the Android screen lifecycle. */
class TodoCountdown {
    private var bridge by mutableStateOf<EventBridge?>(null)
    private var readyCount by mutableStateOf(0)

    fun attach(bridge: EventBridge) {
        this.bridge = bridge
        readyCount += 1
    }

    @Composable
    fun Run(foreground: Boolean) {
        val bridge = bridge
        val paused = bridge?.containsElement("todo-pause-modal", "todos") == true
        LaunchedEffect(bridge, foreground, readyCount) {
            if (foreground && bridge != null) tick(bridge, refresh = true)
        }
        LaunchedEffect(bridge, foreground, paused) {
            if (foreground && paused) {
                while (isActive) {
                    delay(1000)
                    tick(bridge, refresh = false)
                }
            }
        }
        var wasPaused by remember { mutableStateOf(false) }
        var announcement by remember { mutableStateOf("") }
        LaunchedEffect(paused) {
            if (paused != wasPaused) {
                announcement = if (paused) "" else "Input resumed"
                wasPaused = paused
            }
        }
        Box(Modifier.size(1.dp).semantics {
            liveRegion = LiveRegionMode.Polite
            contentDescription = announcement
        })
    }

    private suspend fun tick(bridge: EventBridge, refresh: Boolean) {
        try {
            bridge.triggerStructured("host:tick_todo_countdown", "todos", refresh) { value ->
                "{\"refresh\":$value}"
            }
        } catch (cancel: CancellationException) {
            throw cancel
        } catch (error: Exception) {
            bridge.reportError("Countdown", error.message.orEmpty())
        }
    }
}
