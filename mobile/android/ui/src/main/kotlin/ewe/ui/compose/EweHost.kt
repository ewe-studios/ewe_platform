package ewe.ui.compose

import androidx.compose.foundation.layout.Column
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalView
import ewe.ui.android.CoreBinding
import ewe.ui.android.HttpActionConnection
import ewe.ui.android.MainGapTimer
import ewe.ui.android.OkHttpChannelFactory
import ewe.ui.android.SystemCommandHost
import ewe.ui.compose.behaviours.BehaviourRegistry
import ewe.ui.runtime.*
import io.github.ewe_studios.ewe.core.HostTransport
import io.github.ewe_studios.ewe.core.NativeCore
import kotlinx.coroutines.*
import okhttp3.HttpUrl
import okhttp3.OkHttpClient

/** Render a server screen. The factory creates a core for this screen's action connection. */
@Composable
fun RemoteEweScreen(
    endpoint: HttpUrl,
    initialPath: String,
    createCore: (HostTransport) -> NativeCore,
    modifier: Modifier = Modifier,
    actionPath: String = "action",
) {
    EweHost(listOf(endpoint, initialPath, actionPath), "Connecting…", modifier) { host ->
        val client = OkHttpClient()
        host.onClose {
            client.dispatcher.cancelAll()
            client.connectionPool.evictAll()
        }
        val connection = HttpActionConnection(client, endpoint, actionPath)
        val binding = host.attach(createCore(connection), OkHttpChannelFactory(client, endpoint))
        host.apply(connection.negotiate(binding), "capabilities")
        host.applyInitial(connection.initial(initialPath))
    }
}

/** Render a local screen. The factory transfers ownership of the new core to this host. */
@Composable
fun LocalEweScreen(
    createCore: () -> NativeCore,
    initialFrame: suspend (NativeCore) -> ByteArray,
    modifier: Modifier = Modifier,
) {
    EweHost(emptyList(), "Loading…", modifier) { host ->
        val core = createCore()
        host.attach(core, null)
        host.applyInitial(initialFrame(core))
    }
}

@Composable
private fun EweHost(
    config: List<Any>,
    loading: String,
    modifier: Modifier,
    start: suspend (ScreenHost) -> Unit,
) {
    val view = LocalView.current
    val registry = remember { BehaviourRegistry.standard }
    val setup by rememberUpdatedState(start)
    var current by remember { mutableStateOf<ScreenHost?>(null) }
    DisposableEffect(view, config) {
        val store = NodeStore(
            MainGapTimer(), commands = SystemCommandHost(view), supportsBehaviour = registry::supports,
        )
        val host = ScreenHost(store, loading)
        current = host
        host.start(setup)
        onDispose { host.close() }
    }
    Column(modifier) {
        val host = current
        val status = host?.status ?: loading
        if (status.isNotEmpty()) Text(status)
        host?.bridge?.let { EweScreen(host.store, it, behaviours = registry) }
    }
}

private class ScreenHost(val store: NodeStore, private val loading: String) : AutoCloseable {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val cleanup = mutableListOf<() -> Unit>(store::close)
    var status by mutableStateOf(loading)
        private set
    var bridge by mutableStateOf<EventBridge?>(null)
        private set

    init {
        store.onError = { code, message -> status = "$code $message" }
    }

    fun start(setup: suspend (ScreenHost) -> Unit) {
        scope.launch {
            try {
                setup(this@ScreenHost)
            } catch (cancel: CancellationException) {
                throw cancel
            } catch (error: Exception) {
                status = error.message ?: "Screen setup failed"
            }
        }
    }

    fun onClose(clean: () -> Unit) {
        cleanup.add(clean)
    }

    fun attach(core: NativeCore, channels: ChannelFactory?): CoreBinding {
        val binding = try {
            CoreBinding(core, store, scope)
        } catch (error: Exception) {
            core.close()
            throw error
        }
        onClose(binding::close)
        bridge = EventBridge(store, binding, scope)
        val factory = channels ?: object : ChannelFactory {
            override fun open(
                endpoint: String,
                transport: UByte,
                streamId: ULong,
                resumeFrom: ULong,
                onFrame: (ByteArray) -> Unit,
                onClose: (Throwable?) -> Unit,
            ): Channel = error("A local screen cannot open a remote channel")
        }
        val mounts = LiveMountHost(store, factory)
        onClose(mounts::close)
        return binding
    }

    fun apply(frame: ByteArray, name: String) {
        check(store.receive(frame)) { "Invalid $name frame: ${store.reports.lastOrNull()}" }
    }

    fun applyInitial(frame: ByteArray) {
        if (status == loading) status = ""
        apply(frame, "initial")
    }

    override fun close() {
        scope.cancel()
        var failure: Exception? = null
        cleanup.asReversed().forEach { close ->
            try {
                close()
            } catch (error: Exception) {
                if (failure == null) failure = error else failure?.addSuppressed(error)
            }
        }
        cleanup.clear()
        failure?.let { throw it }
    }
}
