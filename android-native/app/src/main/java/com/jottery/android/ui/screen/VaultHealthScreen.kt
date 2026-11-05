package com.jottery.android.ui.screen

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.jottery.android.R
import com.jottery.android.service.BlobRepair
import com.jottery.android.service.VaultHealthService
import com.jottery.android.ui.component.ConfirmDialog
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.launch

/** What can't be read or found, and what can be done about it. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun VaultHealthScreen(
    appState: AppState,
    onBack: () -> Unit,
    onOpenNote: (String) -> Unit,
) {
    val scope = rememberCoroutineScope()
    var report by remember { mutableStateOf<VaultHealthService.Report?>(null) }
    var scanning by remember { mutableStateOf(false) }
    var toDelete by remember { mutableStateOf<String?>(null) }
    val repairs = remember { mutableStateMapOf<String, BlobRepair>() }

    fun rescan() {
        scope.launch {
            scanning = true
            report = appState.scanVaultHealth()
            scanning = false
        }
    }

    LaunchedEffect(Unit) { rescan() }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.health_title)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = stringResource(R.string.back),
                        )
                    }
                },
            )
        },
    ) { padding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
        ) {
            item {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Text(stringResource(R.string.health_description), style = MaterialTheme.typography.bodyMedium)
                    OutlinedButton(
                        onClick = ::rescan,
                        enabled = !scanning,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(stringResource(if (scanning) R.string.health_scanning else R.string.health_rescan))
                    }
                    if (scanning) LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
                    if (!scanning && report?.isHealthy == true) {
                        Text(
                            stringResource(R.string.health_healthy),
                            color = MaterialTheme.colorScheme.primary,
                            style = MaterialTheme.typography.bodyLarge,
                        )
                    }
                }
            }

            val undecryptable = report?.undecryptable.orEmpty()
            if (undecryptable.isNotEmpty()) {
                item { Section(R.string.health_undecryptable_title, R.string.health_undecryptable_hint) }
                items(undecryptable, key = { it.id }) { note ->
                    ListItem(
                        headlineContent = { Text(note.id) },
                        supportingContent = { Text(stringResource(R.string.conflict_modified, note.modifiedAt)) },
                        trailingContent = {
                            TextButton(onClick = { toDelete = note.id }) {
                                Text(stringResource(R.string.health_delete))
                            }
                        },
                    )
                }
            }

            val missing = report?.missing.orEmpty()
            if (missing.isNotEmpty()) {
                item { Section(R.string.health_missing_title, R.string.health_missing_hint) }
                items(missing, key = { it.noteId + it.blobId }) { attachment ->
                    MissingRow(
                        attachment = attachment,
                        repair = repairs[attachment.blobId],
                        onRepair = {
                            scope.launch {
                                repairs[attachment.blobId] = appState.repairAttachment(attachment.blobId)
                            }
                        },
                        onOpen = { onOpenNote(attachment.noteId) },
                    )
                }
            }
        }
    }

    toDelete?.let { id ->
        ConfirmDialog(
            title = stringResource(R.string.health_delete_title),
            message = stringResource(R.string.health_delete_message),
            confirmText = stringResource(R.string.health_delete),
            onConfirm = {
                toDelete = null
                scope.launch {
                    appState.permanentlyDeleteNotes(listOf(id))
                    report = appState.scanVaultHealth()
                }
            },
            onDismiss = { toDelete = null },
        )
    }
}

@Composable
private fun Section(title: Int, hint: Int) {
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        Text(stringResource(title), style = MaterialTheme.typography.titleSmall)
        Text(
            stringResource(hint),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun MissingRow(
    attachment: VaultHealthService.MissingAttachment,
    repair: BlobRepair?,
    onRepair: () -> Unit,
    onOpen: () -> Unit,
) {
    ListItem(
        headlineContent = { Text(attachment.noteTitle ?: stringResource(R.string.health_unknown_note)) },
        supportingContent = {
            when (repair) {
                BlobRepair.REPAIRED -> Text(stringResource(R.string.health_repair_success))
                BlobRepair.NOT_ON_SERVER -> Text(stringResource(R.string.health_repair_not_on_server))
                BlobRepair.FAILED -> Text(stringResource(R.string.health_repair_failed))
                null -> Unit
            }
        },
        trailingContent = {
            if (repair == BlobRepair.NOT_ON_SERVER) {
                TextButton(onClick = onOpen) { Text(stringResource(R.string.note_row_open)) }
            } else if (repair != BlobRepair.REPAIRED) {
                TextButton(onClick = onRepair) { Text(stringResource(R.string.health_repair)) }
            }
        },
    )
}
