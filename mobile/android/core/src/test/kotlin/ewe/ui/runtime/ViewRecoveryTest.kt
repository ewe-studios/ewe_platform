package ewe.ui.runtime

import kotlinx.coroutines.*
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

class ViewRecoveryTest {
    private fun bytes(name: String) = requireNotNull(javaClass.getResource("/native/$name.bin")).readBytes()

    @Test fun failuresKeepContentAndExplicitRetryUsesTheSnapshotCursor() = runBlocking {
        val store = NodeStore(GapTimer { _,_ -> Cancel {} })
        var calls = 0
        val loader = store.registerLoader(this, maxAttempts = 1) {
            calls++
            bytes(if (calls < 3) "native_full_view" else "recovery_snapshot")
        }
        loader.open()
        val first = loader.generation
        assertTrue(loader.accepts(first))
        assertTrue(runCatching { loader.expire(1u) }.isFailure)
        assertEquals("Next view",store.record(70u)?.text)
        assertFalse(loader.accepts(first))
        assertTrue(runCatching { loader.expire(1u) }.isFailure)
        assertEquals(2,calls)
        loader.retry()
        assertEquals("Through twenty",store.record(71u)?.text)
        assertEquals(20uL,store.cursor(1u))
        store.close()
        assertFalse(loader.accepts(loader.generation))
    }

    @Test fun simultaneousExpirySharesOneLoadAndClosingDropsItsLateResult() = runBlocking {
        val store = NodeStore(GapTimer { _,_ -> Cancel {} })
        val started = CompletableDeferred<Unit>()
        val gate = CompletableDeferred<Unit>()
        var calls = 0
        val loader = store.registerLoader(this) {
            calls++
            started.complete(Unit)
            gate.await()
            bytes("recovery_snapshot")
        }
        val a = async { runCatching { loader.expire(1u) } }
        started.await()
        val b = async(start = CoroutineStart.UNDISPATCHED) { runCatching { loader.expire(1u) } }
        assertEquals(1,calls)
        loader.close()
        gate.complete(Unit)
        assertTrue(a.await().isFailure)
        assertTrue(b.await().isFailure)
        assertNull(store.record(71u))
    }
}
