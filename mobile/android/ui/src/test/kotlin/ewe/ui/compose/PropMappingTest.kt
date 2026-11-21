package ewe.ui.compose

import androidx.compose.ui.unit.dp
import ewe.ui.compose.behaviours.BehaviourRegistry
import ewe.ui.runtime.Property
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

class PropMappingTest {
    @Test
    fun lengthMapsUnitsAndRejectsInvalidNumbers() {
        assertEquals(8.dp, lengthToDp(Property.Length(8f, 0u)))
        assertNull(lengthToDp(Property.Length(Float.NaN, 0u)))
        assertEquals(.25f, fractionOf(Property.Length(25f, 1u)))
        assertEquals(1f, fractionOf(Property.Length(200f, 1u)))
        assertNull(fractionOf(Property.Length(1f, 2u)))
    }

    @Test
    fun standardBehavioursReachCompose() {
        val registry = BehaviourRegistry.standard
        assertFalse(registry["SCROLL_LOCK"]!!.scrollEnabled)
        assertTrue(registry["DIALOG"]!!.dialog)
        assertTrue(registry["DISMISS"]!!.dismiss)
        assertTrue(registry["TRANSITION"]!!.transition)
        assertFalse(registry.supports("absent"))
    }
}
