package com.jottery.android.ui.component

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import com.jottery.android.R

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun TagInputView(
    tags: List<String>,
    allTags: List<String>,
    onTagsChanged: (List<String>) -> Unit,
    modifier: Modifier = Modifier,
) {
    var inputText by remember { mutableStateOf("") }

    FlowRow(
        modifier = modifier.padding(horizontal = 8.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        tags.forEach { tag ->
            AssistChip(
                onClick = {},
                label = { Text(tag, style = MaterialTheme.typography.labelSmall) },
                trailingIcon = {
                    IconButton(
                        onClick = { onTagsChanged(tags - tag) },
                        modifier = Modifier
                            .size(24.dp)
                            .testTag("remove-tag"),
                    ) {
                        Icon(
                            Icons.Default.Close,
                            contentDescription = stringResource(R.string.tag_input_remove_tag),
                            modifier = Modifier.size(16.dp),
                        )
                    }
                },
                modifier = Modifier
                    .heightIn(min = 32.dp)
                    .testTag("tag-chip"),
            )
        }

        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier.heightIn(min = 32.dp),
        ) {
            BasicTextField(
                value = inputText,
                onValueChange = { inputText = it },
                singleLine = true,
                textStyle = MaterialTheme.typography.labelSmall.copy(
                    color = MaterialTheme.colorScheme.onSurface,
                ),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(
                    onDone = {
                        val tag = inputText.trim()
                        if (tag.isNotEmpty() && tag !in tags) {
                            onTagsChanged(tags + tag)
                            inputText = ""
                        }
                    }
                ),
                decorationBox = { inner ->
                    if (inputText.isEmpty()) {
                        Text(
                            stringResource(R.string.tag_input_add_tag),
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
                        )
                    }
                    inner()
                },
            )
        }
    }
}
