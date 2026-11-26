package ewe.ui.compose.behaviours

import androidx.compose.ui.Modifier
import ewe.ui.runtime.NodeRecord

interface BehaviourBinding {
    val name: String

    fun modifier(record: NodeRecord): Modifier = Modifier

    val scrollEnabled: Boolean
        get() = true

    val dialog: Boolean
        get() = false

    val dismiss: Boolean
        get() = false

    val transition: Boolean
        get() = false
}

class BehaviourRegistry {
    private val bindings = mutableMapOf<String, BehaviourBinding>()

    fun register(binding: BehaviourBinding) {
        bindings[binding.name] = binding
    }

    operator fun get(name: String) = bindings[name]

    fun supports(name: String) = name in bindings

    companion object {
        val standard: BehaviourRegistry
            get() =
                BehaviourRegistry().apply {
                    register(DialogBinding)
                    register(DismissBinding)
                    register(ScrollLockBinding)
                    register(TransitionBinding)
                }
    }
}
