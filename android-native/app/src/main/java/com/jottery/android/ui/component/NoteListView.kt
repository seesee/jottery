package com.jottery.android.ui.component

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Done
import androidx.compose.material.icons.filled.Label
import androidx.compose.material.icons.filled.LabelOff
import androidx.compose.material.icons.filled.Share
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.jottery.android.model.DecryptedNote
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun NoteListView(
    appState: AppState,
    notes: List<DecryptedNote>,
    selectedNoteId: String?,
    onNoteSelected: (String) -> Unit,
    showArchive: Boolean,
    modifier: Modifier = Modifier,
) {
    val allNotes by appState.notes.collectAsState()
    val archivedNotes by appState.archivedNotes.collectAsState()
    val syncEnabled by appState.syncEnabled.collectAsState()
    val isSyncing by appState.isSyncing.collectAsState()
    val searchQuery by appState.searchQuery.collectAsState()

    // Derive counts from collected state
    val noteCount = if (showArchive) archivedNotes.size else allNotes.size
    val filteredCount = notes.size
    val scope = rememberCoroutineScope()
    val listState = rememberLazyListState()

    // Multi-select state
    var isSelectMode by remember { mutableStateOf(false) }
    var selectedIds by remember { mutableStateOf(emptySet<String>()) }
    var showBulkDeleteConfirm by remember { mutableStateOf(false) }

    // Reset selection when archive mode changes
    LaunchedEffect(showArchive) {
        isSelectMode = false
        selectedIds = emptySet()
    }

    Column(modifier = modifier) {
        // Match count when searching
        if (searchQuery.isNotEmpty() && notes.isNotEmpty()) {
            Text(
                text = "$filteredCount/$noteCount",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = 4.dp),
            )
        }

        // Note list with pull-to-refresh
        PullToRefreshBox(
            isRefreshing = isSyncing,
            onRefresh = {
                if (syncEnabled) {
                    scope.launch { appState.triggerSync() }
                }
            },
            modifier = Modifier.weight(1f),
        ) {
            if (notes.isEmpty()) {
                // Empty state
                Box(
                    modifier = Modifier.fillMaxSize(),
                    contentAlignment = Alignment.Center,
                ) {
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        modifier = Modifier.padding(32.dp),
                    ) {
                        Text(
                            text = when {
                                showArchive -> "No Archived Notes"
                                searchQuery.isNotEmpty() -> "No Results"
                                else -> "No Notes"
                            },
                            style = MaterialTheme.typography.titleMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = when {
                                showArchive -> "Archived notes will appear here."
                                searchQuery.isNotEmpty() -> "Try a different search term."
                                else -> "Tap + to create your first note."
                            },
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
                            textAlign = TextAlign.Center,
                        )
                    }
                }
            } else {
                LazyColumn(
                    state = listState,
                    contentPadding = PaddingValues(vertical = 4.dp),
                    modifier = Modifier.fillMaxSize(),
                ) {
                    items(
                        items = notes,
                        key = { it.id },
                    ) { note ->
                        val isSelected = note.id == selectedNoteId
                        val isChecked = note.id in selectedIds

                        if (isSelectMode) {
                            // Multi-select mode: tap toggles checkbox
                            NoteRowView(
                                note = note,
                                isSelected = isSelected,
                                isMultiSelectMode = true,
                                isChecked = isChecked,
                                onClick = {
                                    selectedIds = if (isChecked) {
                                        selectedIds - note.id
                                    } else {
                                        selectedIds + note.id
                                    }
                                    if (selectedIds.isEmpty()) {
                                        isSelectMode = false
                                    }
                                },
                                onLongClick = {},
                                modifier = Modifier.animateItem(),
                            )
                        } else {
                            // Normal mode with swipe-to-delete
                            val dismissState = rememberSwipeToDismissBoxState(
                                confirmValueChange = { value ->
                                    when (value) {
                                        SwipeToDismissBoxValue.EndToStart -> {
                                            // Swipe left to delete
                                            scope.launch { appState.deleteNote(note.id) }
                                            true
                                        }
                                        SwipeToDismissBoxValue.StartToEnd -> {
                                            // Swipe right to pin/unpin
                                            scope.launch { appState.togglePin(note.id) }
                                            false // Reset after action
                                        }
                                        SwipeToDismissBoxValue.Settled -> false
                                    }
                                },
                            )

                            SwipeToDismissBox(
                                state = dismissState,
                                backgroundContent = {
                                    SwipeToDismissBackground(dismissState.targetValue)
                                },
                                modifier = Modifier.animateItem(),
                            ) {
                                NoteRowView(
                                    note = note,
                                    isSelected = isSelected,
                                    isMultiSelectMode = false,
                                    isChecked = false,
                                    onClick = { onNoteSelected(note.id) },
                                    onLongClick = {
                                        isSelectMode = true
                                        selectedIds = setOf(note.id)
                                    },
                                )
                            }
                        }

                        if (note.id != notes.lastOrNull()?.id) {
                            HorizontalDivider(
                                modifier = Modifier.padding(start = 16.dp),
                                color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
                            )
                        }
                    }
                }
            }
        }

        // Bulk operations toolbar
        AnimatedVisibility(
            visible = isSelectMode && selectedIds.isNotEmpty(),
            enter = slideInVertically(initialOffsetY = { it }),
            exit = slideOutVertically(targetOffsetY = { it }),
        ) {
            BulkOperationsToolbar(
                selectedCount = selectedIds.size,
                totalCount = notes.size,
                onSelectAll = { selectedIds = notes.map { it.id }.toSet() },
                onDeselectAll = {
                    selectedIds = emptySet()
                    isSelectMode = false
                },
                onAddTags = {
                    // TODO: Show add tags dialog
                },
                onRemoveTags = {
                    // TODO: Show remove tags dialog
                },
                onExport = {
                    scope.launch { appState.exportNotes(selectedIds) }
                    isSelectMode = false
                    selectedIds = emptySet()
                },
                onDelete = { showBulkDeleteConfirm = true },
                onDone = {
                    isSelectMode = false
                    selectedIds = emptySet()
                },
            )
        }
    }

    // Bulk delete confirmation dialog
    if (showBulkDeleteConfirm) {
        ConfirmDialog(
            title = "Delete Notes",
            message = "Are you sure you want to delete ${selectedIds.size} note${if (selectedIds.size == 1) "" else "s"}? They will be moved to the recycle bin.",
            confirmText = "Delete",
            onConfirm = {
                scope.launch {
                    for (id in selectedIds) {
                        appState.deleteNote(id)
                    }
                    isSelectMode = false
                    selectedIds = emptySet()
                }
                showBulkDeleteConfirm = false
            },
            onDismiss = { showBulkDeleteConfirm = false },
        )
    }
}

/**
 * Background content shown when swiping a note row.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SwipeToDismissBackground(
    dismissDirection: SwipeToDismissBoxValue,
) {
    val color = when (dismissDirection) {
        SwipeToDismissBoxValue.EndToStart -> MaterialTheme.colorScheme.error
        SwipeToDismissBoxValue.StartToEnd -> Color(0xFFFF9800) // Orange for pin
        SwipeToDismissBoxValue.Settled -> Color.Transparent
    }
    val alignment = when (dismissDirection) {
        SwipeToDismissBoxValue.EndToStart -> Alignment.CenterEnd
        else -> Alignment.CenterStart
    }
    val icon = when (dismissDirection) {
        SwipeToDismissBoxValue.EndToStart -> Icons.Default.Delete
        else -> Icons.Default.Done // Pin icon placeholder
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 20.dp),
        contentAlignment = alignment,
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = color,
            modifier = Modifier.size(24.dp),
        )
    }
}

/**
 * Toolbar shown at the bottom of the list during multi-select mode.
 */
@Composable
private fun BulkOperationsToolbar(
    selectedCount: Int,
    totalCount: Int,
    onSelectAll: () -> Unit,
    onDeselectAll: () -> Unit,
    onAddTags: () -> Unit,
    onRemoveTags: () -> Unit,
    onExport: () -> Unit,
    onDelete: () -> Unit,
    onDone: () -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxWidth(),
    ) {
        HorizontalDivider()

        // Header row: count + selection controls + done
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp, vertical = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = "$selectedCount selected",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            Spacer(modifier = Modifier.weight(1f))

            TextButton(
                onClick = onSelectAll,
                contentPadding = PaddingValues(horizontal = 8.dp),
            ) {
                Text("Select All", style = MaterialTheme.typography.labelSmall)
            }

            TextButton(
                onClick = onDone,
                contentPadding = PaddingValues(horizontal = 8.dp),
            ) {
                Text("Done", style = MaterialTheme.typography.labelSmall)
            }
        }

        // Action buttons row
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 8.dp, vertical = 4.dp),
            horizontalArrangement = Arrangement.SpaceEvenly,
        ) {
            IconButton(onClick = onAddTags) {
                Icon(
                    Icons.Default.Label,
                    contentDescription = "Add tags",
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            IconButton(onClick = onRemoveTags) {
                Icon(
                    Icons.Default.LabelOff,
                    contentDescription = "Remove tags",
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            IconButton(onClick = onExport) {
                Icon(
                    Icons.Default.Share,
                    contentDescription = "Export",
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            IconButton(onClick = onDelete) {
                Icon(
                    Icons.Default.Delete,
                    contentDescription = "Delete",
                    tint = MaterialTheme.colorScheme.error,
                )
            }
        }
    }
}
