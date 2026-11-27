package ewe.ui.runtime

import ewe.ui.*
import ewe.ui.flatbuffers.FlatBufferBuilder
import kotlinx.coroutines.*
import kotlinx.serialization.json.jsonPrimitive
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
    fun dedicatedMountUsesItsOwnConnectionDuringReplay() {
        val store = NodeStore(Clock())
        val caps = frame({ b ->
            val server = ServerCaps.createServerCaps(b, CapsStatus.Ok, 1u, 1u, 9u, 0u, 0, true, 0u)
            Message.createMessage(b, 0, 0, Control.createControl(b, server, 0, 0, 0))
        })
        assertTrue(store.receive(caps))
        assertTrue(store.subscriptionReplay)
        val direct = mutableListOf<Pair<ULong, ULong>>()
        var directClosed = 0
        val factory = object : ChannelFactory {
            override fun open(
                endpoint: String, transport: UByte, streamId: ULong, resumeFrom: ULong,
                onFrame: (ByteArray) -> Unit, onClose: (Throwable?) -> Unit,
            ): Channel {
                direct += streamId to resumeFrom
                return Channel { directClosed++ }
            }
        }
        var managedOpened = 0
        var managedClosed = 0
        val manager = ConnectionManager()
        manager.register(LiveTransport.WebSocket) { _, _ ->
            managedOpened++
            Channel { managedClosed++ }
        }

        fun attach(id: UInt, stream: ULong, cursor: ULong, sharing: UByte, keepAlive: Boolean = false) {
            assertTrue(store.receive(frame(
                shared(UiOperation.LiveMount) { b ->
                    LiveMount.createLiveMount(
                        b, id, LiveKind.Stream, stream, LiveTransport.WebSocket, b.createString("/live"),
                        cursor, sharing, keepAlive,
                    )
                },
                shared(UiOperation.AppendChild) { AppendChild.createAppendChild(it, 1u, id) },
            )))
        }

        manager.use {
            LiveMountHost(store, factory, manager).use {
                store.setCursor(9u, 23u)
                attach(50u, 9u, 19u, ConnectionSharing.Dedicated)
                attach(51u, 10u, 51u, ConnectionSharing.Dedicated)
                assertEquals(listOf(9uL to 23uL, 10uL to 51uL), direct)
                assertEquals(0, managedOpened)

                store.setCursor(9u, 24u)
                store.onGap?.invoke(9u, 24u)
                assertEquals(listOf(9uL to 23uL, 10uL to 51uL, 9uL to 24uL), direct)
                assertEquals(1, directClosed)

                attach(52u, 11u, 0u, ConnectionSharing.TransportDefault)
                attach(53u, 12u, 0u, ConnectionSharing.Shared)
                assertEquals(1, managedOpened)
                attach(54u, 13u, 0u, ConnectionSharing.Dedicated, keepAlive = true)
                attach(55u, 14u, 0u, ConnectionSharing.Dedicated, keepAlive = true)
                assertEquals(3, managedOpened)
                assertTrue(store.receive(frame(
                    shared(UiOperation.Remove) { Remove.createRemove(it, 54u) },
                    shared(UiOperation.Remove) { Remove.createRemove(it, 55u) },
                )))
                assertEquals(0, managedClosed)
            }
            assertEquals(3, directClosed)
            assertEquals(1, managedClosed)
        }
        assertEquals(3, managedClosed)
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
    fun unknownBehaviourReportsPreserveNamesAndApplyTheNextRow() {
        val store = NodeStore(Clock())
        assertTrue(
            store.receive(
                frame(
                    shared(UiOperation.CreateNode) {
                        CreateNode.createCreateNode(it, 40u, NodeKind.TEXT)
                    },
                    shared(UiOperation.AppendChild) { AppendChild.createAppendChild(it, 1u, 40u) },
                )
            )
        )
        val unknownNumeric = shared(UiOperation.UiBehaviour) {
            UiBehaviour.createUiBehaviour(it, 40u, 999u, 0)
        }
        val unknownName = mobile(MobileOperation.MobileBehaviour) {
            val name = it.createString("fixture_unregistered")
            MobileBehaviour.createMobileBehaviour(it, 40u, MobileBehaviourId.UNKNOWN, name, 0)
        }
        val validText = shared(UiOperation.SetText) {
            SetText.createSetText(it, 40u, it.createString("Still visible"))
        }

        assertTrue(store.receive(frame(unknownNumeric, unknownName, validText)))
        val reports = store.reports.map { report ->
            listOf("code", "row", "node", "name").map { report.getValue(it).jsonPrimitive.content }
        }
        assertEquals(
            listOf(
                listOf("UnknownBehaviour", "0", "40", "999"),
                listOf("UnknownBehaviour", "1", "40", "fixture_unregistered"),
            ),
            reports,
        )
        assertEquals(listOf(40u), store.record(1u)!!.children)
        assertEquals("Still visible", store.record(40u)!!.text)
        assertTrue(store.record(40u)!!.behaviours.isEmpty())
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
