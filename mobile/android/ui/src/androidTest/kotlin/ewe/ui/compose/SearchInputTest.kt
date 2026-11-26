package ewe.ui.compose

import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.v2.createComposeRule
import ewe.ui.EventId
import ewe.ui.NodeKind
import ewe.ui.PropId
import ewe.ui.runtime.*
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

class SearchInputTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun editSubmitClearAndRecoverQuery() {
        val store = NodeStore(GapTimer { _, _ -> Cancel {} })
        val node = requireNotNull(store.record(1u))
        node.kind = NodeKind.SEARCH_INPUT
        node.props[PropId.VALUE] = Property.Text("old")
        node.props[PropId.A11Y_LABEL] = Property.Text("Search todos")
        node.actions[EventId.CHANGE] = "host:filter"
        node.actions[EventId.SUBMIT] = "host:submit"
        val calls = mutableListOf<Pair<String, String>>()
        val core = object : ActionCore {
            override fun hello() = byteArrayOf()
            override fun applyCapsFrame(frame: ByteArray) = Unit
            override suspend fun trigger(reference: String, event: EventInput): ByteArray? {
                calls += reference to event.value
                return null
            }
        }
        compose.setContent {
            val scope = rememberCoroutineScope()
            EweScreen(store, remember { EventBridge(store, core, scope) })
        }
        val field = compose.onNodeWithContentDescription("Search todos")
        field.assertTextContains("old")
        field.performTextReplacement("milk")
        field.performImeAction()
        field.assertIsNotFocused()
        compose.onNodeWithContentDescription("Clear search").performClick()
        compose.onNodeWithContentDescription("Clear search").assertDoesNotExist()
        compose.runOnIdle {
            assertEquals(listOf("host:filter" to "milk", "host:submit" to "milk", "host:filter" to ""), calls)
            node.props[PropId.VALUE] = Property.Text("restored")
            node.props[PropId.ENABLED] = Property.Flag(false)
        }
        field.assertTextContains("restored").assertIsNotEnabled()
        compose.onNodeWithContentDescription("Clear search").assertIsNotEnabled()
    }
}
