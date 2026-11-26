package ewe.ui.compose

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import org.json.JSONObject
import ewe.ui.PropId
import ewe.ui.runtime.NodeRecord
import ewe.ui.runtime.Property

/** Named utilities from the Rust theme, with optional explicit wire token mappings. */
class EweTheme(
    val colors: Map<UInt, Color> = emptyMap(),
    val fonts: Map<UInt, TextStyle> = emptyMap(),
    private val utilities: Map<String, Map<String, Pair<String, String>>> = emptyMap(),
) {
    companion object {
        /** Read the output of GeneratedTheme.native_manifest. */
        fun fromManifest(json: String): EweTheme {
            val manifest = JSONObject(json)
            return EweTheme(utilities = manifest.keys().asSequence().associateWith { name ->
                val properties = manifest.getJSONObject(name)
                properties.keys().asSequence().associateWith { key ->
                    val pair = properties.getJSONArray(key)
                    require(pair.length() == 2)
                    pair.getString(0) to pair.getString(1)
                }
            })
        }

        internal fun length(text: String): Float? =
            text.removeSuffix("px").removeSuffix("pt").toFloatOrNull()?.takeIf { it.isFinite() && it >= 0 }

        internal fun color(text: String): Color? {
            if (!text.startsWith('#')) return null
            var hex = text.substring(1)
            if (hex.length == 3 || hex.length == 4) hex = hex.flatMap { listOf(it, it) }.joinToString("")
            if (hex.length == 6) hex += "ff"
            if (hex.length != 8) return null
            val rgba = hex.toLongOrNull(16) ?: return null
            return Color(((rgba ushr 8) or ((rgba and 255) shl 24)).toInt())
        }
    }

    internal fun values(classes: String, dark: Boolean): Map<String, String> = buildMap {
        val words = classes.split(Regex("\\s+"))
        for (specific in listOf(false, true)) {
            for (word in words) {
                if (word.startsWith("android-") != specific) continue
                for ((key, pair) in utilities[word.removePrefix("android-")].orEmpty()) {
                    val value = if (dark) pair.second else pair.first
                    val supported = when (key) {
                        "color", "background" -> color(value) != null
                        "padding", "font-size" -> length(value) != null
                        else -> false
                    }
                    if (supported) put(key, value)
                }
            }
        }
    }
}

/** Resolve explicit properties and class values before the native control applies its defaults. */
internal class NodeAppearance(
    node: NodeRecord,
    style: ScreenStyle,
    dark: Boolean,
) {
    private val values = style.theme.values(node.props[PropId.UTILITY_CLASSES]?.label.orEmpty(), dark)
    val foreground: Color?
    val background: Color?
    val font: TextStyle?
    val fontSize: Float?
    val padding: Float

    init {
        fun color(prop: UShort, key: String): Color? {
            val explicit = node.props[prop] as? Property.Color
            return if (explicit != null) style.colors(explicit.token) else values[key]?.let(EweTheme::color)
        }
        foreground = color(PropId.COLOR, "color")
        background = color(PropId.BACKGROUND, "background")
        val token = node.props[PropId.FONT] as? Property.Font
        font = token?.let { style.fonts(it.token) }
        fontSize = if (token == null) values["font-size"]?.let(EweTheme::length) else null
        padding = (node.props[PropId.PADDING] as? Property.Length)?.number
            ?: values["padding"]?.let(EweTheme::length) ?: 0f
    }
}
