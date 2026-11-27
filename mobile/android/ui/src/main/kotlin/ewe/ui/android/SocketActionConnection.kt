package ewe.ui.android

import ewe.ui.*
import ewe.ui.runtime.ConnectionManager
import io.github.ewe_studios.ewe.core.HostTransport
import java.io.IOException
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

/** Wait for the reply to each action request on this screen. */
class SocketActionConnection(
    connections: ConnectionManager, endpoint: String, private val onReply: (ByteArray) -> Unit,
    private val onFailure: (Throwable) -> Unit, private val handshakeTimeoutMillis: Long = 60_000,
    private val actionTimeoutMillis: Long = handshakeTimeoutMillis,
) : HostTransport, AutoCloseable {
    private data class Receipt(val stream: ULong, val sequence: ULong)

    val requestTag = connections.nextTag()
    private val lock = Any()
    private var stream: ULong? = null
    private var caps: CompletableDeferred<ByteArray>? = null
    private var hello: ByteArray? = null
    private var reconnect = false
    private var closed = false
    private var generation = 0L
    private var ready = CompletableDeferred<Unit>()
    private val pending = mutableMapOf<Receipt, CompletableDeferred<ByteArray>>()

    private fun packet(bytes: ByteArray): Frame {
        val input = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
        require(Frame.FrameBufferHasIdentifier(input))
        return Frame.getRootAsFrame(input)
    }

    private fun negotiateAgain() {
        val request = synchronized(lock) {
            if (!reconnect || closed) null else hello.also { reconnect = false }
        } ?: return
        try {
            handle.send(request)
        } catch (error: Exception) {
            disconnected(error)
        }
    }

    private val handle: ewe.ui.runtime.ConnectionHandle = connections.acquire(LiveTransport.WebSocket, endpoint,
        accept = { bytes ->
            val frame = packet(bytes)
            val body = if (frame.kindType == FrameKind.FrameMessage) frame.kind(FrameMessage()) as? FrameMessage else null
            val control = body?.takeIf { it.messagesLength > 0 }?.messages(0)?.control?.caps
            if (control == null) body != null && body.ack != 0uL && synchronized(lock) { frame.streamId == stream }
            else control.requestTag == requestTag
        },
        onOpen = ::negotiateAgain,
        onError = ::disconnected,
        receive = ::receive,
    )

    private fun disconnected(cause: Throwable) {
        val waiting = synchronized(lock) {
            if (closed) return
            generation++
            reconnect = true
            stream = null
            if (ready.isCompleted) ready = CompletableDeferred()
            takeWaiters()
        }
        waiting.forEach { it.completeExceptionally(cause) }
        onFailure(cause)
    }

    private fun takeWaiters(): List<CompletableDeferred<ByteArray>> {
        val waiting = pending.values.toList() + listOfNotNull(caps)
        pending.clear()
        caps = null
        return waiting
    }

    private fun receive(bytes: ByteArray) {
        val frame = packet(bytes)
        val body = frame.kind(FrameMessage()) as FrameMessage
        val control = if (body.messagesLength > 0) body.messages(0)?.control?.caps else null
        if (control == null) {
            val key = Receipt(frame.streamId, body.ack)
            val waiting = synchronized(lock) {
                if (closed || stream != key.stream) return
                pending.remove(key)
            }
            if (waiting?.complete(bytes) != true) onReply(bytes)
            return
        }
        val (waiting, receivedGeneration) = synchronized(lock) {
            if (closed) return
            caps to generation
        }
        if (waiting == null) onReply(bytes)
        synchronized(lock) {
            if (closed || generation != receivedGeneration) return
            stream = control.streamId.takeIf { control.status == CapsStatus.Ok }
            if (stream == null) ready.completeExceptionally(IOException("Action connection was rejected"))
            else ready.complete(Unit)
            caps = null
        }
        waiting?.complete(bytes)
    }

    internal suspend fun awaitReady() {
        withTimeoutOrNull(handshakeTimeoutMillis) {
            var connected = false
            while (!connected) {
                val current = synchronized(lock) {
                    if (closed) throw IOException("Action socket closed")
                    ready.takeIf { stream == null }
                }
                connected = current == null
                current?.await()
            }
        } ?: throw IOException("Action connection handshake timed out")
    }

    override suspend fun send(request: ByteArray): ByteArray {
        val frame = packet(request)
        check(frame.kindType == FrameKind.FrameRequest)
        val body = frame.kind(FrameRequest()) as FrameRequest
        body.hello?.let { return negotiate(request, it.requestTag) }
        val key = Receipt(frame.streamId, frame.sequenceId)
        val reply = CompletableDeferred<ByteArray>()
        synchronized(lock) {
            if (closed || stream != key.stream) throw IOException("Action connection needs a handshake")
            check(key.sequence != 0uL && key !in pending)
            pending[key] = reply
        }
        return coroutineScope {
            val timeout = launch {
                delay(actionTimeoutMillis)
                val expired = synchronized(lock) { pending.remove(key, reply) }
                if (expired) reply.completeExceptionally(IOException("Action reply timed out"))
            }
            try {
                if (synchronized(lock) { pending[key] === reply }) handle.send(request)
                reply.await()
            } finally {
                timeout.cancel()
                synchronized(lock) { pending.remove(key, reply) }
            }
        }
    }

    private suspend fun negotiate(request: ByteArray, tag: ULong): ByteArray {
        val reply = CompletableDeferred<ByteArray>()
        synchronized(lock) {
            if (closed) throw IOException("Action socket closed")
            check(caps == null && tag == requestTag)
            caps = reply
            hello = request.copyOf()
        }
        return try {
            handle.send(request)
            withTimeoutOrNull(handshakeTimeoutMillis) { reply.await() }
                ?: throw IOException("Action connection handshake timed out")
        } finally {
            synchronized(lock) { if (caps === reply) caps = null }
        }
    }

    override fun close() {
        val cause = IOException("Action socket closed")
        val waiting = synchronized(lock) {
            if (closed) return
            closed = true
            stream = null
            ready.completeExceptionally(cause)
            takeWaiters()
        }
        waiting.forEach { it.completeExceptionally(cause) }
        handle.close()
    }
}
