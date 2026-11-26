package ewe.ui.compose

import android.graphics.Bitmap
import android.graphics.Color
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.view.PixelCopy
import android.view.Surface
import android.view.SurfaceView
import androidx.compose.foundation.layout.size
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.test.platform.app.InstrumentationRegistry
import io.github.ewe_studios.ewe.core.FfiConverterTypeNativeDeviceGrant
import io.github.ewe_studios.ewe.core.GraphicsException
import io.github.ewe_studios.ewe.core.HostDeviceCompletion
import io.github.ewe_studios.ewe.core.HostDeviceProvider
import io.github.ewe_studios.ewe.core.HostTransport
import io.github.ewe_studios.ewe.core.NativeCore
import io.github.ewe_studios.ewe.core.NativeDeviceRequest
import io.github.ewe_studios.ewe.core.NativePendingLease
import io.github.ewe_studios.ewe.core.NativePlatform
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

internal object VulkanTestHost {
    init { System.loadLibrary("ewe_gpu_test_host") }
    external fun open(sdk: Int): Long
    external fun grant(host: Long, texture: Boolean): Long
    external fun unsupportedGrant(host: Long, texture: Boolean): Long
    external fun pixel(host: Long): Int
    external fun present(host: Long, surface: Surface)
    external fun destroy(host: Long)
}

class HostGraphicsTest {
    @get:Rule val compose = createComposeRule()

    // WHY: The host must resume use of its original device after Rust finishes.
    // WHAT: Read the processed texture and present it on the host's native window.
    @Test fun processHostOwnedVulkanTextureThroughLocalAction() = runBlocking {
        val core = NativeCore.remote(NativePlatform.ANDROID, listOf("foundation_gpu_ready"), object : HostTransport {
            override suspend fun send(request: ByteArray): ByteArray? = error("A graphics grant reached the network")
        })
        val inbox = Channel<NativeDeviceRequest>(Channel.UNLIMITED)
        val provider = object : HostDeviceProvider {
            override fun request(request: NativeDeviceRequest) { inbox.trySend(request).getOrThrow() }
        }
        val assets = InstrumentationRegistry.getInstrumentation().context.assets
        fun source(name: String) = assets.open(name).bufferedReader().use { it.readText() }
        core.use {
            check(runCatching { it.installDeviceProvider("server:foundation_gpu_ready", provider) }.isFailure)
            it.installDeviceProvider("host:foundation_gpu_ready", provider)
            val host = VulkanTestHost.open(Build.VERSION.SDK_INT)
            try {
                suspend fun requestDevice(
                    surface: String?,
                    makeGrant: (Long, Boolean) -> Long = VulkanTestHost::grant,
                    verify: suspend (NativePendingLease) -> Unit,
                ) {
                    val returned = CompletableDeferred<Boolean>()
                    it.requestHostDevice("editor", surface).use { pending ->
                        withTimeout(10_000) { inbox.receive() }.use { request ->
                            val raw = makeGrant(host, surface != null)
                            FfiConverterTypeNativeDeviceGrant.lift(raw).use { grant ->
                                request.ready(grant, 1uL, object : HostDeviceCompletion {
                                    override fun released(deviceLost: Boolean) { returned.complete(deviceLost) }
                                })
                                try {
                                    verify(pending)
                                } finally {
                                    assertEquals(false, withTimeout(10_000) { returned.await() })
                                }
                            }
                        }
                    }
                }
                assertEquals(Color.RED, VulkanTestHost.pixel(host))
                requestDevice("preview") { pending ->
                    pending.wait().use { lease -> lease.processTexture(source("red_to_blue.wgsl"), 1u, 1u, 1u) }
                }
                assertEquals(Color.BLUE, VulkanTestHost.pixel(host))
                assertEquals(Color.BLUE, presentedPixel(host))
                requestDevice(null) { pending ->
                    pending.wait().use { lease ->
                        assertEquals(listOf(1u, 5u, 15u), lease.compute(source("double.wgsl"), listOf(0u, 2u, 7u), 3u))
                    }
                }
                requestDevice(null, VulkanTestHost::unsupportedGrant) { pending ->
                    val failure = runCatching { pending.wait().close() }.exceptionOrNull()
                    assertTrue("Expected an incompatible queue error, received $failure", failure is GraphicsException.IncompatibleSurface)
                }
            } finally {
                VulkanTestHost.destroy(host)
            }
        }
    }

    private suspend fun presentedPixel(host: Long): Int {
        var created: SurfaceView? = null
        compose.setContent {
            AndroidView(factory = { context -> SurfaceView(context).also { created = it } }, modifier = Modifier.size(64.dp))
        }
        compose.waitUntil(10_000) { created?.let { it.holder.surface.isValid && it.width > 0 } == true }
        val view = checkNotNull(created)
        VulkanTestHost.present(host, view.holder.surface)
        val bitmap = Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)
        val status = CompletableDeferred<Int>()
        PixelCopy.request(view, bitmap, { status.complete(it) }, Handler(Looper.getMainLooper()))
        assertEquals(PixelCopy.SUCCESS, withTimeout(10_000) { status.await() })
        return bitmap.getPixel(view.width / 2, view.height / 2)
    }
}
