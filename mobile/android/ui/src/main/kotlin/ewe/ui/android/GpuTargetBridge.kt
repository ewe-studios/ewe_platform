package ewe.ui.android

import android.view.Surface
import io.github.ewe_studios.ewe.core.FfiConverterTypeNativeDrawingTarget
import io.github.ewe_studios.ewe.core.NativeDrawingTarget
import io.github.ewe_studios.ewe.core.NativeDrawingSurface
import io.github.ewe_studios.ewe.core.NativeGraphics

/** Retain an existing platform surface for the graphics owner. Dimensions use physical pixels. */
suspend fun NativeGraphics.attach(name: String, surface: Surface, width: UInt, height: UInt): NativeDrawingSurface {
    return retainDrawingTarget(surface).use { attach(name, it, width, height) }
}

internal fun retainDrawingTarget(surface: Surface): NativeDrawingTarget {
    val handle = GpuTargetBridge.retain(surface)
    require(handle != 0L) { "The surface has no native window" }
    return FfiConverterTypeNativeDrawingTarget.lift(handle)
}

internal fun NativeDrawingTarget.retire() = GpuTargetBridge.retire(uniffiCloneHandle())

internal object GpuTargetBridge {
    init {
        System.loadLibrary(System.getProperty("uniffi.component.foundation_ui_native.libraryOverride") ?: "foundation_ui_native")
    }
    external fun retain(surface: Surface): Long
    external fun retire(handle: Long)
}
