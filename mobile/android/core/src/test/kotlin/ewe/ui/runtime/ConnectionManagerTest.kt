package ewe.ui.runtime

import ewe.ui.LiveTransport
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

class ConnectionManagerTest {
    @Test fun sharedSocketKeepsSenderAndIsolatesBytes() {
        val manager = ConnectionManager()
        val opened = mutableListOf<ConnectionEvents>()
        var closed = 0
        manager.register(LiveTransport.WebSocket) { _, events -> opened.add(events); Channel { closed++ } }
        fun handle(receive: ((ByteArray) -> Unit)? = null) = manager.acquire(LiveTransport.WebSocket, "/live", onError = { throw it }, receive = receive)
        val first = handle { it[0] = 0 }
        var secondValue = 0
        val second = handle { secondValue = it[0].toInt() }
        val sender = handle()
        opened.single().deliver(byteArrayOf(8))
        assertEquals(8, secondValue)
        first.close(); second.close()
        assertFalse(opened.single().listening)
        assertEquals(0, closed)
        sender.close()
        assertEquals(1, closed)
    }

    @Test fun eventConnectionsShareOnlyWithMatchingSettingsAndRetainUntilReleased() {
        val manager = ConnectionManager()
        var opened = 0
        var closed = 0
        manager.register(LiveTransport.Sse) { _, _ -> opened++; Channel { closed++ } }
        fun handle(shared: Boolean? = null, keep: Boolean = false, url: String = "/live") =
            manager.acquire(LiveTransport.Sse, url, shared, keep, onError = { throw it })
        handle().close(); handle().close()
        val retained = handle(true, true)
        handle(true).close()
        handle(true, url = "/live?before=2").close()
        retained.close()
        assertEquals(4, opened)
        assertEquals(3, closed)
        handle(true).close()
        retained.releaseRetention()
        assertEquals(4, closed)
    }
}
