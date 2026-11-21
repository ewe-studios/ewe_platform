package ewe.ui.runtime

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

data class EventInput(
    val nodeId: UInt,
    val eventName: String,
    val value: String,
    val checked: Boolean,
    val parentNode: UInt,
    val state: ByteArray,
)

interface ActionCore {
    fun hello(): ByteArray

    fun applyCapsFrame(frame: ByteArray)

    suspend fun trigger(reference: String, event: EventInput): ByteArray?
}

class EventBridge(
    private val store: NodeStore,
    private val core: ActionCore,
    private val scope: CoroutineScope,
) {
    init {
        store.onCaps = { frame ->
            try {
                core.applyCapsFrame(frame)
            } catch (error: Exception) {
                store.onError?.invoke("ActionFailed", error.message.orEmpty())
            }
        }
    }

    fun send(event: UShort, node: UInt, value: String = "", checked: Boolean = false) {
        val record = store.record(node) ?: return
        val reference = record.actions[event] ?: return
        val name = Names.eventId[event]?.lowercase() ?: return
        val input = EventInput(node, name, value, checked, record.parent ?: 0u, record.state)
        scope.launch {
            try {
                core.trigger(reference, input)?.let { store.receive(it) }
            } catch (cancel: CancellationException) {
                throw cancel
            } catch (error: Exception) {
                store.onError?.invoke("ActionFailed", error.message.orEmpty())
            }
        }
    }
}
