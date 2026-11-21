package ewe.ui.compose

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.*
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import ewe.ui.EventId
import ewe.ui.NodeKind
import ewe.ui.PropId
import ewe.ui.compose.behaviours.BehaviourRegistry
import ewe.ui.runtime.*

data class ScreenStyle(
    val colors: (UInt) -> Color,
    val fonts: (UInt) -> TextStyle,
    val image: @Composable (String, Modifier) -> Unit,
    val webView: @Composable (WebViewSpec, Modifier) -> Unit,
    val behaviours: BehaviourRegistry,
)

@Composable
fun EweScreen(
    store: NodeStore,
    bridge: EventBridge,
    modifier: Modifier = Modifier,
    behaviours: BehaviourRegistry = remember { BehaviourRegistry.standard },
    imageLoader: @Composable (String, Modifier) -> Unit = { _, m -> Spacer(m) },
    webView: @Composable (WebViewSpec, Modifier) -> Unit = { _, m -> Spacer(m) },
    colors: ((UInt) -> Color)? = null,
    fonts: ((UInt) -> TextStyle)? = null,
) {
    val defaultColor = MaterialTheme.colorScheme.onSurface
    val defaultFont = MaterialTheme.typography.bodyMedium
    val style =
        ScreenStyle(
            colors ?: { defaultColor },
            fonts ?: { defaultFont },
            imageLoader,
            webView,
            behaviours,
        )
    NodeView(store, bridge, 1u, style, modifier)
}

private val FormValue = compositionLocalOf<MutableState<String>?> { null }

private fun NodeRecord.flag(key: UShort, fallback: Boolean = false) =
    (props[key] as? Property.Flag)?.value ?: fallback

private fun NodeRecord.length(key: UShort) = (props[key] as? Property.Length)?.number ?: 0f

private fun Modifier.sizeProp(value: Property?, width: Boolean): Modifier {
    val size = value as? Property.Length ?: return this
    lengthToDp(size)?.let {
        return if (width) width(it) else height(it)
    }
    fractionOf(size)?.let {
        return if (width) fillMaxWidth(it) else fillMaxHeight(it)
    }
    return this
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NodeView(
    store: NodeStore,
    bridge: EventBridge,
    id: UInt,
    style: ScreenStyle,
    modifier: Modifier = Modifier,
) {
    val record = store.record(id) ?: return
    val bindings = record.behaviours.sorted().mapNotNull(style.behaviours::get)
    val presentation = record.props[PropId.PRESENTATION]?.label
    val hidden = record.flag(PropId.HIDDEN)
    if (hidden && bindings.none { it.transition }) return
    val enabled = record.flag(PropId.ENABLED, true)
    val scroll = bindings.all { it.scrollEnabled }
    val parent = record.parent?.let(store::record)
    val parentPresentation = parent?.props?.get(PropId.PRESENTATION)?.label
    val formValue = FormValue.current
    var m =
        modifier
            .sizeProp(record.props[PropId.WIDTH], true)
            .sizeProp(record.props[PropId.HEIGHT], false)
    record.props[PropId.A11Y_LABEL]?.let {
        val label = it.label
        m = m.semantics { contentDescription = label }
    }
    (record.props[PropId.BACKGROUND] as? Property.Color)?.let {
        m = m.background(style.colors(it.token))
    }
    m = when (presentation) {
        "inset-list" -> m.fillMaxWidth()
        "segmented-control" -> m.selectableGroup()
        "checklist-row" -> m.fillMaxWidth().padding(vertical = 4.dp)
        else -> m
    }
    m = m.padding(record.length(PropId.PADDING).coerceAtLeast(0f).dp)
    bindings.forEach { m = m.then(it.modifier(record)) }
    val content: @Composable () -> Unit = {
        val web = record.webView
        val children: @Composable () -> Unit = {
            record.children.forEach { child ->
                key(child) { NodeView(store, bridge, child, style) }
            }
        }
        val spacing = Arrangement.spacedBy(record.length(PropId.SPACING).dp)
        if (web != null) style.webView(web, m)
        else
            when (record.kind) {
                NodeKind.STACK -> {
                    val align = record.props[PropId.ALIGN]?.label
                    if (presentation == "segmented-control" || record.props[PropId.AXIS]?.label == "Horizontal")
                        Row(
                            m,
                            horizontalArrangement = spacing,
                            verticalAlignment =
                                when (align) {
                                    "Center" -> Alignment.CenterVertically
                                    "End" -> Alignment.Bottom
                                    else -> Alignment.Top
                                },
                        ) {
                            if (presentation == "checklist-row") {
                                record.children.forEach { child ->
                                    key(child) {
                                        val width = if (store.record(child)?.kind == NodeKind.TEXT)
                                            Modifier.weight(1f) else Modifier
                                        NodeView(store, bridge, child, style, width)
                                    }
                                }
                            } else children()
                        }
                    else
                        Column(
                            m,
                            verticalArrangement = spacing,
                            horizontalAlignment =
                                when (align) {
                                    "Center" -> Alignment.CenterHorizontally
                                    "End" -> Alignment.End
                                    else -> Alignment.Start
                                },
                        ) {
                            children()
                        }
                }
                NodeKind.TEXT ->
                    Text(
                        record.text.orEmpty(),
                        m,
                        color =
                            (record.props[PropId.COLOR] as? Property.Color)?.let {
                                style.colors(it.token)
                            } ?: MaterialTheme.colorScheme.onSurface,
                        textDecoration =
                            if (parentPresentation == "checklist-row" && parent?.flag(PropId.CHECKED) == true)
                                TextDecoration.LineThrough else null,
                        style =
                            (record.props[PropId.FONT] as? Property.Font)?.let {
                                style.fonts(it.token)
                            } ?: when (presentation) {
                                "navigation-title" -> MaterialTheme.typography.headlineMedium
                                "list-footer" -> MaterialTheme.typography.labelLarge
                                "sheet-title" -> MaterialTheme.typography.titleLarge
                                else -> MaterialTheme.typography.bodyMedium
                            },
                    )
                NodeKind.IMAGE -> style.image(record.props[PropId.SOURCE]?.label.orEmpty(), m)
                NodeKind.BUTTON -> {
                    val onTap: () -> Unit = {
                        if (presentation == "submit-action" && formValue != null) {
                            parent?.children?.mapNotNull(store::record)
                                ?.firstOrNull { EventId.SUBMIT in it.actions }
                                ?.let { bridge.send(EventId.SUBMIT, it.id, formValue.value) }
                        } else {
                            bridge.send(EventId.TAP, id, record.props[PropId.VALUE]?.label.orEmpty())
                        }
                    }
                    val label: @Composable () -> Unit = {
                        Text(record.text.orEmpty())
                        children()
                    }
                    if (parentPresentation == "segmented-control") {
                        FilterChip(
                            selected = record.flag(PropId.CHECKED),
                            onClick = onTap,
                            label = label,
                            modifier = m,
                            enabled = enabled,
                        )
                    } else when (presentation) {
                        "destructive-action" -> TextButton(
                            onTap, m, enabled,
                            colors = ButtonDefaults.textButtonColors(
                                contentColor = MaterialTheme.colorScheme.error,
                            ),
                        ) { label() }
                        "toolbar-action" -> FilledTonalButton(onTap, m, enabled) { label() }
                        "checkmark-action", "cancel-action" -> OutlinedButton(onTap, m, enabled) { label() }
                        else -> Button(onTap, m, enabled) { label() }
                    }
                }
                NodeKind.TEXT_INPUT -> {
                    val serverValue = record.props[PropId.VALUE]?.label.orEmpty()
                    var value by remember(record, serverValue) { mutableStateOf(serverValue) }
                    TextField(
                        value,
                        {
                            value = it
                            formValue?.value = it
                            bridge.send(EventId.CHANGE, id, it)
                        },
                        m,
                        enabled = enabled,
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                        keyboardActions =
                            KeyboardActions(onDone = { bridge.send(EventId.SUBMIT, id, value) }),
                        placeholder = { Text(record.props[PropId.PLACEHOLDER]?.label.orEmpty()) },
                    )
                }
                NodeKind.TOGGLE ->
                    Switch(
                        record.flag(PropId.CHECKED),
                        { bridge.send(EventId.CHANGE, id, checked = it) },
                        m,
                        enabled = enabled,
                    )
                NodeKind.LIST ->
                    LazyColumn(m, verticalArrangement = spacing, userScrollEnabled = scroll) {
                        items(record.children, key = { it.toLong() }) { child ->
                            NodeView(store, bridge, child, style)
                        }
                    }
                NodeKind.SCROLL ->
                    Column(m.verticalScroll(rememberScrollState(), enabled = scroll)) { children() }
                NodeKind.SPACER -> Spacer(m)
                NodeKind.OVERLAY -> when (presentation) {
                    "form-sheet", "confirmation-dialog" -> {
                        Surface(m, shape = MaterialTheme.shapes.large) {
                            Column(
                                Modifier.fillMaxWidth().padding(16.dp),
                                verticalArrangement = Arrangement.spacedBy(8.dp),
                            ) { children() }
                        }
                    }
                    else -> Box(m) { children() }
                }
                else ->
                    Column(m) {
                        Text("Unsupported node ${record.kind}")
                        children()
                    }
            }
    }
    val display: @Composable () -> Unit = {
        if (bindings.any { it.transition }) AnimatedVisibility(visible = !hidden) { content() }
        else content()
    }
    val dismiss = {
        record.children.mapNotNull(store::record)
            .firstOrNull { it.props[PropId.PRESENTATION]?.label == "cancel-action" }
            ?.let { bridge.send(EventId.TAP, it.id) }
        Unit
    }
    if (presentation == "form-sheet") {
        val initialValue = record.children.mapNotNull(store::record)
            .firstOrNull { EventId.SUBMIT in it.actions }?.props?.get(PropId.VALUE)?.label.orEmpty()
        val draft = remember(record, initialValue) { mutableStateOf(initialValue) }
        ModalBottomSheet(onDismissRequest = dismiss) {
            CompositionLocalProvider(FormValue provides draft) { display() }
        }
    } else if (presentation == "confirmation-dialog") {
        Dialog(onDismissRequest = dismiss) { display() }
    } else if (bindings.any { it.dialog }) {
        var open by remember(record) { mutableStateOf(true) }
        if (open)
            Dialog(onDismissRequest = { if (bindings.any { it.dismiss }) open = false }) {
                display()
            }
    } else display()
}
