package ewe.ui.runtime

import androidx.compose.runtime.*
import ewe.ui.NodeKind

sealed interface Property {
    val label: String

    data class Length(val number: Float, val unit: UByte) : Property {
        override val label: String
            get() {
                val value = if (number % 1f == 0f) number.toLong().toString() else number.toString()
                return when (unit.toInt()) {
                    0 -> "${value}pt"
                    1 -> "${value}%"
                    else -> "auto"
                }
            }
    }

    data class Color(val token: UInt) : Property {
        override val label = "color:$token"
    }

    data class Font(val token: UInt) : Property {
        override val label = "font:$token"
    }

    data class Align(override val label: String) : Property

    data class Axis(override val label: String) : Property

    data class Flag(val value: Boolean) : Property {
        override val label = value.toString()
    }

    data class Text(override val label: String) : Property
}

data class LiveSpec(
    val kind: UByte,
    val stream: ULong,
    val transport: UByte,
    val endpoint: String,
    val resume: ULong,
)

data class WebViewSpec(val parent: UInt, val url: String)

class NodeRecord(val id: UInt, initialKind: UShort = NodeKind.UNKNOWN) {
    var kind by mutableStateOf(initialKind)
    var text by mutableStateOf<String?>(null)
    val props = mutableStateMapOf<UShort, Property>()
    val children = mutableStateListOf<UInt>()
    val actions = mutableStateMapOf<UShort, String>()
    private var stateBytes = byteArrayOf()
    var state: ByteArray
        get() = stateBytes.copyOf()
        internal set(value) {
            stateBytes = value.copyOf()
        }

    var parent: UInt? = null
        internal set

    var live by mutableStateOf<LiveSpec?>(null)
    var webView by mutableStateOf<WebViewSpec?>(null)
    var behaviours by mutableStateOf(emptySet<String>())
}
