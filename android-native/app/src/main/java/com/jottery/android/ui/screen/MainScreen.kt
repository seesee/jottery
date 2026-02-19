package com.jottery.android.ui.screen

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Sort
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.adaptive.currentWindowAdaptiveInfo
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.window.core.layout.WindowWidthSizeClass
import com.jottery.android.model.SortOrder
import com.jottery.android.ui.component.NoteEditorView
import com.jottery.android.ui.component.NoteListView
import com.jottery.android.ui.component.SearchBar
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MainScreen(
    appState: AppState,
    onNavigateToRecycleBin: () -> Unit,
    onNavigateToSettings: () -> Unit,
) {
    val notes by appState.filteredNotes.collectAsState()
    val allNotes by appState.notes.collectAsState()
    val archivedNotes by appState.archivedNotes.collectAsState()
    val selectedNoteId by appState.selectedNoteId.collectAsState()
    val searchQuery by appState.searchQuery.collectAsState()
    val sortOrder by appState.sortOrder.collectAsState()
    val isSyncing by appState.isSyncing.collectAsState()
    val syncStatusMessage by appState.syncStatusMessage.collectAsState()
    val syncError by appState.syncError.collectAsState()
    val syncEnabled by appState.syncEnabled.collectAsState()
    val showArchive by appState.showArchive.collectAsState()
    val scope = rememberCoroutineScope()

    // Derive counts from collected state
    val noteCount by remember(allNotes, archivedNotes, showArchive) {
        derivedStateOf { if (showArchive) archivedNotes.size else allNotes.size }
    }
    val filteredCount by remember(notes) {
        derivedStateOf { notes.size }
    }

    var searchVisible by remember { mutableStateOf(false) }
    var sortMenuExpanded by remember { mutableStateOf(false) }
    var overflowMenuExpanded by remember { mutableStateOf(false) }

    // Determine layout based on window width
    val windowSizeClass = currentWindowAdaptiveInfo().windowSizeClass
    val isExpandedLayout = windowSizeClass.windowWidthSizeClass != WindowWidthSizeClass.COMPACT

    // Selected note for editor
    val selectedNote = remember(selectedNoteId, notes) {
        notes.firstOrNull { it.id == selectedNoteId }
    }

    // On narrow screens, back from editor deselects note
    val showEditor = selectedNote != null
    val showListOnNarrow = !showEditor || isExpandedLayout

    if (!isExpandedLayout && showEditor) {
        BackHandler {
            appState.setSelectedNoteId(null)
        }
    }

    Scaffold(
        topBar = {
            if (showListOnNarrow) {
                TopAppBar(
                    title = {
                        Text(if (showArchive) "Archive" else "Jottery")
                    },
                    colors = TopAppBarDefaults.topAppBarColors(
                        containerColor = MaterialTheme.colorScheme.surface,
                    ),
                    actions = {
                        // Search toggle
                        IconButton(onClick = { searchVisible = !searchVisible }) {
                            Icon(Icons.Default.Search, contentDescription = "Search")
                        }

                        // Sort menu
                        Box {
                            IconButton(onClick = { sortMenuExpanded = true }) {
                                Icon(Icons.AutoMirrored.Filled.Sort, contentDescription = "Sort")
                            }
                            DropdownMenu(
                                expanded = sortMenuExpanded,
                                onDismissRequest = { sortMenuExpanded = false },
                            ) {
                                SortOrder.entries.forEach { order ->
                                    DropdownMenuItem(
                                        text = { Text(order.displayName) },
                                        onClick = {
                                            appState.setSortOrder(order)
                                            sortMenuExpanded = false
                                        },
                                        leadingIcon = {
                                            RadioButton(
                                                selected = sortOrder == order,
                                                onClick = null,
                                            )
                                        },
                                    )
                                }
                            }
                        }

                        // Overflow menu (settings, recycle bin, archive toggle, lock)
                        Box {
                            IconButton(onClick = { overflowMenuExpanded = true }) {
                                Icon(Icons.Default.Settings, contentDescription = "More")
                            }
                            DropdownMenu(
                                expanded = overflowMenuExpanded,
                                onDismissRequest = { overflowMenuExpanded = false },
                            ) {
                                DropdownMenuItem(
                                    text = {
                                        Text(if (showArchive) "Show Notes" else "Show Archive")
                                    },
                                    onClick = {
                                        appState.toggleArchiveView()
                                        overflowMenuExpanded = false
                                    },
                                    leadingIcon = {
                                        Icon(Icons.Default.Archive, contentDescription = null)
                                    },
                                )
                                DropdownMenuItem(
                                    text = { Text("Recycle Bin") },
                                    onClick = {
                                        overflowMenuExpanded = false
                                        onNavigateToRecycleBin()
                                    },
                                    leadingIcon = {
                                        Icon(Icons.Default.Delete, contentDescription = null)
                                    },
                                )
                                DropdownMenuItem(
                                    text = { Text("Settings") },
                                    onClick = {
                                        overflowMenuExpanded = false
                                        onNavigateToSettings()
                                    },
                                    leadingIcon = {
                                        Icon(Icons.Default.Settings, contentDescription = null)
                                    },
                                )
                                DropdownMenuItem(
                                    text = { Text("Lock") },
                                    onClick = {
                                        overflowMenuExpanded = false
                                        appState.lock()
                                    },
                                    leadingIcon = {
                                        Icon(Icons.Default.Lock, contentDescription = null)
                                    },
                                )
                            }
                        }
                    },
                )
            }
        },
        floatingActionButton = {
            if (showListOnNarrow && !showArchive) {
                FloatingActionButton(
                    onClick = {
                        scope.launch { appState.createNote() }
                    },
                    containerColor = MaterialTheme.colorScheme.primary,
                ) {
                    Icon(
                        Icons.Default.Add,
                        contentDescription = "New note",
                        tint = MaterialTheme.colorScheme.onPrimary,
                    )
                }
            }
        },
    ) { paddingValues ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues),
        ) {
            // Search bar
            AnimatedVisibility(
                visible = searchVisible && showListOnNarrow,
                enter = slideInVertically(),
                exit = slideOutVertically(),
            ) {
                SearchBar(
                    query = searchQuery,
                    onQueryChange = { appState.setSearchQuery(it) },
                    matchCount = filteredCount,
                    totalCount = noteCount,
                    showCounts = searchQuery.isNotEmpty(),
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }

            // Main content area
            if (isExpandedLayout) {
                // Two-pane layout for tablets/landscape
                Row(modifier = Modifier.weight(1f)) {
                    // Note list pane
                    NoteListView(
                        appState = appState,
                        notes = notes,
                        selectedNoteId = selectedNoteId,
                        onNoteSelected = { appState.setSelectedNoteId(it) },
                        showArchive = showArchive,
                        modifier = Modifier
                            .width(360.dp)
                            .fillMaxHeight(),
                    )

                    // Divider
                    HorizontalDivider(
                        modifier = Modifier
                            .fillMaxHeight()
                            .width(1.dp),
                    )

                    // Editor pane
                    if (selectedNote != null) {
                        NoteEditorView(
                            appState = appState,
                            note = selectedNote,
                            modifier = Modifier
                                .weight(1f)
                                .fillMaxHeight(),
                        )
                    } else {
                        // Empty state for editor pane
                        Box(
                            modifier = Modifier
                                .weight(1f)
                                .fillMaxHeight(),
                            contentAlignment = Alignment.Center,
                        ) {
                            Text(
                                text = "Select a note to edit",
                                style = MaterialTheme.typography.bodyLarge,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }
            } else {
                // Single-pane layout for phones
                if (showListOnNarrow) {
                    NoteListView(
                        appState = appState,
                        notes = notes,
                        selectedNoteId = selectedNoteId,
                        onNoteSelected = { appState.setSelectedNoteId(it) },
                        showArchive = showArchive,
                        modifier = Modifier.weight(1f),
                    )
                } else if (selectedNote != null) {
                    NoteEditorView(
                        appState = appState,
                        note = selectedNote,
                        onNavigateBack = { appState.setSelectedNoteId(null) },
                        modifier = Modifier.weight(1f),
                    )
                }
            }

            // Sync status bar at bottom
            SyncStatusBar(
                isSyncing = isSyncing,
                syncStatusMessage = syncStatusMessage,
                syncError = syncError,
            )
        }
    }
}

/**
 * Sync status indicator shown at the bottom of the main screen.
 */
@Composable
private fun SyncStatusBar(
    isSyncing: Boolean,
    syncStatusMessage: String?,
    syncError: String?,
) {
    AnimatedVisibility(
        visible = syncStatusMessage != null || syncError != null,
        enter = slideInVertically(initialOffsetY = { it }),
        exit = slideOutVertically(targetOffsetY = { it }),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (isSyncing) {
                CircularProgressIndicator(
                    modifier = Modifier
                        .size(16.dp)
                        .padding(end = 8.dp),
                    strokeWidth = 2.dp,
                )
            }
            val message = syncError ?: syncStatusMessage
            if (message != null) {
                Text(
                    text = message,
                    style = MaterialTheme.typography.labelSmall,
                    color = if (syncError != null) {
                        MaterialTheme.colorScheme.error
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    },
                )
            }
        }
    }
}
