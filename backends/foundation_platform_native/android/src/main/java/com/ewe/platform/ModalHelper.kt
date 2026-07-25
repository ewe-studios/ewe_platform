package com.ewe.platform

import android.app.Activity
import android.view.ViewGroup
import android.webkit.WebView
import android.widget.FrameLayout
import com.google.android.material.bottomsheet.BottomSheetDialog

/**
 * F42: Native modal helper — BottomSheet + AlertDialog with embedded WebView.
 *
 * TODO: Use `RustWebView` once the generated class is accessible from the
 * plugin module (currently lives in :app generated/). Until then, the plain
 * WebView loads pages via `ewe://` protocol but lacks `__TAURI_INTERNALS__`
 * and the Tauri IPC bridge.
 */
class ModalHelper(private val activity: Activity) {

    fun present(url: String, title: String, style: String): ModalHandle {
        return when (style) {
            "bottom_sheet" -> presentBottomSheet(url, title)
            "dialog" -> presentDialog(url, title)
            "fullscreen" -> presentDialog(url, title)
            else -> presentDialog(url, title)
        }
    }

    fun presentTextDialog(
        title: String,
        message: String,
        positiveButton: String?,
        negativeButton: String?
    ): ModalHandle {
        val builder = android.app.AlertDialog.Builder(activity)
            .setTitle(title)
        if (!message.isNullOrEmpty()) builder.setMessage(message)
        if (positiveButton != null) builder.setPositiveButton(positiveButton) { d, _ -> d.dismiss() }
        if (negativeButton != null) builder.setNegativeButton(negativeButton) { d, _ -> d.dismiss() }
        val dialog = builder.create()
        dialog.show()
        return ModalHandle(dialog.hashCode().toString()) { dialog.dismiss() }
    }

    private fun presentBottomSheet(url: String, title: String): ModalHandle {
        val webView = createWebView(url)
        val dialog = BottomSheetDialog(activity)
        dialog.setContentView(webView)
        dialog.setOnShowListener {
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
