package com.ewe.platform

import android.app.Activity
import android.view.ViewGroup
import android.webkit.WebView
import android.widget.FrameLayout
import com.google.android.material.bottomsheet.BottomSheetDialog

/**
 * F42: Native modal helper — BottomSheet + AlertDialog with embedded WebView.
 *
 * Finds the **Tauri WebView** from the Activity view hierarchy via
 * `findWebViewInstance`, detaches it from the decor view, embeds it
 * in a `BottomSheetDialog` or `AlertDialog`, and re-attaches on dismiss.
 *
 * The embedded WebView retains `__TAURI_INTERNALS__`, VFS asset loading,
 * init scripts, and the Rust IPC bridge — it IS the Tauri WebView,
 * just reparented into the dialog for the duration of the modal.
 * The main dashboard is hidden behind the modal during this time.
 */
class ModalHelper(private val activity: Activity) {

    private var originalParent: ViewGroup? = null
    private var originalIndex: Int = -1

    /**
     * Present a modal with the **Tauri WebView** embedded inside.
     * Detaches the WebView from the decor view, embeds it in a dialog,
     * and re-attaches when the dialog is dismissed.
     */
    fun present(url: String, title: String, style: String): ModalHandle {
        val webView = detachTauriWebView()
        webView.loadUrl(url)

        val handle: ModalHandle = when (style) {
            "bottom_sheet" -> {
                val dialog = BottomSheetDialog(activity)
                dialog.setContentView(webView)
                dialog.setOnShowListener {
                    dialog.behavior.peekHeight =
                        (activity.resources.displayMetrics.heightPixels * 0.6).toInt()
                    dialog.behavior.isDraggable = true
                }
                dialog.setOnDismissListener { reattachTauriWebView(webView) }
                dialog.show()
                ModalHandle(dialog.hashCode().toString()) { dialog.dismiss() }
            }
            "dialog" -> {
                val dialog = android.app.AlertDialog.Builder(activity)
                    .setTitle(title)
                    .setView(webView)
                    .setNegativeButton("Close") { d, _ -> d.dismiss() }
                    .setOnDismissListener { reattachTauriWebView(webView) }
                    .create()
                dialog.show()
                ModalHandle(dialog.hashCode().toString()) { dialog.dismiss() }
            }
            "fullscreen" -> {
                // Full-screen — re-attach directly, no dialog container.
                reattachTauriWebView(webView)
                ModalHandle("fullscreen") { /* no-op — already reattached */ }
            }
            else -> {
                // Default to dialog with WebView
                val dialog = android.app.AlertDialog.Builder(activity)
                    .setTitle(title)
                    .setView(webView)
                    .setNegativeButton("Close") { d, _ -> d.dismiss() }
                    .setOnDismissListener { reattachTauriWebView(webView) }
                    .create()
                dialog.show()
                ModalHandle(dialog.hashCode().toString()) { dialog.dismiss() }
            }
        }

        return handle
    }

    /**
     * Present a native AlertDialog (text-only, no WebView).
     * Does NOT detach the Tauri WebView — the dashboard stays visible.
     */
    fun presentTextDialog(
        title: String, message: String,
        positiveButton: String?, negativeButton: String?
    ): ModalHandle {
        val builder = android.app.AlertDialog.Builder(activity).setTitle(title)
        if (!message.isNullOrEmpty()) builder.setMessage(message)
        if (positiveButton != null) builder.setPositiveButton(positiveButton) { d, _ -> d.dismiss() }
        if (negativeButton != null) builder.setNegativeButton(negativeButton) { d, _ -> d.dismiss() }
        val dialog = builder.create()
        dialog.show()
        return ModalHandle(dialog.hashCode().toString()) { dialog.dismiss() }
    }

    // ── Detach / Re-attach ─────────────────────────────────────────────

    /**
     * Find the Tauri WebView in the Activity hierarchy and detach it.
     * The WebView retains all Tauri integrations (`__TAURI_INTERNALS__`,
     * init scripts, VFS loading, IPC bridge) because we're just moving
     * the view reference, not creating a new WebView.
     */
    private fun detachTauriWebView(): WebView {
        val root = activity.window.decorView as ViewGroup
        val webView = findWebViewInstance(root)
            ?: throw IllegalStateException(
                "No WebView found in Activity hierarchy. " +
                "Is the Tauri WebView initialized?"
            )

        originalParent = webView.parent as? ViewGroup
        originalIndex = originalParent?.indexOfChild(webView) ?: -1
        originalParent?.removeView(webView)

        val dp = activity.resources.displayMetrics
        webView.layoutParams = FrameLayout.LayoutParams(
            dp.widthPixels,
            (dp.heightPixels * 0.75).toInt()
        )
        return webView
    }

    /**
     * Re-attach the Tauri WebView back into the Activity decor view.
     * Restores to the saved parent and index position.
     */
    private fun reattachTauriWebView(webView: WebView) {
        // If already reattached (or was never detached), skip.
        if (webView.parent != null) return

        val parent = originalParent
        if (parent == null) {
            // Fallback: attach to decor view root
            val root = activity.window.decorView as? ViewGroup ?: return
            webView.post {
                val dp = activity.resources.displayMetrics
                root.addView(webView, FrameLayout.LayoutParams(
                    dp.widthPixels, dp.heightPixels
                ))
                root.requestLayout()
            }
            return
        }

        webView.post {
            val dp = activity.resources.displayMetrics
            val lp = FrameLayout.LayoutParams(dp.widthPixels, dp.heightPixels)
            if (originalIndex in 0 until parent.childCount) {
                parent.addView(webView, originalIndex, lp)
            } else {
                parent.addView(webView, lp)
            }
            parent.requestLayout()
        }
    }

    companion object {
        /**
         * Recursively search the view hierarchy for the first WebView.
         * Tauri's `RustWebView` extends `android.webkit.WebView` and
         * is placed as a child of the decor view during window creation.
         */
        fun findWebViewInstance(parent: ViewGroup): WebView? {
            for (i in 0 until parent.childCount) {
                val child = parent.getChildAt(i)
                if (child is WebView) return child
                if (child is ViewGroup) {
                    val found = findWebViewInstance(child)
                    if (found != null) return found
                }
            }
            return null
        }
    }
}

/** Handle to an active modal — allows programmatic dismiss. */
data class ModalHandle(
    val id: String,
    private val onDismiss: () -> Unit
) {
    fun dismiss() = onDismiss()
}
