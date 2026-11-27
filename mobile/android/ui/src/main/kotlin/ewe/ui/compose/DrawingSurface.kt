package ewe.ui.compose

import android.content.Context
import android.view.SurfaceHolder
import android.view.View
import android.view.SurfaceView
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import ewe.ui.android.retainDrawingTarget
import ewe.ui.android.retire
import io.github.ewe_studios.ewe.core.NativeDrawingSurface
import io.github.ewe_studios.ewe.core.NativeDrawingTarget
import io.github.ewe_studios.ewe.core.GraphicsException
import io.github.ewe_studios.ewe.core.NativeGraphics
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

/** Close a drawing view before removing it from composition. Use one control per view. */
class EweDrawingSurfaceControl {
    internal var view: DrawingView? = null
    internal var isClosed = false
        private set

    /** Draw on the current window. Return false when the window changes. Call on the main thread. */
    suspend fun draw(frame: suspend (NativeDrawingSurface) -> Unit): Boolean =
        view?.draw(frame) ?: false

    /** Stop drawing and wait for the graphics owner to release the window. Call on the main thread. */
    suspend fun close() {
        isClosed = true
        view?.detach()?.join()
    }
}

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
) = EweDrawingSurface(name, graphics, report, contentDescription, null, modifier, changed)

/** Connect a drawing view to an explicit close operation. */
@Composable
fun EweDrawingSurface(
    name: String,
    graphics: () -> NativeGraphics,
    report: (Throwable) -> Unit,
    contentDescription: String?,
    control: EweDrawingSurfaceControl?,
    modifier: Modifier = Modifier,
    changed: (NativeDrawingSurface?) -> Unit = {},
) {
    val currentGraphics = rememberUpdatedState(graphics)
    val currentReport = rememberUpdatedState(report)
    val currentChanged = rememberUpdatedState(changed)
    key(name, control) {
        AndroidView(
            factory = {
                DrawingView(
                    it, name, { currentGraphics.value() }, { currentReport.value(it) },
                    { currentChanged.value(it) }, control,
                )
            },
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

internal class DrawingView(
    context: Context,
    private val name: String,
    private val graphics: () -> NativeGraphics,
    private val report: (Throwable) -> Unit,
    private val changed: (NativeDrawingSurface?) -> Unit,
    private val control: EweDrawingSurfaceControl?,
) : SurfaceView(context), SurfaceHolder.Callback {
    private class Window(val target: NativeDrawingTarget) {
        var drawing: NativeDrawingSurface? = null
    }

    // This scope owns cleanup independently of the caller waiting for close.
    private val scope = CoroutineScope(Dispatchers.Main.immediate)
    private var job: Job? = null
    private var window: Window? = null

    init {
        control?.view = this
        holder.addCallback(this)
    }

    private fun enqueue(block: suspend () -> Unit): Job {
        val before = job
        val next = scope.launch(start = CoroutineStart.LAZY) {
            before?.join()
            try {
                block()
            } catch (cancel: CancellationException) {
                throw cancel
            } catch (failure: Exception) {
                report(failure)
            }
        }
        job = next
        next.start()
        return next
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        if (control?.isClosed == true || !holder.surface.isValid) return
        detach()
        val current = try {
            Window(retainDrawingTarget(holder.surface))
        } catch (failure: Exception) {
            report(failure)
            return
        }
        window = current
        val width = holder.surfaceFrame.width().toUInt()
        val height = holder.surfaceFrame.height().toUInt()
        enqueue {
            if (window === current) {
                try {
                    current.drawing = graphics().attach(name, current.target, width, height)
                    if (window === current) changed(current.drawing)
                } catch (stale: GraphicsException.StaleSurface) {
                    if (window === current) throw stale
                }
            }
        }
    }

    suspend fun draw(frame: suspend (NativeDrawingSurface) -> Unit): Boolean {
        val current = window ?: return false
        val drawing = current.drawing ?: return false
        try {
            frame(drawing)
        } catch (cancel: CancellationException) {
            throw cancel
        } catch (failure: Exception) {
            if (window === current) throw failure
        }
        return window === current
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        val current = window ?: return
        enqueue {
            if (window === current) {
                try {
                    current.drawing?.resize(width.toUInt(), height.toUInt())
                } catch (stale: GraphicsException.StaleSurface) {
                    if (window === current) throw stale
                }
            }
        }
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        detach()
    }

    fun detach(): Job? {
        val previous = window ?: return job
        window = null
        previous.target.retire()
        val release = enqueue {
            previous.target.use {
                previous.drawing?.use {
                    try {
                        it.remove()
                    } catch (_: GraphicsException.StaleSurface) {
                        // A replacement already released this window on the graphics owner.
                    }
                }
            }
        }
        if (control == null || control.view === this) changed(null)
        return release
    }

    fun release() {
        holder.removeCallback(this)
        detach()
        if (control?.view === this) control.view = null
    }
}
