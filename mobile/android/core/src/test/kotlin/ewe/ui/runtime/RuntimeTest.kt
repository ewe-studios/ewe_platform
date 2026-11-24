package ewe.ui.runtime

import ewe.ui.*
import ewe.ui.flatbuffers.FlatBufferBuilder
import kotlinx.coroutines.*
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

private typealias Row = (FlatBufferBuilder) -> Int

private fun shared(tag: UByte, make: Row): Row = { b ->
    Message.createMessage(b, Operation.createOperation(b, tag, make(b), 0u, 0, 0u, 0, 0), 0, 0)
}

private fun mobile(tag: UByte, make: Row): Row = { b ->
    Message.createMessage(b, Operation.createOperation(b, 0u, 0, 0u, 0, tag, make(b), 0), 0, 0)
}

private fun frame(vararg rows: Row, seq: ULong = 0u, ack: ULong = 0u): ByteArray {
    val b = FlatBufferBuilder()
    val messages = rows.map { it(b) }.toIntArray()
    val body =
        FrameMessage.createFrameMessage(b, ack, FrameMessage.createMessagesVector(b, messages))
    Frame.finishFrameBuffer(b, Frame.createFrame(b, 1u, 9u, seq, FrameKind.FrameMessage, body))
    return b.sizedByteArray()
}

private fun text(s: String) =
    shared(UiOperation.SetText) { b -> SetText.createSetText(b, 1u, b.createString(s)) }

private class Clock : GapTimer {
    var due: (() -> Unit)? = null

    override fun schedule(delayMs: Long, callback: () -> Unit): Cancel {
        due = callback
        return Cancel { due = null }
    }
}

private class Core : ActionCore {
    val events = mutableListOf<Pair<String, EventInput>>()
    val caps = mutableListOf<ByteArray>()
    var reply: ByteArray? = null
    var fail = false

    override fun hello() = byteArrayOf()

    override fun applyCapsFrame(frame: ByteArray) {
        caps += frame
    }

    override suspend fun trigger(reference: String, event: EventInput): ByteArray? {
        if (fail) error("action refused")
        events += reference to event
        return reply
    }
}

class RuntimeTest {
    @Test
    fun sequencesAndGapRecovery() {
        val timer = Clock()
        val store = NodeStore(timer)
        store.receive(frame(text("one"), seq = 1u))
        store.receive(frame(text("three"), seq = 3u))
        assertEquals("one", store.record(1u)?.text)
        store.receive(frame(text("answer"), seq = 7u, ack = 123u))
        assertEquals("answer", store.record(1u)?.text)
        store.receive(frame(text("two"), seq = 2u))
        assertEquals("three", store.record(1u)?.text)
        assertNull(timer.due)
        store.receive(frame(text("duplicate"), seq = 2u))
        assertEquals("three", store.record(1u)?.text)
        store.receive(frame(text("five"), seq = 5u))
        var gap: ULong? = null
        store.onGap = { stream, cursor ->
            assertEquals(9uL, stream)
            gap = cursor
        }
        timer.due!!.invoke()
        assertEquals(3uL, gap)
        store.receive(frame(text("four"), seq = 4u))
        assertEquals("four", store.record(1u)?.text)
    }

    @Test
    fun eventUsesOnlyBoundNodeAndCopiesUpdatedState() {
        val store = NodeStore(Clock())
        val core = Core()
        val bridge = EventBridge(store, core, CoroutineScope(Dispatchers.Unconfined))
        store.receive(
            frame(
                shared(UiOperation.CreateNode) {
                    CreateNode.createCreateNode(it, 42u, NodeKind.BUTTON)
                },
                shared(UiOperation.AppendChild) { AppendChild.createAppendChild(it, 1u, 42u) },
                shared(UiOperation.AttachState) {
                    AttachState.createAttachState(it, 1u, it.createByteVector(byteArrayOf(9)))
                },
                shared(UiOperation.BindAction) {
                    BindAction.createBindAction(
                        it,
                        42u,
                        EventId.TAP,
                        it.createString("server:increment"),
                    )
                },
            )
        )
        core.reply =
            frame(
                shared(UiOperation.AttachState) {
                    AttachState.createAttachState(it, 42u, it.createByteVector(byteArrayOf(5)))
                }
            )
        bridge.send(EventId.TAP, 42u)
        bridge.send(EventId.TAP, 42u)
        bridge.send(EventId.CHANGE, 42u)
        assertEquals(2, core.events.size)
        val (ref, event) = core.events.first()
        assertEquals("server:increment", ref)
        assertEquals("tap", event.eventName)
        assertEquals(1u, event.parentNode)
        assertArrayEquals(byteArrayOf(), event.state)
        assertArrayEquals(byteArrayOf(5), core.events.last().second.state)
        var error = ""
        store.onError = { code, _ -> error = code }
        core.fail = true
        bridge.send(EventId.TAP, 42u)
        assertEquals("ActionFailed", error)
    }

    @Test
    fun capsGoToCoreOnce() {
        val store = NodeStore(Clock())
        val core = Core()
        EventBridge(store, core, CoroutineScope(Dispatchers.Unconfined))
        val message =
            frame({ b ->
                Message.createMessage(
                    b,
                    0,
                    0,
                    Control.createControl(
                        b,
                        ServerCaps.createServerCaps(b, CapsStatus.Ok, 1u, 1u, 9u, 0u, 0),
                        0,
                        0,
                        0,
                    ),
                )
            })
        assertTrue(store.receive(message))
        assertArrayEquals(message, core.caps.single())
    }

    @Test
    fun liveChannelFollowsAttachmentAndCursor() {
        val clock = Clock()
        val store = NodeStore(clock)
        val opens = mutableListOf<ULong>()
        var closes = 0
        val deliveries = mutableListOf<(ByteArray) -> Unit>()
        val factory = object : ChannelFactory {
            override fun open(
                endpoint: String,
                transport: UByte,
                streamId: ULong,
                resumeFrom: ULong,
                onFrame: (ByteArray) -> Unit,
                onClose: (Throwable?) -> Unit,
            ): Channel {
                assertEquals(9uL, streamId)
                opens += resumeFrom
                deliveries += onFrame
                return Channel { closes++ }
            }
        }
        val host = LiveMountHost(store, factory)
        fun mount(id: UInt, transport: UByte) =
            shared(UiOperation.LiveMount) { b ->
                LiveMount.createLiveMount(
                    b,
                    id,
                    LiveKind.Stream,
                    9u,
                    transport,
                    b.createString("/live"),
                    19u,
                )
            }
        store.receive(frame(mount(50u, LiveTransport.Sse), mount(51u, LiveTransport.Local)))
        assertTrue(opens.isEmpty())
        store.receive(
            frame(
                shared(UiOperation.AppendChild) { AppendChild.createAppendChild(it, 1u, 50u) },
                shared(UiOperation.AppendChild) { AppendChild.createAppendChild(it, 1u, 51u) },
            )
        )
        assertEquals(listOf(19uL), opens)
        store.receive(frame(text("gap"), seq = 21u))
        clock.due!!.invoke()
        assertEquals(listOf(19uL, 19uL), opens)
        assertEquals(1, closes)
        store.receive(frame(shared(UiOperation.Remove) { Remove.createRemove(it, 50u) }))
        assertEquals(2, closes)
        val mountFrame = frame(
            mount(50u, LiveTransport.Sse),
            shared(UiOperation.AppendChild) { AppendChild.createAppendChild(it, 1u, 50u) },
        )
        assertTrue(store.receive(mountFrame))
        val old = deliveries.last()
        host.close()
        assertEquals(3, closes)

        val recreated = NodeStore(clock)
        LiveMountHost(recreated, factory).use {
            assertTrue(recreated.receive(mountFrame))
            var deliveriesToRecreated = 0
            recreated.onError = { _, _ -> deliveriesToRecreated++ }
            old(frame(text("stale")))
            assertNotEquals("stale", store.record(1u)?.text)
            assertNotEquals("stale", recreated.record(1u)?.text)
            deliveries.last()(frame(text("current")))
            assertEquals("current", recreated.record(1u)?.text)
            assertEquals(0, deliveriesToRecreated)
        }
        assertEquals(4, closes)
    }

    @Test
    fun reconnectClearsOnlyTheFailedLiveMount() {
        val store = NodeStore(Clock())
        val manager = ConnectionManager()
        val events = mutableMapOf<String, ConnectionEvents>()
        manager.register(LiveTransport.Sse) { endpoint, connection ->
            events[endpoint] = connection
            Channel {}
        }
        val factory = object : ChannelFactory {
            override fun open(
                endpoint: String, transport: UByte, streamId: ULong, resumeFrom: ULong,
                onFrame: (ByteArray) -> Unit, onClose: (Throwable?) -> Unit,
            ): Channel = error("The shared mount uses the connection manager")
        }
        val failures = mutableMapOf<UInt, String>()
        val errors = mutableListOf<String>()
        store.onError = { code, _ -> errors.add(code) }
        val host = LiveMountHost(store, factory, manager) { owner, id, message ->
            assertSame(store, owner)
            if (message == null) failures.remove(id) else failures[id] = message
        }
        fun attach(id: UInt, endpoint: String) {
            assertTrue(store.receive(frame(
                shared(UiOperation.LiveMount) { b -> LiveMount.createLiveMount(
                    b, id, LiveKind.Stream, 9u, LiveTransport.Sse, b.createString(endpoint),
                    0u, ConnectionSharing.Shared, false,
                ) },
                shared(UiOperation.AppendChild) { AppendChild.createAppendChild(it, 1u, id) },
            )))
        }
        host.use {
            attach(50u, "/left")
            attach(51u, "/right")
            val left = events.getValue("/left")
            val right = events.getValue("/right")
            left.disconnected(java.io.IOException("Left offline"))
            right.disconnected(java.io.IOException("Right offline"))
            assertEquals(mapOf(50u to "Left offline", 51u to "Right offline"), failures)

            store.onError?.invoke("ActionFailed", "Invalid action")
            left.opened()
            assertEquals(mapOf(51u to "Right offline"), failures)
            assertEquals(listOf("ActionFailed"), errors)

            assertTrue(store.receive(frame(shared(UiOperation.Remove) { Remove.createRemove(it, 51u) })))
            assertTrue(failures.isEmpty())
            attach(51u, "/replacement")
            events.getValue("/replacement").disconnected(java.io.IOException("Replacement offline"))
            right.opened()
            assertEquals(mapOf(51u to "Replacement offline"), failures)
        }
        assertTrue(failures.isEmpty())
        manager.close()
    }

    @Test
    fun mobileCommandsAndViews() {
        val calls = mutableListOf<UShort>()
        val store =
            NodeStore(
                Clock(),
                commands = CommandHost { id, _ -> calls.add(id) },
                supportsBehaviour = { it == "custom" },
            )
        assertTrue(
            store.receive(
                frame(
                    mobile(MobileOperation.NativeCommand) {
                        NativeCommand.createNativeCommand(it, 999u, 0)
                    },
                    mobile(MobileOperation.NativeCommand) {
                        NativeCommand.createNativeCommand(it, NativeCommandId.HAPTIC, 0)
                    },
                    mobile(MobileOperation.MobileBehaviour) {
                        MobileBehaviour.createMobileBehaviour(
                            it,
                            1u,
                            0u,
                            it.createString("custom"),
                            0,
                        )
                    },
                    mobile(MobileOperation.MountWebView) {
                        MountWebView.createMountWebView(
                            it,
                            1u,
                            0u,
                            it.createString("https://example.org/"),
                        )
                    },
                )
            )
        )
        assertEquals(listOf(NativeCommandId.HAPTIC), calls)
        assertEquals("https://example.org/", store.record(1u)?.webView?.url)
        assertEquals(setOf("custom"), store.record(1u)?.behaviours)
        assertTrue(store.reports.single().toString().contains("UnknownCommand"))
    }

    @Test
    fun truncatedFramesAndClasspathIsolation() {
        assertNotEquals(
            com.google.flatbuffers.Table::class.java,
            ewe.ui.flatbuffers.Table::class.java,
        )
        for (name in listOf("tree", "columnar", "mixed", "replace", "unknown_kind", "behaviour")) {
            val bytes =
                requireNotNull(javaClass.getResource("/native/native_$name.bin")).readBytes()
            for (len in 0 until bytes.size) NodeStore(Clock()).receive(bytes.copyOf(len))
        }
    }

    @Test
    fun snapshotInvalidatesOnlyTheRecordThatChanged() {
        val store = NodeStore(Clock())
        store.receive(
            frame(
                shared(UiOperation.CreateNode) {
                    CreateNode.createCreateNode(it, 2u, NodeKind.TEXT)
                }
            )
        )
        val snapshot = androidx.compose.runtime.snapshots.Snapshot
        snapshot.sendApplyNotifications()
        val observer = androidx.compose.runtime.snapshots.SnapshotStateObserver { it() }
        val changed = mutableListOf<UInt>()
        val recordChanged: (UInt) -> Unit = { changed.add(it) }
        observer.start()
        try {
            for (id in listOf(1u, 2u)) observer.observeReads(id, recordChanged) {
                store.record(id)!!.text
            }
            assertTrue(store.receive(frame(text("now"))))
            assertEquals("now", store.record(1u)!!.text)
            snapshot.sendApplyNotifications()
            assertEquals(listOf(1u), changed)
        } finally {
            observer.stop()
            observer.clear()
        }
    }

    @Test
    fun everyWebMemberGetsAReport() {
        val fixture =
            requireNotNull(javaClass.getResource("/forms/frame_ops_all_forms.bin")).readBytes()
        val root = Frame.getRootAsFrame(java.nio.ByteBuffer.wrap(fixture))
        val body = root.kind(FrameMessage()) as FrameMessage
        val web =
            (0 until body.messagesLength).filter { i ->
                body.messages(i)?.operation?.let { it.webType != WebOperation.NONE } == true
            }
        assertTrue(web.isNotEmpty())
        val store = NodeStore(Clock())
        assertTrue(store.receive(fixture))
        assertEquals(
            web,
            store.reports
                .filter { it["code"].toString() == "\"UnsupportedPlatformOp\"" }
                .map { it.getValue("row").toString().toInt() },
        )
    }
}
