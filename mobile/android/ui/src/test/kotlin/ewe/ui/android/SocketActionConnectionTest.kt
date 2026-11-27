package ewe.ui.android

import ewe.ui.*
import ewe.ui.flatbuffers.FlatBufferBuilder
import ewe.ui.runtime.Channel
import ewe.ui.runtime.ConnectionEvents
import ewe.ui.runtime.ConnectionManager
import ewe.ui.runtime.SubscriptionFrames
import java.io.IOException
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.async
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.yield
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

class SocketActionConnectionTest {
    private class Socket : Channel {
        lateinit var events: ConnectionEvents
        val requests = mutableListOf<ByteArray>()
        override fun send(bytes: ByteArray) { requests.add(bytes) }
        override fun close() {}
    }

    private class Fixture {
        val socket = Socket()
        val delivered = mutableListOf<ByteArray>()
        val failures = mutableListOf<Throwable>()
        val manager = ConnectionManager().register(LiveTransport.WebSocket) { _, events ->
            socket.events = events
            socket
        }

        fun action(timeout: Long = 1_000) = SocketActionConnection(
            manager, "ws://example.test/socket", { delivered.add(it) }, { failures.add(it) },
            handshakeTimeoutMillis = 1_000, actionTimeoutMillis = timeout,
        )
    }

    @Test fun matchesOutOfOrderRepliesOnTheSharedSocket() = runBlocking {
        val fixture = Fixture()
        fixture.action().use { first ->
            fixture.action().use { second ->
                val hello1 = async(start = CoroutineStart.UNDISPATCHED) {
                    first.send(SubscriptionFrames.hello(first.requestTag))
                }
                val hello2 = async(start = CoroutineStart.UNDISPATCHED) {
                    second.send(SubscriptionFrames.hello(second.requestTag))
                }
                fixture.socket.events.deliver(capabilities(16u, second.requestTag))
                assertFalse(hello1.isCompleted)
                fixture.socket.events.deliver(capabilities(15u, first.requestTag))
                assertArrayEquals(capabilities(15u, first.requestTag), hello1.await())
                assertArrayEquals(capabilities(16u, second.requestTag), hello2.await())

                val call1 = async(start = CoroutineStart.UNDISPATCHED) { first.send(request(15u, 2u)) }
                val call2 = async(start = CoroutineStart.UNDISPATCHED) { first.send(request(15u, 3u)) }
                val call3 = async(start = CoroutineStart.UNDISPATCHED) { second.send(request(16u, 2u)) }
                fixture.socket.events.deliver(acknowledge(15u, 3u))
                assertArrayEquals(acknowledge(15u, 3u), call2.await())
                assertFalse(call1.isCompleted)
                assertFalse(call3.isCompleted)

                val rejection = acknowledge(15u, 2u, reject = true)
                fixture.socket.events.deliver(rejection)
                assertArrayEquals(rejection, call1.await())
                fixture.socket.events.deliver(acknowledge(16u, 2u))
                assertArrayEquals(acknowledge(16u, 2u), call3.await())
                assertTrue(fixture.delivered.isEmpty())

                fixture.socket.events.disconnected(IOException("Shared socket closed"))
                val ready1 = async(start = CoroutineStart.UNDISPATCHED) { first.awaitReady() }
                val ready2 = async(start = CoroutineStart.UNDISPATCHED) { second.awaitReady() }
                fixture.socket.events.opened()
                val caps1 = capabilities(17u, first.requestTag)
                val caps2 = capabilities(18u, second.requestTag)
                fixture.socket.events.deliver(caps2)
                ready2.await()
                assertFalse(ready1.isCompleted)
                fixture.socket.events.deliver(acknowledge(15u, 2u))
                fixture.socket.events.deliver(caps1)
                ready1.await()
                assertEquals(2, fixture.delivered.size)
                assertArrayEquals(caps2, fixture.delivered[0])
                assertArrayEquals(caps1, fixture.delivered[1])
            }
        }
    }

    @Test fun timeoutFailsTheWaitAndDeliversALateReply() = runBlocking {
        val fixture = Fixture()
        fixture.action(timeout = 35).use { connection ->
            val hello = async(start = CoroutineStart.UNDISPATCHED) {
                connection.send(SubscriptionFrames.hello(connection.requestTag))
            }
            fixture.socket.events.deliver(capabilities(15u, connection.requestTag))
            hello.await()

            val reply = runCatching { connection.send(request(15u, 2u)) }
            assertInstanceOf(IOException::class.java, reply.exceptionOrNull())
            assertEquals("Action reply timed out", reply.exceptionOrNull()?.message)
            val late = acknowledge(15u, 2u)
            fixture.socket.events.deliver(late)
            assertArrayEquals(late, fixture.delivered.single())
            assertTrue(fixture.failures.isEmpty())
        }
    }

    @Test fun cancellationReleasesTheActionWait() = runBlocking {
        val fixture = Fixture()
        fixture.action().use { connection ->
            val hello = async(start = CoroutineStart.UNDISPATCHED) {
                connection.send(SubscriptionFrames.hello(connection.requestTag))
            }
            fixture.socket.events.deliver(capabilities(15u, connection.requestTag))
            hello.await()

            val action = request(15u, 2u)
            val pending = async(start = CoroutineStart.UNDISPATCHED) { connection.send(action) }
            assertArrayEquals(action, fixture.socket.requests.last())
            pending.cancelAndJoin()
            assertTrue(pending.isCancelled)

            val reply = acknowledge(15u, 2u)
            fixture.socket.events.deliver(reply)
            assertArrayEquals(reply, fixture.delivered.single())
            assertTrue(fixture.failures.isEmpty())
        }
    }

    @Test fun failsPendingActionsAndWaitsForFreshCapabilitiesBeforeRecovery() = runBlocking {
        val fixture = Fixture()
        fixture.action().use { connection ->
            val hello = async(start = CoroutineStart.UNDISPATCHED) {
                connection.send(SubscriptionFrames.hello(connection.requestTag))
            }
            fixture.socket.events.deliver(capabilities(15u, connection.requestTag))
            hello.await()

            val reply = async(start = CoroutineStart.UNDISPATCHED) {
                runCatching { connection.send(request(15u, 2u)) }
            }
            val cause = IOException("Disconnected after sending")
            fixture.socket.events.disconnected(cause)
            val failure = reply.await().exceptionOrNull()
            assertInstanceOf(IOException::class.java, failure)
            assertEquals(cause.message, failure?.message)
            assertSame(cause, fixture.failures.single())

            val recovery = async(start = CoroutineStart.UNDISPATCHED) {
                connection.awaitReady()
                assertArrayEquals(capabilities(16u, connection.requestTag), fixture.delivered.single())
                connection.send(request(16u, 3u))
            }
            assertFalse(recovery.isCompleted)
            fixture.socket.events.opened()
            assertArrayEquals(SubscriptionFrames.hello(connection.requestTag), fixture.socket.requests.last())
            fixture.socket.events.deliver(acknowledge(15u, 2u))
            assertTrue(fixture.delivered.isEmpty())
            fixture.socket.events.deliver(capabilities(16u, connection.requestTag))
            yield()
            assertArrayEquals(request(16u, 3u), fixture.socket.requests.last())
            fixture.socket.events.deliver(acknowledge(16u, 3u))
            assertArrayEquals(acknowledge(16u, 3u), recovery.await())
        }
    }

    private fun request(stream: ULong, sequence: ULong): ByteArray {
        val builder = FlatBufferBuilder()
        val body = FrameRequest.createFrameRequest(builder, 0, 0)
        val frame = Frame.createFrame(builder, 1u, stream, sequence, FrameKind.FrameRequest, body)
        Frame.finishFrameBuffer(builder, frame)
        return builder.sizedByteArray()
    }

    private fun capabilities(stream: ULong, tag: ULong): ByteArray {
        val builder = FlatBufferBuilder()
        val caps = ServerCaps.createServerCaps(builder, CapsStatus.Ok, 1u, 13u, stream, 0u, 0, true, tag)
        return reply(builder, stream, 0u, Control.createControl(builder, caps, 0, 0, 0))
    }

    private fun acknowledge(stream: ULong, sequence: ULong, reject: Boolean = false): ByteArray {
        val builder = FlatBufferBuilder()
        val control = if (reject) {
            val text = builder.createString("Rejected")
            val error = ActionError.createActionError(builder, ErrorCode.InvalidInput, text, 0u)
            Control.createControl(builder, 0, 0, 0, error)
        } else 0
        return reply(builder, stream, sequence, control)
    }

    private fun reply(builder: FlatBufferBuilder, stream: ULong, sequence: ULong, control: Int): ByteArray {
        val messages = if (control != 0) {
            val message = Message.createMessage(builder, 0, 0, control)
            FrameMessage.createMessagesVector(builder, intArrayOf(message))
        } else 0
        val body = FrameMessage.createFrameMessage(builder, sequence, messages)
        val frame = Frame.createFrame(builder, 1u, stream, 1u, FrameKind.FrameMessage, body)
        Frame.finishFrameBuffer(builder, frame)
        return builder.sizedByteArray()
    }
}
