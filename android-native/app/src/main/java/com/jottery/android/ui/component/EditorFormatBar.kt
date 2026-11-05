package com.jottery.android.ui.component

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.FormatBold
import androidx.compose.material.icons.filled.FormatItalic
import androidx.compose.material.icons.filled.FormatListBulleted
import androidx.compose.material.icons.filled.Link
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.TextFieldValue
import com.jottery.android.R
import com.jottery.android.ui.editor.QuickCommand
import com.jottery.android.ui.editor.TextFormatting

/**
 * A row above the keyboard: markdown formatting where it means something, and
 * quick insertion everywhere. A full visual editor doesn't suit a phone.
 */
@Composable
fun EditorFormatBar(
    showFormatting: Boolean,
    onEdit: ((TextFieldValue) -> TextFieldValue) -> Unit,
) {
    var insertMenuExpanded by remember { mutableStateOf(false) }

    Column(modifier = Modifier.fillMaxWidth()) {
        HorizontalDivider()
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .horizontalScroll(rememberScrollState()),
        ) {
            if (showFormatting) {
                FormatButton(Icons.Default.FormatBold, R.string.format_bold) { onEdit { TextFormatting.wrap(it, "**") } }
                FormatButton(Icons.Default.FormatItalic, R.string.format_italic) { onEdit { TextFormatting.wrap(it, "*") } }
                FormatButton(Icons.Default.FormatListBulleted, R.string.format_bullet_list) {
                    onEdit { TextFormatting.prefixLines(it, "- ") }
                }
                FormatButton(Icons.Default.Link, R.string.format_link) { onEdit(TextFormatting::link) }
            }
            Box {
                FormatButton(Icons.Default.Add, R.string.format_insert) { insertMenuExpanded = true }
                DropdownMenu(expanded = insertMenuExpanded, onDismissRequest = { insertMenuExpanded = false }) {
                    for ((command, label) in insertions) {
                        DropdownMenuItem(
                            text = { Text(stringResource(label)) },
                            onClick = {
                                insertMenuExpanded = false
                                onEdit { TextFormatting.insert(it, command.generate()) }
                            },
                        )
                    }
                }
            }
        }
    }
}

private val insertions = listOf(
    QuickCommand.NOW to R.string.format_insert_now,
    QuickCommand.DATE to R.string.format_insert_date,
    QuickCommand.TIME to R.string.format_insert_time,
    QuickCommand.UUID_V4 to R.string.format_insert_uuid,
)

@Composable
private fun FormatButton(icon: ImageVector, label: Int, onClick: () -> Unit) {
    IconButton(onClick = onClick) {
        Icon(icon, contentDescription = stringResource(label))
    }
}
