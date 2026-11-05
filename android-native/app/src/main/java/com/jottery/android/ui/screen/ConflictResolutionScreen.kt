package com.jottery.android.ui.screen

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedCard
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.jottery.android.R
import com.jottery.android.model.ConflictInfo
import com.jottery.android.model.ConflictResolutionStrategy
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ConflictResolutionScreen(
    appState: AppState,
    onBack: () -> Unit,
) {
    val conflicts by appState.pendingConflicts.collectAsState()
    val scope = rememberCoroutineScope()
    val snackbarHostState = remember { SnackbarHostState() }
    val failedMessage = stringResource(R.string.conflict_resolve_failed)

    Scaffold(
        snackbarHost = { SnackbarHost(snackbarHostState) },
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.conflict_screen_title, conflicts.size)) },
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
        if (conflicts.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    stringResource(R.string.conflict_none),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        } else {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                Spacer(Modifier.height(8.dp))

                for (conflict in conflicts) {
                    ConflictCard(
                        conflict = conflict,
                        onResolve = { strategy ->
                            scope.launch {
                                try {
                                    appState.resolveConflict(conflict.id, strategy)
                                } catch (e: CancellationException) {
                                    throw e
                                } catch (e: Exception) {
                                    // The conflict stays, so nothing is lost
                                    snackbarHostState.showSnackbar(failedMessage)
                                }
                            }
                        },
                    )
                }

                Spacer(Modifier.height(16.dp))
            }
        }
    }
}

@Composable
private fun ConflictCard(
    conflict: ConflictInfo,
    onResolve: (ConflictResolutionStrategy) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }

    Card(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp),
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                text = conflict.title.ifEmpty { stringResource(R.string.notes_untitled) },
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold,
            )

            // Local version
            OutlinedCard(modifier = Modifier.fillMaxWidth()) {
                Column(modifier = Modifier.padding(12.dp)) {
                    Text(
                        stringResource(R.string.conflict_local_version),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    Text(
                        stringResource(R.string.conflict_modified, conflict.localModifiedAt),
                        style = MaterialTheme.typography.bodySmall,
                    )
                    if (conflict.localTags.isNotEmpty()) {
                        Text(
                            stringResource(R.string.conflict_tags, conflict.localTags.joinToString(", ")),
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                    Text(
                        text = conflict.localContent,
                        style = MaterialTheme.typography.bodySmall,
                        maxLines = if (expanded) Int.MAX_VALUE else 3,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }

            // Server version
            OutlinedCard(modifier = Modifier.fillMaxWidth()) {
                Column(modifier = Modifier.padding(12.dp)) {
                    Text(
                        stringResource(R.string.conflict_server_version),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.tertiary,
                    )
                    Text(
                        stringResource(R.string.conflict_modified, conflict.serverModifiedAt),
                        style = MaterialTheme.typography.bodySmall,
                    )
                    if (conflict.serverTags.isNotEmpty()) {
                        Text(
                            stringResource(R.string.conflict_tags, conflict.serverTags.joinToString(", ")),
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                    Text(
                        text = conflict.serverContent ?: stringResource(R.string.conflict_server_unreadable),
                        style = MaterialTheme.typography.bodySmall,
                        maxLines = if (expanded) Int.MAX_VALUE else 3,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }

            // What both sides started from, when the server still has it
            conflict.ancestorContent?.let { ancestor ->
                OutlinedCard(modifier = Modifier.fillMaxWidth()) {
                    Column(modifier = Modifier.padding(12.dp)) {
                        Text(
                            stringResource(R.string.conflict_ancestor),
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        Text(
                            text = ancestor,
                            style = MaterialTheme.typography.bodySmall,
                            maxLines = if (expanded) Int.MAX_VALUE else 3,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
            }

            TextButton(onClick = { expanded = !expanded }) {
                Text(stringResource(if (expanded) R.string.conflict_show_less else R.string.conflict_show_more))
            }

            // Resolution buttons
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedButton(
                    onClick = { onResolve(ConflictResolutionStrategy.KEEP_LOCAL) },
                    modifier = Modifier.weight(1f),
                ) { Text(stringResource(R.string.conflict_keep_local)) }

                OutlinedButton(
                    onClick = { onResolve(ConflictResolutionStrategy.KEEP_SERVER) },
                    enabled = !conflict.serverUnreadable,
                    modifier = Modifier.weight(1f),
                ) { Text(stringResource(R.string.conflict_keep_server)) }

                Button(
                    onClick = { onResolve(ConflictResolutionStrategy.KEEP_BOTH) },
                    enabled = !conflict.serverUnreadable,
                    modifier = Modifier.weight(1f),
                ) { Text(stringResource(R.string.conflict_keep_both)) }
            }
        }
    }
}
