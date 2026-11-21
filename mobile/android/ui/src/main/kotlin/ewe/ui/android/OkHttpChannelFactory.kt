package ewe.ui.android

import android.os.Handler
import android.os.Looper
import android.util.Base64
import ewe.ui.LiveTransport
import ewe.ui.runtime.Channel
import ewe.ui.runtime.ChannelFactory
import java.io.IOException
import java.util.concurrent.atomic.AtomicBoolean
import okhttp3.*
import okio.ByteString

class OkHttpChannelFactory(
    private val client: OkHttpClient,
    private val baseUrl: HttpUrl,
    private val maxFrameBytes: Int = 4 * 1024 * 1024,
    private val main: Handler = Handler(Looper.getMainLooper()),
) : ChannelFactory {
    override fun open(
        endpoint: String,
        transport: UByte,
        streamId: ULong,
        resumeFrom: ULong,
        onFrame: (ByteArray) -> Unit,
        onClose: (Throwable?) -> Unit,
    ): Channel {
        val resource =
            baseUrl.resolve(endpoint.replaceFirst(Regex("^ws"), "http"))
                ?: error("Invalid endpoint")
        val url =
            resource
                .newBuilder()
                .setQueryParameter("stream_id", streamId.toString())
                .setQueryParameter("resume_from", resumeFrom.toString())
                .build()
        val request =
            Request.Builder()
                .url(url)
                .apply {
                    if (transport == LiveTransport.Sse) header("Accept", "text/event-stream")
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
                        override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
                            if (bytes.size > maxFrameBytes) {
                                webSocket.close(1009, "Frame too large")
                                finish(IOException("Frame too large"))
                            } else deliver(bytes.toByteArray())
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
            return Channel { if (closed.compareAndSet(false, true)) ws.cancel() }
        }
        require(transport == LiveTransport.Sse || transport == LiveTransport.Http)
        val transportClient =
            if (transport == LiveTransport.Sse)
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
                            if (!r.isSuccessful) throw IOException("HTTP ${r.code}")
                            val source = r.body.source()
                            if (transport == LiveTransport.Sse) {
                                val data = StringBuilder()
                                val maxEncoded = (maxFrameBytes.toLong() + 2) / 3 * 4
                                while (!source.exhausted()) {
                                    val line = source.readUtf8LineStrict(maxEncoded + 6)
                                    when {
                                        line.isEmpty() && data.isNotEmpty() -> {
                                            deliver(Base64.decode(data.toString(), Base64.DEFAULT))
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
                                    deliver(source.readByteArray(length.toLong()))
                                }
                        }
                        finish(null)
                    } catch (error: Exception) {
                        finish(error)
                    }
                }
            }
        )
        return Channel { if (closed.compareAndSet(false, true)) call.cancel() }
    }
}
