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
import androidx.compose.material.icons.automirrored.filled.Label
import androidx.compose.material.icons.automirrored.filled.Sort
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.Bookmarks
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarResult
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.window.core.layout.WindowHeightSizeClass
import androidx.window.core.layout.WindowWidthSizeClass
import com.jottery.android.R
import com.jottery.android.model.SortOrder
import com.jottery.android.ui.component.NoteEditorView
import com.jottery.android.ui.component.NoteListView
import com.jottery.android.model.tagCounts
import com.jottery.android.ui.component.SearchBar
import com.jottery.android.ui.component.SearchHelpSheet
import com.jottery.android.ui.component.SearchSuggestionChips
import com.jottery.android.ui.component.enableBiometricUnlock
import com.jottery.android.ui.component.sortOrderLabel
import com.jottery.android.viewmodel.AppState
import com.jottery.android.viewmodel.resolveSelectedNote
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun MainScreen(
    appState: AppState,
    onNavigateToRecycleBin: () -> Unit,
    onNavigateToSettings: () -> Unit,
    onNavigateToConflicts: () -> Unit,
    onNavigateToInbox: () -> Unit,
    onNavigateToSavedSearches: () -> Unit,
    onNavigateToTags: () -> Unit,
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
    val undoableDelete by appState.undoableDelete.collectAsState()
    val scope = rememberCoroutineScope()
    val snackbarHostState = remember { SnackbarHostState() }

    LaunchedEffect(userMessage) {
        userMessage?.let {
            snackbarHostState.showSnackbar(it)
            appState.clearMessage()
        }
    }

    val deletedMessage = stringResource(R.string.note_deleted)
    val undoLabel = stringResource(R.string.action_undo)
    LaunchedEffect(undoableDelete) {
        val id = undoableDelete ?: return@LaunchedEffect
        val result = snackbarHostState.showSnackbar(
            message = deletedMessage,
            actionLabel = undoLabel,
            duration = SnackbarDuration.Short,
        )
        if (result == SnackbarResult.ActionPerformed) appState.restoreNote(id)
        appState.clearUndoableDelete()
    }

    // Derive counts from collected state
    val noteCount by remember(allNotes, archivedNotes, showArchive) {
        derivedStateOf { if (showArchive) archivedNotes.size else allNotes.size }
    }
    val filteredCount by remember(notes) {
        derivedStateOf { notes.size }
    }

    var searchVisible by remember { mutableStateOf(false) }
    var showSearchHelp by remember { mutableStateOf(false) }
    var searchFocused by remember { mutableStateOf(false) }
    val recentSearches by appState.recentSearches.collectAsState()
    val searchRequested by appState.searchRequested.collectAsState()
    LaunchedEffect(searchRequested) {
        if (searchRequested) searchVisible = true
    }
    if (showSearchHelp) {
        SearchHelpSheet(
            onChoose = {
                showSearchHelp = false
                appState.setSearchQuery(it)
            },
            onDismiss = { showSearchHelp = false },
        )
    }
    var showSaveSearch by remember { mutableStateOf(false) }

    // A saved search applied from its own screen has to be visible, and clearable
    LaunchedEffect(searchQuery) {
        if (searchQuery.isNotEmpty()) searchVisible = true
    }
    var sortMenuExpanded by remember { mutableStateOf(false) }
    var overflowMenuExpanded by remember { mutableStateOf(false) }

    // Determine layout based on window width
    val windowSizeClass = currentWindowAdaptiveInfo().windowSizeClass
    val isExpandedLayout = windowSizeClass.windowWidthSizeClass != WindowWidthSizeClass.COMPACT

    // A phone in landscape has no height to spare: with the keyboard up, the app
    // bars would leave the editor no room at all.
    val keyboardNeedsTheSpace = WindowInsets.isImeVisible &&
        windowSizeClass.windowHeightSizeClass == WindowHeightSizeClass.COMPACT

    // Selected note for editor. Not from the filtered list: a note must stay open
    // when an edit stops it matching the search, and a new one matches nothing yet.
    val selectedNote = remember(selectedNoteId, allNotes, archivedNotes) {
        resolveSelectedNote(selectedNoteId, allNotes, archivedNotes)
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
                        Text(if (showArchive) stringResource(R.string.editor_archive) else "Jottery")
                    },
                    colors = TopAppBarDefaults.topAppBarColors(
                        containerColor = MaterialTheme.colorScheme.surface,
                    ),
                    actions = {
                        // Search toggle
                        IconButton(onClick = { searchVisible = !searchVisible }) {
                            Icon(Icons.Default.Search, contentDescription = stringResource(R.string.search_bar_search))
                        }

                        IconButton(onClick = onNavigateToSavedSearches) {
                            Icon(
                                Icons.Default.Bookmarks,
                                contentDescription = stringResource(R.string.saved_searches_title),
                            )
                        }

                        // Sort menu
                        Box {
                            IconButton(onClick = { sortMenuExpanded = true }) {
                                Icon(Icons.AutoMirrored.Filled.Sort, contentDescription = stringResource(R.string.main_sort))
                            }
                            DropdownMenu(
                                expanded = sortMenuExpanded,
                                onDismissRequest = { sortMenuExpanded = false },
                            ) {
                                SortOrder.entries.forEach { order ->
                                    DropdownMenuItem(
                                        text = { Text(sortOrderLabel(order)) },
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
                                Icon(Icons.Default.MoreVert, contentDescription = stringResource(R.string.action_more))
                            }
                            DropdownMenu(
                                expanded = overflowMenuExpanded,
                                onDismissRequest = { overflowMenuExpanded = false },
                            ) {
                                DropdownMenuItem(
                                    text = {
                                        Text(if (showArchive) stringResource(R.string.main_show_notes) else stringResource(R.string.main_show_archive))
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
                                    text = { Text(stringResource(R.string.note_info_tags)) },
                                    onClick = {
                                        overflowMenuExpanded = false
                                        onNavigateToTags()
                                    },
                                    leadingIcon = {
                                        Icon(Icons.AutoMirrored.Filled.Label, contentDescription = null)
                                    },
                                )
                                DropdownMenuItem(
                                    text = { Text(stringResource(R.string.recycle_bin_title)) },
                                    onClick = {
                                        overflowMenuExpanded = false
                                        onNavigateToRecycleBin()
                                    },
                                    leadingIcon = {
                                        Icon(Icons.Default.Delete, contentDescription = null)
                                    },
                                )
                                DropdownMenuItem(
                                    text = { Text(stringResource(R.string.settings_title)) },
                                    onClick = {
                                        overflowMenuExpanded = false
                                        onNavigateToSettings()
                                    },
                                    leadingIcon = {
                                        Icon(Icons.Default.Settings, contentDescription = null)
                                    },
                                )
                                DropdownMenuItem(
                                    text = { Text(stringResource(R.string.editor_lock)) },
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
                        contentDescription = stringResource(R.string.main_new_note),
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
                    onSave = { showSaveSearch = true },
                    matchCount = filteredCount,
                    totalCount = noteCount,
                    showCounts = searchQuery.isNotEmpty(),
                    focusRequested = searchRequested,
                    onFocusHandled = appState::searchRequestHandled,
                    onSubmit = { appState.rememberSearch(searchQuery) },
                    onHelp = { showSearchHelp = true },
                    onFocusChanged = { searchFocused = it },
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }

            if (searchVisible && showListOnNarrow && searchFocused) {
                SearchSuggestionChips(
                    query = searchQuery,
                    tags = remember(allNotes, archivedNotes) {
                        (allNotes + archivedNotes).tagCounts().map { it.first }
                    },
                    recent = recentSearches,
                    onChoose = { appState.setSearchQuery(it) },
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
                                text = stringResource(R.string.main_select_a_note_to_edit),
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

    val offerBiometric by appState.offerBiometric.collectAsState()
    if (offerBiometric) {
        val context = LocalContext.current
        AlertDialog(
            onDismissRequest = { appState.dismissBiometricOffer() },
            title = { Text(stringResource(R.string.biometric_offer_title)) },
            text = { Text(stringResource(R.string.biometric_offer_message)) },
            confirmButton = {
                TextButton(onClick = {
                    appState.dismissBiometricOffer()
                    enableBiometricUnlock(context, appState) { appState.showMessage(it) }
                }) { Text(stringResource(R.string.biometric_offer_accept)) }
            },
            dismissButton = {
                TextButton(onClick = { appState.dismissBiometricOffer() }) {
                    Text(stringResource(R.string.biometric_offer_decline))
                }
            },
        )
    }

    if (showSaveSearch) {
        SaveSearchDialog(
            query = searchQuery,
            onSave = { name ->
                showSaveSearch = false
                scope.launch { appState.saveSearch(name, searchQuery) }
            },
            onDismiss = { showSaveSearch = false },
        )
    }
}

/**
 * Asks what to call a search before saving it.
 */
@Composable
private fun SaveSearchDialog(
    query: String,
    onSave: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var name by remember { mutableStateOf(query) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.saved_search_save_title)) },
        text = {
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                label = { Text(stringResource(R.string.saved_search_name)) },
                singleLine = true,
            )
        },
        confirmButton = {
            TextButton(onClick = { onSave(name.trim()) }, enabled = name.isNotBlank()) {
                Text(stringResource(R.string.action_save))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.action_cancel)) }
        },
    )
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
