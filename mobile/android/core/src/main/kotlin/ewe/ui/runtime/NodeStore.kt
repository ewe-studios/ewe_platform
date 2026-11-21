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
    var onGap: ((ULong, ULong) -> Unit)? = null
    var onChanged: (() -> Unit)? = null
    var onError: ((String, String) -> Unit)? = null
    var onCaps: ((ByteArray) -> Unit)? = null

    fun record(id: UInt): NodeRecord? = records[id]

    fun cursor(stream: ULong): ULong? = cursors[stream]

    fun setCursor(stream: ULong, sequence: ULong) {
        cursors[stream] = sequence
    }

    fun close() {
        timers.values.forEach { it.cancel() }
        timers.clear()
        pending.clear()
    }

    internal fun report(code: String, row: Int? = null, node: UInt? = null, name: String? = null) {
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
            records.remove(pending.removeLast())?.let { pending.addAll(it.children) }
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

    fun receive(bytes: ByteArray, mount: UInt = 1u): Boolean {
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
                if (message.control?.caps != null) hasCaps = true
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
