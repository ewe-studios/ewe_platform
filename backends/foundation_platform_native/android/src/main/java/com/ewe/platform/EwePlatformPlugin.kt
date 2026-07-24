package com.ewe.platform

import android.app.Activity
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin

/**
 * F42: Base Tauri plugin for all foundation_platform native capabilities.
 *
 * Each @Command method is called by PluginManager.runCommand() from the
 * Rust side. The command name matches the @Command method name.
 *
 * Registered via tauri_plugin::Builder in build.rs. The Kotlin source
 * is auto-injected into the app's Gradle project as a library module.
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

    override fun load(webView: WebView, config: String) {
        // Plugin initialized — config is JSON from Rust Plugin::initialize()
    }

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
            val response = org.json.JSONObject()
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
            val response = org.json.JSONObject()
            response.put("ok", true)
            invoke.resolve(response)
        } catch (ex: Exception) {
            invoke.reject("dismissModal failed: ${ex.message}")
        }
    }

    @Command
    fun dismissAllModals(invoke: Invoke) {
        try {
            activeModals.values.forEach { it.dismiss() }
            activeModals.clear()
            val response = org.json.JSONObject()
            response.put("ok", true)
            invoke.resolve(response)
        } catch (ex: Exception) {
            invoke.reject("dismissAllModals failed: ${ex.message}")
        }
    }
}
