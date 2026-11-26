package ewe.ui.runtime

import ewe.ui.*
import kotlinx.coroutines.*

/** A mounted view retains its loader and its pending recovery. Call this object on the store's thread. */
class ViewRecovery internal constructor(
    private val store: NodeStore,
    private val scope: CoroutineScope,
    private val maxAttempts: Int,
    private val load: suspend () -> ByteArray,
) : AutoCloseable {
    init { require(maxAttempts > 0) }
    private val required = mutableSetOf<ULong>()
    private var flight: Deferred<Result<Unit>>? = null
    private var attempts = 0
    private var closed = false
    private var ready = false
    var generation: Long = 0
        private set

    var onInstalled: (() -> Unit)? = null

    fun accepts(token: Long) = !closed && ready && generation == token
    suspend fun open() = run()

    suspend fun expire(stream: ULong) {
        check(!closed)
        required.add(stream)
        flight?.let { return it.await().getOrThrow() }
        if (attempts >= maxAttempts) {
            ready = false
            error("View recovery attempt limit reached")
        }
        attempts++
        run()
    }

    suspend fun retry() {
        flight?.let { return it.await().getOrThrow() }
        attempts = 0
        run()
    }

    private suspend fun run() {
        check(!closed)
        flight?.let { return it.await().getOrThrow() }
        val current = ++generation
        ready = false
        val task = scope.async(start = CoroutineStart.LAZY) { runCatching {
            val bytes = load()
            ensureActive()
            check(!closed && generation == current)
            ready = true
            try {
                store.installSnapshot(bytes, required)
                required.clear()
                onInstalled?.invoke()
            } catch (error: Exception) {
                ready = false
                throw error
            }
            Unit
        } }
        flight = task
        try { task.await().getOrThrow() } finally { if (flight === task && task.isCompleted) flight = null }
    }

    fun request(stream: ULong) {
        scope.launch {
            try { expire(stream) }
            catch (cancel: CancellationException) { throw cancel }
            catch (error: Exception) { store.onError?.invoke("ViewRecovery",error.message.orEmpty()) }
        }
    }

    override fun close() {
        closed = true
        ready = false
        generation++
        flight?.cancel()
        flight = null
        required.clear()
    }
}
