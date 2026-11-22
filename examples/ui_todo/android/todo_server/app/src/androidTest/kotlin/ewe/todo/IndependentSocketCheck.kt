package ewe.todo

import ewe.ui.android.AppWebSockets
import ewe.ui.android.OkHttpChannelFactory
import ewe.ui.android.SocketActionConnection
import ewe.ui.runtime.Cancel
import ewe.ui.runtime.GapTimer
import ewe.ui.runtime.NodeStore
import io.github.ewe_studios.ewe.core.*
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient

internal suspend fun verifyIndependentSocket(base: String, echo: String) = withContext(Dispatchers.Main) {
    System.setProperty("uniffi.component.foundation_ui_native.libraryOverride", "ui_todo")
    val client = OkHttpClient()
    val manager = OkHttpChannelFactory(client, base.toHttpUrl()).connectionManager()
    val reply = CompletableDeferred<ByteArray>()
    try {
        SocketActionConnection(manager, "/socket", { reply.complete(it) }, { reply.completeExceptionally(it) }).use { sender ->
            NativeCore.remote(NativePlatform.ANDROID, listOf("refresh_count"), sender).use { core ->
                core.applyCapsFrame(requireNotNull(sender.send(core.helloFor(sender.requestTag))))
                core.installWebSockets(AppWebSockets(client))
                core.openWebSocket(echo).use { socket ->
                    val text = SocketMessage.Text("hello")
                    socket.send(text)
                    check(socket.receive() == text)
                    val bytes = byteArrayOf(0, -1)
                    socket.send(SocketMessage.Binary(bytes))
                    check((socket.receive() as SocketMessage.Binary).value.contentEquals(bytes))
                    socket.closeSocket()
                }
                core.trigger("server:refresh_count", EventInput(0u, "tap", "", false, 0u, byteArrayOf()))
                val store = NodeStore(GapTimer { _, _ -> Cancel {} })
                check(store.receive(reply.await()))
                check(store.nodes.any { it.text == "Open todos" })
            }
        }
    } finally { manager.close() }
}
