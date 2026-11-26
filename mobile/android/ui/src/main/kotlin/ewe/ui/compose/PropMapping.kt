package ewe.ui.compose

import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import ewe.ui.runtime.Property

internal fun lengthToDp(length: Property.Length?): Dp? =
    length?.takeIf { it.unit.toInt() == 0 && it.number.isFinite() && it.number >= 0f }?.number?.dp

internal fun fractionOf(length: Property.Length): Float? =
    length
        .takeIf { it.unit.toInt() == 1 && it.number.isFinite() && it.number >= 0f }
        ?.let { (it.number / 100f).coerceAtMost(1f) }
