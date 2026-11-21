package ewe.ui.runtime

import ewe.ui.LiveTransport

fun interface Channel {
    fun close()
}

interface ChannelFactory {
    fun open(
        endpoint: String,
        transport: UByte,
        streamId: ULong,
        resumeFrom: ULong,
        onFrame: (ByteArray) -> Unit,
        onClose: (Throwable?) -> Unit,
    ): Channel
}

class LiveMountHost(private val store: NodeStore, private val factory: ChannelFactory) :
    AutoCloseable {
    private class Mount(val spec: LiveSpec, var channel: Channel? = null)

    private val open = mutableMapOf<UInt, Mount>()
    private var disposed = false

    init {
        store.onChanged = ::reconcile
        store.onGap = { stream, _ ->
            open.filterValues { it.spec.stream == stream }.keys.toList().forEach(::detach)
            reconcile()
        }
        reconcile()
    }

    private fun attached(n: NodeRecord): Boolean {
        var cur: NodeRecord? = n
        while (cur != null) {
            if (cur.id == 1u) return true
            cur = cur.parent?.let(store::record)
        }
        return false
    }

    private fun detach(id: UInt) {
        open.remove(id)?.channel?.close()
    }

    private fun reconcile() {
        if (disposed) return
        val mounts =
            store.nodes.filter {
                it.live != null && it.live?.transport != LiveTransport.Local && attached(it)
            }
        val current = mounts.associate { it.id to requireNotNull(it.live) }
        open.filter { (id, entry) -> current[id] != entry.spec }.keys.toList().forEach(::detach)
        for ((id, spec) in current) {
            if (id in open) continue
            val entry = Mount(spec)
            open[id] = entry
            val cursor = store.cursor(spec.stream) ?: spec.resume
            store.setCursor(spec.stream, cursor)
            try {
                val ch =
                    factory.open(
                        spec.endpoint,
                        spec.transport,
                        spec.stream,
                        cursor,
                        { if (open[id] === entry) store.receive(it, mount = id) },
                        { error ->
                            if (open[id] === entry && error != null)
                                store.onError?.invoke("ChannelFailed", error.message.orEmpty())
                        },
                    )
                if (open[id] === entry) entry.channel = ch else ch.close()
            } catch (error: Exception) {
                open.remove(id)
                store.onError?.invoke("ChannelFailed", error.message.orEmpty())
            }
        }
    }

    override fun close() {
        disposed = true
        store.onChanged = null
        store.onGap = null
        open.keys.toList().forEach(::detach)
    }
}
