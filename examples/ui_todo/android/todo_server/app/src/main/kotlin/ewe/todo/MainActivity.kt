package ewe.todo

import ewe.ui.android.AndroidResourceSource
import java.security.SecureRandom
import ewe.todo.game.MathalleyRoute
import androidx.compose.runtime.mutableStateOf
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.enableEdgeToEdge
import androidx.activity.compose.setContent
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.material3.MaterialTheme
import ewe.ui.compose.ActionTransport
import ewe.ui.compose.EweTheme
import io.github.ewe_studios.ewe.todo.todoThemeManifest
import ewe.ui.compose.RemoteEweScreen
import io.github.ewe_studios.ewe.core.NativePlatform
import io.github.ewe_studios.ewe.todo.createPersistentTodoCore
import java.security.MessageDigest
import okhttp3.HttpUrl.Companion.toHttpUrl

class MainActivity : ComponentActivity() {
    private val countdown = TodoCountdown()
    private val foreground = mutableStateOf(false)
    private val game = MathalleyRoute {
        intent.getStringExtra("gameSeed")?.toULongOrNull() ?: SecureRandom().nextLong().toULong()
    }
    override fun onResume() { super.onResume(); foreground.value = true }
    override fun onPause() { foreground.value = false; super.onPause() }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // Use the same native library for both sets of callback tables.
        System.setProperty("uniffi.component.foundation_ui_native.libraryOverride", "ui_todo")
        val endpoint = (intent.getStringExtra("server") ?: "http://10.0.2.2:8180/").toHttpUrl()
        val key = MessageDigest.getInstance("SHA-256").digest(endpoint.toString().toByteArray()).joinToString("") { "%02x".format(it) }
        val pendingFile = filesDir.resolve("todo-pending/$key.json").absolutePath
        val theme = if (intent.getBooleanExtra("branded", false))
            EweTheme.fromManifest(todoThemeManifest()) else EweTheme()
        val transport = when {
            intent.getBooleanExtra("rpc", false) -> ActionTransport.Connect(endpoint)
            intent.getBooleanExtra("socket", false) -> ActionTransport.WebSocket("/socket")
            else -> ActionTransport.Http
        }
        setContent {
            MaterialTheme(colorScheme = if (isSystemInDarkTheme()) darkColorScheme() else lightColorScheme()) {
                RemoteEweScreen(endpoint, "initial", { sender ->
                    createPersistentTodoCore(NativePlatform.ANDROID, sender, pendingFile).also { core ->
                        core.installResources(AndroidResourceSource(assets))
                    }
                }, theme = theme, transport = transport, viewDataAction = "host:apply_todo_delivery", onAttached = game::attach, onViewReady = countdown::attach)
                countdown.Run(foreground.value)
                game.Screen(foreground.value)
            }
        }
    }
}
