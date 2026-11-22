package ewe.todo

import android.content.res.AssetManager
import ewe.ui.android.AndroidResourceSource
import io.github.ewe_studios.ewe.core.NativePlatform
import io.github.ewe_studios.ewe.core.NativeResourceException
import io.github.ewe_studios.ewe.todo.createLocalTodoCore
import java.security.MessageDigest

internal suspend fun verifyInstalledResources(assets: AssetManager, expectedHash: String) {
    System.setProperty("uniffi.component.foundation_ui_native.libraryOverride", "ui_todo")
    createLocalTodoCore(NativePlatform.ANDROID).use { core ->
        try {
            core.openResource("art/todo.svg")
            error("Expected a missing host")
        } catch (_: NativeResourceException.MissingHost) {}
        core.installResources(AndroidResourceSource(assets))
        val digest = MessageDigest.getInstance("SHA-256")
        core.openResource("art/todo.svg").use { file ->
            while (true) digest.update(file.nextChunk() ?: break)
        }
        check(digest.digest().joinToString("") { "%02x".format(it) } == expectedHash)
        try {
            core.openResource("app.js")
            error("Unexpected browser-only file")
        } catch (_: NativeResourceException.Missing) {}
        try {
            core.openResource("../outside")
            error("Expected an invalid name")
        } catch (_: NativeResourceException.InvalidName) {}
    }
}
