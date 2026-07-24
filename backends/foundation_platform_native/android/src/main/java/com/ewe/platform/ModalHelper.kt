package com.ewe.platform

import android.app.Activity
import android.view.ViewGroup
import android.webkit.WebView
import android.widget.FrameLayout
import com.google.android.material.bottomsheet.BottomSheetDialog

/**
 * F42: Native modal helper — BottomSheet + AlertDialog with embedded WebView.
 *
 * Called by the Tauri plugin bridge when Rust sends `presentModal` /
 * `dismissModal` commands. Does NOT navigate the main WebView — the
 * dialog overlays on top of the existing Activity.
 *
 * Usage from EwePlatformPlugin.kt:
 *   @Command fun presentModal(invoke: Invoke) {
 *       val args = invoke.parseArgs(PresentModalArgs::class.java)
 *       val handle = ModalHelper(activity).present(args)
 *       invoke.resolve(mapOf("modal_id" to handle.id))
 *   }
 */
class ModalHelper(private val activity: Activity) {

    /** Present a modal dialog with an embedded WebView loading the given URL. */
    fun present(url: String, title: String, style: String): ModalHandle {
        return when (style) {
            "bottom_sheet" -> presentBottomSheet(url, title)
            "dialog" -> presentDialog(url, title)
            "fullscreen" -> presentDialog(url, title) // TODO: new Activity
            else -> presentDialog(url, title)
        }
    }

    private fun presentBottomSheet(url: String, title: String): ModalHandle {
        val webView = createWebView(url)
        val dialog = BottomSheetDialog(activity)
        dialog.setContentView(webView)
        dialog.setOnShowListener {
            // Optionally set peek height
            dialog.behavior.peekHeight = (activity.resources.displayMetrics.heightPixels * 0.6).toInt()
            dialog.behavior.isDraggable = true
        }
        dialog.show()
        return ModalHandle(dialog.hashCode().toString()) { dialog.dismiss() }
    }

    private fun presentDialog(url: String, title: String): ModalHandle {
        val webView = createWebView(url)
        val dialog = android.app.AlertDialog.Builder(activity)
            .setTitle(title)
            .setView(webView)
            .setNegativeButton("Close") { d, _ -> d.dismiss() }
            .create()
        dialog.show()
        return ModalHandle(dialog.hashCode().toString()) { dialog.dismiss() }
    }

    /** Create a WebView suitable for embedding in a dialog. */
    private fun createWebView(url: String): WebView {
        val webView = WebView(activity)
        webView.settings.javaScriptEnabled = true
        webView.settings.domStorageEnabled = true
        webView.loadUrl(url)
        val displayMetrics = activity.resources.displayMetrics
        webView.layoutParams = FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            (displayMetrics.heightPixels * 0.75).toInt()
        )
        return webView
    }
}

/** Handle to an active modal — allows programmatic dismiss. */
data class ModalHandle(
    val id: String,
    private val onDismiss: () -> Unit
) {
    fun dismiss() = onDismiss()
}
