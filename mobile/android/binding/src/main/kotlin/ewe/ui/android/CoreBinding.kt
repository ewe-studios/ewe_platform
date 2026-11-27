package ewe.ui.android

import ewe.ui.runtime.*
import io.github.ewe_studios.ewe.core.FrameListener
import io.github.ewe_studios.ewe.core.NativeCore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

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

    fun helloFor(tag: ULong): ByteArray = native.helloFor(tag)

    override fun hello() = native.hello()

    override fun applyCapsFrame(frame: ByteArray) = native.applyCapsFrame(frame)

    override suspend fun trigger(reference: String, event: EventInput): ByteArray? =
        withContext(Dispatchers.Default) {
            val input = io.github.ewe_studios.ewe.core.EventInput(
                event.nodeId, event.eventName, event.value, event.checked, event.parentNode, event.state
            )
            // A native future can wake a waiting action while releasing a lock.
            // Run these calls on a dispatcher that queues the waiting continuation.
            val json = event.json
            if (json == null) native.trigger(reference, input)
            else native.triggerStructured(reference, input, json)
        }

    override fun close() {
        try {
            native.clearFrameListener()
        } finally {
            native.close()
        }
    }
}
