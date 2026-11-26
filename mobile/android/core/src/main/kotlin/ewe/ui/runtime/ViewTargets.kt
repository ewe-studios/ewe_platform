package ewe.ui.runtime

import ewe.ui.*

/** Portable element names belong to a registered receiving view. */
internal class ViewTargets(private val store: NodeStore) {
    val roots = mutableMapOf<String, UInt>()
    val tags = mutableMapOf<UInt, String>()
    val names = mutableMapOf<UInt, String>()
    val validName = Regex("[A-Za-z_][A-Za-z0-9_-]*")
    fun scope(mount: UInt): UInt? {
        if (mount == 0u) return roots.values.singleOrNull()
        var node = store.record(mount)
        while (node != null) {
            if (node.id == 1u || node.id in roots.values) return node.id
            node = node.parent?.let(store::record)
        }
        return store.record(mount)?.id
    }
    private fun inside(id: UInt, root: UInt): Boolean {
        var current = store.record(id)
        while (current != null) {
            if (current.id == root) return true
            if (current.id in roots.values) return false
            current = current.parent?.let(store::record)
        }
        return false
    }
    fun setName(id: UInt, name: String, row: Int): Boolean {
        if (!name.matches(validName)) { store.report("InvalidName",row,id); return false }
        val root = scope(id)
        if (root != null && names.any { (key,value) -> key != id && value == name && inside(key,root) }) {
            store.report("DuplicateName",row,id); return false
        }
        names[id] = name
        return true
    }
    fun resolve(selector: TargetSelector, scope: UInt?, row: Int): NodeRecord? {
        fun fail(code: String): NodeRecord? { store.report(code, row); return null }
        when (selector.kind) {
            SelectorKind.NodeId -> return store.record(selector.nodeId) ?: fail("MissingTarget")
            SelectorKind.View -> return roots[selector.text]?.let(store::record) ?: fail("MissingTarget")
            SelectorKind.Receiving -> return scope?.let(store::record) ?: fail("AmbiguousView")
            SelectorKind.Id, SelectorKind.TypedId -> Unit
            else -> return fail("UnsupportedSelector")
        }
        if (scope == null) return fail("AmbiguousView")
        val parts = selector.text?.split('#') ?: return fail("InvalidName")
        val name = parts.last()
        if (!name.matches(validName) || parts.size != (if (selector.kind == SelectorKind.Id) 1 else 2) ||
            !parts.first().matches(validName)) return fail("InvalidName")
        val matches = names.filter { (id, value) -> value == name && inside(id, scope) }
        if (matches.size != 1) return fail(if (matches.isEmpty()) "MissingTarget" else "DuplicateName")
        val record = store.record(matches.keys.first()) ?: return fail("MissingTarget")
        if (selector.kind == SelectorKind.TypedId) {
            fun compact(tag: String) = tag.lowercase().replace("_", "").replace("-", "")
            val tag = compact(parts.first())
            if (compact(tags[record.id] ?: Names.nodeKind[record.kind].orEmpty()) != (if (tag == "sheet") "bottomsheet" else tag))
                return fail("WrongTargetKind")
        }
        return record
    }
    fun validate(p: Place, scope: UInt?, removing: List<UInt>, row: Int): Boolean {
        val used = names.filterKeys { id -> scope != null && inside(id, scope) && removing.none { inside(id,it) } }.values.toMutableSet()
        fun check(name: String?): Boolean {
            if (name == null || !name.matches(validName)) { store.report("InvalidName", row); return false }
            if (!used.add(name)) { store.report("DuplicateName", row); return false }
            return true
        }
        repeat(p.operationsLength) { i ->
            val op = requireNotNull(p.operations(i))
            if (op.uiType == UiOperation.SetProp) {
                val prop = op.ui(SetProp()) as SetProp
                if (prop.prop == PropId.ID && !check((prop.value(TextValue()) as? TextValue)?.v)) return false
            }
            val batch = op.columnar ?: return@repeat
            for (j in 0 until batch.opCodesLength) {
                if (batch.opCodes(j) != UiOperation.SetProp || batch.argA(j).toUShort() != PropId.ID) continue
                val start = batch.blobAOff(j).toInt()
                val end = batch.blobAOff(j+1).toInt()
                require(start in 0..end && end <= batch.blobBytesLength)
                val bytes = ByteArray(end-start) { batch.blobBytes(start+it).toByte() }
                if (!check(bytes.decodeToString(throwOnInvalidSequence = true))) return false
            }
        }
        return true
    }
}
