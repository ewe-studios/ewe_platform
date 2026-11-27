package ewe.ui.compose

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Surface
import androidx.compose.material3.TextButton
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.platform.LocalView
import ewe.ui.android.OkHttpRpcClient
import io.github.ewe_studios.ewe.core.ConnectSender
import io.github.ewe_studios.ewe.core.NativeHttpClient
import io.github.ewe_studios.ewe.core.NativePlatform
import ewe.ui.android.CoreBinding
import ewe.ui.android.SocketActionConnection
import ewe.ui.runtime.ConnectionManager
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

/** Selects the sender for server actions. */
sealed interface ActionTransport {
    data object Http : ActionTransport
    data class Connect(val base: HttpUrl) : ActionTransport
    data class WebSocket(val path: String) : ActionTransport
}

/** Render a server screen. The factory creates a core for this screen's action connection. */
@Composable
fun RemoteEweScreen(
    endpoint: HttpUrl,
    initialPath: String,
    createCore: (HostTransport) -> NativeCore,
    modifier: Modifier = Modifier,
    actionPath: String = "action",
    theme: EweTheme = EweTheme(),
    transport: ActionTransport = ActionTransport.Http,
    connections: ConnectionManager? = null,
    viewDataAction: String? = null,
    onAttached: (NativeCore, NodeStore) -> Unit = { _, _ -> },
) {
    EweHost(listOf(endpoint, initialPath, actionPath, transport, connections ?: ""), "Connecting…", modifier, theme) { host ->
        val client = OkHttpClient()
        host.onClose {
            client.dispatcher.cancelAll()
            client.connectionPool.evictAll()
        }
        val connection = HttpActionConnection(client, endpoint, actionPath)
        val factory = OkHttpChannelFactory(client, endpoint)
        val manager = connections ?: factory.connectionManager().also { host.onClose(it::close) }
        val socket = (transport as? ActionTransport.WebSocket)?.let { socketTransport ->
            SocketActionConnection(manager, socketTransport.path, { host.apply(it, "action reply") },
                { host.store.onError?.invoke("Transport", it.message.orEmpty()) }).also { host.onClose(it::close) }
        }
        val sender = if (transport !is ActionTransport.Connect) socket ?: connection else {
            val network = NativeHttpClient(OkHttpRpcClient(client))
            host.onClose(network::close)
            val rpc = ConnectSender(network, transport.base.toString(), NativePlatform.ANDROID, host.store.maxFrameBytes.toUInt(), 16_384u)
            host.onClose(rpc::close)
            object : HostTransport {
                override suspend fun send(request: ByteArray): ByteArray? = rpc.exchange(request)
            }
        }
        val core = createCore(sender)
        val binding = host.attach(core, factory, manager, viewDataAction)
        onAttached(core, host.store)
        val hello = socket?.let { binding.helloFor(it.requestTag) } ?: binding.hello()
        connection.configure(hello)
        host.apply(requireNotNull(sender.send(hello)) { "Missing server capabilities" }, "capabilities")
        host.loadView { connection.initial(initialPath) }
    }
}

/** Render a local screen. The factory transfers ownership of the new core to this host. */
@Composable
fun LocalEweScreen(
    createCore: () -> NativeCore,
    initialFrame: suspend (NativeCore) -> ByteArray,
    modifier: Modifier = Modifier,
    theme: EweTheme = EweTheme(),
    viewDataAction: String? = null,
    onAttached: (NativeCore, NodeStore) -> Unit = { _, _ -> },
) {
    EweHost(emptyList(), "Loading…", modifier, theme) { host ->
        val core = createCore()
        host.attach(core, null, viewDataAction = viewDataAction)
        onAttached(core, host.store)
        host.applyInitial(initialFrame(core))
    }
}

/** Display a startup message and an optional recovery action. */
@Composable
fun EweStartupView(
    message: String,
    modifier: Modifier = Modifier,
    actionLabel: String = "Retry",
    onAction: (() -> Unit)? = null,
) {
    Surface(modifier.fillMaxSize()) {
        Column(
            Modifier.fillMaxSize().padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterVertically),
        ) {
            if (onAction == null) CircularProgressIndicator()
            Text(message)
            if (onAction != null) TextButton(onClick = onAction) { Text(actionLabel) }
        }
    }
}

@Composable
private fun EweHost(
    config: List<Any>,
    loading: String,
    modifier: Modifier,
    theme: EweTheme,
    start: suspend (ScreenHost) -> Unit,
) {
    val view = LocalView.current
    val registry = remember { BehaviourRegistry.standard }
    val setup by rememberUpdatedState(start)
    var current by remember { mutableStateOf<ScreenHost?>(null) }
    var attempt by remember(config) { mutableIntStateOf(0) }
    DisposableEffect(view, config, attempt) {
        val store = NodeStore(
            MainGapTimer(), commands = SystemCommandHost(view), supportsBehaviour = registry::supports,
        )
        val host = ScreenHost(store, loading)
        current = host
        host.start(setup)
        onDispose { host.close() }
    }
    val host = current
    val bridge = host?.bridge
    if (host == null || bridge == null || !host.ready) {
        val status = host?.status ?: loading
        if (status == loading) EweStartupView(status, modifier)
        else EweStartupView(status, modifier, onAction = { attempt += 1 })
    } else {
        Surface(modifier.fillMaxSize()) {
            Column {
                if (host.status.isNotEmpty()) {
                    Text(host.status)
                    if (host.store.recovery != null && host.status != loading) {
                        TextButton(onClick = { host.start { it.retryView() } }) { Text("Retry") }
                    }
                }
                EweScreen(host.store, bridge, behaviours = registry, theme = theme, graphics = host.graphics)
            }
        }
    }
}

private class ScreenHost(val store: NodeStore, private val loading: String) : AutoCloseable {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val cleanup = mutableListOf<() -> Unit>(store::close)
    var ready by mutableStateOf(false)
        private set
    var status by mutableStateOf(loading)
        private set
    var graphics: (() -> io.github.ewe_studios.ewe.core.NativeGraphics)? = null
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

    fun attach(core: NativeCore, channels: ChannelFactory?, manager: ConnectionManager? = null, viewDataAction: String? = null): CoreBinding {
        val binding = try {
            core.installWebSockets(ewe.ui.android.AppWebSockets())
            CoreBinding(core, store, scope)
        } catch (error: Exception) {
            core.close()
            throw error
        }
        if (viewDataAction != null) {
            require(viewDataAction.startsWith("host:"))
            val queue = kotlinx.coroutines.channels.Channel<Pair<UInt, ByteArray>>(kotlinx.coroutines.channels.Channel.BUFFERED)
            store.onData = { id, message ->
                message.msgpack?.let { packet ->
                    val json = (0 until packet.bytesLength).joinToString(",", "[", "]") { packet.bytes(it).toString() }
                    queue.trySend(id to json.toByteArray()).getOrThrow()
                }
            }
            onClose { store.onData = null; queue.close() }
            scope.launch {
                for ((id, json) in queue) {
                    try {
                        val event = EventInput(id, "data", "", false, 0u, byteArrayOf(), json)
                        binding.trigger(viewDataAction, event)?.let { check(store.receive(it, id)) { "Invalid view data reply" } }
                    } catch (cancel: CancellationException) { throw cancel }
                    catch (error: Exception) { store.onError?.invoke("ViewData", error.message.orEmpty()) }
                }
            }
        }
        graphics = core::graphics
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
        val mounts = LiveMountHost(store, factory, manager)
        onClose(mounts::close)
        return binding
    }

    fun apply(frame: ByteArray, name: String) {
        check(store.receive(frame)) { "Invalid $name frame: ${store.reports.lastOrNull()}" }
    }

    suspend fun loadView(load: suspend () -> ByteArray) {
        val loader = store.registerLoader(scope, load = load)
        loader.onInstalled = { ready = true; status = "" }
        loader.open()
    }

    suspend fun retryView() {
        store.recovery?.retry()
        status = ""
    }

    fun applyInitial(frame: ByteArray) {
        apply(frame, "initial")
        ready = true
        if (status == loading) status = ""
    }

    override fun close() {
        scope.cancel()
        var failure: Exception? = null
        cleanup.asReversed().forEach { close ->
            try {
                close()
            } catch (error: Exception) {
                if (failure == null) failure = error else failure.addSuppressed(error)
            }
        }
        cleanup.clear()
        failure?.let { throw it }
    }
}
