package ewe.ui.compose

import android.graphics.Bitmap
import android.graphics.Color
import android.os.Handler
import android.os.Looper
import android.view.PixelCopy
import android.view.SurfaceView
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.test.platform.app.InstrumentationRegistry
import ewe.ui.android.attach
import io.github.ewe_studios.ewe.core.HostTransport
import io.github.ewe_studios.ewe.core.NativeCore
import io.github.ewe_studios.ewe.core.NativePlatform
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

class GraphicsTest {
    @get:Rule val compose = createComposeRule()

    @Test fun computeAndPresentOnTwoNativeWindows() = runBlocking {
        val shader = InstrumentationRegistry.getInstrumentation().context.assets.open("double.wgsl").bufferedReader().use { it.readText() }
        val core = NativeCore.remote(NativePlatform.ANDROID, emptyList(), object : HostTransport {
            override suspend fun send(request: ByteArray): ByteArray? = error("Graphics must remain local")
        })
        core.use {
            it.graphics().use { gpu ->
                android.util.Log.i("EweGraphicsTest", "Adapter: ${gpu.adapter()}")
                suspend fun compute() = assertEquals(listOf(1u, 5u, 15u), gpu.compute(shader, listOf(0u, 2u, 7u), 3u))
                compute()
                val views = mutableListOf<SurfaceView>()
                compose.setContent {
                    Row {
                        repeat(2) { AndroidView(factory = { context -> SurfaceView(context).also(views::add) }, modifier = Modifier.size(64.dp)) }
                    }
                }
                compose.waitUntil(10000) { views.size == 2 && views.all { v -> v.holder.surface.isValid && v.width > 0 } }
                val first = views[0]
                val second = views[1]
                suspend fun attach(name: String, view: SurfaceView) = gpu.attach(name, view.holder.surface, view.width.toUInt(), view.height.toUInt())
                suspend fun pixel(v: SurfaceView): Int {
                    val buffer = Bitmap.createBitmap(v.width, v.height, Bitmap.Config.ARGB_8888)
                    val status = CompletableDeferred<Int>()
                    PixelCopy.request(v, buffer, { result -> status.complete(result) }, Handler(Looper.getMainLooper()))
                    assertEquals(PixelCopy.SUCCESS, withTimeout(5000) { status.await() })
                    return buffer.getPixel(v.width/2, v.height/2)
                }
                attach("first", first).use { a ->
                    attach("second", second).use { b ->
                        a.resize(0u, 0u)
                        check(runCatching { a.clear(0.0,1.0,0.0,1.0) }.isFailure)
                        a.resize(first.width.toUInt(), first.height.toUInt())
                        a.clear(0.0,1.0,0.0,1.0)
                        assertEquals(Color.GREEN, pixel(first))
                        a.remove()
                        check(runCatching { a.clear(0.0,1.0,0.0,1.0) }.isFailure)
                        attach("first", first).use { c ->
                            c.clear(1.0,0.0,0.0,1.0)
                            b.clear(0.0,0.0,1.0,1.0)
                            assertEquals(Color.RED, pixel(first))
                            assertEquals(Color.BLUE, pixel(second))
                            compute()
                            c.remove()
                        }
                        b.remove()
                    }
                }
            }
        }
    }
}
