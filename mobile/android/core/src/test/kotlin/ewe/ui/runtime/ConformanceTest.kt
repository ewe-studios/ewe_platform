package ewe.ui.runtime

import kotlinx.serialization.json.Json
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

class ConformanceTest {
    private val timer = GapTimer { _, _ -> Cancel {} }

    private fun fixture(name: String) =
        requireNotNull(javaClass.getResource("/native/$name")).readBytes()

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
