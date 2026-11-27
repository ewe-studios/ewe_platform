package ewe.todo.local

import ewe.todo.TodoCountdown
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
import ewe.ui.compose.EweTheme
import io.github.ewe_studios.ewe.todo.todoThemeManifest
import ewe.ui.compose.LocalEweScreen
import io.github.ewe_studios.ewe.core.NativePlatform
import io.github.ewe_studios.ewe.todo.createLocalTodoCore
import io.github.ewe_studios.ewe.todo.initialTodoFrame

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
        val theme = if (intent.getBooleanExtra("branded", false))
            EweTheme.fromManifest(todoThemeManifest()) else EweTheme()
        setContent {
            MaterialTheme(colorScheme = if (isSystemInDarkTheme()) darkColorScheme() else lightColorScheme()) {
                LocalEweScreen(
                    createCore = {
                        createLocalTodoCore(NativePlatform.ANDROID).also { core ->
                            core.installResources(AndroidResourceSource(assets))
                        }
                    },
                    initialFrame = ::initialTodoFrame,
                    theme = theme,
                    viewDataAction = "host:apply_todo_delivery",
                    onAttached = game::attach,
                    onViewReady = countdown::attach,
                )
                countdown.Run(foreground.value)
                game.Screen(foreground.value)
            }
        }
    }
}
