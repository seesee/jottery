package com.jottery.android.ui.screen

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Sort
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Inbox
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
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.adaptive.currentWindowAdaptiveInfo
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.window.core.layout.WindowHeightSizeClass
import androidx.window.core.layout.WindowWidthSizeClass
import com.jottery.android.R
import com.jottery.android.model.SortOrder
import com.jottery.android.ui.component.NoteEditorView
import com.jottery.android.ui.component.NoteListView
import com.jottery.android.ui.component.SearchBar
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun MainScreen(
    appState: AppState,
    onNavigateToRecycleBin: () -> Unit,
    onNavigateToSettings: () -> Unit,
    onNavigateToConflicts: () -> Unit,
    onNavigateToInbox: () -> Unit,
) {
    val conflicts by appState.pendingConflicts.collectAsState()
    val inboxItems by appState.inboxItems.collectAsState()
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
    val userMessage by appState.userMessage.collectAsState()
    val scope = rememberCoroutineScope()
    val snackbarHostState = remember { SnackbarHostState() }

    LaunchedEffect(userMessage) {
        userMessage?.let {
            snackbarHostState.showSnackbar(it)
            appState.clearMessage()
        }
    }

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

    // A phone in landscape has no height to spare: with the keyboard up, the app
    // bars would leave the editor no room at all.
    val keyboardNeedsTheSpace = WindowInsets.isImeVisible &&
        windowSizeClass.windowHeightSizeClass == WindowHeightSizeClass.COMPACT

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
        snackbarHost = { SnackbarHost(snackbarHostState) },
        topBar = {
            if (showListOnNarrow && !(keyboardNeedsTheSpace && selectedNote != null)) {
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
                                if (syncEnabled) {
                                    DropdownMenuItem(
                                        text = { Text(stringResource(R.string.inbox_menu, inboxItems.size)) },
                                        onClick = {
                                            overflowMenuExpanded = false
                                            onNavigateToInbox()
                                        },
                                        leadingIcon = {
                                            Icon(Icons.Default.Inbox, contentDescription = null)
                                        },
                                    )
                                }
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
                .padding(paddingValues)
                // Edge-to-edge windows don't resize for the keyboard
                .consumeWindowInsets(paddingValues)
                .imePadding(),
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
                        // A new composition per note: no undo stack, WebView
                        // buffer or pending save may carry over to another note.
                        key(selectedNote.id) {
                            NoteEditorView(
                                appState = appState,
                                note = selectedNote,
                                showAppBar = !keyboardNeedsTheSpace,
                                modifier = Modifier
                                    .weight(1f)
                                    .fillMaxHeight(),
                            )
                        }
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
                    key(selectedNote.id) {
                        NoteEditorView(
                            appState = appState,
                            note = selectedNote,
                            onNavigateBack = { appState.setSelectedNoteId(null) },
                            showAppBar = !keyboardNeedsTheSpace,
                            modifier = Modifier.weight(1f),
                        )
                    }
                }
            }

            // A conflict is held until the user decides, so it must stay visible
            if (conflicts.isNotEmpty()) {
                Surface(
                    onClick = onNavigateToConflicts,
                    color = MaterialTheme.colorScheme.errorContainer,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(
                        text = pluralStringResource(R.plurals.conflict_banner, conflicts.size, conflicts.size),
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.onErrorContainer,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
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
