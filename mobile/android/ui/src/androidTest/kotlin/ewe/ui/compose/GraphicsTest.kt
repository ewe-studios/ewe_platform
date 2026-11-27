package ewe.ui.compose

import android.graphics.Bitmap
import android.graphics.Color
import android.os.Handler
import android.os.Looper
import android.view.PixelCopy
import android.view.SurfaceView
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
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
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.withContext
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class GraphicsTest {
    @get:Rule val compose = createComposeRule()

    private suspend fun pixel(view: SurfaceView): Int {
        val buffer = Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)
        try {
            val status = CompletableDeferred<Int>()
            PixelCopy.request(view, buffer, { status.complete(it) }, Handler(Looper.getMainLooper()))
            assertEquals(PixelCopy.SUCCESS, withTimeout(5000) { status.await() })
            return buffer.getPixel(view.width / 2, view.height / 2)
        } finally {
            buffer.recycle()
        }
    }

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

    @Test fun sameNameWindowDrawsAfterReplacementDuringAFrame() = runBlocking {
        val core = NativeCore.remote(NativePlatform.ANDROID, emptyList(), object : HostTransport {
            override suspend fun send(request: ByteArray): ByteArray? = error("Graphics must remain local")
        })
        core.use {
            it.graphics().use { gpu ->
                val generation = mutableIntStateOf(0)
                val controls = List(2) { EweDrawingSurfaceControl() }
                val ready = List(2) { CompletableDeferred<Unit>() }
                val errors = mutableListOf<Throwable>()
                compose.setContent {
                    val current = generation.intValue
                    key(current) {
                        EweDrawingSurface(
                            "replaceable", { gpu }, { errors.add(it) },
                            contentDescription = null, control = controls[current],
                            modifier = Modifier.size(64.dp),
                            changed = { if (it != null) ready[current].complete(Unit) },
                        )
                    }
                }
                fun awaitWindow(index: Int) {
                    compose.waitUntil(conditionDescription = "Drawing window ${index + 1}", timeoutMillis = 10000) {
                        ready[index].isCompleted || errors.isNotEmpty()
                    }
                    compose.runOnIdle { assertTrue(errors.toString(), errors.isEmpty()) }
                }
                awaitWindow(0)
                val firstFrame = CompletableDeferred<Unit>()
                val continueFrame = CompletableDeferred<Unit>()
                val old = async(Dispatchers.Main) {
                    controls[0].draw { surface ->
                        surface.clear(0.0, 1.0, 0.0, 1.0)
                        firstFrame.complete(Unit)
                        continueFrame.await()
                        surface.clear(0.0, 0.0, 1.0, 1.0)
                    }
                }
                try {
                    withTimeout(10000) { firstFrame.await() }
                    compose.runOnIdle { generation.intValue = 1 }
                    awaitWindow(1)
                    assertTrue(withContext(Dispatchers.Main) {
                        controls[1].draw { it.clear(1.0, 0.0, 0.0, 1.0) }
                    })
                    continueFrame.complete(Unit)
                    assertFalse(withTimeout(10000) { old.await() })
                    val replacement = withContext(Dispatchers.Main) { checkNotNull(controls[1].view) }
                    assertEquals(Color.RED, pixel(replacement))
                    withTimeout(10000) {
                        withContext(Dispatchers.Main) {
                            controls[1].close()
                            controls[1].close()
                            assertFalse(controls[1].draw { error("A closed view accepted a frame") })
                            assertTrue(errors.toString(), errors.isEmpty())
                        }
                    }
                } finally {
                    continueFrame.complete(Unit)
                }
            }
        }
    }
}
