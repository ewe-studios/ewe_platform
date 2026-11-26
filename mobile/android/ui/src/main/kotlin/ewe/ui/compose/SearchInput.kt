package ewe.ui.compose

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.SearchBarDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import ewe.ui.EventId
import ewe.ui.PropId
import ewe.ui.runtime.EventBridge
import ewe.ui.runtime.NodeRecord
import io.github.ewe_studios.ui.ui.R

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SearchInput(record: NodeRecord, bridge: EventBridge, modifier: Modifier, enabled: Boolean) {
    val initial = record.props[PropId.VALUE]?.label.orEmpty()
    var query by remember(record, initial) { mutableStateOf(initial) }
    val focus = LocalFocusManager.current
    val change: (String) -> Unit = {
        query = it
        bridge.send(EventId.CHANGE, record.id, it)
    }
    val container = SearchBarDefaults.colors().containerColor
    TextField(
        value = query,
        onValueChange = change,
        singleLine = true,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
        keyboardActions = KeyboardActions(onSearch = {
            bridge.send(EventId.SUBMIT, record.id, query)
            focus.clearFocus()
        }),
        modifier = modifier.fillMaxWidth().padding(vertical = 8.dp),
        shape = SearchBarDefaults.inputFieldShape,
        colors = TextFieldDefaults.colors(
            focusedContainerColor = container,
            unfocusedContainerColor = container,
            disabledContainerColor = container,
            focusedIndicatorColor = Color.Transparent,
            unfocusedIndicatorColor = Color.Transparent,
            disabledIndicatorColor = Color.Transparent,
        ),
        enabled = enabled,
        placeholder = { Text(record.props[PropId.PLACEHOLDER]?.label.orEmpty()) },
        leadingIcon = { Icon(painterResource(R.drawable.ewe_search), contentDescription = null) },
        trailingIcon = if (query.isEmpty()) null else {
            {
                IconButton(onClick = { change("") }, enabled = enabled) {
                    Icon(painterResource(R.drawable.ewe_clear), stringResource(R.string.ewe_clear_search))
                }
            }
        },
    )
}
