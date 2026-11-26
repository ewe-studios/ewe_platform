package ewe.ui.android

import android.util.Base64
import ewe.ui.runtime.ActionCore
import io.github.ewe_studios.ewe.core.HostTransport
import java.io.IOException
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.suspendCancellableCoroutine
import okhttp3.Call
import okhttp3.Callback
import okhttp3.HttpUrl
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response

/** Sends one HTTP action and returns its complete reply to the native core. */
class HttpActionConnection(
    private val client: OkHttpClient,
    private val base: HttpUrl,
    private val actionPath: String = "action",
    private val maxFrameBytes: Int = 4 * 1024 * 1024,
) : HostTransport {
    private lateinit var hello: ByteArray

    fun configure(frame: ByteArray) { hello = frame.copyOf() }

    suspend fun negotiate(core: ActionCore): ByteArray {
        hello = core.hello()
        return post(hello)
    }

    override suspend fun send(request: ByteArray): ByteArray = post(request)

    suspend fun initial(path: String): ByteArray = exchange(path, null)

    private suspend fun post(frame: ByteArray) = exchange(actionPath, frame)

    private suspend fun exchange(path: String, frame: ByteArray?): ByteArray =
        suspendCancellableCoroutine { pending ->
            val request = Request.Builder().url(requireNotNull(base.resolve(path)))
            request.header(
                "X-Ewe-Hello",
                Base64.encodeToString(hello, Base64.NO_PADDING or Base64.NO_WRAP or Base64.URL_SAFE),
            )
            if (frame != null) request.post(frame.toRequestBody("application/primal-frame".toMediaType()))
            else request.header("Accept", "application/vnd.ewe.frame")
            val call = client.newCall(request.build())
            pending.invokeOnCancellation { call.cancel() }
            call.enqueue(object : Callback {
                override fun onFailure(call: Call, error: IOException) {
                    pending.resumeWithException(error)
                }

                override fun onResponse(call: Call, response: Response) {
                    val result = runCatching { response.use { readFrame(it, frame != null) } }
                    pending.resumeWith(result)
                }
            })
        }

    private fun readFrame(response: Response, action: Boolean): ByteArray {
        check(response.isSuccessful) { "HTTP ${response.code}" }
        val input = response.body.source()
        if (action) {
            val size = input.readIntLe()
            require(size in 1..maxFrameBytes) { "Invalid action reply length" }
            return input.readByteArray(size.toLong())
        }
        // The initial view is one frame. Check its length before copying it.
        val length = response.body.contentLength()
        if (length >= 0) {
            require(length in 1..maxFrameBytes.toLong()) { "Invalid initial view length" }
            return input.readByteArray(length)
        }
        require(!input.request(maxFrameBytes.toLong() + 1)) { "Initial view exceeds limit" }
        return input.readByteArray()
    }
}
