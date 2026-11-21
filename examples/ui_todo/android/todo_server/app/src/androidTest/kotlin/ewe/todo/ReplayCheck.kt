package ewe.todo

import android.app.Activity
import android.app.Instrumentation
import android.os.Bundle
import ewe.ui.EventId
import ewe.ui.PropId
import ewe.ui.android.CoreBinding
import ewe.ui.android.HttpActionConnection
import ewe.ui.runtime.*
import io.github.ewe_studios.ewe.core.NativeCore
import io.github.ewe_studios.ewe.core.NativePlatform
import kotlinx.coroutines.*
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient

/** Exercises the business refusal after the first deletion has removed its control. */
class ReplayCheck : Instrumentation() {
    private var base = "http://127.0.0.1:8180/"

    override fun onCreate(arguments: Bundle?) {
        super.onCreate(arguments)
        base = arguments?.getString("server") ?: base
        start()
    }

    override fun onStart() {
        val results = Bundle()
        try {
            runBlocking {
                val connection = HttpActionConnection(OkHttpClient(), base.toHttpUrl())
                val store = NodeStore(GapTimer { _, _ -> Cancel {} })
                val native =
                    NativeCore.remote(
                        NativePlatform.ANDROID,
                        listOf("add_todo", "confirm_delete"),
                        connection,
                    )
                CoreBinding(native, store, this).use { core ->
                    check(store.receive(connection.negotiate(core)))
                    check(store.receive(connection.initial("native-initial")))
                    check(store.nodes.any { it.text == "Android server todo" }) {
                        "The second client did not receive the server app's todo"
                    }
                    val title = "Android replay verification"
                    val add = EventInput(1u, "submit", title, false, 0u, byteArrayOf())
                    check(store.receive(requireNotNull(core.trigger("server:add_todo", add))))
                    val label = store.nodes.last { it.text == title }
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
                    check(
                        store.receive(requireNotNull(core.trigger("server:confirm_delete", event)))
                    )
                    check(failure != null) { "Repeated delete was accepted" }
                    results.putString("stream", "Second client received the todo. Second delete refused: $failure")
                }
            }
            finish(Activity.RESULT_OK, results)
        } catch (error: Throwable) {
            results.putString("stream", error.stackTraceToString())
            finish(Activity.RESULT_CANCELED, results)
        }
    }
}
