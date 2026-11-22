package ewe.ui.runtime

import kotlinx.serialization.json.*
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

class ConformanceTest {
    private val timer = GapTimer { _, _ -> Cancel {} }

    private fun fixture(name: String) =
        requireNotNull(javaClass.getResource("/native/$name")).readBytes()

    @Test
    fun portableTargets() {
        val store = NodeStore(GapTimer { _, _ -> Cancel {} })
        store.registerView("main", 1u)
        store.registerView("second", 7u)
        val steps = Json.parseToJsonElement(fixture("portable_targets.json").decodeToString()).jsonArray
        for (step in steps.map { it.jsonObject }) {
            if (step.getValue("case").jsonPrimitive.content == "custom") store.registerElement("save","InvoiceEditor",42u)
            val bytes = step.getValue("bytes").jsonArray.map { it.jsonPrimitive.int.toByte() }.toByteArray()
            val count = store.reports.size
            val mount = step["mount"]?.jsonPrimitive?.intOrNull?.toUInt()
            assertTrue(if (mount == null) store.receive(bytes) else store.receive(bytes, mount))
            val error = step["error"]?.jsonPrimitive?.contentOrNull
            assertEquals(listOfNotNull(error),store.reports.drop(count).map { it.getValue("code").jsonPrimitive.content },step.getValue("case").jsonPrimitive.content)
        }
        assertEquals(listOf(44u,48u),store.record(1u)?.children)
        assertEquals(listOf(41u,47u),store.record(7u)?.children)
        assertTrue(store.receive(fixture("portable_count.bin"),1u))
        val countNode = requireNotNull(store.record(1u)).children.last()
        assertEquals("1 open todo",store.record(countNode)?.text)
        val second = requireNotNull(store.viewStore("second"))
        second.registerElement("open-count","ListFooter",47u)
        assertTrue(store.receive(fixture("portable_count.bin"),7u))
        assertEquals(countNode, second.record(7u)?.children?.last())
        assertEquals("1 open todo",second.record(countNode)?.text)
        assertNotSame(store.record(countNode), second.record(countNode))
        store.unregisterView("main")
        assertEquals("1 open todo",second.record(countNode)?.text)
        assertNull(store.record(44u))
        assertEquals(listOf(41u,countNode),store.record(7u)?.children)
    }

    @Test
    fun sharedSnapshots() {
        for (name in listOf("tree", "full_view", "columnar", "mixed", "replace", "unknown_kind", "behaviour", "presentation")) {
            val store = NodeStore(timer)
            if (name in listOf("full_view", "mixed", "replace", "behaviour", "presentation"))
                assertTrue(store.receive(fixture("native_tree.bin")))
            assertTrue(store.receive(fixture("native_$name.bin")), name)
            val expected =
                Json.parseToJsonElement(fixture("native_$name.expected.json").decodeToString())
            assertEquals(expected, Json.parseToJsonElement(store.snapshotJson()), name)
        }
    }

    @Test
    fun reloadReplacesOnlyTheReceivingMount() {
        val store = NodeStore(timer)
        assertTrue(store.receive(fixture("native_tree.bin")))
        val bytes = fixture("native_full_view.bin")
        repeat(2) { assertTrue(store.receive(bytes, mount = 41u)) }
        assertEquals(listOf(70u), store.record(41u)?.children)
        assertEquals(listOf(41u, 42u), store.record(40u)?.children)
        assertEquals("Next view", store.record(70u)?.text)
        assertTrue(store.reports.isEmpty())
    }

    @Test
    fun invalidInputLeavesTheRoot() {
        val store = NodeStore(timer, maxFrameBytes = 64)
        assertFalse(store.receive(ByteArray(65)))
        assertFalse(store.receive(byteArrayOf(1, 2)))
        assertFalse(store.receive(byteArrayOf(0, 0, 0, 0, 69, 87, 70, 49)))
        assertEquals(1u, store.nodes.single().id)
        assertEquals(3, store.reports.size)
    }

    @Test
    fun customSignerBytesReachBothControlsAndRequest() {
        fun signed(name: String) =
            requireNotNull(javaClass.getResource("/signed/$name")).readBytes()
        val expected = signed("state_counter_custom.json")
        val store = NodeStore(timer)
        assertTrue(store.receive(fixture("native_tree.bin")))
        assertTrue(store.receive(signed("frame_state_custom.bin")))
        for (id in listOf(40u, 42u)) assertArrayEquals(expected, store.record(id)?.state)
        val root =
            ewe.ui.Frame.getRootAsFrame(
                java.nio.ByteBuffer.wrap(signed("action_increment_custom.bin"))
            )
        val action = (root.kind(ewe.ui.FrameRequest()) as ewe.ui.FrameRequest).action!!
        assertArrayEquals(expected, ByteArray(action.stateLength) { action.state(it).toByte() })
    }
}
