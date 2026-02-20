package com.jottery.android.ui.component

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.ColorLens
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.LockOpen
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Preview
import androidx.compose.material.icons.filled.PushPin
import androidx.compose.material.icons.filled.WrapText
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.jottery.android.model.DecryptedNote
import com.jottery.android.model.NoteVersionRecord
import com.jottery.android.ui.theme.NoteColors
import com.jottery.android.ui.webview.MarkdownPreviewView
import com.jottery.android.ui.webview.WebCalcEditorView
import com.jottery.android.ui.webview.WebOutlinerEditorView
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Note editor composable with auto-save, context menu, tag input area, and
 * pinch-to-zoom font size.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NoteEditorView(
    appState: AppState,
    note: DecryptedNote?,
    onNavigateBack: (() -> Unit)? = null,
    modifier: Modifier = Modifier,
) {
    if (note == null) return

    val scope = rememberCoroutineScope()
    val currentNotes by appState.notes.collectAsState()
    val archivedNotes by appState.archivedNotes.collectAsState()
    val editorFontScale by appState.editorFontScale.collectAsState()

    // Derive all tags from note collections
    val allTags = remember(currentNotes, archivedNotes) {
        (currentNotes.flatMap { it.tags } + archivedNotes.flatMap { it.tags })
            .toSet()
            .sorted()
    }

    // Local editor state, kept in sync with the note
    var textFieldValue by remember(note.id) {
        mutableStateOf(TextFieldValue(note.content))
    }
    var tags by remember(note.id) { mutableStateOf(note.tags) }
    var syntaxLanguage by remember(note.id) { mutableStateOf(note.syntaxLanguage) }
    var wordWrap by remember(note.id) { mutableStateOf(note.wordWrap) }
    var color by remember(note.id) { mutableStateOf(note.color) }
    var showPreview by remember(note.id) { mutableStateOf(note.showPreview) }
    var didSaveDuringSession by remember(note.id) { mutableStateOf(false) }

    var menuExpanded by remember { mutableStateOf(false) }
    var colorMenuExpanded by remember { mutableStateOf(false) }
    var languageMenuExpanded by remember { mutableStateOf(false) }
    var showDeleteConfirm by remember { mutableStateOf(false) }
    var showVersionHistory by remember { mutableStateOf(false) }
    var showNoteInfo by remember { mutableStateOf(false) }
    var versions by remember(note.id) { mutableStateOf<List<NoteVersionRecord>>(emptyList()) }

    // Pinch-to-zoom state
    var pinchBaseScale by remember { mutableFloatStateOf(editorFontScale) }

    // Auto-save debounce
    var saveJob by remember { mutableStateOf<Job?>(null) }

    val isReadOnly = note.locked || note.archived

    // Effective font size (base 14sp * scale)
    val effectiveFontSize = (14f * editorFontScale).coerceIn(8f, 42f).sp

    fun hasChanges(): Boolean {
        return textFieldValue.text != note.content ||
                tags != note.tags ||
                syntaxLanguage != note.syntaxLanguage ||
                wordWrap != note.wordWrap ||
                color != note.color ||
                showPreview != note.showPreview
    }

    fun saveImmediately() {
        saveJob?.cancel()
        if (!hasChanges()) return
        val updated = note.copy(
            content = textFieldValue.text,
            tags = tags,
            syntaxLanguage = syntaxLanguage,
            wordWrap = wordWrap,
            color = color,
            showPreview = showPreview,
        )
        scope.launch {
            appState.saveNote(updated)
            didSaveDuringSession = true
        }
    }

    fun scheduleSave() {
        appState.recordActivity()
        saveJob?.cancel()
        saveJob = scope.launch {
            delay(1000L) // 1-second debounce
            saveImmediately()
        }
    }

    // Save on dispose (navigating away).
    // Uses viewModelScope via appState methods because the composable scope
    // (rememberCoroutineScope) is cancelled when leaving the composition,
    // which would silently drop the save and sync coroutines.
    DisposableEffect(note.id) {
        onDispose {
            saveJob?.cancel()
            if (hasChanges()) {
                val updated = note.copy(
                    content = textFieldValue.text,
                    tags = tags,
                    syntaxLanguage = syntaxLanguage,
                    wordWrap = wordWrap,
                    color = color,
                    showPreview = showPreview,
                )
                appState.saveNoteAndSync(updated)
            } else if (didSaveDuringSession && appState.syncEnabledValue) {
                appState.triggerSyncBackground()
            }
        }
    }

    // Sync text state when note changes externally (e.g. sync)
    LaunchedEffect(note.content, note.tags, note.syntaxLanguage, note.wordWrap, note.color, note.showPreview) {
        if (textFieldValue.text != note.content) {
            textFieldValue = TextFieldValue(note.content)
        }
        tags = note.tags
        syntaxLanguage = note.syntaxLanguage
        wordWrap = note.wordWrap
        color = note.color
        showPreview = note.showPreview
    }

    Column(modifier = modifier) {
        // Top bar with back button (phone) and overflow menu
        TopAppBar(
            title = {
                Text(
                    text = note.title,
                    maxLines = 1,
                    style = MaterialTheme.typography.titleMedium,
                )
            },
            navigationIcon = {
                if (onNavigateBack != null) {
                    IconButton(onClick = {
                        saveImmediately()
                        onNavigateBack()
                    }) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back",
                        )
                    }
                }
            },
            actions = {
                // Overflow context menu
                Box {
                    IconButton(onClick = { menuExpanded = true }) {
                        Icon(Icons.Default.MoreVert, contentDescription = "More options")
                    }
                    EditorContextMenu(
                        expanded = menuExpanded,
                        onDismiss = { menuExpanded = false },
                        note = note,
                        wordWrap = wordWrap,
                        showPreview = showPreview,
                        syntaxLanguage = syntaxLanguage,
                        color = color,
                        onTogglePin = {
                            scope.launch { appState.togglePin(note.id) }
                            menuExpanded = false
                        },
                        onToggleLock = {
                            scope.launch { appState.toggleLock(note.id) }
                            menuExpanded = false
                        },
                        onToggleArchive = {
                            scope.launch {
                                if (note.archived) {
                                    appState.unarchiveNote(note.id)
                                } else {
                                    appState.archiveNote(note.id)
                                }
                            }
                            menuExpanded = false
                        },
                        onToggleWordWrap = {
                            wordWrap = !wordWrap
                            scheduleSave()
                            menuExpanded = false
                        },
                        onTogglePreview = {
                            showPreview = !showPreview
                            scheduleSave()
                            menuExpanded = false
                        },
                        onDuplicate = {
                            scope.launch { appState.duplicateNote(note.id) }
                            menuExpanded = false
                        },
                        onDelete = {
                            menuExpanded = false
                            showDeleteConfirm = true
                        },
                        onShowColourMenu = {
                            menuExpanded = false
                            colorMenuExpanded = true
                        },
                        onShowLanguageMenu = {
                            menuExpanded = false
                            languageMenuExpanded = true
                        },
                        onShowVersionHistory = {
                            scope.launch {
                                versions = appState.getVersionsForNote(note.id)
                            }
                            showVersionHistory = true
                            menuExpanded = false
                        },
                        onShowNoteInfo = {
                            showNoteInfo = true
                            menuExpanded = false
                        },
                        onResetTextSize = {
                            appState.setEditorFontScale(1.0f)
                            pinchBaseScale = 1.0f
                            menuExpanded = false
                        },
                        showResetTextSize = editorFontScale != 1.0f,
                    )

                    // Colour picker sub-menu
                    DropdownMenu(
                        expanded = colorMenuExpanded,
                        onDismissRequest = { colorMenuExpanded = false },
                    ) {
                        NoteColors.allColors.forEach { (name, noteColor) ->
                            DropdownMenuItem(
                                text = { Text(name.replaceFirstChar { it.uppercase() }) },
                                onClick = {
                                    color = name
                                    scheduleSave()
                                    colorMenuExpanded = false
                                },
                                leadingIcon = {
                                    Box(
                                        modifier = Modifier
                                            .padding(4.dp)
                                            .then(
                                                Modifier
                                                    .fillMaxSize()
                                            ),
                                    ) {
                                        // Simple colour indicator would go here
                                    }
                                },
                            )
                        }
                        HorizontalDivider()
                        DropdownMenuItem(
                            text = { Text("None") },
                            onClick = {
                                color = null
                                scheduleSave()
                                colorMenuExpanded = false
                            },
                        )
                    }

                    // Language picker sub-menu
                    DropdownMenu(
                        expanded = languageMenuExpanded,
                        onDismissRequest = { languageMenuExpanded = false },
                    ) {
                        syntaxLanguages.forEach { lang ->
                            DropdownMenuItem(
                                text = {
                                    Text(
                                        text = lang.replaceFirstChar { it.uppercase() },
                                        color = if (lang == syntaxLanguage) {
                                            MaterialTheme.colorScheme.primary
                                        } else {
                                            MaterialTheme.colorScheme.onSurface
                                        },
                                    )
                                },
                                onClick = {
                                    syntaxLanguage = lang
                                    scheduleSave()
                                    languageMenuExpanded = false
                                },
                            )
                        }
                    }
                }
            },
            colors = TopAppBarDefaults.topAppBarColors(
                containerColor = MaterialTheme.colorScheme.surface,
            ),
        )

        // Tag input area
        if (tags.isNotEmpty() || !isReadOnly) {
            TagInputView(
                tags = tags,
                allTags = allTags,
                onTagsChanged = { newTags ->
                    if (!isReadOnly) {
                        tags = newTags
                        scheduleSave()
                    }
                },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 4.dp),
            )
            HorizontalDivider()
        }

        // Editor area with pinch-to-zoom (only intercepts two-finger gestures)
        Box(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth()
                .pointerInput(Unit) {
                    awaitEachGesture {
                        awaitFirstDown(requireUnconsumed = false)
                        var prevSpan = 0f
                        do {
                            val event = awaitPointerEvent()
                            val pointers = event.changes.filter { it.pressed }
                            if (pointers.size >= 2) {
                                val p0 = pointers[0].position
                                val p1 = pointers[1].position
                                val span = (p0 - p1).getDistance()
                                if (prevSpan > 0f && span > 0f) {
                                    val zoom = span / prevSpan
                                    if (zoom != 1f) {
                                        val newScale = (pinchBaseScale * zoom).coerceIn(0.5f, 3.0f)
                                        appState.setEditorFontScale(newScale)
                                        pinchBaseScale = newScale
                                        event.changes.forEach { it.consume() }
                                    }
                                }
                                prevSpan = span
                            } else {
                                prevSpan = 0f
                            }
                        } while (event.changes.any { it.pressed })
                    }
                },
        ) {
            when {
                showPreview -> {
                    MarkdownPreviewView(
                        content = textFieldValue.text,
                        fontSize = 14f * editorFontScale,
                        modifier = Modifier.fillMaxSize(),
                    )
                }
                syntaxLanguage == "calc" -> {
                    WebCalcEditorView(
                        content = textFieldValue.text,
                        fontSize = 14f * editorFontScale,
                        onContentChanged = { newContent ->
                            if (!isReadOnly) {
                                textFieldValue = TextFieldValue(newContent)
                                scheduleSave()
                            }
                        },
                        modifier = Modifier.fillMaxSize(),
                    )
                }
                syntaxLanguage == "outliner" -> {
                    WebOutlinerEditorView(
                        content = textFieldValue.text,
                        fontSize = 14f * editorFontScale,
                        onContentChanged = { newContent ->
                            if (!isReadOnly) {
                                textFieldValue = TextFieldValue(newContent)
                                scheduleSave()
                            }
                        },
                        modifier = Modifier.fillMaxSize(),
                    )
                }
                else -> {
                    BasicTextField(
                        value = textFieldValue,
                        onValueChange = { newValue ->
                            if (!isReadOnly) {
                                textFieldValue = newValue
                                scheduleSave()
                            }
                        },
                        readOnly = isReadOnly,
                        textStyle = TextStyle(
                            fontFamily = FontFamily.Monospace,
                            fontSize = effectiveFontSize,
                            color = MaterialTheme.colorScheme.onSurface,
                            lineHeight = effectiveFontSize * 1.5f,
                        ),
                        cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(horizontal = 16.dp, vertical = 12.dp),
                        decorationBox = { innerTextField ->
                            Box {
                                if (textFieldValue.text.isEmpty() && !isReadOnly) {
                                    Text(
                                        text = "Start typing\u2026",
                                        style = TextStyle(
                                            fontFamily = FontFamily.Monospace,
                                            fontSize = effectiveFontSize,
                                            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.4f),
                                        ),
                                    )
                                }
                                innerTextField()
                            }
                        },
                    )
                }
            }
        }
    }

    // Delete confirmation dialog
    if (showDeleteConfirm) {
        ConfirmDialog(
            title = "Delete Note",
            message = "Are you sure you want to delete this note? It will be moved to the recycle bin.",
            confirmText = "Delete",
            onConfirm = {
                scope.launch { appState.deleteNote(note.id) }
                showDeleteConfirm = false
                onNavigateBack?.invoke()
            },
            onDismiss = { showDeleteConfirm = false },
        )
    }

    // Version history bottom sheet
    if (showVersionHistory) {
        VersionHistoryView(
            versions = versions,
            onRestore = { version ->
                scope.launch {
                    appState.restoreVersion(note.id, version)
                }
                showVersionHistory = false
            },
            onDismiss = { showVersionHistory = false },
        )
    }

    // Note info bottom sheet
    if (showNoteInfo) {
        NoteInfoView(
            note = note.copy(
                content = textFieldValue.text,
                tags = tags,
            ),
            onDismiss = { showNoteInfo = false },
        )
    }
}

/**
 * Context menu (overflow menu) for the note editor.
 */
@Composable
private fun EditorContextMenu(
    expanded: Boolean,
    onDismiss: () -> Unit,
    note: DecryptedNote,
    wordWrap: Boolean,
    showPreview: Boolean,
    syntaxLanguage: String,
    color: String?,
    onTogglePin: () -> Unit,
    onToggleLock: () -> Unit,
    onToggleArchive: () -> Unit,
    onToggleWordWrap: () -> Unit,
    onTogglePreview: () -> Unit,
    onDuplicate: () -> Unit,
    onDelete: () -> Unit,
    onShowColourMenu: () -> Unit,
    onShowLanguageMenu: () -> Unit,
    onShowVersionHistory: () -> Unit,
    onShowNoteInfo: () -> Unit,
    onResetTextSize: () -> Unit,
    showResetTextSize: Boolean,
) {
    DropdownMenu(
        expanded = expanded,
        onDismissRequest = onDismiss,
    ) {
        // Pin toggle
        DropdownMenuItem(
            text = { Text(if (note.pinned) "Unpin" else "Pin") },
            onClick = onTogglePin,
            leadingIcon = {
                Icon(Icons.Default.PushPin, contentDescription = null)
            },
        )

        // Word wrap toggle
        DropdownMenuItem(
            text = { Text(if (wordWrap) "Disable Word Wrap" else "Enable Word Wrap") },
            onClick = onToggleWordWrap,
            leadingIcon = {
                Icon(Icons.Default.WrapText, contentDescription = null)
            },
        )

        // Reset text size
        if (showResetTextSize) {
            DropdownMenuItem(
                text = { Text("Reset Text Size") },
                onClick = onResetTextSize,
            )
        }

        // Preview toggle
        DropdownMenuItem(
            text = { Text(if (showPreview) "Hide Preview" else "Show Preview") },
            onClick = onTogglePreview,
            leadingIcon = {
                Icon(Icons.Default.Preview, contentDescription = null)
            },
        )

        // Syntax language
        DropdownMenuItem(
            text = { Text("Language: ${syntaxLanguage.replaceFirstChar { it.uppercase() }}") },
            onClick = onShowLanguageMenu,
        )

        // Colour
        DropdownMenuItem(
            text = { Text("Colour: ${color?.replaceFirstChar { it.uppercase() } ?: "None"}") },
            onClick = onShowColourMenu,
            leadingIcon = {
                Icon(Icons.Default.ColorLens, contentDescription = null)
            },
        )

        HorizontalDivider()

        // Lock toggle
        DropdownMenuItem(
            text = { Text(if (note.locked) "Unlock" else "Lock") },
            onClick = onToggleLock,
            leadingIcon = {
                Icon(
                    if (note.locked) Icons.Default.LockOpen else Icons.Default.Lock,
                    contentDescription = null,
                )
            },
        )

        // Archive toggle
        DropdownMenuItem(
            text = { Text(if (note.archived) "Unarchive" else "Archive") },
            onClick = onToggleArchive,
            leadingIcon = {
                Icon(Icons.Default.Archive, contentDescription = null)
            },
        )

        // Version history
        if (note.version > 0) {
            DropdownMenuItem(
                text = { Text("Version History") },
                onClick = onShowVersionHistory,
                leadingIcon = {
                    Icon(Icons.Default.History, contentDescription = null)
                },
            )
        }

        // Note info
        DropdownMenuItem(
            text = { Text("Note Info") },
            onClick = onShowNoteInfo,
            leadingIcon = {
                Icon(Icons.Default.Info, contentDescription = null)
            },
        )

        // Duplicate
        DropdownMenuItem(
            text = { Text("Duplicate") },
            onClick = onDuplicate,
            leadingIcon = {
                Icon(Icons.Default.ContentCopy, contentDescription = null)
            },
        )

        HorizontalDivider()

        // Delete
        DropdownMenuItem(
            text = {
                Text(
                    "Delete",
                    color = MaterialTheme.colorScheme.error,
                )
            },
            onClick = onDelete,
            leadingIcon = {
                Icon(
                    Icons.Default.Delete,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.error,
                )
            },
        )
    }
}

/** Supported syntax languages matching the iOS editor. */
private val syntaxLanguages = listOf(
    "markdown",
    "calc",
    "outliner",
    "javascript",
    "typescript",
    "python",
    "perl",
    "json",
    "xml",
    "css",
    "bash",
    "sql",
    "plain",
)
