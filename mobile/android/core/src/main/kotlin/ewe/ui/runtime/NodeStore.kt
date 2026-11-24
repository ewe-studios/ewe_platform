package ewe.ui.runtime

import androidx.compose.runtime.mutableStateMapOf
import ewe.ui.*
import java.nio.ByteBuffer
import java.util.TreeMap
import kotlinx.serialization.json.*

fun interface Cancel {
    fun cancel()
}

fun interface GapTimer {
    fun schedule(delayMs: Long, callback: () -> Unit): Cancel
}

fun interface CommandHost {
    fun run(command: UShort, args: Map<String, Property>): Boolean
}

class NodeStore(
    private val timer: GapTimer,
    private val gapTimeoutMs: Long = 1000,
    val maxFrameBytes: Int = 4 * 1024 * 1024,
    internal val commands: CommandHost? = null,
    val supportsBehaviour: (String) -> Boolean = {
        it in Names.behaviourId.values && it != "UNKNOWN"
    },
) {
    var recovery: ViewRecovery? = null
        private set
    internal val targets = ViewTargets(this)
    internal var liveHost: LiveMountHost? = null
    private var parentStore: NodeStore? = null
    private val childStores = mutableMapOf<UInt, NodeStore>()

    /** The named view owns its numeric node identifiers and stream positions. */
    fun viewStore(name: String): NodeStore? {
        val host = parentStore ?: this
        val root = host.targets.roots[name] ?: return null
        return host.childStores[root] ?: host
    }

    /** Register a named view and allocate its root in this host. */
    fun registerView(name: String): NodeStore {
        var root = 1u
        if (root in targets.roots.values) {
            do {
                check(root < UInt.MAX_VALUE) { "No available view root" }
                root++
            } while (root in records || root in targets.roots.values)
        }
        registerView(name, root)
        return childStores[root] ?: this
    }

    fun registerView(name: String, root: UInt) {
        require(name.matches(targets.validName) && root != 0u)
        require(name !in targets.roots && root !in targets.roots.values)
        if (root != 1u) {
            val child = NodeStore(timer, gapTimeoutMs, maxFrameBytes, commands, supportsBehaviour)
            child.parentStore = this
            child.targets.roots[name] = root
            child.subscriptionReplay = subscriptionReplay
            child.onData = { id, data -> onData?.invoke(id, data) }
            child.onSnapshot = { id, stream -> onSnapshot?.invoke(id, stream) }
            child.onError = { code, message -> onError?.invoke(code, message) }
            child.onCaps = { bytes -> onCaps?.invoke(bytes) }
            val pending = ArrayDeque(listOf(root))
            while (pending.isNotEmpty()) {
                val id = pending.removeLast()
                val node = records.remove(id) ?: continue
                child.records[id] = node
                targets.names.remove(id)?.let { child.targets.names[id] = it }
                targets.tags.remove(id)?.let { child.targets.tags[id] = it }
                pending.addAll(node.children)
            }
            if (child.record(root) == null) child.create(root, NodeKind.STACK, 0)
            child.onData = { id, data -> onData?.invoke(id, data) }
            childStores[root] = child
            liveHost?.forView(child)
        }
        targets.roots[name] = root
    }

    /** Give an existing custom control its portable tag and element name. */
    fun registerElement(name: String, tag: String, id: UInt) {
        require(tag.matches(targets.validName) && !tag.equals("view",ignoreCase = true))
        val node = requireNotNull(record(id))
        require(targets.setName(id, name, 0))
        node.props[ewe.ui.PropId.ID] = Property.Text(name)
        targets.tags[id] = tag
    }

    fun unregisterView(name: String) {
        val root = requireNotNull(targets.roots.remove(name))
        childStores.remove(root)?.let { child ->
            child.close()
            child.remove(root)
            return
        }
        recovery?.close()
        clearOrdering()
        if (root == 1u) record(root)?.children?.toList()?.forEach(::remove) else remove(root)
    }

    private val records = mutableStateMapOf(1u to NodeRecord(1u, NodeKind.STACK))
    val nodes: Collection<NodeRecord>
        get() = records.values

    val reports = mutableListOf<JsonObject>()
    internal val sentCommands = mutableListOf<JsonObject>()
    internal val errors = mutableListOf<JsonObject>()
    internal val data = mutableListOf<JsonObject>()
    internal val caps = mutableListOf<JsonObject>()
    private val cursors = mutableMapOf<ULong, ULong>()
    private data class PendingFrame(val bytes: ByteArray, val mount: UInt)
    private val pending = mutableMapOf<ULong, TreeMap<ULong, PendingFrame>>()
    private val timers = mutableMapOf<ULong, Cancel>()
    var onData: ((UInt, Data) -> Unit)? = null
    var onGap: ((ULong, ULong) -> Unit)? = null
    var onChanged: (() -> Unit)? = null
    var onError: ((String, String) -> Unit)? = null
    var onSnapshot: ((UInt, ULong) -> Unit)? = null
    var subscriptionReplay = false
        private set
    var onCaps: ((ByteArray) -> Unit)? = null

    fun record(id: UInt): NodeRecord? = childStores[id]?.records?.get(id) ?: records[id]

    fun cursor(stream: ULong): ULong? = cursors[stream]

    fun setCursor(stream: ULong, sequence: ULong) {
        cursors[stream] = sequence
    }

    fun close() {
        recovery?.close()
        childStores.values.forEach { it.close() }
        liveHost?.close()
        clearOrdering()
    }

    private fun clearOrdering() {
        cursors.clear()
        timers.values.forEach { it.cancel() }
        timers.clear()
        pending.clear()
    }

    internal fun report(code: String, row: Int? = null, node: UInt? = null, name: String? = null) {
        parentStore?.report(code, row, node, name)
        reports.add(
            buildJsonObject {
                put("code", code)
                row?.let { put("row", it) }
                node?.let { put("node", it.toLong()) }
                name?.let { put("name", it) }
            }
        )
    }

    internal fun node(id: UInt, row: Int): NodeRecord? =
        records[id].also { if (it == null) report("UnknownNode", row, id) }

    internal fun create(id: UInt, kind: UShort, row: Int) {
        if (records.containsKey(id)) {
            report("DuplicateNode", row, id)
            if (id == 1u) return
            remove(id)
        }
        if (kind == NodeKind.UNKNOWN || !Names.nodeKind.containsKey(kind))
            report("UnknownKind", row, id)
        records[id] = NodeRecord(id, kind)
    }

    internal fun remove(id: UInt) {
        if (id == 1u) return
        val node = records[id] ?: return
        node.parent?.let { records[it]?.children?.remove(id) }
        val pending = ArrayDeque<UInt>()
        pending.add(id)
        while (pending.isNotEmpty()) {
            val removed = pending.removeLast()
            targets.tags.remove(removed)
            targets.names.remove(removed)
            targets.roots.entries.removeAll { it.value == removed }
            records.remove(removed)?.let { pending.addAll(it.children) }
        }
    }

    internal fun insert(parent: UInt, child: UInt, before: UInt?, row: Int) {
        val container = node(parent, row) ?: return
        val item = node(child, row) ?: return
        if (before != null && before !in container.children) {
            report("UnknownNode", row, before)
            return
        }
        if (child == before) return
        var check: UInt? = parent
        while (check != null) {
            if (check == child || child == 1u) {
                report("InvalidParent", row, child)
                return
            }
            check = records[check]?.parent
        }
        item.parent?.let { records[it]?.children?.remove(child) }
        val at = before?.let(container.children::indexOf) ?: container.children.size
        container.children.add(at, child)
        item.parent = parent
    }

    internal fun behaviour(id: UInt, name: String, row: Int) {
        val item = node(id, row) ?: return
        if (supportsBehaviour(name)) item.behaviours += name
        else report("UnknownBehaviour", row, id, name)
    }

    private fun frame(bytes: ByteArray): Frame {
        require(bytes.size >= 8)
        val input = ByteBuffer.wrap(bytes).order(java.nio.ByteOrder.LITTLE_ENDIAN)
        require(Frame.FrameBufferHasIdentifier(input))
        val f = Frame.getRootAsFrame(input)
        require(f.schemaVersion.toInt() == 1 && f.kindType == FrameKind.FrameMessage)
        return f
    }

    /** The initial view and its recovery use the same loader and current credentials. */
    fun registerLoader(scope: kotlinx.coroutines.CoroutineScope, maxAttempts: Int = 3, load: suspend () -> ByteArray): ViewRecovery {
        recovery?.close()
        val result = ViewRecovery(this, scope, maxAttempts, load)
        recovery = result
        onSnapshot = { _, stream -> result.request(stream) }
        return result
    }

    internal fun installSnapshot(bytes: ByteArray, required: Set<ULong>) {
        require(bytes.size <= maxFrameBytes)
        val packet = frame(bytes)
        val body = packet.kind(FrameMessage()) as FrameMessage
        require(body.messagesLength >= 1)
        for (index in 1 until body.messagesLength) {
            val message = requireNotNull(body.messages(index))
            require(message.data != null && message.operation == null && message.control == null)
        }
        val op = requireNotNull(body.messages(0)?.operation)
        require(op.uiType == UiOperation.Place && (op.ui(Place()) as Place).kind == PlacementKind.FullView)
        val probe = NodeStore(timer, gapTimeoutMs, maxFrameBytes, supportsBehaviour = supportsBehaviour)
        require(probe.receive(bytes) && probe.reports.isEmpty()) { "Invalid snapshot content" }
        val positions = mutableMapOf<ULong,ULong>()
        for (spec in probe.nodes.mapNotNull { it.live }) {
            if (spec.kind != LiveKind.Stream || spec.transport == LiveTransport.Local ||
                (spec.shared == true && spec.transport == LiveTransport.Sse)) continue
            val previous = positions.put(spec.stream,spec.resume)
            require(previous == null || previous == spec.resume) { "Conflicting stream cursors" }
        }
        require(positions.keys.containsAll(required)) { "Missing snapshot cursor" }
        clearOrdering()
        positions.forEach(::setCursor)
        check(receive(bytes)) { "Invalid snapshot content" }
    }

    fun matches(bytes: ByteArray, stream: ULong, subscription: ULong = 0uL): Boolean {
        if (bytes.size > maxFrameBytes) return false
        return try {
            val packet = frame(bytes)
            val body = packet.kind(FrameMessage()) as FrameMessage
            packet.streamId == stream && body.ack == 0uL && (body.subscriptionId == 0uL || body.subscriptionId == subscription)
        } catch (_: RuntimeException) { false }
    }

    fun receive(bytes: ByteArray, mount: UInt = if (targets.roots.isEmpty()) 1u else targets.roots.values.singleOrNull() ?: 0u): Boolean {
        childStores[mount]?.let { return it.receive(bytes, mount) }
        if (bytes.size > maxFrameBytes) {
            report("TooLarge")
            return false
        }
        return try {
            val f = frame(bytes)
            val body = f.kind(FrameMessage()) as FrameMessage
            val id = f.streamId
            val sequence = f.sequenceId
            val last = cursors[id]
            when {
                sequence == 0uL || body.ack != 0uL -> apply(body, bytes, mount)
                last != null && sequence <= last -> true
                last != null && sequence - last > 1uL -> {
                    pending.getOrPut(id) { TreeMap() }.putIfAbsent(sequence, PendingFrame(bytes.copyOf(), mount))
                    if (id !in timers)
                        timers[id] =
                            timer.schedule(gapTimeoutMs) {
                                timers.remove(id)
                                if (!pending.remove(id).isNullOrEmpty())
                                    onGap?.invoke(id, cursors.getValue(id))
                            }
                    true
                }
                else -> {
                    if (!apply(body, bytes, mount)) return false
                    cursors[id] = sequence
                    while (cursors[id] != ULong.MAX_VALUE) {
                        val next = cursors.getValue(id) + 1u
                        val replay = pending[id]?.remove(next) ?: break
                        if (!apply(frame(replay.bytes).kind(FrameMessage()) as FrameMessage, replay.bytes, replay.mount))
                            return false
                        cursors[id] = next
                    }
                    if (pending[id].isNullOrEmpty()) {
                        pending.remove(id)
                        timers.remove(id)?.cancel()
                    }
                    true
                }
            }
        } catch (error: RuntimeException) {
            report("BadFrame")
            false
        }
    }

    private fun apply(body: FrameMessage, bytes: ByteArray, mount: UInt): Boolean {
        val applier = FrameApplier(this, bytes.size, mount)
        var hasCaps = false
        for (row in 0 until body.messagesLength) {
            try {
                val message = requireNotNull(body.messages(row))
                applier.message(message, row)
                message.control?.caps?.let { caps -> subscriptionReplay = caps.subscriptionResume; hasCaps = true }
                message.control?.replayExpired?.let { expired ->
                    onSnapshot?.invoke(mount, expired.streamId) ?: onError?.invoke("ReplayExpired", "Request a new view")
                }
            } catch (error: RuntimeException) {
                report("BadFrame", row)
                return false
            }
        }
        if (hasCaps) onCaps?.invoke(bytes)
        onChanged?.invoke()
        return true
    }
}
