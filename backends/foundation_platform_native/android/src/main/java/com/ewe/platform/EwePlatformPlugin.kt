package com.ewe.platform

import android.app.Activity
import android.webkit.WebView
import app.tauri.plugin.Plugin

/**
 * F42: Base plugin class for all foundation_platform native modules.
 *
 * Each IPC handler (modal, camera, biometric) extends the plugin's
 * capabilities. Individual helpers (ModalHelper, CameraHelper) are
 * called by the Rust side via PluginManager.runCommand().
 */
open class EwePlatformPlugin(activity: Activity) : Plugin(activity) {
    override fun load(webView: WebView, config: String) {
        // config = JSON plugin configuration from Rust
    }
}
