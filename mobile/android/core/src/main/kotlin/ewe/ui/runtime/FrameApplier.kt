package ewe.ui.runtime

import ewe.ui.*
import ewe.ui.flatbuffers.Table
import kotlinx.serialization.json.*

internal class FrameApplier(internal val store: NodeStore, private val bytes: Int, private val mount: UInt) {
    fun message(m: Message, row: Int) {
        m.operation?.let { operation(it, row) }
        m.data?.let { d ->
            val target = if (d.targetNode != 0u) d.targetNode else store.targets.scope(mount)
            if (target == null) {
                store.report("AmbiguousView", row)
                return@let
            }
            val kind =
                when {
                    d.arrow != null -> "ArrowIpc"
                    d.table != null -> "Table"
                    else -> "MessagePack"
                }
            store.onData?.invoke(target, d)
            store.data.add(
                buildJsonObject {
                    put("data", kind)
                    put("target", target.toLong())
                }
            )
        }
        m.control?.let { control ->
            control.error?.let { e ->
                val code = Names.errorCode[e.code] ?: "Unknown"
                store.errors.add(
                    buildJsonObject {
                        put("code", code)
                        put("message", e.message)
                        put("row", e.rowIndex.toLong())
                    }
                )
                store.onError?.invoke(code, e.message.orEmpty())
            }
            for ((present, code) in
                listOf(
                    (control.stateExpired != null) to "StateExpired",
                    (control.updateRequired != null) to "UpdateRequired",
                )) {
                if (present) {
                    store.errors.add(buildJsonObject { put("code", code) })
                    store.onError?.invoke(code, code)
                }
            }
            control.caps?.let { c ->
                store.caps.add(
                    buildJsonObject {
                        put("stream_id", JsonPrimitive(c.streamId))
                        put("resume_from", JsonPrimitive(c.resumeFrom))
                        put("status", Names.capsStatus[c.status])
                    }
                )
            }
        }
    }

    internal fun operation(op: Operation, row: Int) {
        if (
            op.uiType == UiOperation.NONE &&
                op.webType == WebOperation.NONE &&
                op.mobileType == MobileOperation.NONE &&
                op.columnar == null
        )
            store.report("UnknownOp", row)
        when (op.uiType) {
            UiOperation.NONE -> Unit
            UiOperation.CreateNode ->
                (op.ui(CreateNode()) as CreateNode).let { store.create(it.nodeId, it.kind, row) }
            UiOperation.SetText ->
                (op.ui(SetText()) as SetText).let { store.node(it.nodeId, row)?.text = it.text }
            UiOperation.SetProp ->
                (op.ui(SetProp()) as SetProp).let {
                    setProp(it.nodeId, it.prop, property(it.valueType, it::value), row)
                }
            UiOperation.AppendChild ->
                (op.ui(AppendChild()) as AppendChild).let {
                    store.insert(it.parentId, it.childId, null, row)
                }
            UiOperation.InsertBefore ->
                (op.ui(InsertBefore()) as InsertBefore).let {
                    store.insert(it.parentId, it.childId, it.beforeId, row)
                }
            UiOperation.Remove ->
                (op.ui(Remove()) as Remove).let {
                    store.node(it.nodeId, row)?.let { n -> store.remove(n.id) }
                }
            UiOperation.ReplaceSubtree ->
                (op.ui(ReplaceSubtree()) as ReplaceSubtree).let { replace ->
                    store.node(replace.nodeId, row)?.let { n ->
                        n.children.toList().forEach(store::remove)
                        repeat(replace.operationsLength) {
                            operation(requireNotNull(replace.operations(it)), row)
                        }
                    }
                }
            UiOperation.BindAction ->
                (op.ui(BindAction()) as BindAction).let { b ->
                    store.node(b.nodeId, row)?.let { n ->
                        b.action?.let { n.actions[b.event] = it }
                    }
                }
            UiOperation.UnbindAction ->
                (op.ui(UnbindAction()) as UnbindAction).let {
                    store.node(it.nodeId, row)?.actions?.remove(it.event)
                }
            UiOperation.AttachState ->
                (op.ui(AttachState()) as AttachState).let {
                    require(it.stateLength in 0..bytes)
                    store.node(it.rootNode, row)?.state =
                        ByteArray(it.stateLength) { index -> it.state(index).toByte() }
                }
            UiOperation.LiveMount ->
                (op.ui(LiveMount()) as LiveMount).let {
                    store.create(it.nodeId, NodeKind.STACK, row)
                    store.record(it.nodeId)?.live =
                        LiveSpec(
                            it.kind,
                            it.channelId,
                            it.transport,
                            it.endpoint.orEmpty(),
                            it.resumeFrom,
                            when (it.sharing) {
                                ewe.ui.ConnectionSharing.Shared -> true
                                ewe.ui.ConnectionSharing.Dedicated -> false
                                else -> null
                            },
                            it.keepAlive,
                        )
                }
            UiOperation.UiBehaviour ->
                (op.ui(UiBehaviour()) as UiBehaviour).let {
                    store.behaviour(it.nodeId, Names.behaviourId[it.behaviour] ?: "UNKNOWN", row)
                }
            UiOperation.Place -> place(op.ui(Place()) as Place, row)
            else -> store.report("UnknownOp", row)
        }
        if (op.webType != WebOperation.NONE) store.report("UnsupportedPlatformOp", row)
        when (op.mobileType) {
            MobileOperation.NONE -> Unit
            MobileOperation.MountWebView ->
                (op.mobile(MountWebView()) as MountWebView).let {
                    store.node(it.nodeId, row)?.webView = WebViewSpec(it.parentId, it.url.orEmpty())
                }
            MobileOperation.MobileBehaviour ->
                (op.mobile(MobileBehaviour()) as MobileBehaviour).let {
                    store.behaviour(it.nodeId, it.custom ?: "UNKNOWN", row)
                }
            MobileOperation.NativeCommand ->
                command(op.mobile(NativeCommand()) as NativeCommand, row)
            else -> store.report("UnknownOp", row)
        }
        op.columnar?.let { applyColumns(it, row) }
    }

    internal fun setProp(id: UInt, key: UShort, value: Property?, row: Int) {
        val n = store.node(id, row) ?: return
        if (key == PropId.UNKNOWN || key !in Names.propId || value == null)
            store.report("UnknownProp", row, id)
        else {
            if (key == PropId.ID && !store.targets.setName(id, value.label, row)) return
            n.props[key] = value
        }
    }

    private fun command(c: NativeCommand, row: Int) {
        val name = Names.nativeCommandId[c.command]
        if (name == null || c.command == NativeCommandId.UNKNOWN) {
            store.report("UnknownCommand", row)
            return
        }
        val args = buildMap {
            repeat(c.argsLength) { i ->
                c.args(i)?.let { a ->
                    a.name?.let { name -> property(a.valueType, a::value)?.let { put(name, it) } }
                }
            }
        }
        store.sentCommands.add(
            buildJsonObject {
                put("command", name)
                putJsonObject("args") { args.forEach { (key, v) -> put(key, v.label) } }
            }
        )
        if (store.commands?.run(c.command, args) == false) store.report("UnknownCommand", row)
    }

    private fun place(p: Place, row: Int) {
        p.target?.takeIf { it.kind == SelectorKind.View }?.text?.let { name ->
            val destination = store.viewStore(name)
            if (destination != null && destination !== store) {
                val root = requireNotNull(destination.targets.roots[name])
                FrameApplier(destination, bytes, root).place(p, row)
                destination.onChanged?.invoke()
                return
            }
        }
        if (p.kind == PlacementKind.FullView) {
            if (mount == 0u) { store.report("AmbiguousView", row); return }
            val owner = store.node(mount, row) ?: return
            if (!store.targets.validate(p, store.targets.scope(mount), owner.children.toList(), row)) return
            owner.children.toList().forEach(store::remove)
            repeat(p.operationsLength) { operation(requireNotNull(p.operations(it)), row) }
            store.insert(mount, p.rootNode, null, row)
            return
        }
        val target = p.target ?: return
        val scope = if (target.kind == SelectorKind.View) store.targets.roots[target.text] else store.targets.scope(mount)
        val node = store.targets.resolve(target, scope, row) ?: return
        if (!store.targets.validate(p, scope, if (p.kind == PlacementKind.Swap) listOf(node.id) else emptyList(), row)) return
        if (p.kind == PlacementKind.Remove) {
            store.remove(node.id)
            return
        }
        val parent = if (p.kind == PlacementKind.Swap) node.parent?.let(store::record) else node
        if (parent == null) {
            store.report("InvalidParent", row, node.id)
            return
        }
        val anchor = p.child?.let { (store.targets.resolve(it, scope, row) ?: return).id }
        if (anchor != null && anchor !in parent.children) {
            store.report("UnknownNode", row, anchor)
            return
        }
        fun after(id: UInt) = parent.children.getOrNull(parent.children.indexOf(id) + 1)
        val before =
            when (p.kind) {
                PlacementKind.Swap -> after(node.id)
                PlacementKind.Before -> anchor ?: parent.children.firstOrNull()
                PlacementKind.After -> anchor?.let(::after)
                PlacementKind.Append -> null
                else -> return
            }
        if (p.kind == PlacementKind.Swap) store.remove(node.id)
        repeat(p.operationsLength) { operation(requireNotNull(p.operations(it)), row) }
        store.insert(parent.id, p.rootNode, before, row)
    }
}

private fun property(type: UByte, read: (Table) -> Table?): Property? =
    when (type) {
        PropValue.Length ->
            (read(LengthValue()) as? LengthValue)?.v?.let { Property.Length(it.value, it.unit) }
        PropValue.ColorToken -> (read(ColorToken()) as? ColorToken)?.let { Property.Color(it.id) }
        PropValue.FontToken -> (read(FontToken()) as? FontToken)?.let { Property.Font(it.id) }
        PropValue.Axis ->
            (read(AxisValue()) as? AxisValue)?.let { Names.axis[it.v]?.let(Property::Axis) }
        PropValue.Alignment ->
            (read(AlignmentValue()) as? AlignmentValue)?.let {
                Names.alignment[it.v]?.let(Property::Align)
            }
        PropValue.Bool -> (read(BoolValue()) as? BoolValue)?.let { Property.Flag(it.v) }
        PropValue.Text -> (read(TextValue()) as? TextValue)?.let { Property.Text(it.v.orEmpty()) }
        else -> null
    }
