package com.jottery.android.ui.editor

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import com.jottery.android.R

/** Find, and replace unless the note is read-only. */
@Composable
fun FindBar(
    query: String,
    onQueryChange: (String) -> Unit,
    replacement: String,
    onReplacementChange: (String) -> Unit,
    matchCount: Int,
    currentMatch: Int,
    canReplace: Boolean,
    onNext: () -> Unit,
    onPrevious: () -> Unit,
    onReplace: () -> Unit,
    onReplaceAll: () -> Unit,
    onClose: () -> Unit,
) {
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }

    Surface(tonalElevation = 2.dp, modifier = Modifier.fillMaxWidth()) {
        Column(
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                OutlinedTextField(
                    value = query,
                    onValueChange = onQueryChange,
                    label = { Text(stringResource(R.string.editor_find)) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    keyboardActions = KeyboardActions(onSearch = { onNext() }),
                    modifier = Modifier
                        .weight(1f)
                        .focusRequester(focus),
                )
                if (query.isNotEmpty()) {
                    Text(
                        text = if (matchCount == 0) {
                            stringResource(R.string.editor_find_none)
                        } else {
                            stringResource(R.string.editor_find_count, currentMatch + 1, matchCount)
                        },
                        style = MaterialTheme.typography.labelMedium,
                        modifier = Modifier.padding(horizontal = 4.dp),
                    )
                }
                IconButton(onClick = onPrevious, enabled = matchCount > 0) {
                    Icon(Icons.Default.KeyboardArrowUp, contentDescription = stringResource(R.string.editor_find_previous))
                }
                IconButton(onClick = onNext, enabled = matchCount > 0) {
                    Icon(Icons.Default.KeyboardArrowDown, contentDescription = stringResource(R.string.editor_find_next))
                }
                IconButton(onClick = onClose) {
                    Icon(Icons.Default.Close, contentDescription = stringResource(R.string.editor_find_close))
                }
            }
            if (canReplace) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    OutlinedTextField(
                        value = replacement,
                        onValueChange = onReplacementChange,
                        label = { Text(stringResource(R.string.editor_replace)) },
                        singleLine = true,
                        modifier = Modifier.weight(1f),
                    )
                    TextButton(onClick = onReplace, enabled = matchCount > 0) {
                        Text(stringResource(R.string.editor_replace_one))
                    }
                    TextButton(onClick = onReplaceAll, enabled = matchCount > 0) {
                        Text(stringResource(R.string.editor_replace_all))
                    }
                }
            }
        }
    }
}
