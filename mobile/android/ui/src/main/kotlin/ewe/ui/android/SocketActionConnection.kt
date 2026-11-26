package ewe.ui.android

import ewe.ui.*
import ewe.ui.runtime.ConnectionManager
import io.github.ewe_studios.ewe.core.HostTransport
import java.io.IOException
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.withTimeout

/** Send action bytes and deliver later replies to the originating screen. */
class SocketActionConnection(
    connections: ConnectionManager, endpoint: String, private val onReply: (ByteArray) -> Unit,
    private val onFailure: (Throwable) -> Unit, private val handshakeTimeoutMillis: Long = 60_000,
) : HostTransport, AutoCloseable {
    val requestTag = connections.nextTag()
    @Volatile private var stream: ULong? = null
    @Volatile private var caps: CompletableDeferred<ByteArray>? = null

    private fun packet(bytes: ByteArray): Frame {
        val input = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
        require(Frame.FrameBufferHasIdentifier(input))
        return Frame.getRootAsFrame(input)
    }

    private var hello: ByteArray? = null
    private var reconnect = false
    private fun negotiateAgain() { hello?.let { handle.send(it) } }
    private val handle: ewe.ui.runtime.ConnectionHandle = connections.acquire(LiveTransport.WebSocket, endpoint,
        accept = { bytes ->
            val frame = packet(bytes)
            val body = frame.kind(FrameMessage()) as FrameMessage
            val control = if (body.messagesLength > 0) body.messages(0)?.control?.caps else null
            if (control == null) body.ack != 0uL && frame.streamId == stream
            else control.requestTag == requestTag
        },
        onOpen = { if (reconnect) { reconnect = false; negotiateAgain() } },
        onError = { cause -> reconnect = true; stream = null; caps?.completeExceptionally(cause); onFailure(cause) },
        receive = { bytes ->
            val frame = packet(bytes)
            val body = frame.kind(FrameMessage()) as FrameMessage
            val control = if (body.messagesLength > 0) body.messages(0)?.control?.caps else null
            if (control == null) onReply(bytes) else {
                stream = if (control.status == CapsStatus.Ok) control.streamId else null
                if (caps?.complete(bytes) != true) onReply(bytes)
            }
        })

    override suspend fun send(request: ByteArray): ByteArray? {
        val frame = packet(request)
        val body = frame.kind(FrameRequest()) as FrameRequest
        if (body.hello == null) {
            if (stream != frame.streamId) throw IOException("Action connection needs a handshake")
            handle.send(request)
            return null
        }
        val result = CompletableDeferred<ByteArray>()
        synchronized(this) {
            check(caps == null && body.hello?.requestTag == requestTag)
            caps = result
            hello = request.copyOf()
        }
        return try {
            handle.send(request)
            withTimeout(handshakeTimeoutMillis) { result.await() }
        } finally { caps = null }
    }

    override fun close() {
        caps?.completeExceptionally(IOException("Action socket closed"))
        handle.close()
    }
}
