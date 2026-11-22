package ewe.todo

import android.app.Activity
import android.app.Instrumentation
import android.os.Bundle
import ewe.ui.EventId
import ewe.ui.PropId
import ewe.ui.android.CoreBinding
import ewe.ui.android.HttpActionConnection
import ewe.ui.android.OkHttpChannelFactory
import ewe.ui.runtime.*
import io.github.ewe_studios.ewe.core.NativeCore
import io.github.ewe_studios.ewe.core.NativePlatform
import kotlinx.coroutines.*
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient

/** Replays a deletion after the first request removes its control. */
class ReplayCheck : Instrumentation() {
    private var base = "http://127.0.0.1:8180/"

    private var echoEndpoint: String? = null
    private var resourceHash: String? = null
    private var dataOnly = false
    private var subscriptions = false

    override fun onCreate(arguments: Bundle?) {
        super.onCreate(arguments)
        base = arguments?.getString("server") ?: base
        subscriptions = arguments?.getString("subscriptions") == "true"
        dataOnly = arguments?.getString("data") == "true"
        echoEndpoint = arguments?.getString("socket_echo")
        resourceHash = arguments?.getString("resource_hash")
        start()
    }

    private suspend fun verifySubscriptions() = withContext(Dispatchers.Main) {
        val manager = ewe.ui.android.OkHttpChannelFactory(OkHttpClient(), base.toHttpUrl()).connectionManager()
        try {
            for (transport in listOf(ewe.ui.LiveTransport.Sse, ewe.ui.LiveTransport.Http, ewe.ui.LiveTransport.WebSocket)) {
                val socket = transport == ewe.ui.LiveTransport.WebSocket
                val id = manager.nextTag()
                val session = CompletableDeferred<ULong>()
                val expired = CompletableDeferred<Unit>()
                val opened = CompletableDeferred<Unit>()
                fun context(bytes: ByteArray) {
                    val caps = SubscriptionFrames.caps(bytes) ?: return
                    check(caps.subscriptionResume)
                    session.complete(caps.streamId)
                }
                val handle = manager.acquire(transport, if (socket) "/socket" else "/live", shared = true,
                    onOpen = { opened.complete(Unit) }, onContext = ::context,
                    onError = { session.completeExceptionally(it); expired.completeExceptionally(it); opened.completeExceptionally(it) },
                    receive = { bytes ->
                        if (SubscriptionFrames.caps(bytes) != null) context(bytes)
                        else {
                            val frame = ewe.ui.Frame.getRootAsFrame(java.nio.ByteBuffer.wrap(bytes))
                            val body = frame.kind(ewe.ui.FrameMessage()) as ewe.ui.FrameMessage
                            if (body.subscriptionId == id && body.messages(0)?.control?.replayExpired?.streamId == 1uL) expired.complete(Unit)
                        }
                    })
                try {
                    opened.await()
                    if (socket) handle.send(SubscriptionFrames.hello(id))
                    handle.send(SubscriptionFrames.resume(session.await(), id, 1uL, ULong.MAX_VALUE))
                    expired.await()
                } finally { handle.close() }
            }
        } finally { manager.close() }
    }

    override fun onStart() {
        val results = Bundle()
        try {
            runBlocking {
                echoEndpoint?.let {
                    withTimeout(30000) { verifyIndependentSocket(base, it) }
                    results.putString("stream", "The action socket still returns the count.")
                    return@runBlocking
                }
                resourceHash?.let {
                    verifyInstalledResources(targetContext.assets, it)
                    return@runBlocking
                }
                if (subscriptions) {
                    withTimeout(30000) { verifySubscriptions() }
                    results.putString("stream", "SSE, streamed HTTP, and WebSocket delivered targeted expiry.")
                    return@runBlocking
                }
                if (dataOnly) {
                    System.setProperty("uniffi.component.foundation_ui_native.libraryOverride", "ui_todo")
                    val transport = ewe.ui.android.OkHttpRpcClient(OkHttpClient())
                    io.github.ewe_studios.ewe.core.NativeHttpClient(transport).use { client ->
                        val id = "00000000-0000-4000-8000-000000000042"
                        val todo = io.github.ewe_studios.ewe.todo.getTodo(client, base, id)
                        check(todo.id == id && todo.title == "Buy milk")
                        try {
                            io.github.ewe_studios.ewe.todo.getTodo(client, base, "00000000-0000-4000-8000-000000000999")
                            error("Expected NotFound")
                        } catch (_: io.github.ewe_studios.ewe.todo.TodoServiceException.NotFound) { }
                        try {
                            io.github.ewe_studios.ewe.todo.getTodo(client, base, "42")
                            error("Expected the client to reject a numeric identifier")
                        } catch (error: io.github.ewe_studios.ewe.todo.TodoServiceException.Failed) {
                            check(error.detail.contains("UUID"))
                        }
                    }
                    results.putString("stream", "GetTodo returned the requested UUID, Buy milk, and NotFound.")
                    return@runBlocking
                }
                withContext(Dispatchers.Main) {
                    val client = OkHttpClient()
                    val endpoint = base.toHttpUrl()
                    val connection = HttpActionConnection(client, endpoint)
                    val store = NodeStore(GapTimer { _, _ -> Cancel {} })
                    val native =
                        NativeCore.remote(
                            NativePlatform.ANDROID,
                            listOf("add_todo", "confirm_delete"),
                            connection,
                        )
                    LiveMountHost(store, OkHttpChannelFactory(client, endpoint)).use {
                        CoreBinding(native, store, this).use { core ->
                            check(store.receive(connection.negotiate(core)))
                            check(store.receive(connection.initial("initial")))
                            check(store.nodes.any { it.text == "Android server todo" }) {
                                "The second client did not receive the server app's todo"
                            }
                            val title = "Android replay verification"
                            val payload = org.json.JSONObject().put("id", java.util.UUID.randomUUID().toString()).put("title", title)
                            val add = EventInput(1u, "submit", title, false, 0u, byteArrayOf(), payload.toString().encodeToByteArray())
                            check(store.receive(requireNotNull(core.trigger("server:add_todo", add))))
                            val label = withTimeout(5000) {
                                var updated = store.nodes.lastOrNull { it.text == title }
                                while (updated == null) {
                                    delay(50)
                                    updated = store.nodes.lastOrNull { it.text == title }
                                }
                                updated
                            }
                            var row = label
                            while (
                                row.children.none {
                                    store.record(it)?.actions?.get(EventId.TAP) == "server:toggle_todo"
                                }
                            ) {
                                row = store.record(requireNotNull(row.parent))!!
                            }
                            val toggle =
                                row.children.mapNotNull(store::record).first {
                                    it.actions[EventId.TAP] == "server:toggle_todo"
                                }
                            val id = toggle.props.getValue(PropId.VALUE).label
                            val event = EventInput(toggle.id, "tap", id, false, row.id, byteArrayOf())
                            var failure: String? = null
                            store.onError = { code, message -> failure = "$code $message" }
                            check(
                                store.receive(requireNotNull(core.trigger("server:confirm_delete", event)))
                            )
                            check(failure == null) { "First deletion failed: $failure" }
                            withTimeout(5000) {
                                while (store.nodes.any { it.text == title }) delay(50)
                            }
                            check(
                                store.receive(requireNotNull(core.trigger("server:confirm_delete", event)))
                            )
                            check(failure == null) { "Repeated delete failed: $failure" }
                            check(store.nodes.none { it.text == title })
                            results.putString("stream", "Second client received the todo. Repeated delete succeeded.")
                        }
                    }
                }
            }
            finish(Activity.RESULT_OK, results)
        } catch (error: Throwable) {
            results.putString("stream", error.stackTraceToString())
            finish(Activity.RESULT_CANCELED, results)
        }
    }
}
