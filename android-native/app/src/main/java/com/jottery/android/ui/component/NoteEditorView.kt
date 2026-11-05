package com.jottery.android.ui.component

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Intent
import android.net.Uri
import android.os.PersistableBundle
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.draganddrop.dragAndDropTarget
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.isImeVisible
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Redo
import androidx.compose.material.icons.automirrored.filled.Undo
import androidx.compose.material.icons.filled.Archive
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.ColorLens
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Image
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Link
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.LockOpen
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Preview
import androidx.compose.material.icons.filled.Print
import androidx.compose.material.icons.filled.PushPin
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Share
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
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.draganddrop.DragAndDropEvent
import androidx.compose.ui.draganddrop.DragAndDropTarget
import androidx.compose.ui.draganddrop.mimeTypes
import androidx.compose.ui.draganddrop.toAndroidDragEvent
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.jottery.android.R
import com.jottery.android.model.AttachmentRef
import com.jottery.android.model.DecryptedNote
import com.jottery.android.model.NoteVersionRecord
import com.jottery.android.ui.editor.QuickCommand
import com.jottery.android.ui.editor.EditHistory
import com.jottery.android.ui.editor.EditorSearch
import com.jottery.android.ui.editor.EditorTextArea
import com.jottery.android.ui.editor.FindBar
import com.jottery.android.ui.theme.NoteColors
import com.jottery.android.ui.theme.noteColourLabel
import com.jottery.android.ui.webview.MarkdownPreviewView
import com.jottery.android.ui.webview.WebCalcEditorView
import com.jottery.android.ui.webview.WebOutlinerEditorView
import com.jottery.android.ui.webview.isOpenableLink
import com.jottery.android.util.AttachmentFiles
import com.jottery.android.util.NotePrinter
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Note editor composable with auto-save, context menu, tag input area, and
 * pinch-to-zoom font size.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class, ExperimentalLayoutApi::class)
@Composable
fun NoteEditorView(
    appState: AppState,
    note: DecryptedNote?,
    onNavigateBack: (() -> Unit)? = null,
    showAppBar: Boolean = true,
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

    // The last text this editor wrote. Its own save coming back through the note
    // must not replace what the user has typed since.
    var lastSavedContent by remember(note.id) { mutableStateOf(note.content) }

    // onDispose outlives every recomposition, so it must read the current note
    val latestNote by rememberUpdatedState(note)

    val history = remember(note.id) { EditHistory(TextFieldValue(note.content)) }
    var historyVersion by remember(note.id) { mutableIntStateOf(0) }
    val editorFocus = remember { FocusRequester() }
    var showFind by remember(note.id) { mutableStateOf(false) }
    val findRequested by appState.findRequested.collectAsState()
    LaunchedEffect(findRequested) {
        if (findRequested) {
            showFind = true
            appState.findRequestHandled()
        }
    }
    var findQuery by remember(note.id) { mutableStateOf("") }
    var replacement by remember(note.id) { mutableStateOf("") }

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

    // Web offers preview for these and no others. Calc and outliner have their own view.
    val canPreview = syntaxLanguage in PREVIEWABLE_LANGUAGES
    val isPreviewing = showPreview && canPreview
    val isPlainEditor = !isPreviewing && syntaxLanguage != "calc" && syntaxLanguage != "outliner"
    val restoreFailedMessage = stringResource(R.string.version_restore_failed)

    // Effective font size (base 14sp * scale)
    val effectiveFontSize = (14f * editorFontScale).coerceIn(8f, 42f).sp

    fun hasChanges(): Boolean {
        val current = latestNote
        return textFieldValue.text != current.content ||
                tags != current.tags ||
                syntaxLanguage != current.syntaxLanguage ||
                wordWrap != current.wordWrap ||
                color != current.color ||
                showPreview != current.showPreview
    }

    fun editedNote() = latestNote.copy(
        content = textFieldValue.text,
        tags = tags,
        syntaxLanguage = syntaxLanguage,
        wordWrap = wordWrap,
        color = color,
        showPreview = showPreview,
    )

    fun saveImmediately() {
        saveJob?.cancel()
        if (!hasChanges()) return
        val updated = editedNote()
        lastSavedContent = updated.content
        appState.setPendingEditorNote(updated)
        scope.launch {
            appState.saveNote(updated)
            appState.clearPendingEditorNote(updated)
            didSaveDuringSession = true
        }
    }

    val findMatches = remember(textFieldValue.text, findQuery, showFind) {
        if (showFind) EditorSearch.matches(textFieldValue.text, findQuery) else emptyList()
    }
    val currentMatch = findMatches.indexOfFirst { it.first == textFieldValue.selection.min }.coerceAtLeast(0)

    fun scheduleSave() {
        appState.recordActivity()
        // Kept current, so a lock or a sync in the next second can't lose this
        appState.setPendingEditorNote(editedNote())
        saveJob?.cancel()
        saveJob = scope.launch {
            delay(1000L) // 1-second debounce
            saveImmediately()
        }
    }

    /** A change of text from the user, which undo can take back. */
    fun applyEdit(value: TextFieldValue) {
        textFieldValue = value
        history.record(value)
        historyVersion++
        scheduleSave()
    }

    fun restoreFromHistory(value: TextFieldValue?) {
        if (value == null) return
        textFieldValue = value
        historyVersion++
        scheduleSave()
    }

    fun selectMatch(index: Int) {
        val match = findMatches.getOrNull(index) ?: return
        textFieldValue = textFieldValue.copy(selection = TextRange(match.first, match.last + 1))
        history.record(textFieldValue)
        // The field only scrolls to the selection while it has focus
        editorFocus.requestFocus()
    }

    val context = LocalContext.current
    val copiedMessage = stringResource(R.string.note_copied)
    val noteLinkMissingMessage = stringResource(R.string.note_link_missing)
    val linkCopiedMessage = stringResource(R.string.note_link_copied)

    fun copyToClipboard(label: String, text: String, sensitive: Boolean, message: String) {
        val clip = ClipData.newPlainText(label, text)
        if (sensitive) {
            // Keeps the text out of the system's clipboard preview
            clip.description.extras = PersistableBundle().apply {
                putBoolean(ClipDescription.EXTRA_IS_SENSITIVE, true)
            }
        }
        context.getSystemService(ClipboardManager::class.java)?.setPrimaryClip(clip)
        appState.showMessage(message)
    }

    fun shareNote() {
        val text = textFieldValue.text
        if (text.length <= MAX_SHARE_CHARS) {
            val send = Intent(Intent.ACTION_SEND)
                .setType("text/plain")
                .putExtra(Intent.EXTRA_TEXT, text)
                .putExtra(Intent.EXTRA_SUBJECT, note.title)
            context.startActivity(Intent.createChooser(send, null))
        } else {
            // Too much for an intent, which fails outright past about a megabyte
            scope.launch {
                val name = AttachmentFiles.safeName(note.title) + "." + fileExtension(syntaxLanguage)
                context.startActivity(
                    AttachmentFiles.shareIntent(context, name, "text/plain", text.toByteArray(Charsets.UTF_8))
                )
            }
        }
    }

    var attachmentToRemove by remember { mutableStateOf<AttachmentRef?>(null) }
    val unreadableMessage = stringResource(R.string.attachment_unreadable)
    val noAppMessage = stringResource(R.string.attachment_no_app)
    val tooLargeMessage = stringResource(R.string.attachment_too_large, AttachmentFiles.MAX_ATTACHMENT_BYTES / (1024 * 1024))
    val addFailedMessage = stringResource(R.string.attachment_add_failed)
    val noteTooLargeMessage = stringResource(R.string.note_too_large)
    val nothingToPasteMessage = stringResource(R.string.attachment_nothing_to_paste)

    // A note the server would refuse could never leave this device
    fun acceptsText(text: String): Boolean {
        if (text.length <= MAX_NOTE_CHARS) return true
        appState.showMessage(noteTooLargeMessage)
        return false
    }

    fun attach(uris: List<Uri>) {
        scope.launch {
            for (uri in uris) {
                try {
                    val picked = AttachmentFiles.read(context.contentResolver, uri)
                    val id = appState.addAttachment(note.id, picked.data, picked.name, picked.mimeType)
                    if (id != null && picked.mimeType.startsWith("image/") && syntaxLanguage == "markdown") {
                        val text = textFieldValue.text
                        val caret = textFieldValue.selection.end.coerceIn(0, text.length)
                        val insert = "![](attachment:$id)"
                        applyEdit(
                            TextFieldValue(
                                text.substring(0, caret) + insert + text.substring(caret),
                                TextRange(caret + insert.length),
                            )
                        )
                    }
                } catch (e: CancellationException) {
                    throw e
                } catch (e: AttachmentFiles.TooLarge) {
                    appState.showMessage(tooLargeMessage)
                } catch (e: Exception) {
                    appState.showMessage(addFailedMessage)
                }
            }
        }
    }

    val photoPicker = rememberLauncherForActivityResult(
        ActivityResultContracts.PickMultipleVisualMedia()
    ) { uris -> attach(uris) }
    val filePicker = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenMultipleDocuments()
    ) { uris -> attach(uris) }

    // Files dragged in from another app, on large screens and in split screen
    val dropTarget = remember {
        object : DragAndDropTarget {
            override fun onDrop(event: DragAndDropEvent): Boolean {
                val dragEvent = event.toAndroidDragEvent()
                (context as? Activity)?.requestDragAndDropPermissions(dragEvent)
                val uris = dragEvent.clipData?.urisOrEmpty().orEmpty()
                if (uris.isEmpty()) return false
                attach(uris)
                return true
            }
        }
    }

    fun handOff(attachment: AttachmentRef, buildIntent: suspend (ByteArray) -> Intent) {
        scope.launch {
            val data = appState.attachmentData(attachment)
            if (data == null) {
                appState.showMessage(unreadableMessage)
                return@launch
            }
            try {
                context.startActivity(buildIntent(data))
            } catch (_: ActivityNotFoundException) {
                appState.showMessage(noAppMessage)
            }
        }
    }


    // Save on dispose (navigating away).
    // Uses viewModelScope via appState methods because the composable scope
    // (rememberCoroutineScope) is cancelled when leaving the composition,
    // which would silently drop the save and sync coroutines.
    DisposableEffect(note.id) {
        onDispose {
            saveJob?.cancel()
            appState.setPendingEditorNote(null)
            if (hasChanges()) {
                appState.saveNoteAndSync(editedNote())
            } else if (didSaveDuringSession && appState.syncEnabledValue) {
                appState.triggerSyncBackground()
            }
        }
    }

    // Sync text state when note changes externally (e.g. sync)
    LaunchedEffect(note.content, note.tags, note.syntaxLanguage, note.wordWrap, note.color, note.showPreview) {
        // Our own save coming back: whatever was typed since is newer
        if (note.content == lastSavedContent) return@LaunchedEffect

        // A change from elsewhere (sync, restored version). Unsaved typing wins
        // and goes out in the next save; otherwise show the new text, with the
        // caret kept where it was as far as the new text allows.
        if (textFieldValue.text == lastSavedContent) {
            lastSavedContent = note.content
            val caret = textFieldValue.selection.end.coerceAtMost(note.content.length)
            textFieldValue = TextFieldValue(note.content, TextRange(caret))
            history.reset(textFieldValue)
            historyVersion++
        }
        tags = note.tags
        syntaxLanguage = note.syntaxLanguage
        wordWrap = note.wordWrap
        color = note.color
        showPreview = note.showPreview
    }

    Column(
        modifier = modifier.dragAndDropTarget(
            shouldStartDragAndDrop = { event -> !isReadOnly && event.mimeTypes().isNotEmpty() },
            target = dropTarget,
        ),
    ) {
        // Top bar with back button (phone) and overflow menu
        if (showAppBar) TopAppBar(
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
                            contentDescription = stringResource(R.string.back),
                        )
                    }
                }
            },
            actions = {
                if (isPlainEditor && !isReadOnly) {
                    // Reading the version makes these update as the history changes
                    val canUndo = historyVersion >= 0 && history.canUndo
                    val canRedo = historyVersion >= 0 && history.canRedo
                    IconButton(onClick = { restoreFromHistory(history.undo()) }, enabled = canUndo) {
                        Icon(
                            Icons.AutoMirrored.Filled.Undo,
                            contentDescription = stringResource(R.string.action_undo),
                        )
                    }
                    IconButton(onClick = { restoreFromHistory(history.redo()) }, enabled = canRedo) {
                        Icon(
                            Icons.AutoMirrored.Filled.Redo,
                            contentDescription = stringResource(R.string.editor_redo),
                        )
                    }
                }

                // Overflow context menu
                Box {
                    IconButton(onClick = { menuExpanded = true }) {
                        Icon(Icons.Default.MoreVert, contentDescription = stringResource(R.string.note_editor_more_options))
                    }
                    EditorContextMenu(
                        expanded = menuExpanded,
                        onDismiss = { menuExpanded = false },
                        note = note,
                        wordWrap = wordWrap,
                        showPreview = showPreview,
                        canPreview = canPreview,
                        canFind = isPlainEditor,
                        syntaxLanguage = syntaxLanguage,
                        color = color,
                        onFind = {
                            showFind = true
                            menuExpanded = false
                        },
                        onCopy = {
                            menuExpanded = false
                            copyToClipboard(note.title, textFieldValue.text, sensitive = true, message = copiedMessage)
                        },
                        onCopyLink = {
                            menuExpanded = false
                            copyToClipboard(note.title, "[](link:${note.id})", sensitive = false, message = linkCopiedMessage)
                        },
                        onShare = {
                            menuExpanded = false
                            shareNote()
                        },
                        onPrint = {
                            menuExpanded = false
                            NotePrinter.print(context, note.title, textFieldValue.text)
                        },
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
                        canAddAttachments = !isReadOnly,
                        onAddPhoto = {
                            menuExpanded = false
                            photoPicker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageAndVideo))
                        },
                        onAddFile = {
                            menuExpanded = false
                            filePicker.launch(arrayOf("*/*"))
                        },
                        onPasteAttachment = {
                            menuExpanded = false
                            val clipboard = context.getSystemService(ClipboardManager::class.java)
                            val uris = clipboard?.primaryClip?.urisOrEmpty().orEmpty()
                            if (uris.isEmpty()) appState.showMessage(nothingToPasteMessage) else attach(uris)
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
                                text = { Text(noteColourLabel(name)) },
                                onClick = {
                                    color = name
                                    scheduleSave()
                                    colorMenuExpanded = false
                                },
                                leadingIcon = {
                                    Box(
                                        modifier = Modifier
                                            .size(16.dp)
                                            .clip(CircleShape)
                                            .background(noteColor),
                                    )
                                },
                            )
                        }
                        HorizontalDivider()
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.editor_colour_none)) },
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
            // The enclosing Scaffold has already applied the status bar inset
            windowInsets = WindowInsets(0),
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

        if (showFind && isPlainEditor) {
            FindBar(
                query = findQuery,
                onQueryChange = { findQuery = it },
                replacement = replacement,
                onReplacementChange = { replacement = it },
                matchCount = findMatches.size,
                currentMatch = currentMatch,
                canReplace = !isReadOnly,
                onNext = { selectMatch(EditorSearch.next(findMatches, textFieldValue.selection.max)) },
                onPrevious = { selectMatch(EditorSearch.previous(findMatches, textFieldValue.selection.min)) },
                onReplace = {
                    findMatches.getOrNull(currentMatch)?.let { match ->
                        val text = EditorSearch.replace(textFieldValue.text, match, replacement)
                        if (acceptsText(text)) applyEdit(TextFieldValue(text, TextRange(match.first + replacement.length)))
                    }
                },
                onReplaceAll = {
                    val text = EditorSearch.replaceAll(textFieldValue.text, findQuery, replacement)
                    if (acceptsText(text)) applyEdit(TextFieldValue(text, TextRange(0)))
                },
                onClose = { showFind = false },
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
                isPreviewing -> {
                    MarkdownPreviewView(
                        content = textFieldValue.text,
                        fontSize = 14f * editorFontScale,
                        onRequestAttachment = { id, callback ->
                            scope.launch {
                                val dataUrl = appState.resolveAttachmentDataUrl(
                                    attachmentId = id,
                                    attachments = note.attachments,
                                )
                                if (dataUrl != null) callback(dataUrl)
                            }
                        },
                        onOpenLink = { url ->
                            // A note must not be able to start anything but these
                            if (isOpenableLink(url)) {
                                try {
                                    context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
                                } catch (_: ActivityNotFoundException) {
                                    appState.showMessage(noAppMessage)
                                }
                            }
                        },
                        noteTitles = remember(currentNotes, archivedNotes) {
                            (currentNotes + archivedNotes).associate { it.id to it.title }
                        },
                        onOpenNote = { id ->
                            val target = (currentNotes + archivedNotes).firstOrNull { it.id.equals(id, ignoreCase = true) }
                            if (target != null) {
                                appState.setSelectedNoteId(target.id)
                            } else {
                                appState.showMessage(noteLinkMissingMessage)
                            }
                        },
                        modifier = Modifier.fillMaxSize(),
                    )
                }
                syntaxLanguage == "calc" -> {
                    WebCalcEditorView(
                        content = textFieldValue.text,
                        fontSize = 14f * editorFontScale,
                        readOnly = isReadOnly,
                        onContentChanged = { newContent ->
                            if (!isReadOnly && acceptsText(newContent)) {
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
                        readOnly = isReadOnly,
                        onContentChanged = { newContent ->
                            if (!isReadOnly && acceptsText(newContent)) {
                                textFieldValue = TextFieldValue(newContent)
                                scheduleSave()
                            }
                        },
                        modifier = Modifier.fillMaxSize(),
                    )
                }
                else -> {
                    EditorTextArea(
                        value = textFieldValue,
                        onValueChange = { newValue ->
                            when {
                                newValue.text == textFieldValue.text -> {
                                    // The caret moved. It's not an edit, but a read-only note can still select.
                                    textFieldValue = newValue
                                    history.record(newValue)
                                }
                                !isReadOnly && acceptsText(newValue.text) -> {
                                    applyEdit(QuickCommand.expand(textFieldValue, newValue) ?: newValue)
                                }
                            }
                        },
                        readOnly = isReadOnly,
                        language = syntaxLanguage,
                        wordWrap = wordWrap,
                        fontSize = effectiveFontSize,
                        matches = findMatches,
                        focusRequester = editorFocus,
                        modifier = Modifier.fillMaxSize(),
                    )
                }
            }
        }

        if (isPlainEditor && !isReadOnly && WindowInsets.isImeVisible) {
            EditorFormatBar(
                showFormatting = syntaxLanguage == "markdown",
                onEdit = { change ->
                    val changed = change(textFieldValue)
                    if (acceptsText(changed.text)) applyEdit(changed)
                },
            )
        }

        if (note.attachments.isNotEmpty()) {
            HorizontalDivider()
            AttachmentListView(
                attachments = note.attachments,
                onOpen = { attachment ->
                    handOff(attachment) { data ->
                        AttachmentFiles.viewIntent(context, attachment.filename, attachment.mimeType, data)
                    }
                },
                onShare = { attachment ->
                    handOff(attachment) { data ->
                        AttachmentFiles.shareIntent(context, attachment.filename, attachment.mimeType, data)
                    }
                },
                onRemove = if (isReadOnly) null else { attachment -> attachmentToRemove = attachment },
                loadThumbnail = { attachment ->
                    appState.attachmentData(attachment)?.let { AttachmentFiles.thumbnail(it) }
                },
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }

    attachmentToRemove?.let { attachment ->
        ConfirmDialog(
            title = stringResource(R.string.attachment_remove_title),
            message = stringResource(
                R.string.attachment_remove_message,
                attachment.filename.ifEmpty { stringResource(R.string.attachment_unnamed) },
            ),
            confirmText = stringResource(R.string.attachment_remove_confirm),
            onConfirm = {
                attachmentToRemove = null
                scope.launch { appState.removeAttachment(note.id, attachment.id) }
            },
            onDismiss = { attachmentToRemove = null },
        )
    }

    // Delete confirmation dialog
    if (showDeleteConfirm) {
        ConfirmDialog(
            title = stringResource(R.string.note_editor_delete_note),
            message = stringResource(R.string.note_editor_are_you_sure_you_want),
            confirmText = stringResource(R.string.editor_delete),
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
                // Typing still waiting to save must not land on top of the restore
                saveJob?.cancel()
                scope.launch {
                    if (!appState.restoreVersion(note.id, version)) {
                        appState.showMessage(restoreFailedMessage)
                    }
                }
                showVersionHistory = false
            },
            onPreview = { version ->
                appState.decryptVersionContent(version)
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
    canPreview: Boolean,
    canFind: Boolean,
    syntaxLanguage: String,
    color: String?,
    onFind: () -> Unit,
    onCopy: () -> Unit,
    onCopyLink: () -> Unit,
    onShare: () -> Unit,
    onPrint: () -> Unit,
    onTogglePin: () -> Unit,
    onToggleLock: () -> Unit,
    onToggleArchive: () -> Unit,
    onToggleWordWrap: () -> Unit,
    onTogglePreview: () -> Unit,
    onDuplicate: () -> Unit,
    canAddAttachments: Boolean,
    onAddPhoto: () -> Unit,
    onAddFile: () -> Unit,
    onPasteAttachment: () -> Unit,
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
        if (canFind) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.editor_find_menu)) },
                onClick = onFind,
                leadingIcon = { Icon(Icons.Default.Search, contentDescription = null) },
            )
        }
        DropdownMenuItem(
            text = { Text(stringResource(R.string.note_copy)) },
            onClick = onCopy,
            leadingIcon = { Icon(Icons.Default.ContentCopy, contentDescription = null) },
        )
        DropdownMenuItem(
            text = { Text(stringResource(R.string.note_copy_link)) },
            onClick = onCopyLink,
            leadingIcon = { Icon(Icons.Default.Link, contentDescription = null) },
        )
        DropdownMenuItem(
            text = { Text(stringResource(R.string.note_share)) },
            onClick = onShare,
            leadingIcon = { Icon(Icons.Default.Share, contentDescription = null) },
        )
        DropdownMenuItem(
            text = { Text(stringResource(R.string.note_print)) },
            onClick = onPrint,
            leadingIcon = { Icon(Icons.Default.Print, contentDescription = null) },
        )

        HorizontalDivider()

        // Pin toggle
        DropdownMenuItem(
            text = { Text(if (note.pinned) stringResource(R.string.editor_unpin) else stringResource(R.string.editor_pin)) },
            onClick = onTogglePin,
            leadingIcon = {
                Icon(Icons.Default.PushPin, contentDescription = null)
            },
        )

        // Word wrap toggle
        DropdownMenuItem(
            text = { Text(if (wordWrap) stringResource(R.string.note_editor_disable_word_wrap) else stringResource(R.string.note_editor_enable_word_wrap)) },
            onClick = onToggleWordWrap,
            leadingIcon = {
                Icon(Icons.Default.WrapText, contentDescription = null)
            },
        )

        // Reset text size
        if (showResetTextSize) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.note_editor_reset_text_size)) },
                onClick = onResetTextSize,
            )
        }

        // Preview toggle
        if (canPreview) {
            DropdownMenuItem(
                text = { Text(if (showPreview) stringResource(R.string.note_editor_hide_preview) else stringResource(R.string.note_editor_show_preview)) },
                onClick = onTogglePreview,
                leadingIcon = {
                    Icon(Icons.Default.Preview, contentDescription = null)
                },
            )
        }

        // Syntax language
        DropdownMenuItem(
            text = { Text(stringResource(R.string.editor_language, syntaxLanguage.replaceFirstChar { it.uppercase() })) },
            onClick = onShowLanguageMenu,
        )

        // Colour
        DropdownMenuItem(
            text = {
                Text(
                    stringResource(
                        R.string.editor_colour_value,
                        color?.let { noteColourLabel(it) } ?: stringResource(R.string.editor_colour_none),
                    )
                )
            },
            onClick = onShowColourMenu,
            leadingIcon = {
                Icon(Icons.Default.ColorLens, contentDescription = null)
            },
        )

        HorizontalDivider()

        // Lock toggle
        DropdownMenuItem(
            text = { Text(if (note.locked) stringResource(R.string.unlock_button) else stringResource(R.string.editor_lock)) },
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
            text = { Text(if (note.archived) stringResource(R.string.editor_unarchive) else stringResource(R.string.editor_archive)) },
            onClick = onToggleArchive,
            leadingIcon = {
                Icon(Icons.Default.Archive, contentDescription = null)
            },
        )

        // Version history
        if (note.version > 0) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.editor_versions)) },
                onClick = onShowVersionHistory,
                leadingIcon = {
                    Icon(Icons.Default.History, contentDescription = null)
                },
            )
        }

        // Note info
        DropdownMenuItem(
            text = { Text(stringResource(R.string.editor_info)) },
            onClick = onShowNoteInfo,
            leadingIcon = {
                Icon(Icons.Default.Info, contentDescription = null)
            },
        )

        if (canAddAttachments) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.attachment_add_photo)) },
                onClick = onAddPhoto,
                leadingIcon = { Icon(Icons.Default.Image, contentDescription = null) },
            )
            DropdownMenuItem(
                text = { Text(stringResource(R.string.attachment_add_file)) },
                onClick = onAddFile,
                leadingIcon = { Icon(Icons.Default.AttachFile, contentDescription = null) },
            )
            DropdownMenuItem(
                text = { Text(stringResource(R.string.attachment_paste)) },
                onClick = onPasteAttachment,
                leadingIcon = { Icon(Icons.Default.ContentPaste, contentDescription = null) },
            )
        }

        // Duplicate
        DropdownMenuItem(
            text = { Text(stringResource(R.string.editor_duplicate)) },
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
                    stringResource(R.string.editor_delete),
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

/**
 * Encryption and base64 add a third, and the server refuses content over 10 MB
 * by default, so this is the most that can still sync.
 */
private const val MAX_NOTE_CHARS = 7_000_000

/** Above this a note is shared as a file: an intent can't carry much. */
private const val MAX_SHARE_CHARS = 100_000

private val PREVIEWABLE_LANGUAGES = setOf("markdown", "html", "xml")

/** The extension for a note saved as a file, as web's export names them. */
private fun fileExtension(language: String): String = when (language) {
    "markdown" -> "md"
    "javascript" -> "js"
    "typescript" -> "ts"
    "python" -> "py"
    "perl" -> "pl"
    "json" -> "json"
    "xml" -> "xml"
    "css" -> "css"
    "bash" -> "sh"
    "sql" -> "sql"
    else -> "txt"
}

/** The files a clip or a drop is carrying. */
private fun ClipData.urisOrEmpty(): List<Uri> =
    (0 until itemCount).mapNotNull { getItemAt(it).uri }

/** Supported syntax languages matching the iOS editor. */
internal val syntaxLanguages = listOf(
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
