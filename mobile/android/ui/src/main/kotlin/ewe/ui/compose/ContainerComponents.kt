package ewe.ui.compose

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import ewe.ui.EventId
import ewe.ui.NodeKind
import ewe.ui.PropId
import ewe.ui.runtime.EventBridge
import ewe.ui.runtime.NodeRecord
import ewe.ui.runtime.NodeStore
import ewe.ui.runtime.Property

/** Each screen or modal owns its overlay entries and their drawing order. */
private val LocalLayers = compositionLocalOf<MutableList<@Composable () -> Unit>?> { null }
private val LocalSheetWindow = compositionLocalOf { false }
private val LocalOverlayAlignment = compositionLocalOf<BoxScope?> { null }

/** Draw the supplied screen first. Overlay entries share the same bounds and input system. */
@Composable
internal fun LayerHost(modifier: Modifier = Modifier, children: @Composable () -> Unit) {
    val layers = remember { mutableStateListOf<@Composable () -> Unit>() }
    CompositionLocalProvider(LocalLayers provides layers) {
        Box(modifier) {
            children()
            layers.forEach { entry ->
                key(entry) { Box(Modifier.matchParentSize()) { entry() } }
            }
        }
    }
}

/** Place children above the current screen. Empty overlay space accepts no input. */
internal object OverlayComponent {
    @Composable
    fun Render(modifier: Modifier, children: @Composable () -> Unit) {
        val layers = checkNotNull(LocalLayers.current)
        val context = currentCompositionLocalContext
        val latest by rememberUpdatedState<@Composable () -> Unit> {
            CompositionLocalProvider(context) {
                Box(modifier.fillMaxSize()) {
                    CompositionLocalProvider(LocalOverlayAlignment provides this) { children() }
                }
            }
        }
        val entry = remember { @Composable { latest() } }
        DisposableEffect(layers, entry) {
            layers.add(entry)
            onDispose { layers.remove(entry) }
        }
    }
}

/** The native window owns background input, focus, and the system back action. */
internal object ModalComponent {
    @Composable
    fun Render(
        record: NodeRecord,
        store: NodeStore,
        bridge: EventBridge,
        modifier: Modifier,
        children: @Composable () -> Unit,
    ) {
        val sheet = record.children.singleOrNull()?.let(store::record)
        val cancel = findCancelAction(record, store::record)
        val dismiss: (() -> Unit)? = cancel?.let { { bridge.send(EventId.TAP, it.id) } }
        Render(modifier, sheet?.kind == NodeKind.BOTTOM_SHEET, dismiss, children)
    }

    @OptIn(ExperimentalMaterial3Api::class)
    @Composable
    fun Render(
        modifier: Modifier,
        bottomSheet: Boolean,
        dismiss: (() -> Unit)?,
        children: @Composable () -> Unit,
    ) {
        if (bottomSheet) {
            ModalBottomSheet(
                onDismissRequest = { dismiss?.invoke() },
                sheetGesturesEnabled = dismiss != null,
                properties = ModalBottomSheetProperties(
                    shouldDismissOnBackPress = dismiss != null,
                    shouldDismissOnClickOutside = dismiss != null,
                ),
            ) {
                CompositionLocalProvider(LocalSheetWindow provides true) {
                    LayerHost(modifier.fillMaxWidth(), children)
                }
            }
        } else {
            Dialog(
                onDismissRequest = { dismiss?.invoke() },
                properties = DialogProperties(
                    dismissOnBackPress = dismiss != null,
                    dismissOnClickOutside = dismiss != null,
                ),
            ) {
                Surface(modifier, shape = MaterialTheme.shapes.large) {
                    LayerHost { Column { children() } }
                }
            }
        }
    }
}

/** Measure the panel from its children. A modal parent can supply the Material sheet surface. */
internal object BottomSheetComponent {
    @Composable
    fun Render(modifier: Modifier, children: @Composable () -> Unit) {
        val anchor = LocalOverlayAlignment.current?.let { scope ->
            with(scope) { Modifier.align(Alignment.BottomCenter) }
        } ?: Modifier
        val content: @Composable () -> Unit = {
            Column(
                Modifier.fillMaxWidth().padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) { FormScope(children) }
        }
        if (LocalSheetWindow.current) {
            CompositionLocalProvider(LocalSheetWindow provides false) { Box(anchor.then(modifier)) { content() } }
        } else {
            CompositionLocalProvider(LocalOverlayAlignment provides null) {
                Surface(
                    anchor.then(modifier).fillMaxWidth()
                        .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom)),
                    shape = MaterialTheme.shapes.large,
                    content = content,
                )
            }
        }
    }
}

/** Measure a trailing sheet first. The preceding children receive the remaining height. */
@Composable
internal fun InlineSheetLayout(
    modifier: Modifier,
    leading: @Composable () -> Unit,
    sheet: @Composable () -> Unit,
) {
    Layout(modifier = modifier, content = {
        Box { leading() }
        Box { sheet() }
    }) { children, constraints ->
        val loose = constraints.copy(minWidth = 0, minHeight = 0)
        val panel = children[1].measure(loose)
        val available = if (constraints.hasBoundedHeight) loose.maxHeight - panel.height else loose.maxHeight
        val body = children[0].measure(loose.copy(maxHeight = available))
        val height = if (constraints.hasBoundedHeight) constraints.maxHeight else body.height + panel.height
        val placedHeight = height.coerceAtLeast(constraints.minHeight)
        layout(maxOf(constraints.minWidth, body.width, panel.width), placedHeight) {
            body.placeRelative(0, 0)
            panel.placeRelative(0, placedHeight - panel.height)
        }
    }
}

/** Find the cancel action inside this modal without entering another modal. */
private fun findCancelAction(root: NodeRecord, lookup: (UInt) -> NodeRecord?): NodeRecord? {
    for (id in root.children) {
        val child = lookup(id) ?: continue
        if (child.kind == NodeKind.MODAL) continue
        if (child.props[PropId.PRESENTATION]?.label == "cancel-action" &&
            !child.disabled && EventId.TAP in child.actions) {
            return child
        }
        findCancelAction(child, lookup)?.let { return it }
    }
    return null
}

private val NodeRecord.disabled: Boolean
    get() = props[PropId.ENABLED] == Property.Flag(false)

/** Read an input from its original control. Keep each form's active fields separate. */
internal class SheetInputs {
    val fields = mutableMapOf<NodeRecord, () -> String>()

    fun submit(button: NodeRecord, parent: NodeRecord?, lookup: (UInt) -> NodeRecord?, bridge: EventBridge) {
        val field = parent?.children?.mapNotNull(lookup)?.singleOrNull {
            it in fields && !it.disabled && EventId.SUBMIT in it.actions
        }
        val value = field?.let { fields.getValue(it)() }.orEmpty()
        if (EventId.TAP in button.actions) bridge.send(EventId.TAP, button.id, value)
        else if (field != null) bridge.send(EventId.SUBMIT, field.id, value)
    }
}

internal val FormInputs = compositionLocalOf<SheetInputs?> { null }

@Composable
internal fun FormScope(children: @Composable () -> Unit) {
    val inputs = remember { SheetInputs() }
    CompositionLocalProvider(FormInputs provides inputs, content = children)
}
