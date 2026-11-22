package {{ APP_ID }}

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.ui.Modifier
import androidx.compose.material3.MaterialTheme
import ewe.ui.compose.LocalEweScreen
import io.github.ewe_studios.ewe.core.NativePlatform
import {{ APP_ID }}.counter.createCounterCore
import {{ APP_ID }}.counter.initialCounterFrame

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        System.setProperty("uniffi.component.foundation_ui_native.libraryOverride", "counter_app")
        setContent {
            MaterialTheme {
                LocalEweScreen({ createCounterCore(NativePlatform.ANDROID) }, ::initialCounterFrame, modifier = Modifier.safeDrawingPadding())
            }
        }
    }
}
