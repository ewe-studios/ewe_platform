package ewe.ui.runtime

import ewe.ui.*

internal fun FrameApplier.applyColumns(batch: ColumnarBatch, start: Int) {
    val count = batch.opCodesLength
    val actions = mutableListOf<() -> Unit>()
    try {
        require(
            count >= 0 &&
                batch.nodeIdsLength == count &&
                batch.argALength == count &&
                batch.argBLength == count
        )
        require(batch.blobAOffLength == count + 1 && batch.blobBOffLength == count + 1)
        require(batch.blobAOff(0) == 0u && batch.blobAOff(count) == batch.blobBytesLength.toUInt())
        require(batch.blobBOff(0) == 0u || batch.blobBOff(0) == batch.blobAOff(count))
        repeat(count) { i ->
            val from = batch.blobAOff(i).toInt()
            val to = batch.blobAOff(i + 1).toInt()
            require(
                from >= 0 &&
                    to >= from &&
                    to <= batch.blobBytesLength &&
                    batch.blobBOff(i) == batch.blobBOff(i + 1)
            )
            val blob = ByteArray(to - from) { batch.blobBytes(from + it).toByte() }
            val id = batch.nodeIds(i)
            val a = batch.argA(i)
            val b = batch.argB(i)
            val row = start + i
            val action: () -> Unit =
                when (batch.opCodes(i)) {
                    UiOperation.CreateNode -> {
                        require(a <= 65535u && b == 0u && blob.isEmpty());
                        { store.create(id, a.toUShort(), row) }
                    }
                    UiOperation.SetText -> {
                        require(a == 0u && b == 0u)
                        val value = blob.decodeToString(throwOnInvalidSequence = true);
                        { store.node(id, row)?.text = value }
                    }
                    UiOperation.SetProp -> {
                        val key = a.toUShort()
                        val value = columnProp(key, a shr 16, b, blob);
                        { setProp(id, key, value, row) }
                    }
                    UiOperation.AppendChild -> {
                        require(b == 0u && blob.isEmpty());
                        { store.insert(id, a, null, row) }
                    }
                    UiOperation.InsertBefore -> {
                        require(blob.isEmpty());
                        { store.insert(id, a, b, row) }
                    }
                    UiOperation.Remove -> {
                        require(a == 0u && b == 0u && blob.isEmpty());
                        {
                            store.node(id, row)?.let { store.remove(id) }
                        }
                    }
                    UiOperation.BindAction -> {
                        require(a <= 65535u && b == 0u)
                        val value = blob.decodeToString(throwOnInvalidSequence = true);
                        {
                            store.node(id, row)?.actions?.set(a.toUShort(), value)
                        }
                    }
                    UiOperation.UnbindAction -> {
                        require(a <= 65535u && b == 0u && blob.isEmpty());
                        {
                            store.node(id, row)?.actions?.remove(a.toUShort())
                        }
                    }
                    UiOperation.AttachState -> {
                        require(a == 0u && b == 0u);
                        { store.node(id, row)?.state = blob }
                    }
                    else -> throw IllegalArgumentException("Unknown column operation")
                }
            actions.add(action)
        }
    } catch (error: Exception) {
        store.report("InvalidColumnar", start)
        return
    }
    actions.forEach { it() }
}

private fun columnProp(id: UShort, unit: UInt, bits: UInt, blob: ByteArray): Property {
    val length = id in listOf(PropId.PADDING, PropId.SPACING, PropId.WIDTH, PropId.HEIGHT)
    val text = id in listOf(PropId.VALUE, PropId.PLACEHOLDER, PropId.A11Y_LABEL, PropId.SOURCE, PropId.PRESENTATION)
    require((length || unit == 0u) && (text || blob.isEmpty()))
    return when {
        length -> {
            require(unit <= 2u)
            Property.Length(Float.fromBits(bits.toInt()), unit.toUByte())
        }
        text -> {
            require(bits == 0u)
            Property.Text(blob.decodeToString(throwOnInvalidSequence = true))
        }
        id == PropId.AXIS -> {
            require(bits <= 1u)
            Property.Axis(Names.axis.getValue(bits.toUByte()))
        }
        id == PropId.ALIGN -> {
            require(bits <= 3u)
            Property.Align(Names.alignment.getValue(bits.toUByte()))
        }
        id == PropId.COLOR || id == PropId.BACKGROUND -> Property.Color(bits)
        id == PropId.FONT -> Property.Font(bits)
        id in listOf(PropId.ENABLED, PropId.HIDDEN, PropId.CHECKED) -> {
            require(bits <= 1u)
            Property.Flag(bits == 1u)
        }
        else -> throw IllegalArgumentException("Unknown property")
    }
}
