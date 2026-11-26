package ewe.ui.android

import io.github.ewe_studios.ewe.core.CoreException
import io.github.ewe_studios.ewe.core.HostHttpClient
import io.github.ewe_studios.ewe.core.HttpHeader
import io.github.ewe_studios.ewe.core.ResponseHead
import java.io.IOException
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import okhttp3.Call
import okhttp3.Callback
import okhttp3.MediaType.Companion.toMediaTypeOrNull
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response

/** Runs each Rust RPC exchange on the supplied application client. */
class OkHttpRpcClient(private val client: OkHttpClient) : HostHttpClient {
    private class Exchange(val method: String, val request: Request.Builder) {
        var call: Call? = null
        var response: Response? = null
        var closed = false
    }

    private val last = AtomicLong()
    private val exchanges = ConcurrentHashMap<ULong, Exchange>()

    override fun newExchange(method: String, url: String, headers: List<HttpHeader>): ULong {
        val builder = Request.Builder().url(url)
        headers.forEach { builder.addHeader(it.name, it.value) }
        val id = last.incrementAndGet().toULong()
        exchanges[id] = Exchange(method, builder)
        return id
    }

    private fun exchange(handle: ULong) = exchanges[handle] ?: throw CoreException.Transport("RPC exchange closed")

    override suspend fun start(handle: ULong, body: ByteArray): ResponseHead {
        val exchange = exchange(handle)
        return suspendCancellableCoroutine { pending ->
            val call = synchronized(exchange) {
                if (exchange.closed || exchange.call != null) throw CoreException.Transport("RPC exchange unavailable")
                val req = exchange.request.build()
                val payload = if (exchange.method == "GET" || exchange.method == "HEAD") null else
                    body.toRequestBody(req.header("Content-Type")?.toMediaTypeOrNull())
                client.newCall(req.newBuilder().method(exchange.method, payload).build()).also { exchange.call = it }
            }
            pending.invokeOnCancellation { call.cancel() }
            call.enqueue(object : Callback {
                override fun onFailure(call: Call, error: IOException) {
                    pending.resumeWithException(CoreException.Transport(error.message ?: "RPC request failed"))
                }
                override fun onResponse(call: Call, response: Response) {
                    synchronized(exchange) {
                        if (exchange.closed) {
                            response.close()
                            pending.resumeWithException(CoreException.Transport("RPC exchange closed"))
                            return
                        }
                        exchange.response = response
                    }
                    val headers = response.headers.map { (name, value) -> HttpHeader(name, value) }
                    pending.resume(ResponseHead(response.code.toUShort(), headers))
                }
            })
        }
    }

    override suspend fun read(handle: ULong, maxBytes: UInt): ByteArray? = withContext(Dispatchers.IO) {
        val exchange = exchange(handle)
        val response = synchronized(exchange) { exchange.response } ?: throw CoreException.Transport("Missing RPC response")
        val buffer = ByteArray(maxBytes.toInt())
        try {
            val size = response.body.byteStream().read(buffer)
            if (size < 0) null else buffer.copyOf(size)
        } catch (err: IOException) {
            throw CoreException.Transport(err.message ?: "RPC read failed")
        }
    }

    override fun close(handle: ULong) {
        val exchange = exchanges.remove(handle) ?: return
        synchronized(exchange) {
            exchange.closed = true
            exchange.call?.cancel()
            exchange.response?.close()
        }
    }
}
