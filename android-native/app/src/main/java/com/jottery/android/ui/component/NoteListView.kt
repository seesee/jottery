package com.jottery.android.ui.component

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.material.icons.filled.PushPin
import androidx.compose.material.icons.filled.Unarchive
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.jottery.android.R
import com.jottery.android.model.DecryptedNote
import com.jottery.android.viewmodel.AppState
import com.jottery.android.viewmodel.defaultExportName
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
    val conflicts by appState.pendingConflicts.collectAsState()
    val conflictIds = remember(conflicts) { conflicts.map { it.id }.toSet() }
    val otherViewMatches by appState.otherViewMatches.collectAsState()

    // Derive counts from collected state
    val noteCount = if (showArchive) archivedNotes.size else allNotes.size
    val filteredCount = notes.size
    val scope = rememberCoroutineScope()
    val listState = rememberLazyListState()

    // Multi-select state
    var isSelectMode by remember { mutableStateOf(false) }
    var selectedIds by remember { mutableStateOf(emptySet<String>()) }
    var showBulkDeleteConfirm by remember { mutableStateOf(false) }

    var showAddTags by remember { mutableStateOf(false) }
    var showRemoveTags by remember { mutableStateOf(false) }
    var showColours by remember { mutableStateOf(false) }
    var showCombineConfirm by remember { mutableStateOf(false) }

    fun endSelection() {
        isSelectMode = false
        selectedIds = emptySet()
    }

    val context = LocalContext.current
    val protectedMessage = stringResource(R.string.note_protected)
    val cantCombineMessage = stringResource(R.string.bulk_combine_needs_two)
    val exportLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/json")
    ) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        val ids = selectedIds
        scope.launch {
            // The selection clears only once the file has really been written
            val message = appState.exportTo(context, uri, ids)
            appState.showMessage(message)
            if (message.isNotEmpty()) {
                isSelectMode = false
                selectedIds = emptySet()
            }
        }
    }

    // Reset selection when archive mode changes
    LaunchedEffect(showArchive) {
        isSelectMode = false
        selectedIds = emptySet()
    }

    val pinLabel = stringResource(R.string.editor_pin)
    val unpinLabel = stringResource(R.string.editor_unpin)
    val unarchiveLabel = stringResource(R.string.editor_unarchive)
    val deleteLabel = stringResource(R.string.editor_delete)

    // A swipe, and the same rules as a swipe, for anyone who can't make one
    fun rowActions(note: DecryptedNote): List<CustomAccessibilityAction> = buildList {
        val rightSwipe = when {
            showArchive -> unarchiveLabel
            note.pinned -> unpinLabel
            else -> pinLabel
        }
        add(
            CustomAccessibilityAction(rightSwipe) {
                scope.launch {
                    if (showArchive) appState.unarchiveNote(note.id) else appState.togglePin(note.id)
                }
                true
            }
        )
        if (!note.pinned && !note.locked) {
            add(
                CustomAccessibilityAction(deleteLabel) {
                    scope.launch { appState.deleteNoteWithUndo(note) }
                    true
                }
            )
        }
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
                                showArchive -> stringResource(R.string.note_list_no_archived_notes)
                                searchQuery.isNotEmpty() -> stringResource(R.string.note_list_no_results)
                                else -> stringResource(R.string.note_list_no_notes)
                            },
                            style = MaterialTheme.typography.titleMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = when {
                                showArchive -> stringResource(R.string.note_list_archived_notes_will_appear_here)
                                searchQuery.isNotEmpty() -> stringResource(R.string.note_list_try_a_different_search_term)
                                else -> stringResource(R.string.note_list_tap_to_create_your_first)
                            },
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
                            textAlign = TextAlign.Center,
                        )
                        if (searchQuery.isNotEmpty() && otherViewMatches > 0) {
                            Spacer(modifier = Modifier.height(16.dp))
                            TextButton(onClick = { appState.toggleArchiveView() }) {
                                Text(
                                    stringResource(
                                        if (showArchive) R.string.search_in_notes else R.string.search_in_archive,
                                        otherViewMatches,
                                    )
                                )
                            }
                        }
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
                                hasConflict = note.id in conflictIds,
                            )
                        } else {
                            // Normal mode with swipe-to-delete
                            val dismissState = rememberSwipeToDismissBoxState(
                                confirmValueChange = { value ->
                                    when (value) {
                                        SwipeToDismissBoxValue.EndToStart -> {
                                            // A swipe is easy to make by accident, so
                                            // protected notes refuse and the rest can be undone
                                            if (note.pinned || note.locked) {
                                                appState.showMessage(protectedMessage)
                                                false
                                            } else {
                                                scope.launch { appState.deleteNoteWithUndo(note) }
                                                true
                                            }
                                        }
                                        SwipeToDismissBoxValue.StartToEnd -> {
                                            // Pin here, or back to the notes from the archive
                                            scope.launch {
                                                if (showArchive) appState.unarchiveNote(note.id) else appState.togglePin(note.id)
                                            }
                                            false // Reset after action
                                        }
                                        SwipeToDismissBoxValue.Settled -> false
                                    }
                                },
                            )

                            SwipeToDismissBox(
                                state = dismissState,
                                backgroundContent = {
                                    SwipeToDismissBackground(dismissState.targetValue, showArchive)
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
                                    hasConflict = note.id in conflictIds,
                                    onTagClick = { appState.setSearchQuery("#$it") },
                                    accessibilityActions = rowActions(note),
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
                onAddTags = { showAddTags = true },
                onRemoveTags = { showRemoveTags = true },
                onSetColour = { showColours = true },
                onArchive = if (showArchive) null else { ->
                    val ids = selectedIds
                    endSelection()
                    scope.launch {
                        val skipped = appState.archiveNotes(ids)
                        if (skipped > 0) {
                            appState.showMessage(
                                context.resources.getQuantityString(R.plurals.bulk_skipped_locked, skipped, skipped)
                            )
                        }
                    }
                },
                onExport = { exportLauncher.launch(defaultExportName()) },
                onDelete = { showBulkDeleteConfirm = true },
                onCombine = if (selectedIds.size < 2) null else { -> showCombineConfirm = true },
                onSelectAll = { selectedIds = notes.map { it.id }.toSet() },
                onDone = { endSelection() },
            )
        }
    }

    if (showColours) {
        ColourSheet(
            onChoose = { colour ->
                showColours = false
                val ids = selectedIds
                scope.launch { appState.setColourOnNotes(ids, colour) }
            },
            onDismiss = { showColours = false },
        )
    }

    if (showCombineConfirm) {
        ConfirmDialog(
            title = stringResource(R.string.bulk_combine),
            message = pluralStringResource(R.plurals.bulk_combine_message, selectedIds.size, selectedIds.size),
            confirmText = stringResource(R.string.bulk_combine),
            onConfirm = {
                showCombineConfirm = false
                val ids = selectedIds
                endSelection()
                scope.launch {
                    if (appState.combineNotes(ids) == null) appState.showMessage(cantCombineMessage)
                }
            },
            onDismiss = { showCombineConfirm = false },
        )
    }

    if (showAddTags) {
        AddTagsSheet(
            onConfirm = { tags ->
                showAddTags = false
                val ids = selectedIds
                scope.launch { appState.addTagsToNotes(ids, tags) }
            },
            onDismiss = { showAddTags = false },
        )
    }

    if (showRemoveTags) {
        RemoveTagsSheet(
            availableTags = notes.filter { it.id in selectedIds }.flatMap { it.tags }.distinct().sorted(),
            onConfirm = { tags ->
                showRemoveTags = false
                val ids = selectedIds
                scope.launch { appState.removeTagsFromNotes(ids, tags) }
            },
            onDismiss = { showRemoveTags = false },
        )
    }

    // Bulk delete confirmation dialog
    if (showBulkDeleteConfirm) {
        ConfirmDialog(
            title = stringResource(R.string.bulk_delete_title),
            message = pluralStringResource(R.plurals.bulk_delete_message, selectedIds.size, selectedIds.size),
            confirmText = stringResource(R.string.action_delete),
            onConfirm = {
                showBulkDeleteConfirm = false
                val ids = selectedIds
                endSelection()
                scope.launch {
                    val skipped = appState.deleteNotes(ids)
                    if (skipped > 0) {
                        appState.showMessage(
                            context.resources.getQuantityString(R.plurals.bulk_skipped_protected, skipped, skipped)
                        )
                    }
                }
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
    showArchive: Boolean,
) {
    val color = when (dismissDirection) {
        SwipeToDismissBoxValue.EndToStart -> MaterialTheme.colorScheme.error
        SwipeToDismissBoxValue.StartToEnd ->
            if (showArchive) MaterialTheme.colorScheme.primary else Color(0xFFFF9800) // Orange for pin
        SwipeToDismissBoxValue.Settled -> Color.Transparent
    }
    val alignment = when (dismissDirection) {
        SwipeToDismissBoxValue.EndToStart -> Alignment.CenterEnd
        else -> Alignment.CenterStart
    }
    val icon = when (dismissDirection) {
        SwipeToDismissBoxValue.EndToStart -> Icons.Default.Delete
        else -> if (showArchive) Icons.Default.Unarchive else Icons.Default.PushPin
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
