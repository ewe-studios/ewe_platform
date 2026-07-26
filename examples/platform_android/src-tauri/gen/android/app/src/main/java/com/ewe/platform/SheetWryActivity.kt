/* THIS FILE IS AUTO-GENERATED. DO NOT MODIFY!! */

package com.ewe.platform

import android.view.Gravity
import android.view.ViewGroup
import android.view.WindowManager

/**
 * F44: Partial-height bottom sheet Activity.
 * Extends TauriActivity so getPluginManager() is available.
 *
 * Window layout is applied in onWebViewReady(), which fires after
 * the Activity and WebView are fully wired (IPC, scripts, clients)
 * but before setContentView — avoiding the flicker from Android
 * resetting layout params during Activity initialization.
 */
open class SheetWryActivity : TauriActivity() {
    override val handleBackNavigation: Boolean = false

    override fun onWebViewReady(webView: RustWebView) {
        val heightFrac = intent.getFloatExtra("height_fraction", 0.6f)
        if (heightFrac < 1.0f) {
            val pxHeight = (resources.displayMetrics.heightPixels * heightFrac).toInt()
            window.setLayout(ViewGroup.LayoutParams.MATCH_PARENT, pxHeight)
            window.setGravity(Gravity.BOTTOM)
            window.addFlags(WindowManager.LayoutParams.FLAG_DIM_BEHIND)
            window.setDimAmount(0.5f)
        }

        // Prevent white flicker: match WebView background to dashboard theme (#0a0a1a)
        webView.setBackgroundColor(0xFF0a0a1a.toInt())

        super.onWebViewReady(webView)
    }
}
