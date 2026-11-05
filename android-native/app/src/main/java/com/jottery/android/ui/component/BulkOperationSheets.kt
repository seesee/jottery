package com.jottery.android.ui.component

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Label
import androidx.compose.material.icons.automirrored.filled.LabelOff
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.CallMerge
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Download
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Palette
import androidx.compose.material.icons.filled.SelectAll
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.jottery.android.R
import com.jottery.android.ui.theme.NoteColors
import com.jottery.android.ui.theme.noteColourLabel

/**
 * Bottom toolbar shown when notes are multi-selected. Six actions fit a phone, and
 * the two used least go in the overflow.
 *
 * @param onArchive null in the archive itself.
 * @param onCombine null unless at least two notes are selected.
 */
@Composable
fun BulkOperationsToolbar(
    selectedCount: Int,
    onAddTags: () -> Unit,
    onRemoveTags: () -> Unit,
    onSetColour: () -> Unit,
    onArchive: (() -> Unit)?,
    onExport: () -> Unit,
    onDelete: () -> Unit,
    onCombine: (() -> Unit)?,
    onSelectAll: () -> Unit,
    onDone: () -> Unit,
) {
    var overflowExpanded by remember { mutableStateOf(false) }

    Column(modifier = Modifier.fillMaxWidth()) {
        HorizontalDivider()
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp, vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = pluralStringResource(R.plurals.bulk_selected, selectedCount, selectedCount),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(modifier = Modifier.weight(1f))
            TextButton(onClick = onDone) { Text(stringResource(R.string.action_done)) }
        }
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 8.dp),
            horizontalArrangement = Arrangement.SpaceEvenly,
        ) {
            IconButton(onClick = onAddTags) {
                Icon(Icons.AutoMirrored.Filled.Label, contentDescription = stringResource(R.string.bulk_add_tags))
            }
            IconButton(onClick = onRemoveTags) {
                Icon(Icons.AutoMirrored.Filled.LabelOff, contentDescription = stringResource(R.string.bulk_remove_tags))
            }
            IconButton(onClick = onSetColour) {
                Icon(Icons.Default.Palette, contentDescription = stringResource(R.string.bulk_set_colour))
            }
            if (onArchive != null) {
                IconButton(onClick = onArchive) {
                    Icon(Icons.Default.Archive, contentDescription = stringResource(R.string.bulk_archive))
                }
            }
            IconButton(onClick = onExport) {
                Icon(Icons.Default.Download, contentDescription = stringResource(R.string.bulk_export))
            }
            IconButton(onClick = onDelete) {
                Icon(
                    Icons.Default.Delete,
                    contentDescription = stringResource(R.string.action_delete),
                    tint = MaterialTheme.colorScheme.error,
                )
            }
            Box {
                IconButton(onClick = { overflowExpanded = true }) {
                    Icon(Icons.Default.MoreVert, contentDescription = stringResource(R.string.action_more))
                }
                DropdownMenu(expanded = overflowExpanded, onDismissRequest = { overflowExpanded = false }) {
                    if (onCombine != null) {
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.bulk_combine)) },
                            leadingIcon = { Icon(Icons.Default.CallMerge, contentDescription = null) },
                            onClick = {
                                overflowExpanded = false
                                onCombine()
                            },
                        )
                    }
                    DropdownMenuItem(
                        text = { Text(stringResource(R.string.bulk_select_all)) },
                        leadingIcon = { Icon(Icons.Default.SelectAll, contentDescription = null) },
                        onClick = {
                            overflowExpanded = false
                            onSelectAll()
                        },
                    )
                }
            }
        }
    }
}

/**
 * Bottom sheet for choosing a colour for the selected notes, or none.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun ColourSheet(
    onChoose: (String?) -> Unit,
    onDismiss: () -> Unit,
) {
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState()) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(24.dp),
        ) {
            Text(stringResource(R.string.bulk_set_colour), style = MaterialTheme.typography.titleMedium)
            Spacer(modifier = Modifier.height(16.dp))
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(12.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                NoteColors.allColors.forEach { (name, colour) ->
                    Box(
                        modifier = Modifier
                            .size(40.dp)
                            .clip(CircleShape)
                            .background(colour)
                            .clickable(onClickLabel = noteColourLabel(name)) { onChoose(name) },
                    )
                }
            }
            Spacer(modifier = Modifier.height(16.dp))
            TextButton(onClick = { onChoose(null) }) { Text(stringResource(R.string.bulk_clear_colour)) }
            Spacer(modifier = Modifier.height(16.dp))
        }
    }
}

/**
 * Bottom sheet for adding tags to selected notes.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AddTagsSheet(
    onConfirm: (List<String>) -> Unit,
    onDismiss: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState()
    var tagInput by remember { mutableStateOf("") }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(24.dp),
        ) {
            Text(stringResource(R.string.bulk_operation_sheets_add_tags), style = MaterialTheme.typography.titleMedium)
            Spacer(modifier = Modifier.height(12.dp))
            OutlinedTextField(
                value = tagInput,
                onValueChange = { tagInput = it },
                label = { Text(stringResource(R.string.bulk_operation_sheets_tags_comma_separated)) },
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(modifier = Modifier.height(16.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.End,
            ) {
                TextButton(onClick = onDismiss) { Text(stringResource(R.string.dialog_cancel)) }
                TextButton(
                    onClick = {
                        val tags = tagInput.split(",").map { it.trim() }.filter { it.isNotEmpty() }
                        onConfirm(tags)
                    }
                ) { Text(stringResource(R.string.bulk_operation_sheets_add)) }
            }
            Spacer(modifier = Modifier.height(16.dp))
        }
    }
}

/**
 * Bottom sheet for removing tags from selected notes.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun RemoveTagsSheet(
    availableTags: List<String>,
    onConfirm: (List<String>) -> Unit,
    onDismiss: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState()
    var selectedTags by remember { mutableStateOf(emptySet<String>()) }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(24.dp),
        ) {
            Text(stringResource(R.string.bulk_operation_sheets_remove_tags), style = MaterialTheme.typography.titleMedium)
            Spacer(modifier = Modifier.height(12.dp))
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(4.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                availableTags.forEach { tag ->
                    FilterChip(
                        selected = tag in selectedTags,
                        onClick = {
                            selectedTags = if (tag in selectedTags) selectedTags - tag
                            else selectedTags + tag
                        },
                        label = { Text(tag) },
                    )
                }
            }
            Spacer(modifier = Modifier.height(16.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.End,
            ) {
                TextButton(onClick = onDismiss) { Text(stringResource(R.string.dialog_cancel)) }
                TextButton(
                    onClick = { onConfirm(selectedTags.toList()) }
                ) { Text(stringResource(R.string.attachment_remove_confirm)) }
            }
            Spacer(modifier = Modifier.height(16.dp))
        }
    }
}
