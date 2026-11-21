package ewe.ui.android

import ewe.ui.runtime.*
import io.github.ewe_studios.ewe.core.FrameListener
import io.github.ewe_studios.ewe.core.NativeCore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

class CoreBinding(private val native: NativeCore, store: NodeStore, scope: CoroutineScope) :
    ActionCore, AutoCloseable {
    init {
        native.setFrameListener(
            object : FrameListener {
                override fun onFrame(frame: ByteArray) {
                    scope.launch { store.receive(frame) }
                }
            }
        )
    }

    override fun hello() = native.hello()

    override fun applyCapsFrame(frame: ByteArray) = native.applyCapsFrame(frame)

    override suspend fun trigger(reference: String, event: EventInput): ByteArray? =
        native.trigger(
            reference,
            io.github.ewe_studios.ewe.core.EventInput(
                event.nodeId,
                event.eventName,
                event.value,
                event.checked,
                event.parentNode,
                event.state,
            ),
        )

    override fun close() {
        try {
            native.clearFrameListener()
        } finally {
            native.close()
        }
    }
}
