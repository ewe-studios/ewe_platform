package ewe.ui.compose

import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.v2.createComposeRule
import ewe.ui.*
import ewe.ui.flatbuffers.FlatBufferBuilder
import ewe.ui.runtime.*
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test

class ContainerActionTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun typedTitleReachesOneBoundActionFromEachSubmission() {
        val store = NodeStore(GapTimer { _, _ -> Cancel {} })
        assertTrue(store.receive(formFrame()))
        assertTrue(store.reports.isEmpty())
        val calls = mutableListOf<EventInput>()
        val core = object : ActionCore {
            override fun hello() = byteArrayOf()
            override fun applyCapsFrame(frame: ByteArray) = Unit
            override suspend fun trigger(reference: String, event: EventInput): ByteArray? {
                calls += event
                return null
            }
        }
        val theme = androidx.compose.runtime.mutableStateOf(EweTheme())
        store.record(4u)!!.props[PropId.UTILITY_CLASSES] = Property.Text("p-md web-only")
        compose.setContent {
            val scope = rememberCoroutineScope()
            EweScreen(store, androidx.compose.runtime.remember { EventBridge(store, core, scope) }, theme = theme.value)
        }
        val field = compose.onNode(hasSetTextAction())
        field.performTextInput("Buy milk")
        compose.runOnIdle {
            theme.value = EweTheme.fromManifest("""{"p-md":{"padding":["8px","8px"]}}""")
        }
        field.assertIsFocused().assertTextEquals("Buy milk")
        assertTrue(calls.isEmpty())
        compose.onNodeWithText("Add").performClick()
        field.performImeAction()
        compose.runOnIdle {
            assertEquals(listOf(4u, 4u), calls.map { it.nodeId })
            assertTrue(calls.all { it.eventName == "submit" && it.value == "Buy milk" })
            store.record(5u)!!.actions[EventId.TAP] = "host:custom_add"
        }
        compose.onNodeWithText("Add").performClick()
        compose.runOnIdle {
            assertEquals(3, calls.size)
            assertEquals(5u, calls.last().nodeId)
            assertEquals("tap", calls.last().eventName)
            assertEquals("Buy milk", calls.last().value)
        }
    }

    private fun formFrame(): ByteArray {
        val b = FlatBufferBuilder()
        val rows = mutableListOf<Int>()
        fun emit(type: UByte, payload: Int) {
            val op = Operation.createOperation(b, type, payload, 0u, 0, 0u, 0, 0)
            rows += Message.createMessage(b, op, 0, 0)
        }
        fun add(parent: UInt, id: UInt, kind: UShort) {
            emit(UiOperation.CreateNode, CreateNode.createCreateNode(b, id, kind))
            emit(UiOperation.AppendChild, AppendChild.createAppendChild(b, parent, id))
        }
        add(1u, 2u, NodeKind.MODAL)
        add(2u, 3u, NodeKind.BOTTOM_SHEET)
        add(3u, 4u, NodeKind.TEXT_INPUT)
        add(3u, 5u, NodeKind.SUBMIT_BUTTON)
        emit(UiOperation.BindAction, BindAction.createBindAction(b, 4u, EventId.SUBMIT, b.createString("host:add")))
        emit(UiOperation.SetText, SetText.createSetText(b, 5u, b.createString("Add")))
        val body = FrameMessage.createFrameMessage(b, 0uL, FrameMessage.createMessagesVector(b, rows.toIntArray()))
        Frame.finishFrameBuffer(b, Frame.createFrame(b, 1u, 0u, 0uL, FrameKind.FrameMessage, body))
        return b.sizedByteArray()
    }
}
