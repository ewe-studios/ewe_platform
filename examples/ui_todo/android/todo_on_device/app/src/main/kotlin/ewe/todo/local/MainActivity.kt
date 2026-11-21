package ewe.todo.local

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import ewe.ui.compose.LocalEweScreen
import io.github.ewe_studios.ewe.core.NativePlatform
import io.github.ewe_studios.ewe.todo.createLocalTodoCore
import io.github.ewe_studios.ewe.todo.initialTodoFrame

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // Use the same native library for both sets of callback tables.
        System.setProperty("uniffi.component.foundation_ui_native.libraryOverride", "ui_todo_native")
        setContent {
            MaterialTheme {
                LocalEweScreen({ createLocalTodoCore(NativePlatform.ANDROID) }, ::initialTodoFrame)
            }
        }
    }
}
