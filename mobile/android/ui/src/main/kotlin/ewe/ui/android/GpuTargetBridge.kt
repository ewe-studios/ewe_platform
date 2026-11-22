package ewe.ui.android

import android.view.Surface
import io.github.ewe_studios.ewe.core.FfiConverterTypeNativeDrawingTarget
import io.github.ewe_studios.ewe.core.NativeDrawingSurface
import io.github.ewe_studios.ewe.core.NativeGraphics

/** Retain an existing platform surface for the graphics owner. Dimensions use physical pixels. */
suspend fun NativeGraphics.attach(name: String, surface: Surface, width: UInt, height: UInt): NativeDrawingSurface {
    val handle = GpuTargetBridge.retain(surface)
    require(handle != 0L) { "The surface has no native window" }
    val target = FfiConverterTypeNativeDrawingTarget.lift(handle)
    return target.use { attach(name, it, width, height) }
}

internal object GpuTargetBridge {
    init {
        System.loadLibrary(System.getProperty("uniffi.component.foundation_ui_native.libraryOverride") ?: "foundation_ui_native")
    }
    external fun retain(surface: Surface): Long
}
