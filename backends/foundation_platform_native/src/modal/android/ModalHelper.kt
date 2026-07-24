package com.ewe.platform.native

import android.app.Activity
import android.os.Bundle
import android.view.ViewGroup
import android.webkit.WebView
import android.widget.FrameLayout
import com.google.android.material.bottomsheet.BottomSheetDialog
import com.google.android.material.bottomsheet.BottomSheetDialogFragment
import com.ewe.platform.generated.RustWebView

/**
 * F42: Native modal helper for Android.
 *
 * Creates either a BottomSheetDialogFragment or a full DialogFragment
 * with an embedded WebView loading the given route. Does NOT navigate
 * the main WebView — the dialog overlays on top.
 *
 * Usage from Rust via JNI:
 *   ModalHelper.presentModal(activity, "ewe://localhost/app/settings", "Settings", "bottom_sheet")
 *
 * Returns a ModalHandle that the Rust side can call dismiss() on.
 */
class ModalHelper(private val activity: Activity) {

    fun presentModal(
        url: String,
        title: String,
        style: String, // "bottom_sheet", "dialog", "fullscreen"
    ): ModalHandle {
        return when (style) {
            "bottom_sheet" -> presentBottomSheet(url, title)
            "dialog" -> presentDialog(url, title)
            else -> presentDialog(url, title)
        }
    }

    private fun presentBottomSheet(url: String, title: String): ModalHandle {
        val webView = createWebView(url)
        val dialog = BottomSheetDialog(activity)
        dialog.setContentView(webView)
        dialog.show()
        return ModalHandle { dialog.dismiss() }
    }

    private fun presentDialog(url: String, title: String): ModalHandle {
        val webView = createWebView(url)
        val dialog = android.app.AlertDialog.Builder(activity)
            .setView(webView)
            .setNegativeButton("Close") { d, _ -> d.dismiss() }
            .create()
        dialog.show()
        return ModalHandle { dialog.dismiss() }
    }

    private fun createWebView(url: String): WebView {
        val webView = WebView(activity)
        webView.layoutParams = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            600 * activity.resources.displayMetrics.density.toInt()
        )
        webView.settings.javaScriptEnabled = true
        webView.settings.domStorageEnabled = true
        webView.loadUrl(url)
        return webView
    }
}

/** Handle returned to Rust — calling dismiss() removes the dialog. */
class ModalHandle(private val onDismiss: () -> Unit) {
    fun dismiss() = onDismiss()
}
