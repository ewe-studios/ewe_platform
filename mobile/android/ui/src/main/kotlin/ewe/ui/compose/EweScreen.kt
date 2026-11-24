package ewe.ui.compose

import android.animation.ValueAnimator
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.ui.draw.alpha
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.isSystemInDarkTheme
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
import androidx.compose.ui.unit.sp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import io.github.ewe_studios.ewe.core.NativeGraphics
import ewe.ui.EventId
import ewe.ui.NodeKind
import ewe.ui.PropId
import ewe.ui.compose.behaviours.BehaviourRegistry
import ewe.ui.runtime.*

data class ScreenStyle(
    val colors: (UInt) -> Color?,
    val fonts: (UInt) -> TextStyle?,
    val image: @Composable (String, Modifier) -> Unit,
    val webView: @Composable (WebViewSpec, Modifier) -> Unit,
    val behaviours: BehaviourRegistry,
    val theme: EweTheme = EweTheme(),
    val graphics: (() -> NativeGraphics)? = null,
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
    theme: EweTheme = EweTheme(),
    graphics: (() -> NativeGraphics)? = null,
) {
    val style =
        ScreenStyle(
            colors ?: theme.colors::get,
            fonts ?: theme.fonts::get,
            imageLoader,
            webView,
            behaviours,
            theme,
            graphics,
        )
    NodeView(store, bridge, 1u, style, modifier.fillMaxSize())
}

internal fun NodeRecord.flag(key: UShort, fallback: Boolean = false) =
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

@Composable
fun NodeView(
    store: NodeStore,
    bridge: EventBridge,
    id: UInt,
    style: ScreenStyle,
    modifier: Modifier = Modifier,
) {
    LayerHost(modifier) {
        RenderNode(store, bridge, id, style)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun RenderNode(
    store: NodeStore,
    bridge: EventBridge,
    id: UInt,
    style: ScreenStyle,
    modifier: Modifier = Modifier,
) {
    val record = store.record(id) ?: return
    val bindings = record.behaviours.sorted().mapNotNull(style.behaviours::get)
    val hidden = record.flag(PropId.HIDDEN)
    if (hidden && bindings.none { it.transition }) return
    val enabled = record.flag(PropId.ENABLED, true)
    val scroll = bindings.all { it.scrollEnabled }
    val parent = record.parent?.let(store::record)
    val formInputs = FormInputs.current
    var m =
        modifier
            .sizeProp(record.props[PropId.WIDTH], true)
            .sizeProp(record.props[PropId.HEIGHT], false)
    record.props[PropId.A11Y_LABEL]?.let {
        val label = it.label
        m = m.semantics { contentDescription = label }
    }
    val dark = isSystemInDarkTheme()
    val appearance = NodeAppearance(record, style, dark)
    if (record.props[PropId.UTILITY_CLASSES]?.label?.split(' ')?.contains("animate-pulse") == true) {
        val opacity = if (ValueAnimator.areAnimatorsEnabled()) {
            val transition = rememberInfiniteTransition(label = "Pulse")
            transition.animateFloat(0.7f, 0.5f, infiniteRepeatable(tween(2000), RepeatMode.Reverse), label = "Opacity").value
        } else 0.7f
        m = m.alpha(opacity)
    }
    appearance.background?.let { m = m.background(it) }
    m = when (record.kind) {
        NodeKind.INSET_LIST -> m.fillMaxWidth()
        NodeKind.SEGMENTED_CONTROL -> m.selectableGroup()
        NodeKind.CHECKLIST_ROW -> m.fillMaxWidth().padding(vertical = 4.dp)
        else -> m
    }
    m = m.padding(appearance.padding.coerceAtLeast(0f).dp)
    bindings.forEach { m = m.then(it.modifier(record)) }
    val content: @Composable () -> Unit = {
        val web = record.webView
        val children: @Composable () -> Unit = {
            record.children.forEach { child ->
                key(child) { RenderNode(store, bridge, child, style) }
            }
        }
        val spacing = Arrangement.spacedBy(record.length(PropId.SPACING).dp)
        if (web != null) style.webView(web, m)
        else
            when (record.kind) {
                NodeKind.DRAWING_SURFACE -> EweDrawingSurface(record.props[PropId.ID]?.label ?: record.id.toString(),
                    { requireNotNull(style.graphics) { "This screen has no graphics provider" }.invoke() },
                    { store.onError?.invoke("Graphics", it.message ?: it.toString()) }, m)
                NodeKind.STACK, NodeKind.SEGMENTED_CONTROL, NodeKind.CHECKLIST_ROW -> {
                    val align = record.props[PropId.ALIGN]?.label
                    val nodes = record.children.mapNotNull(store::record)
                    val title = nodes.firstOrNull { it.kind == NodeKind.NAVIGATION_TITLE }
                    if (title != null) {
                        val actions = nodes.filter { it.kind == NodeKind.TOOLBAR_BUTTON }
                        Column(m.fillMaxWidth()) {
                            TopAppBar(
                                title = { RenderNode(store, bridge, title.id, style) },
                                actions = { actions.forEach { RenderNode(store, bridge, it.id, style) } },
                            )
                            nodes.filter { it !== title && it !in actions }.forEach { child ->
                                key(child.id) { RenderNode(store, bridge, child.id, style, Modifier.padding(horizontal = 16.dp)) }
                            }
                        }
                    } else if (record.kind == NodeKind.SEGMENTED_CONTROL && nodes.all { it.kind == NodeKind.BUTTON }) {
                        SegmentedControl(nodes, bridge, style, dark, m)
                    } else if (record.kind == NodeKind.CHECKLIST_ROW) {
                        ChecklistRow(record, nodes, bridge, m) { RenderNode(store, bridge, it.id, style) }
                    } else if (record.props[PropId.AXIS]?.label == "Horizontal") {
                        Row(m, horizontalArrangement = spacing, verticalAlignment = when (align) {
                            "Center" -> Alignment.CenterVertically
                            "End" -> Alignment.Bottom
                            else -> Alignment.Top
                        }) { children() }
                    }
                    else if (record.children.lastOrNull()?.let(store::record)?.kind == NodeKind.BOTTOM_SHEET) {
                        val sheetId = record.children.last()
                        InlineSheetLayout(
                            m,
                            leading = {
                                Column(verticalArrangement = spacing) {
                                    record.children.dropLast(1).forEach { child ->
                                        key(child) { RenderNode(store, bridge, child, style) }
                                    }
                                }
                            },
                            sheet = { key(sheetId) { RenderNode(store, bridge, sheetId, style) } },
                        )
                    } else
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
                NodeKind.TEXT -> ControlText(record, m, appearance, MaterialTheme.typography.bodyMedium,
                    if (parent?.kind == NodeKind.CHECKLIST_ROW && parent.flag(PropId.CHECKED)) TextDecoration.LineThrough else null)
                NodeKind.NAVIGATION_TITLE -> NavigationTitle(record, m, appearance)
                NodeKind.SHEET_TITLE -> SheetTitle(record, m, appearance)
                NodeKind.LIST_FOOTER -> ListFooter(record, m, appearance)
                NodeKind.IMAGE -> style.image(record.props[PropId.SOURCE]?.label.orEmpty(), m)
                NodeKind.BUTTON, NodeKind.TOOLBAR_BUTTON, NodeKind.SUBMIT_BUTTON, NodeKind.CANCEL_BUTTON, NodeKind.DESTRUCTIVE_BUTTON, NodeKind.CHECKMARK_BUTTON -> {
                    val onTap: () -> Unit = {
                        if (record.kind == NodeKind.SUBMIT_BUTTON && formInputs != null) {
                            formInputs.submit(record, parent, store::record, bridge)
                        } else {
                            bridge.send(EventId.TAP, id, record.props[PropId.VALUE]?.label.orEmpty())
                        }
                    }
                    val label: @Composable () -> Unit = {
                        StyledLabel(record, appearance, enabled)
                        children()
                    }
                    if (parent?.kind == NodeKind.SEGMENTED_CONTROL) {
                        FilterChip(
                            selected = record.flag(PropId.CHECKED),
                            onClick = onTap,
                            label = label,
                            modifier = m,
                            enabled = enabled,
                        )
                    } else when (record.kind) {
                        NodeKind.DESTRUCTIVE_BUTTON -> DestructiveButton(onTap, m, enabled, label)
                        NodeKind.TOOLBAR_BUTTON -> ToolbarButton(onTap, m, enabled, label)
                        NodeKind.CANCEL_BUTTON -> CancelButton(onTap, m, enabled, label)
                        NodeKind.SUBMIT_BUTTON -> SubmitButton(onTap, m, enabled, label)
                        NodeKind.CHECKMARK_BUTTON -> CheckmarkButton(record, record.flag(PropId.CHECKED), onTap)
                        else -> Button(onTap, m, enabled) { label() }
                    }
                }
                NodeKind.SEARCH_INPUT -> SearchInput(record, bridge, m, enabled)
                NodeKind.TEXT_INPUT -> {
                    val serverValue = record.props[PropId.VALUE]?.label.orEmpty()
                    var value by remember(record, serverValue) { mutableStateOf(serverValue) }
                    DisposableEffect(formInputs, record, serverValue) {
                        formInputs?.fields?.set(record) { value }
                        onDispose { formInputs?.fields?.remove(record) }
                    }
                    TextField(
                        value,
                        {
                            value = it
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
                NodeKind.LIST, NodeKind.INSET_LIST ->
                    InsetList(record.children, m, spacing, scroll) { RenderNode(store, bridge, it, style) }
                NodeKind.SCROLL ->
                    Column(m.verticalScroll(rememberScrollState(), enabled = scroll)) { children() }
                NodeKind.SPACER -> Spacer(m)
                NodeKind.MODAL -> {
                    ModalComponent.Render(record, store, bridge, m, children)
                }
                NodeKind.BOTTOM_SHEET -> {
                    BottomSheetComponent.Render(m) {
                        BottomSheetComponent.Children(record.children.mapNotNull(store::record)) { node ->
                            RenderNode(store, bridge, node.id, style)
                        }
                    }
                }
                NodeKind.OVERLAY -> if (bindings.any { it.dialog }) Box(m) { children() }
                    else OverlayComponent.Render(m, children)
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
            .firstOrNull { it.kind == NodeKind.CANCEL_BUTTON }
            ?.let { bridge.send(EventId.TAP, it.id) }
        Unit
    }
    if (record.kind == NodeKind.CONFIRMATION_DIALOG) {
        ConfirmationDialog(record.children.mapNotNull(store::record), m, dismiss) { RenderNode(store, bridge, it.id, style) }
    } else if (bindings.any { it.dialog }) {
        var open by remember(record) { mutableStateOf(true) }
        if (open)
            Dialog(onDismissRequest = { if (bindings.any { it.dismiss }) open = false }) {
                display()
            }
    } else display()
}


@Composable
internal fun StyledLabel(record: NodeRecord, appearance: NodeAppearance, enabled: Boolean) {
    Text(
        record.text.orEmpty(),
        color = if (enabled) appearance.foreground ?: Color.Unspecified else Color.Unspecified,
        style = appearance.font ?: LocalTextStyle.current,
        fontSize = appearance.fontSize?.sp ?: androidx.compose.ui.unit.TextUnit.Unspecified,
    )
}
