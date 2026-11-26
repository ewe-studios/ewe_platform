package ewe.ui.android

import io.github.ewe_studios.ewe.core.AppWebSocket
import io.github.ewe_studios.ewe.core.CoreException
import io.github.ewe_studios.ewe.core.HostWebSocket
import io.github.ewe_studios.ewe.core.HostWebSockets
import io.github.ewe_studios.ewe.core.SocketMessage
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.channels.Channel
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import okio.ByteString.Companion.toByteString

/** Opens independent application sockets with the supplied network client. */
class AppWebSockets(
    private val client: OkHttpClient = OkHttpClient(),
    private val queueCapacity: Int = Channel.BUFFERED,
) : HostWebSockets {
    override suspend fun openSocket(endpoint: String): AppWebSocket {
        if (!endpoint.startsWith("ws://", true) && !endpoint.startsWith("wss://", true)) {
            throw CoreException.Transport("Invalid WebSocket endpoint")
        }
        val request = try {
            Request.Builder().url(endpoint).build()
        } catch (error: IllegalArgumentException) {
            throw CoreException.Transport(error.message.orEmpty())
        }
        val listener = ApplicationListener(queueCapacity)
        val socket = client.newWebSocket(request, listener)
        try {
            listener.opened.await()
            return AppWebSocket(object : HostWebSocket {
                override suspend fun sendMessage(message: SocketMessage) {
                    val accepted = when (message) {
                        is SocketMessage.Text -> socket.send(message.value)
                        is SocketMessage.Binary -> socket.send(message.value.toByteString())
                    }
                    if (!accepted) throw CoreException.Transport("The socket rejected the message")
                }

                override suspend fun receiveMessage(): SocketMessage? {
                    val result = listener.messages.receiveCatching()
                    result.exceptionOrNull()?.let { throw it }
                    return result.getOrNull()
                }

                override fun closeSocket() {
                    listener.messages.close()
                    socket.close(1000, null)
                }
            })
        } catch (error: Throwable) {
            socket.cancel()
            throw error
        }
    }
}

private class ApplicationListener(capacity: Int) : WebSocketListener() {
    val opened = CompletableDeferred<Unit>()
    val messages = Channel<SocketMessage>(capacity)
    override fun onOpen(webSocket: WebSocket, response: Response) { opened.complete(Unit) }
    override fun onMessage(webSocket: WebSocket, text: String) = deliver(webSocket, SocketMessage.Text(text))
    override fun onMessage(webSocket: WebSocket, bytes: ByteString) = deliver(webSocket, SocketMessage.Binary(bytes.toByteArray()))

    private fun deliver(socket: WebSocket, message: SocketMessage) {
        val result = messages.trySend(message)
        if (result.isFailure && !result.isClosed) {
            messages.close(CoreException.Transport("The socket receive queue is full"))
            socket.cancel()
        }
    }

    override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
        webSocket.close(1000, null)
        messages.close(if (code == 1000) null else CoreException.Transport("Peer closed the socket: $code $reason"))
    }

    override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
        val failure = CoreException.Transport(t.message ?: t.toString())
        opened.completeExceptionally(failure)
        messages.close(failure)
    }
}
