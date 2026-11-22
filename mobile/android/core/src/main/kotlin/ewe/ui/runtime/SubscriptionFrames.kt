package ewe.ui.runtime

import ewe.ui.*
import ewe.ui.flatbuffers.FlatBufferBuilder
import java.nio.ByteBuffer
import java.nio.ByteOrder

/** Application messages for a live view. A connection can carry several subscriptions. */
object SubscriptionFrames {
    fun hello(tag: ULong): ByteArray {
        val b = FlatBufferBuilder()
        val hello = ClientHello.createClientHello(b, 12u, Platform.Android, MountSurface.Native, 0u, true, tag)
        return request(b, 0u, FrameRequest.createFrameRequest(b, 0, hello, 0))
    }

    fun resume(session: ULong, id: ULong, stream: ULong, after: ULong): ByteArray {
        require(session != 0uL && id != 0uL && stream != 0uL)
        val b = FlatBufferBuilder()
        val cursor = ResumeSubscription.createResumeSubscription(b, session, id, stream, after)
        return request(b, session, FrameRequest.createFrameRequest(b, 0, 0, cursor))
    }

    /** Deliver status 410 to the listener that sent the resume request. */
    fun expiredRequest(bytes: ByteArray): ByteArray {
        val frame = Frame.getRootAsFrame(ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN))
        require(frame.kindType == FrameKind.FrameRequest)
        val resume = requireNotNull((frame.kind(FrameRequest()) as FrameRequest).resume)
        return expired(resume.streamId,resume.after,resume.subscriptionId)
    }

    fun expired(stream: ULong, after: ULong, subscription: ULong = 0uL): ByteArray {
        val b = FlatBufferBuilder()
        val expired = ReplayExpired.createReplayExpired(b,stream,after)
        val control = Control.createControl(b,0,0,0,0,expired)
        val message = Message.createMessage(b,0,0,control)
        val items = FrameMessage.createMessagesVector(b,intArrayOf(message))
        val body = FrameMessage.createFrameMessage(b,0u,items,subscription)
        Frame.finishFrameBuffer(b,Frame.createFrame(b,1u,stream,0u,FrameKind.FrameMessage,body))
        return b.sizedByteArray()
    }

    private fun request(b: FlatBufferBuilder, session: ULong, body: Int): ByteArray {
        Frame.finishFrameBuffer(b, Frame.createFrame(b, 1u, session, 0u, FrameKind.FrameRequest, body))
        return b.sizedByteArray()
    }

    fun caps(bytes: ByteArray): ServerCaps? {
        val input = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
        require(Frame.FrameBufferHasIdentifier(input))
        val packet = Frame.getRootAsFrame(input)
        val body = packet.kind(FrameMessage()) as FrameMessage
        return if (body.messagesLength > 0) body.messages(0)?.control?.caps else null
    }
}
