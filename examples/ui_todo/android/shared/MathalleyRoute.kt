package ewe.todo.game

import android.media.AudioFormat
import android.media.AudioFocusRequest
import android.media.AudioAttributes
import android.media.AudioManager
import android.media.AudioTrack
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import ewe.ui.compose.EweDrawingSurface
import ewe.ui.runtime.NodeStore
import io.github.ewe_studios.ewe.core.NativeCore
import io.github.ewe_studios.ewe.core.NativeDrawingSurface
import io.github.ewe_studios.ewe.todo.MathalleyGame
import kotlinx.coroutines.*

class MathalleyRoute(private val seed: () -> ULong) {
    private var core: NativeCore? = null
    private var store: NodeStore? = null
    private var visible by mutableStateOf(false)
    fun attach(core: NativeCore, store: NodeStore) {
        this.core = core
        this.store = store
        store.registerView("todos")
        val previous = store.onData
        val open = byteArrayOf(0xad.toByte()) + "open:mathalley".encodeToByteArray()
        store.onData = { id, data ->
            previous?.invoke(id, data)
            val packed = data.msgpack
            if (!visible && packed?.bytesLength == open.size && open.indices.all { open[it] == packed.bytes(it).toByte() }) {
                store.registerView("mathalley")
                visible = true
            }
        }
    }

    @Composable fun Screen(foreground: Boolean) {
        if (!visible) return
        val graphics = checkNotNull(core)::graphics
        val rate = remember { AudioTrack.getNativeOutputSampleRate(AudioManager.STREAM_MUSIC) }
        val session = remember { runCatching { MathalleyGame(checkNotNull(core), seed(), rate.toUInt()) } }
        val game = session.getOrNull()
        if (game == null) {
            val close = { store?.unregisterView("mathalley"); visible = false }
            AlertDialog(onDismissRequest = close, title = { Text("Mathalley") },
                text = { Text(session.exceptionOrNull()?.localizedMessage ?: "Cannot open game") },
                confirmButton = { TextButton(onClick = close) { Text("Back to Todos") } })
            return
        }
        var presented by remember { mutableStateOf(false) }
        var surface by remember { mutableStateOf<NativeDrawingSurface?>(null) }
        var sound by remember { mutableStateOf(false) }
        var error by remember { mutableStateOf<String?>(null) }
        var audioError by remember { mutableStateOf<String?>(null) }
        var audioFocus by remember { mutableStateOf(false) }
        val manager = LocalContext.current.getSystemService(AudioManager::class.java)
        DisposableEffect(sound,foreground) {
            val focus = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                .setAudioAttributes(AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_GAME).build())
                .setOnAudioFocusChangeListener { audioFocus = it == AudioManager.AUDIOFOCUS_GAIN }
                .build()
            if (sound && foreground) {
                audioFocus = manager.requestAudioFocus(focus) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
                if (!audioFocus) audioError = "The host declined audio focus"
            }
            onDispose { if (sound && foreground) manager.abandonAudioFocusRequest(focus) }
        }
        fun close() {
            game.active(false)
            store?.unregisterView("mathalley")
            visible = false
        }
        DisposableEffect(game) { onDispose { game.stopSound(); game.close() } }
        LaunchedEffect(surface, foreground, error) {
            game.active(foreground)
            val target = surface ?: return@LaunchedEffect
            if (foreground && error == null) try {
                while (isActive) {
                    game.draw(target, withFrameNanos { it.toULong() })
                    presented = true
                }
            } catch (cancel: CancellationException) { throw cancel }
            catch (failure: Exception) { error = failure.message }
        }
        LaunchedEffect(sound,foreground,error,audioFocus) {
            if (!sound || !foreground || error != null || !audioFocus) return@LaunchedEffect
            try {
                withContext(Dispatchers.IO) {
                    val size = AudioTrack.getMinBufferSize(rate,AudioFormat.CHANNEL_OUT_MONO,AudioFormat.ENCODING_PCM_FLOAT)
                    require(size > 0) { "The speaker has no supported audio format" }
                    val output = AudioTrack.Builder().setAudioFormat(AudioFormat.Builder().setEncoding(AudioFormat.ENCODING_PCM_FLOAT)
                        .setChannelMask(AudioFormat.CHANNEL_OUT_MONO).setSampleRate(rate).build())
                        .setBufferSizeInBytes(size).setTransferMode(AudioTrack.MODE_STREAM).build()
                    try {
                        output.play()
                        var written = 0uL
                        while (isActive) {
                            val samples = game.samples(written,(size / Float.SIZE_BYTES).toUInt()).toFloatArray()
                            var offset = 0
                            while (offset < samples.size && isActive) {
                                val count = output.write(samples,offset,samples.size-offset,AudioTrack.WRITE_BLOCKING)
                                check(count > 0) { "Audio output failed ($count)" }
                                offset += count
                                written += count.toULong()
                            }
                        }
                    } finally { output.release() }
                }
            } catch (cancel: CancellationException) { throw cancel }
            catch (failure: Exception) { audioError = failure.message }
        }
        Dialog(onDismissRequest = ::close, properties = DialogProperties(usePlatformDefaultWidth = false)) {
            Surface(Modifier.fillMaxSize()) {
                Column(Modifier.safeDrawingPadding()) {
                    TextButton(onClick = ::close) { Text("Back to Todos") }
                    audioError?.let { Text("Sound unavailable. " + it) }
                    if (error != null) Text(error.orEmpty())
                    else EweDrawingSurface("mathalley", graphics, { error = it.message }, Modifier.fillMaxSize().semantics { contentDescription = if (presented) "Mathalley game" else "Loading Mathalley" }.pointerInput(game) {
                        detectTapGestures { p ->
                            game.tap(p.x,p.y,size.width.toFloat(),size.height.toFloat())
                            sound = true
                        }
                    }, changed = { surface = it })
                }
            }
        }
    }
}
