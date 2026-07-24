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

@TauriPlugin
class EwePlatformPlugin(activity: Activity) : Plugin(activity) {
    private val modalHelper = ModalHelper(activity)
    private val activeModals = mutableMapOf<String, ModalHandle>()

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
}
