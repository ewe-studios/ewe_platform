package ewe.ui.compose

import org.junit.Assert.assertEquals
import org.junit.Test

class ThemeTest {
    @Test
    fun platformOverrideAndDarkAppearanceUseTheSharedManifest() {
        val theme = EweTheme.fromManifest("""{"text-primary":{"color":["#123456","#abcdef"]},
            "p-md":{"padding":["16px","16px"]},"p-sm":{"padding":["8px","8px"]},
            "p-relative":{"padding":["20%","20%"]}}""")
        val classes = "android-p-sm p-md text-primary ios-p-lg android-p-relative web-only"
        assertEquals(mapOf("color" to "#123456", "padding" to "8px"), theme.values(classes, false))
        assertEquals(mapOf("color" to "#abcdef", "padding" to "8px"), theme.values(classes, true))
    }
}
