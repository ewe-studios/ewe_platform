package ewe.ui.runtime

import ewe.ui.LiveTransport

fun interface Channel {
    fun send(bytes: ByteArray): Unit = throw java.io.IOException("Channel is read-only")
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

interface ManagedChannelFactory : ChannelFactory {
    fun connectionManager(): ConnectionManager
}

class LiveMountHost(
    private val store: NodeStore, private val factory: ChannelFactory,
    manager: ConnectionManager?,
    private val onChannelStatus: ((NodeStore, UInt, String?) -> Unit)?,
) :
    AutoCloseable {
    constructor(store: NodeStore, factory: ChannelFactory, manager: ConnectionManager? = null) :
        this(store, factory, manager, null)

    private class Mount(val spec: LiveSpec, var channel: Channel? = null, var resume: ((ULong) -> Unit)? = null)

    private val open = mutableMapOf<UInt, Mount>()
    private val failed = mutableSetOf<UInt>()
    private var disposed = false
    private val ownsConnections = manager == null
    val connections = manager ?: (factory as? ManagedChannelFactory)?.connectionManager()

    internal fun forView(child: NodeStore) = LiveMountHost(child, factory, connections, onChannelStatus)

    init {
        store.liveHost = this
        store.onChanged = ::reconcile
        store.onGap = { stream, after ->
            open.filterValues { it.spec.stream == stream }.toMap().forEach { (id, entry) ->
                entry.resume?.invoke(after) ?: detach(id)
            }
            reconcile()
        }
        reconcile()
    }

    private fun attached(n: NodeRecord): Boolean {
        var cur: NodeRecord? = n
        while (cur != null) {
            if (cur.id == 1u || cur.id in store.targets.roots.values) return true
            cur = cur.parent?.let(store::record)
        }
        return false
    }

    private fun reportFailure(id: UInt, error: Throwable) {
        failed.add(id)
        val message = error.message.orEmpty()
        if (onChannelStatus != null) onChannelStatus.invoke(store, id, message)
        else store.onError?.invoke("ChannelFailed", message)
    }

    private fun clearFailure(id: UInt) {
        if (failed.remove(id)) onChannelStatus?.invoke(store, id, null)
    }

    private fun detach(id: UInt) {
        clearFailure(id)
        open.remove(id)?.channel?.close()
    }

    private fun reconcile() {
        if (disposed) return
        val mounts =
            store.nodes.filter {
                it.live != null && it.live?.transport != LiveTransport.Local && attached(it)
            }
        val current = mounts.associate { it.id to requireNotNull(it.live) }
        failed.filterNot(current::containsKey).forEach(::clearFailure)
        open.filter { (id, entry) -> current[id] != entry.spec }.keys.toList().forEach(::detach)
        for ((id, spec) in current) {
            if (id in open) continue
            val entry = Mount(spec)
            open[id] = entry
            val generation = store.recovery?.generation
            fun current() = open[id] === entry && (generation == null || store.recovery?.accepts(generation) == true)
            val cursor = store.cursor(spec.stream) ?: spec.resume
            store.setCursor(spec.stream, cursor)
            try {
                val ch =
                    if (connections != null && connections.supports(spec.transport) &&
                        (spec.shared == true || spec.keepAlive || store.subscriptionReplay)) {
                        val subscription = connections.nextTag()
                        var session: ULong? = null
                        val latestOnly = spec.transport == LiveTransport.Sse && spec.shared == true
                        var pending = cursor.takeIf { !latestOnly && (it != 0uL || store.recovery != null) }
                        val socket = spec.transport == LiveTransport.WebSocket
                        var needsHello = false
                        fun resume(after: ULong) {
                            val target = session
                            if (target == null || entry.channel == null) pending = after
                            else entry.channel?.send(SubscriptionFrames.resume(target, subscription, spec.stream, after))
                        }
                        fun context(bytes: ByteArray) {
                            val caps = SubscriptionFrames.caps(bytes) ?: return
                            if (socket && caps.requestTag != subscription) return
                            if (caps.subscriptionResume) {
                                session = caps.streamId
                                entry.resume = ::resume
                                pending?.let { pending = null; resume(it) }
                            }
                        }
                        connections.acquire(spec.transport, spec.endpoint, spec.shared, spec.keepAlive, owner = store,
                            accept = { bytes -> val caps = SubscriptionFrames.caps(bytes)
                                if (caps != null) socket && caps.requestTag == subscription
                                else store.matches(bytes, spec.stream, subscription) },
                            onError = { error ->
                                session = null
                                if (socket || spec.shared != true) pending = store.cursor(spec.stream)
                                if (current()) reportFailure(id, error)
                            },
                            onOpen = {
                                if (!current()) return@acquire
                                clearFailure(id)
                                if (socket && store.subscriptionReplay) {
                                    entry.channel?.let { it.send(SubscriptionFrames.hello(subscription)) } ?: run { needsHello = true }
                                }
                            }, onContext = ::context, receive = { bytes ->
                                if (current()) {
                                    if (SubscriptionFrames.caps(bytes) != null) context(bytes) else store.receive(bytes, mount = id)
                                }
                            }).also { acquired ->
                                entry.channel = acquired
                                if (needsHello) acquired.send(SubscriptionFrames.hello(subscription))
                                if (session != null) pending?.let { pending = null; resume(it) }
                            }
                    } else factory.open(
                        spec.endpoint,
                        spec.transport,
                        spec.stream,
                        cursor,
                        { if (current() && store.receive(it, mount = id)) clearFailure(id) },
                        { error ->
                            if (current() && error != null) reportFailure(id, error)
                        },
                    )
                if (open[id] === entry) entry.channel = ch else ch.close()
            } catch (error: Exception) {
                if (current()) {
                    open.remove(id)
                    reportFailure(id, error)
                }
            }
        }
    }

    override fun close() {
        if (disposed) return
        disposed = true
        store.liveHost = null
        store.onChanged = null
        store.onGap = null
        open.keys.toList().forEach(::detach)
        failed.toList().forEach(::clearFailure)
        if (ownsConnections) connections?.close()
    }
}
