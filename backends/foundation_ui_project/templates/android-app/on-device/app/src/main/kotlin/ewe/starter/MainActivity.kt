package {{ APP_ID }}

import ewe.ui.android.AndroidResourceSource
{% if COMPOSED %}import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
{% endif %}import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.ui.Modifier
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import ewe.ui.compose.LocalEweScreen
import io.github.ewe_studios.ewe.core.NativePlatform
import {{ APP_ID }}.counter.createCounterCore
import {{ APP_ID }}.counter.initialCounterFrame

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
{% if COMPOSED %}        installSplashScreen()
{% endif %}        super.onCreate(savedInstanceState)
        System.setProperty("uniffi.component.foundation_ui_native.libraryOverride", "counter_app")
        setContent {
            MaterialTheme(colorScheme = if (isSystemInDarkTheme()) darkColorScheme() else lightColorScheme()) {
                LocalEweScreen({ createCounterCore(NativePlatform.ANDROID).also { core -> core.installResources(AndroidResourceSource(assets)) } }, ::initialCounterFrame, modifier = Modifier.safeDrawingPadding())
            }
        }
    }
}
