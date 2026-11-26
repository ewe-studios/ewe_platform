package ewe.ui.compose

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.sp
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import ewe.ui.EventId
import ewe.ui.NodeKind
import ewe.ui.PropId
import ewe.ui.runtime.EventBridge
import ewe.ui.runtime.NodeRecord

@Composable
internal fun SegmentedControl(
    nodes: List<NodeRecord>, bridge: EventBridge, style: ScreenStyle, dark: Boolean, m: Modifier,
) {
    SingleChoiceSegmentedButtonRow(m.fillMaxWidth()) {
        nodes.forEachIndexed { index, child ->
            key(child.id) {
                SegmentedButton(
                    selected = child.flag(PropId.CHECKED),
                    onClick = { bridge.send(EventId.TAP, child.id, child.props[PropId.VALUE]?.label.orEmpty()) },
                    enabled = child.flag(PropId.ENABLED, true),
                    shape = SegmentedButtonDefaults.itemShape(index, nodes.size),
                ) { StyledLabel(child, NodeAppearance(child, style, dark), child.flag(PropId.ENABLED, true)) }
            }
        }
    }
}

@Composable
internal fun ChecklistRow(
    record: NodeRecord, nodes: List<NodeRecord>, bridge: EventBridge, m: Modifier,
    child: @Composable (NodeRecord) -> Unit,
) {
    val toggle = nodes.firstOrNull { it.kind == NodeKind.CHECKMARK_BUTTON }
    val delete = nodes.firstOrNull { it.kind == NodeKind.DESTRUCTIVE_BUTTON }
    ListItem(
        modifier = m,
        headlineContent = {
            Column { nodes.filter { it !== toggle && it !== delete }.forEach { child(it) } }
        },
        leadingContent = toggle?.let { action -> {
            CheckmarkButton(action, record.flag(PropId.CHECKED)) {
                bridge.send(EventId.TAP, action.id, action.props[PropId.VALUE]?.label.orEmpty())
            }
        } },
        trailingContent = delete?.let { action -> { child(action) } },
    )
}

@Composable
internal fun ConfirmationDialog(
    nodes: List<NodeRecord>, m: Modifier, dismiss: () -> Unit,
    child: @Composable (NodeRecord) -> Unit,
) {
    fun control(kind: UShort) = nodes.firstOrNull { it.kind == kind }
    val title = control(NodeKind.SHEET_TITLE)
    val cancel = control(NodeKind.CANCEL_BUTTON)
    val confirm = control(NodeKind.DESTRUCTIVE_BUTTON) ?: control(NodeKind.SUBMIT_BUTTON)
    val other = nodes.filter { it !== title && it !== cancel && it !== confirm }
    AlertDialog(
        onDismissRequest = dismiss,
        modifier = m,
        title = title?.let { node -> { child(node) } },
        text = if (other.isEmpty()) null else { {
            Column { other.forEach { child(it) } }
        } },
        confirmButton = { confirm?.let { child(it) } },
        dismissButton = cancel?.let { node -> { child(node) } },
    )
}

@Composable
internal fun CheckmarkButton(node: NodeRecord, checked: Boolean, onTap: () -> Unit) {
    Checkbox(
        checked = checked,
        onCheckedChange = { onTap() },
        enabled = node.flag(PropId.ENABLED, true),
        modifier = Modifier.semantics { contentDescription = node.props[PropId.A11Y_LABEL]?.label ?: node.text.orEmpty() },
    )
}

@Composable
internal fun DestructiveButton(onTap: () -> Unit, modifier: Modifier, enabled: Boolean, label: @Composable () -> Unit) {
    TextButton(onTap, modifier, enabled, colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.error)) { label() }
}

@Composable
internal fun ToolbarButton(onTap: () -> Unit, modifier: Modifier, enabled: Boolean, label: @Composable () -> Unit) {
    FilledTonalButton(onTap, modifier, enabled) { label() }
}

@Composable
internal fun CancelButton(onTap: () -> Unit, modifier: Modifier, enabled: Boolean, label: @Composable () -> Unit) {
    OutlinedButton(onTap, modifier, enabled) { label() }
}

@Composable
internal fun SubmitButton(onTap: () -> Unit, modifier: Modifier, enabled: Boolean, label: @Composable () -> Unit) {
    Button(onTap, modifier, enabled) { label() }
}

@Composable
internal fun ControlText(
    node: NodeRecord,
    modifier: Modifier,
    appearance: NodeAppearance,
    defaultStyle: TextStyle,
    decoration: TextDecoration? = null,
) {
    Text(
        node.text.orEmpty(), modifier,
        color = appearance.foreground ?: MaterialTheme.colorScheme.onSurface,
        fontSize = appearance.fontSize?.sp ?: TextUnit.Unspecified,
        textDecoration = decoration,
        style = appearance.font ?: defaultStyle,
    )
}

@Composable
internal fun NavigationTitle(node: NodeRecord, modifier: Modifier, appearance: NodeAppearance) {
    ControlText(node, modifier.semantics { heading() }, appearance, MaterialTheme.typography.titleLarge)
}

@Composable
internal fun SheetTitle(node: NodeRecord, modifier: Modifier, appearance: NodeAppearance) {
    ControlText(node, modifier.semantics { heading() }, appearance, MaterialTheme.typography.titleLarge)
}

@Composable
internal fun ListFooter(node: NodeRecord, modifier: Modifier, appearance: NodeAppearance) {
    ControlText(node, modifier, appearance, MaterialTheme.typography.labelLarge)
}

@Composable
internal fun InsetList(
    children: List<UInt>, modifier: Modifier, spacing: Arrangement.Vertical,
    scroll: Boolean, child: @Composable (UInt) -> Unit,
) {
    LazyColumn(modifier, verticalArrangement = spacing, userScrollEnabled = scroll) {
        items(children, key = { it.toLong() }) { child(it) }
    }
}
