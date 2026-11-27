package {{ APP_ID }}

import ewe.ui.android.AndroidResourceSource
{% if COMPOSED %}import androidx.core.splashscreen.SplashScreen.Companion.installSplashScreen
{% endif %}import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
{% if COMPOSED %}import ewe.ui.compose.EweStartupView
{% else %}import androidx.compose.material3.Text
{% endif %}import ewe.ui.compose.RemoteEweScreen
import io.github.ewe_studios.ewe.core.NativeCore
import io.github.ewe_studios.ewe.core.NativePlatform
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
{% if COMPOSED %}        installSplashScreen()
{% endif %}        super.onCreate(savedInstanceState)
        val defaultServer = "http://10.0.2.2:8180/"
        setContent {
            MaterialTheme(colorScheme = if (isSystemInDarkTheme()) darkColorScheme() else lightColorScheme()) {
                var address by rememberSaveable { mutableStateOf(intent.getStringExtra("server") ?: defaultServer) }
                val endpoint = address.toHttpUrlOrNull()
                if (endpoint == null) {
{% if COMPOSED %}                    EweStartupView(
                        "Invalid server address. Use an http or https address.",
                        Modifier.safeDrawingPadding(),
                        actionLabel = "Use default server",
                        onAction = { address = defaultServer },
                    )
{% else %}                    Text("Invalid server address. Use an http or https address.")
{% endif %}                } else RemoteEweScreen(endpoint, "open-count", {
                    NativeCore.remote(NativePlatform.ANDROID, listOf("refresh_count"), it).also { core -> core.installResources(AndroidResourceSource(assets)) }
                }, modifier = Modifier.safeDrawingPadding())
            }
        }
    }
}
