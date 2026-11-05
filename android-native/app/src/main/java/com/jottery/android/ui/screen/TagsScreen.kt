package com.jottery.android.ui.screen

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import com.jottery.android.R
import com.jottery.android.model.tagCounts
import com.jottery.android.viewmodel.AppState
import java.text.NumberFormat

/** Every tag in use, with how many notes carry it. Tapping one searches for it. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TagsScreen(
    appState: AppState,
    onBack: () -> Unit,
) {
    val notes by appState.notes.collectAsState()
    val archivedNotes by appState.archivedNotes.collectAsState()
    val tags = remember(notes, archivedNotes) { (notes + archivedNotes).tagCounts() }
    val numbers = remember { NumberFormat.getInstance() }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.note_info_tags)) },
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
        if (tags.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    stringResource(R.string.tags_empty),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        } else {
            LazyColumn(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
            ) {
                items(tags, key = { it.first }) { (tag, count) ->
                    ListItem(
                        headlineContent = { Text("#$tag") },
                        trailingContent = { Text(numbers.format(count)) },
                        modifier = Modifier.clickable {
                            appState.setSearchQuery("#$tag")
                            onBack()
                        },
                    )
                }
            }
        }
    }
}
