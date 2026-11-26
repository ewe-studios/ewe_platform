package ewe.ui.compose

import android.content.Context
import android.view.SurfaceHolder
import android.view.View
import android.view.SurfaceView
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import ewe.ui.android.attach
import io.github.ewe_studios.ewe.core.NativeDrawingSurface
import io.github.ewe_studios.ewe.core.NativeGraphics
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

/** Attach a native window when Android creates it. Keep the graphics service across window recreation. */
@Composable
fun EweDrawingSurface(name: String, graphics: () -> NativeGraphics, report: (Throwable) -> Unit, modifier: Modifier = Modifier, changed: (NativeDrawingSurface?) -> Unit = {}) {
    EweDrawingSurface(name, graphics, report, contentDescription = null, modifier = modifier, changed = changed)
}

/** Set the native surface label for Android accessibility services. */
@Composable
fun EweDrawingSurface(
    name: String,
    graphics: () -> NativeGraphics,
    report: (Throwable) -> Unit,
    contentDescription: String?,
    modifier: Modifier = Modifier,
    changed: (NativeDrawingSurface?) -> Unit = {},
) {
    key(name) {
        AndroidView(
            factory = { DrawingView(it, name, graphics, report, changed) },
            modifier = modifier,
            onRelease = { it.release() },
            update = { view ->
                view.contentDescription = contentDescription
                view.importantForAccessibility = if (contentDescription == null) {
                    View.IMPORTANT_FOR_ACCESSIBILITY_AUTO
                } else {
                    View.IMPORTANT_FOR_ACCESSIBILITY_YES
                }
            },
        )
    }
}

private class DrawingView(context: Context, private val name: String, private val graphics: () -> NativeGraphics,
    private val report: (Throwable) -> Unit, private val changed: (NativeDrawingSurface?) -> Unit) : SurfaceView(context), SurfaceHolder.Callback {
    private val scope = CoroutineScope(Dispatchers.Main.immediate)
    private var job: Job? = null
    private var drawing: NativeDrawingSurface? = null
    private var generation = 0L
    init { holder.addCallback(this) }
    private fun enqueue(block: suspend () -> Unit) {
        val before = job
        job = scope.launch {
            before?.join()
            try { block() } catch (e: Exception) { report(e) }
        }
    }
    override fun surfaceCreated(holder: SurfaceHolder) {
        val current = ++generation
        enqueue {
            if (current == generation && holder.surface.isValid) {
                drawing = graphics().attach(name, holder.surface, holder.surfaceFrame.width().toUInt(), holder.surfaceFrame.height().toUInt())
                changed(drawing)
            }
        }
    }
    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        val current = generation
        enqueue { if (current == generation) drawing?.resize(width.toUInt(), height.toUInt()) }
    }
    override fun surfaceDestroyed(holder: SurfaceHolder) { detach() }
    private fun detach() {
        changed(null)
        generation++
        enqueue {
            val previous = drawing
            drawing = null
            previous?.use { it.remove() }
        }
    }
    fun release() {
        holder.removeCallback(this)
        detach()
    }
}
