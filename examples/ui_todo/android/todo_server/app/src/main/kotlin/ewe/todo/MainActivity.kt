package ewe.todo

import java.security.SecureRandom
import ewe.todo.game.MathalleyRoute
import androidx.compose.runtime.mutableStateOf
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.material3.MaterialTheme
import ewe.ui.compose.EweTheme
import io.github.ewe_studios.ewe.todo.todoThemeManifest
import ewe.ui.compose.RemoteEweScreen
import io.github.ewe_studios.ewe.core.NativePlatform
import io.github.ewe_studios.ewe.todo.createTodoCore
import okhttp3.HttpUrl.Companion.toHttpUrl

class MainActivity : ComponentActivity() {
    private val foreground = mutableStateOf(false)
    private val game = MathalleyRoute {
        intent.getStringExtra("gameSeed")?.toULongOrNull() ?: SecureRandom().nextLong().toULong()
    }
    override fun onResume() { super.onResume(); foreground.value = true }
    override fun onPause() { foreground.value = false; super.onPause() }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Use the same native library for both sets of callback tables.
        System.setProperty("uniffi.component.foundation_ui_native.libraryOverride", "ui_todo")
        val endpoint = (intent.getStringExtra("server") ?: "http://10.0.2.2:8180/").toHttpUrl()
        val theme = if (intent.getBooleanExtra("branded", false))
            EweTheme.fromManifest(todoThemeManifest()) else EweTheme()
        setContent {
            MaterialTheme(colorScheme = if (isSystemInDarkTheme()) darkColorScheme() else lightColorScheme()) {
                RemoteEweScreen(endpoint, "initial", { createTodoCore(NativePlatform.ANDROID, it) }, theme = theme, onAttached = game::attach,
                    rpcBase = endpoint.takeIf { intent.getBooleanExtra("rpc", false) },
                    socketEndpoint = "/socket".takeIf { intent.getBooleanExtra("socket", false) })
                game.Screen(foreground.value)
            }
        }
    }
}
