package com.jottery.android.ui.screen

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.jottery.android.R
import com.jottery.android.model.DecryptedSavedSearch
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SavedSearchesScreen(
    appState: AppState,
    onBack: () -> Unit,
    onApplySearch: (String) -> Unit,
) {
    val savedSearches by appState.savedSearches.collectAsState()
    val scope = rememberCoroutineScope()
    var renaming by remember { mutableStateOf<DecryptedSavedSearch?>(null) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.saved_searches_title)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.action_back))
                    }
                },
            )
        },
    ) { padding ->
        if (savedSearches.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
                contentAlignment = Alignment.Center,
            ) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(
                        stringResource(R.string.saved_searches_empty),
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(Modifier.height(8.dp))
                    Text(
                        stringResource(R.string.saved_searches_empty_hint),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        } else {
            LazyColumn(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
            ) {
                itemsIndexed(savedSearches, key = { _, search -> search.id }) { index, search ->
                    ListItem(
                        headlineContent = { Text(search.name) },
                        supportingContent = { Text(search.query) },
                        trailingContent = {
                            SavedSearchMenu(
                                canMoveUp = index > 0,
                                canMoveDown = index < savedSearches.lastIndex,
                                onRename = { renaming = search },
                                onMove = { offset -> scope.launch { appState.moveSavedSearch(search.id, offset) } },
                                onDelete = { scope.launch { appState.deleteSavedSearch(search.id) } },
                            )
                        },
                        modifier = Modifier.clickable {
                            appState.applySavedSearch(search.id)
                            onApplySearch(search.query)
                        },
                    )
                }
            }
        }
    }

    renaming?.let { search ->
        var name by remember(search.id) { mutableStateOf(search.name) }
        AlertDialog(
            onDismissRequest = { renaming = null },
            title = { Text(stringResource(R.string.saved_search_rename)) },
            text = {
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text(stringResource(R.string.saved_search_name)) },
                    singleLine = true,
                )
            },
            confirmButton = {
                TextButton(
                    enabled = name.isNotBlank(),
                    onClick = {
                        renaming = null
                        scope.launch { appState.renameSavedSearch(search.id, name.trim()) }
                    },
                ) { Text(stringResource(R.string.action_save)) }
            },
            dismissButton = {
                TextButton(onClick = { renaming = null }) { Text(stringResource(R.string.action_cancel)) }
            },
        )
    }
}

@Composable
private fun SavedSearchMenu(
    canMoveUp: Boolean,
    canMoveDown: Boolean,
    onRename: () -> Unit,
    onMove: (Int) -> Unit,
    onDelete: () -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { expanded = true }) {
            Icon(Icons.Default.MoreVert, contentDescription = stringResource(R.string.action_more))
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.saved_search_rename)) },
                onClick = { expanded = false; onRename() },
            )
            if (canMoveUp) {
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.saved_search_move_up)) },
                    onClick = { expanded = false; onMove(-1) },
                )
            }
            if (canMoveDown) {
                DropdownMenuItem(
                    text = { Text(stringResource(R.string.saved_search_move_down)) },
                    onClick = { expanded = false; onMove(1) },
                )
            }
            DropdownMenuItem(
                text = { Text(stringResource(R.string.action_delete)) },
                onClick = { expanded = false; onDelete() },
            )
        }
    }
}
