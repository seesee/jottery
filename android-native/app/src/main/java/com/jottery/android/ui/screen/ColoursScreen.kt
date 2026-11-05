package com.jottery.android.ui.screen

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.jottery.android.R
import com.jottery.android.model.NoteCategories
import com.jottery.android.model.tagCounts
import com.jottery.android.ui.component.ColourSheet
import com.jottery.android.ui.theme.NoteColors
import com.jottery.android.ui.theme.noteColourLabel
import com.jottery.android.ui.theme.standardColourLabel
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.launch

/** Name the colours, and give tags a colour. Both stay on this device. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ColoursScreen(
    appState: AppState,
    onBack: () -> Unit,
) {
    val settings by appState.settings.collectAsState()
    val notes by appState.notes.collectAsState()
    val archivedNotes by appState.archivedNotes.collectAsState()
    val categories = remember(settings) { NoteCategories.from(settings) }
    val tags = remember(notes, archivedNotes) { (notes + archivedNotes).tagCounts().map { it.first } }
    val scope = rememberCoroutineScope()
    var choosingFor by rememberSaveable { mutableStateOf<String?>(null) }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.settings_colours)) },
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
                Text(
                    stringResource(R.string.settings_colours_detail),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(16.dp),
                )
            }
            items(NoteColors.allColors.keys.toList(), key = { it }) { colour ->
                ColourNameRow(
                    colour = colour,
                    name = categories.names[colour].orEmpty(),
                    onNameChange = { scope.launch { appState.setCategoryName(colour, it) } },
                )
            }
            item {
                HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))
                Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
                    Text(stringResource(R.string.settings_tag_categories), style = MaterialTheme.typography.titleSmall)
                    Text(
                        stringResource(R.string.settings_tag_categories_detail),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            if (tags.isEmpty()) {
                item {
                    Text(
                        stringResource(R.string.tags_empty),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(16.dp),
                    )
                }
            }
            items(tags, key = { "tag-$it" }) { tag ->
                val colour = categories.colourOfTag(tag)
                ListItem(
                    headlineContent = { Text("#$tag") },
                    trailingContent = {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Text(
                                if (colour != null) noteColourLabel(colour) else stringResource(R.string.editor_colour_none),
                                style = MaterialTheme.typography.bodyMedium,
                            )
                            Spacer(Modifier.width(8.dp))
                            Swatch(NoteColors.fromString(colour))
                        }
                    },
                    modifier = Modifier.clickable { choosingFor = tag },
                )
            }
        }
    }

    choosingFor?.let { tag ->
        ColourSheet(
            onChoose = { colour ->
                choosingFor = null
                scope.launch { appState.setTagColour(tag, colour) }
            },
            onDismiss = { choosingFor = null },
        )
    }
}

@Composable
private fun ColourNameRow(colour: String, name: String, onNameChange: (String) -> Unit) {
    var text by rememberSaveable(colour) { mutableStateOf(name) }
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 4.dp),
    ) {
        Swatch(NoteColors.fromString(colour))
        Spacer(Modifier.width(16.dp))
        OutlinedTextField(
            value = text,
            onValueChange = {
                text = it
                onNameChange(it)
            },
            label = { Text(standardColourLabel(colour)) },
            singleLine = true,
            modifier = Modifier.weight(1f),
        )
    }
}

@Composable
private fun Swatch(colour: Color?) {
    Box(
        modifier = Modifier
            .size(20.dp)
            .background(colour ?: Color.Transparent, CircleShape),
    )
}
