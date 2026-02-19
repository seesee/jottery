package com.jottery.android.ui.screen

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.DeleteForever
import androidx.compose.material.icons.filled.DeleteSweep
import androidx.compose.material.icons.filled.RestoreFromTrash
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.jottery.android.model.DecryptedNote
import com.jottery.android.ui.component.ConfirmDialog
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.launch
import java.time.Duration
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RecycleBinScreen(
    appState: AppState,
    onNavigateBack: () -> Unit,
) {
    val scope = rememberCoroutineScope()
    var deletedNotes by remember { mutableStateOf(emptyList<DecryptedNote>()) }
    var showEmptyBinConfirm by remember { mutableStateOf(false) }

    // Load deleted notes on appear
    LaunchedEffect(Unit) {
        deletedNotes = appState.loadDeletedNotes()
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Recycle Bin") },
                navigationIcon = {
                    IconButton(onClick = onNavigateBack) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back",
                        )
                    }
                },
                actions = {
                    if (deletedNotes.isNotEmpty()) {
                        TextButton(
                            onClick = { showEmptyBinConfirm = true },
                        ) {
                            Text(
                                text = "Empty Bin",
                                color = MaterialTheme.colorScheme.error,
                            )
                        }
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surface,
                ),
            )
        },
    ) { paddingValues ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues),
        ) {
            if (deletedNotes.isEmpty()) {
                // Empty state
                Column(
                    modifier = Modifier.align(Alignment.Center),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Icon(
                        imageVector = Icons.Default.DeleteSweep,
                        contentDescription = null,
                        modifier = Modifier.padding(bottom = 16.dp),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f),
                    )
                    Text(
                        text = "Recycle Bin is Empty",
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    Text(
                        text = "Deleted notes will appear here for 30 days.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f),
                    )
                }
            } else {
                LazyColumn(
                    contentPadding = PaddingValues(vertical = 4.dp),
                    modifier = Modifier.fillMaxSize(),
                ) {
                    items(
                        items = deletedNotes,
                        key = { it.id },
                    ) { note ->
                        val dismissState = rememberSwipeToDismissBoxState(
                            confirmValueChange = { value ->
                                when (value) {
                                    SwipeToDismissBoxValue.StartToEnd -> {
                                        // Swipe right to restore
                                        scope.launch {
                                            appState.restoreNote(note.id)
                                            deletedNotes = deletedNotes.filter { it.id != note.id }
                                        }
                                        true
                                    }
                                    SwipeToDismissBoxValue.EndToStart -> {
                                        // Swipe left to permanently delete
                                        scope.launch {
                                            appState.permanentlyDeleteNote(note.id)
                                            deletedNotes = deletedNotes.filter { it.id != note.id }
                                        }
                                        true
                                    }
                                    SwipeToDismissBoxValue.Settled -> false
                                }
                            },
                        )

                        SwipeToDismissBox(
                            state = dismissState,
                            backgroundContent = {
                                DeletedNoteSwipeBackground(dismissState.targetValue)
                            },
                        ) {
                            DeletedNoteRow(note = note)
                        }

                        if (note.id != deletedNotes.lastOrNull()?.id) {
                            HorizontalDivider(
                                modifier = Modifier.padding(start = 16.dp),
                                color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f),
                            )
                        }
                    }
                }
            }
        }
    }

    // Empty bin confirmation dialog
    if (showEmptyBinConfirm) {
        ConfirmDialog(
            title = "Empty Recycle Bin",
            message = "This will permanently delete ${deletedNotes.size} note${if (deletedNotes.size == 1) "" else "s"}. This cannot be undone.",
            confirmText = "Empty Bin",
            onConfirm = {
                scope.launch {
                    for (note in deletedNotes) {
                        appState.permanentlyDeleteNote(note.id)
                    }
                    deletedNotes = emptyList()
                }
                showEmptyBinConfirm = false
            },
            onDismiss = { showEmptyBinConfirm = false },
        )
    }
}

/**
 * A single row in the recycle bin list showing a deleted note.
 */
@Composable
private fun DeletedNoteRow(note: DecryptedNote) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 12.dp),
    ) {
        // Title
        Text(
            text = note.title,
            style = MaterialTheme.typography.titleMedium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )

        // Preview
        if (note.preview.isNotEmpty()) {
            Spacer(modifier = Modifier.height(2.dp))
            Text(
                text = note.preview,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }

        // Deleted date
        if (note.deletedAt != null) {
            Spacer(modifier = Modifier.height(4.dp))
            Text(
                text = "Deleted ${formatDeletedDate(note.deletedAt)}",
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
            )
        }
    }
}

/**
 * Background content shown when swiping a deleted note row.
 * Swipe right = restore (green), swipe left = permanent delete (red).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DeletedNoteSwipeBackground(
    dismissDirection: SwipeToDismissBoxValue,
) {
    val alignment = when (dismissDirection) {
        SwipeToDismissBoxValue.StartToEnd -> Alignment.CenterStart
        SwipeToDismissBoxValue.EndToStart -> Alignment.CenterEnd
        SwipeToDismissBoxValue.Settled -> Alignment.Center
    }
    val icon = when (dismissDirection) {
        SwipeToDismissBoxValue.StartToEnd -> Icons.Default.RestoreFromTrash
        SwipeToDismissBoxValue.EndToStart -> Icons.Default.DeleteForever
        SwipeToDismissBoxValue.Settled -> Icons.Default.DeleteForever
    }
    val tint = when (dismissDirection) {
        SwipeToDismissBoxValue.StartToEnd -> MaterialTheme.colorScheme.primary
        SwipeToDismissBoxValue.EndToStart -> MaterialTheme.colorScheme.error
        SwipeToDismissBoxValue.Settled -> MaterialTheme.colorScheme.onSurfaceVariant
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 20.dp),
        contentAlignment = alignment,
    ) {
        Icon(
            imageVector = icon,
            contentDescription = when (dismissDirection) {
                SwipeToDismissBoxValue.StartToEnd -> "Restore"
                SwipeToDismissBoxValue.EndToStart -> "Permanently delete"
                SwipeToDismissBoxValue.Settled -> null
            },
            tint = tint,
        )
    }
}

/**
 * Format a deleted-at timestamp as a relative description.
 */
private fun formatDeletedDate(instant: Instant): String {
    val now = Instant.now()
    val duration = Duration.between(instant, now)

    return when {
        duration.isNegative -> "just now"
        duration.toMinutes() < 60 -> "${duration.toMinutes()}m ago"
        duration.toHours() < 24 -> "${duration.toHours()}h ago"
        duration.toDays() < 7 -> "${duration.toDays()}d ago"
        else -> {
            val formatter = DateTimeFormatter.ofPattern("d MMM yyyy")
                .withZone(ZoneId.systemDefault())
            formatter.format(instant)
        }
    }
}
