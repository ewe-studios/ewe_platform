package ewe.ui.runtime

import ewe.ui.LiveTransport
import java.io.IOException
import java.net.URI

/** One adapter opens a connection and owns its protocol reader. */
fun interface ConnectionAdapter {
    fun open(endpoint: String, events: ConnectionEvents): Channel
}

interface ConnectionEvents {
    val listening: Boolean
    fun opened()
    fun context(bytes: ByteArray)
    fun deliver(bytes: ByteArray)
    fun disconnected(error: Throwable)
}

interface ConnectionHandle : Channel {
    fun releaseRetention()
}

/** Create one manager for each authenticated application context. */
class ConnectionManager(private val resolve: (String) -> String = { it }) : AutoCloseable {
    private class Consumer(
        val owner: Any?, val accept: (ByteArray) -> Boolean,
        val receive: ((ByteArray) -> Unit)?, val onError: (Throwable) -> Unit,
        val onOpen: () -> Unit, val onContext: (ByteArray) -> Unit,
    )

    private class Entry(
        val transport: UByte, val endpoint: String, val shared: Boolean, val settings: Any?,
    ) {
        val consumers = mutableSetOf<Consumer>()
        val retainers = mutableSetOf<Any>()
        var channel: Channel? = null
        var opened = false
        var context: ByteArray? = null
    }

    private val adapters = mutableMapOf<UByte, ConnectionAdapter>()
    private val entries = mutableSetOf<Entry>()
    private var closed = false
    private var tag = 0uL

    @Synchronized
    fun nextTag(): ULong {
        check(tag != ULong.MAX_VALUE)
        return ++tag
    }

    @Synchronized
    fun supports(transport: UByte): Boolean = transport in adapters

    @Synchronized
    fun register(transport: UByte, adapter: ConnectionAdapter): ConnectionManager {
        check(!closed && transport !in adapters)
        adapters[transport] = adapter
        return this
    }

    @Synchronized
    fun acquire(
        transport: UByte, endpoint: String, shared: Boolean? = null, keepAlive: Boolean = false,
        settings: Any? = null, owner: Any? = null, accept: (ByteArray) -> Boolean = { true },
        onError: (Throwable) -> Unit, onOpen: () -> Unit = {}, onContext: (ByteArray) -> Unit = {}, receive: ((ByteArray) -> Unit)? = null,
    ): ConnectionHandle {
        check(!closed)
        val adapter = requireNotNull(adapters[transport]) { "Unknown connection transport" }
        val address = URI(resolve(endpoint)).normalize()
        require(address.userInfo == null && address.fragment == null)
        val key = address.toString()
        val share = shared ?: (transport == LiveTransport.WebSocket)
        val previous = entries.find { share && it.shared && it.transport == transport && it.endpoint == key && it.settings === settings }
        val entry = previous ?: Entry(transport, key, share, settings)
        val consumer = Consumer(owner, accept, receive, onError, onOpen, onContext)
        entry.consumers.add(consumer)
        val retention = Any()
        if (keepAlive) entry.retainers.add(retention)
        if (previous == null) {
            entries.add(entry)
            try {
                val channel = adapter.open(key, object : ConnectionEvents {
                    override val listening get() = synchronized(this@ConnectionManager) { entry.consumers.any { it.receive != null } }
                    override fun opened() {
                        val listeners = synchronized(this@ConnectionManager) { entry.opened = true; entry.consumers.toList() }
                        listeners.forEach { it.onOpen() }
                    }
                    override fun context(bytes: ByteArray) {
                        val listeners = synchronized(this@ConnectionManager) { entry.context = bytes.copyOf(); entry.consumers.toList() }
                        listeners.forEach { it.onContext(bytes.copyOf()) }
                    }
                    override fun deliver(bytes: ByteArray) = dispatch(entry, bytes)
                    override fun disconnected(error: Throwable) {
                        val active = synchronized(this@ConnectionManager) { entry.opened = false; entry.context = null; entry.consumers.toList() }
                        active.forEach { it.onError(error) }
                    }
                })
                if (entry in entries) entry.channel = channel else channel.close()
            } catch (cause: Exception) {
                entries.remove(entry)
                throw cause
            }
        }
        if (previous != null) {
            if (entry.opened) onOpen()
            entry.context?.let { onContext(it.copyOf()) }
        }
        return object : ConnectionHandle {
            override fun send(bytes: ByteArray) {
                val channel = synchronized(this@ConnectionManager) {
                    if (consumer !in entry.consumers) throw IOException("Connection handle is closed")
                    checkNotNull(entry.channel)
                }
                channel.send(bytes)
            }
            override fun close() = release(entry, consumer, null)
            override fun releaseRetention() = release(entry, consumer, retention)
        }
    }

    private fun dispatch(entry: Entry, bytes: ByteArray) {
        val targets = synchronized(this) { entry.consumers.toList() }
        val delivered = java.util.IdentityHashMap<Any, Boolean>()
        for (consumer in targets) {
            val receive = consumer.receive ?: continue
            if (!synchronized(this) { consumer in entry.consumers } || delivered.containsKey(consumer.owner)) continue
            try {
                val copy = bytes.copyOf()
                if (!consumer.accept(copy)) continue
                receive(copy)
                consumer.owner?.let { delivered[it] = true }
            } catch (cause: Exception) {
                consumer.onError(cause)
            }
        }
    }

    @Synchronized
    private fun release(entry: Entry, consumer: Consumer, retention: Any?) {
        if (retention != null) entry.retainers.remove(retention) else entry.consumers.remove(consumer)
        if (entry.consumers.isEmpty() && entry.retainers.isEmpty() && entries.remove(entry)) entry.channel?.close()
    }

    @Synchronized
    override fun close() {
        closed = true
        val active = entries.toList()
        entries.clear()
        active.forEach { it.consumers.clear(); it.retainers.clear(); it.channel?.close() }
    }
}
