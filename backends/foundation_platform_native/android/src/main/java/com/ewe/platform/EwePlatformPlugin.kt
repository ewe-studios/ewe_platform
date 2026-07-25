package com.ewe.platform

import android.app.Activity
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

/**
 * F42: Tauri plugin for foundation_platform native capabilities.
 *
 * Commands are called by Rust via `PluginHandle::run_mobile_plugin()`.
 * For `presentModal`, Rust creates a child Tauri WebView via
 * `Window::add_child(WebviewBuilder)`, then passes the label here so
 * Kotlin can find the WebView in the Activity hierarchy and embed it
 * in a BottomSheetDialog or AlertDialog.
 */

@InvokeArg
class PresentModalArgs {
    lateinit var url: String
    var title: String? = null
    var style: String? = "bottom_sheet"
}

@InvokeArg
class DismissModalArgs {
    lateinit var modalId: String
}

@InvokeArg
class ShowDialogArgs {
    lateinit var title: String
    var message: String? = null
    var positiveButton: String? = null
    var negativeButton: String? = null
    var route: String? = null
}

@TauriPlugin
class EwePlatformPlugin(activity: Activity) : Plugin(activity) {
    private val modalHelper = ModalHelper(activity)
    private val activeModals = mutableMapOf<String, ModalHandle>()
    private val activeDialogs = mutableMapOf<String, ModalHandle>()

    @Command
    fun presentModal(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(PresentModalArgs::class.java)
            val handle = modalHelper.present(
                url = args.url,
                title = args.title ?: "",
                style = args.style ?: "bottom_sheet"
            )
            activeModals[handle.id] = handle
            val response = JSObject()
            response.put("modal_id", handle.id)
            response.put("webview_label", handle.id)
            invoke.resolve(response)
        } catch (ex: Exception) {
            invoke.reject("presentModal failed: ${ex.message}")
        }
    }

    @Command
    fun dismissModal(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(DismissModalArgs::class.java)
            activeModals.remove(args.modalId)?.dismiss()
            invoke.resolve()
        } catch (ex: Exception) {
            invoke.reject("dismissModal failed: ${ex.message}")
        }
    }

    @Command
    fun dismissAllModals(invoke: Invoke) {
        try {
            activeModals.values.forEach { it.dismiss() }
            activeModals.clear()
            invoke.resolve()
        } catch (ex: Exception) {
            invoke.reject("dismissAllModals failed: ${ex.message}")
        }
    }

    @Command
    fun showDialog(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(ShowDialogArgs::class.java)
            val handle = modalHelper.presentTextDialog(
                title = args.title,
                message = args.message ?: "",
                positiveButton = args.positiveButton,
                negativeButton = args.negativeButton
            )
            activeDialogs[handle.id] = handle
            val response = JSObject()
            response.put("dialog_id", handle.id)
            invoke.resolve(response)
        } catch (ex: Exception) {
            invoke.reject("showDialog failed: ${ex.message}")
        }
    }

    @Command
    fun dismissDialog(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(DismissModalArgs::class.java)
            activeDialogs.remove(args.modalId)?.dismiss()
            invoke.resolve()
        } catch (ex: Exception) {
            invoke.reject("dismissDialog failed: ${ex.message}")
        }
    }
}
