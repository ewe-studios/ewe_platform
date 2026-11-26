package ewe.ui.android

import android.os.Handler
import android.os.Looper
import android.util.Base64
import ewe.ui.LiveTransport
import ewe.ui.runtime.SubscriptionFrames
import ewe.ui.runtime.Channel
import ewe.ui.runtime.ConnectionManager
import ewe.ui.runtime.ConnectionAdapter
import ewe.ui.runtime.ManagedChannelFactory
import java.io.IOException
import java.util.concurrent.atomic.AtomicBoolean
import okhttp3.*
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.MediaType.Companion.toMediaType
import okio.ByteString
import okio.ByteString.Companion.toByteString

class OkHttpChannelFactory(
    private val client: OkHttpClient,
    private val baseUrl: HttpUrl,
    private val maxFrameBytes: Int = 4 * 1024 * 1024,
    private val main: Handler = Handler(Looper.getMainLooper()),
    private val reconnectMillis: Long = 1000,
) : ManagedChannelFactory {
    private fun resolve(endpoint: String): HttpUrl = baseUrl.resolve(endpoint.replaceFirst(Regex("^ws"), "http"))
        ?: error("Invalid endpoint")

    override fun connectionManager(): ConnectionManager {
        val manager = ConnectionManager { resolve(it).toString() }
        for (kind in listOf(LiveTransport.Sse, LiveTransport.WebSocket, LiveTransport.Http)) {
            manager.register(kind, ConnectionAdapter { endpoint, events ->
                val url = resolve(endpoint)
                var active: Channel? = null
                var stopped = false
                var retry: Runnable? = null
                fun connect() {
                    if (stopped) return
                    active = openConnection(url, kind, { events.listening }, { bytes ->
                        if (kind != LiveTransport.WebSocket && SubscriptionFrames.caps(bytes) != null) events.context(bytes)
                        else events.deliver(bytes)
                    }, { error ->
                        if (!stopped) {
                            events.disconnected(error ?: IOException("Live connection closed"))
                            retry = Runnable { connect() }.also { main.postDelayed(it, reconnectMillis) }
                        }
                    }, managed = true, onOpen = events::opened)
                }
                connect()
                object : Channel {
                    override fun close() { stopped = true; retry?.let(main::removeCallbacks); active?.close() }
                    override fun send(bytes: ByteArray) { requireNotNull(active).send(bytes) }
                }
            })
        }
        return manager
    }
    override fun open(
        endpoint: String,
        transport: UByte,
        streamId: ULong,
        resumeFrom: ULong,
        onFrame: (ByteArray) -> Unit,
        onClose: (Throwable?) -> Unit,
    ): Channel {
        val url = resolve(endpoint).newBuilder()
            .setQueryParameter("stream_id", streamId.toString())
            .setQueryParameter("resume_from", resumeFrom.toString()).build()
        return openConnection(url, transport, { true }, onFrame, onClose, expired = { SubscriptionFrames.expired(streamId,resumeFrom) })
    }

    private fun openConnection(
        url: HttpUrl, transport: UByte, listening: () -> Boolean,
        onFrame: (ByteArray) -> Unit, onClose: (Throwable?) -> Unit,
        managed: Boolean = false, onOpen: () -> Unit = {}, expired: (() -> ByteArray)? = null,
    ): Channel {
        val request =
            Request.Builder()
                .url(url)
                .apply {
                    if (transport != LiveTransport.WebSocket) {
                        header("Accept", if (transport == LiveTransport.Sse) "text/event-stream" else "application/primal-frame")
                        if (managed) header("X-Ewe-Hello", Base64.encodeToString(SubscriptionFrames.hello(0u), Base64.URL_SAFE or Base64.NO_PADDING or Base64.NO_WRAP))
                    }
                }
                .build()
        val closed = AtomicBoolean(false)
        fun deliver(bytes: ByteArray) {
            require(bytes.size <= maxFrameBytes)
            main.post { if (!closed.get()) onFrame(bytes) }
        }
        fun finish(error: Throwable?) {
            main.post { if (closed.compareAndSet(false, true)) onClose(error) }
        }
        if (transport == LiveTransport.WebSocket) {
            val ws =
                client.newWebSocket(
                    request,
                    object : WebSocketListener() {
                        override fun onOpen(webSocket: WebSocket, response: Response) { main.post { if (!closed.get()) onOpen() } }
                        override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
                            if (bytes.size > maxFrameBytes) {
                                webSocket.close(1009, "Frame too large")
                                finish(IOException("Frame too large"))
                            } else if (listening()) deliver(bytes.toByteArray())
                        }

                        override fun onClosed(webSocket: WebSocket, code: Int, reason: String) =
                            finish(null)

                        override fun onFailure(
                            webSocket: WebSocket,
                            t: Throwable,
                            response: Response?,
                        ) = finish(t)
                    },
                )
            return object : Channel {
                override fun send(bytes: ByteArray) {
                    if (closed.get() || !ws.send(bytes.toByteString())) throw IOException("WebSocket did not accept the message")
                }
                override fun close() { if (closed.compareAndSet(false, true)) ws.cancel() }
            }
        }
        require(transport == LiveTransport.Sse || transport == LiveTransport.Http)
        val transportClient =
            if (transport == LiveTransport.Sse || managed)
                client
                    .newBuilder()
                    .readTimeout(0, java.util.concurrent.TimeUnit.MILLISECONDS)
                    .build()
            else client
        val call = transportClient.newCall(request)
        call.enqueue(
            object : Callback {
                override fun onFailure(call: Call, e: IOException) = finish(e)

                override fun onResponse(call: Call, response: Response) {
                    try {
                        response.use { r ->
                            if (r.code == 410 && expired != null) {
                                deliver(expired())
                                return
                            }
                            if (!r.isSuccessful) throw IOException("HTTP ${r.code}")
                            main.post { if (!closed.get()) onOpen() }
                            val source = r.body.source()
                            if (transport == LiveTransport.Sse) {
                                val data = StringBuilder()
                                val maxEncoded = (maxFrameBytes.toLong() + 2) / 3 * 4
                                while (!source.exhausted()) {
                                    val line = source.readUtf8LineStrict(maxEncoded + 6)
                                    when {
                                        line.isEmpty() && data.isNotEmpty() -> {
                                            if (listening()) deliver(Base64.decode(data.toString(), Base64.DEFAULT))
                                            data.clear()
                                        }
                                        line.startsWith("data:") -> {
                                            val chunk = line.substring(5).trimStart()
                                            require(data.length + chunk.length <= maxEncoded)
                                            data.append(chunk)
                                        }
                                    }
                                }
                            } else
                                while (!source.exhausted()) {
                                    val length = source.readIntLe()
                                    require(length in 1..maxFrameBytes)
                                    if (listening()) deliver(source.readByteArray(length.toLong()))
                                    else source.skip(length.toLong())
                                }
                        }
                        finish(null)
                    } catch (error: Exception) {
                        finish(error)
                    }
                }
            }
        )
        fun controlFailed(error: IOException) {
            finish(error)
            call.cancel()
        }
        return object : Channel {
            override fun close() { if (closed.compareAndSet(false, true)) call.cancel() }
            override fun send(bytes: ByteArray) {
                require(managed && !closed.get())
                val request = Request.Builder().url(url).post(bytes.toRequestBody("application/primal-frame".toMediaType())).build()
                client.newCall(request).enqueue(object : Callback {
                    override fun onFailure(call: Call, e: IOException) { if (!closed.get()) controlFailed(e) }
                    override fun onResponse(call: Call, response: Response) {
                        response.use {
                            if (it.code == 410) deliver(SubscriptionFrames.expiredRequest(bytes))
                            else if (!it.isSuccessful && !closed.get()) controlFailed(IOException("Resume HTTP ${it.code}"))
                        }
                    }
                })
            }
        }
    }
}
