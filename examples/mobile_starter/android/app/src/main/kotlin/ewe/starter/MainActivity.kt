package ewe.starter

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import ewe.ui.compose.RemoteEweScreen
import io.github.ewe_studios.ewe.core.NativeCore
import io.github.ewe_studios.ewe.core.NativePlatform
import okhttp3.HttpUrl.Companion.toHttpUrl

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val endpoint = (intent.getStringExtra("server") ?: "http://10.0.2.2:8180/").toHttpUrl()
        setContent {
            MaterialTheme {
                RemoteEweScreen(endpoint, "native-starter", {
                    NativeCore.remote(NativePlatform.ANDROID, listOf("refresh_count"), it)
                })
            }
        }
    }
}
