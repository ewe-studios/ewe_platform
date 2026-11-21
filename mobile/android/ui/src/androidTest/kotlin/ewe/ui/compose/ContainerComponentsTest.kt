package ewe.ui.compose

import androidx.compose.foundation.layout.*
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.unit.dp
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class ContainerComponentsTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun overlaySheetUsesBottomEdgeAndAllowsBackgroundTaps() {
        val shown = mutableStateOf(true)
        var taps = 0
        compose.setContent {
            LayerHost(Modifier.size(250.dp).testTag("screen")) {
                Button(onClick = { taps++ }) { Text("Background") }
                if (shown.value) {
                    OverlayComponent.Render(Modifier) {
                        BottomSheetComponent.Render(Modifier.testTag("sheet")) {
                            Text("First")
                            Text("Second")
                        }
                    }
                }
            }
        }
        val screen = compose.onNodeWithTag("screen").getUnclippedBoundsInRoot()
        val panel = compose.onNodeWithTag("sheet").getUnclippedBoundsInRoot()
        assertEquals(screen.bottom.value, panel.bottom.value, 1f)
        assertTrue(panel.bottom - panel.top < screen.bottom - screen.top)
        compose.onNodeWithText("Background").performTouchInput { click() }
        compose.runOnIdle {
            assertEquals(1, taps)
            shown.value = false
        }
        compose.onNodeWithText("Second").assertDoesNotExist()
    }

    @Test
    fun inlineSheetMeasuresItsChildrenAndLeavesRoomForTheBody() {
        val extra = mutableStateOf(false)
        compose.setContent {
            InlineSheetLayout(Modifier.size(300.dp).testTag("screen"), {
                Spacer(Modifier.fillMaxSize().testTag("body"))
            }, {
                BottomSheetComponent.Render(Modifier.testTag("sheet")) {
                    Text("Editor")
                    if (extra.value) Spacer(Modifier.height(32.dp))
                }
            })
        }
        val initial = compose.onNodeWithTag("sheet").getUnclippedBoundsInRoot()
        compose.runOnIdle { extra.value = true }
        val updated = compose.onNodeWithTag("sheet").getUnclippedBoundsInRoot()
        val body = compose.onNodeWithTag("body").getUnclippedBoundsInRoot()
        assertEquals(initial.bottom.value, updated.bottom.value, 1f)
        assertTrue(initial.top - updated.top >= 32.dp)
        assertEquals(updated.top.value, body.bottom.value, 1f)
    }

    @Test
    fun modalHostsItsChildrenAndHandlesBackOnce() {
        val shown = mutableStateOf(true)
        var dismissals = 0
        compose.setContent {
            if (shown.value) ModalComponent.Render(Modifier, false, {
                dismissals++
                shown.value = false
            }) {
                Text("Modal contents")
            }
        }
        compose.onNodeWithText("Modal contents").assertIsDisplayed()
        compose.onAllNodes(isDialog()).assertCountEquals(1)
        InstrumentationRegistry.getInstrumentation().sendKeyDownUpSync(android.view.KeyEvent.KEYCODE_BACK)
        compose.waitUntil { !shown.value }
        compose.runOnIdle { assertEquals(1, dismissals) }
        compose.onNodeWithText("Modal contents").assertDoesNotExist()
    }

    @Test
    fun modalSheetKeepsOneInteractiveCopyOfEachChild() {
        var taps = 0
        compose.setContent {
            ModalComponent.Render(Modifier, true, {}) {
                BottomSheetComponent.Render(Modifier) {
                    Button(onClick = { taps++ }) { Text("Save") }
                }
            }
        }
        compose.onNodeWithText("Save").performClick()
        compose.runOnIdle { assertEquals(1, taps) }
    }
}
