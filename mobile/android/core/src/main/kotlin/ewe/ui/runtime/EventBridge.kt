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
    val json: ByteArray? = null,
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
        submit(event, node, value, checked, null)
    }

    /** Find a named element in the view without exposing its renderer handle. */
    fun containsElement(name: String, view: String): Boolean =
        store.viewStore(view)?.nodes?.any { it.props[ewe.ui.PropId.ID] == Property.Text(name) } == true

    /** Send typed input to a host action in a named view. */
    suspend fun <T> triggerStructured(reference: String, view: String, value: T, encode: (T) -> String) {
        val destination = requireNotNull(store.viewStore(view)) { "Unknown view: $view" }
        val input = EventInput(0u, "host", "", false, 0u, byteArrayOf(), encode(value).encodeToByteArray())
        core.trigger(reference, input)?.let {
            check(destination.receive(it)) { "Invalid frame for view: $view" }
        }
    }

    fun reportError(code: String, message: String) {
        store.onError?.invoke(code, message)
    }

    /** Encode one application value with the serializer supplied by the form author. */
    fun <T> sendStructured(event: UShort, node: UInt, value: T, encode: (T) -> String) {
        submit(event, node, "", false, encode(value).encodeToByteArray())
    }

    private fun submit(event: UShort, node: UInt, value: String, checked: Boolean, json: ByteArray?) {
        val record = store.record(node) ?: return
        val reference = record.actions[event] ?: return
        val name = Names.eventId[event]?.lowercase() ?: return
        val input = EventInput(node, name, value, checked, record.parent ?: 0u, record.state, json)
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
