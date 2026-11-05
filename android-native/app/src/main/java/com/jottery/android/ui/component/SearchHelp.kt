package com.jottery.android.ui.component

import androidx.annotation.StringRes
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.AssistChip
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import com.jottery.android.R
import com.jottery.android.util.SearchSuggestions

private class Example(val query: String, @StringRes val label: Int)

private class Section(@StringRes val title: Int, val examples: List<Example>)

private val sections = listOf(
    Section(
        R.string.search_help_tags,
        listOf(
            Example("#tagname", R.string.search_help_tags),
            Example("#tag1 #tag2", R.string.search_help_tags_and),
            Example("#tag1 | #tag2", R.string.search_help_tags_or),
        ),
    ),
    Section(
        R.string.search_help_text,
        listOf(
            Example("dog", R.string.search_help_text),
            Example("\"exact phrase\"", R.string.search_help_exact_phrase),
            Example("dog cat", R.string.search_help_both_words),
            Example("dog | cat", R.string.search_help_either_word),
            Example("-cat", R.string.search_help_not_containing),
        ),
    ),
    Section(
        R.string.search_help_wildcards,
        listOf(
            Example("dog*", R.string.search_help_starts_with),
            Example("*dog", R.string.search_help_ends_with),
            Example("*dog*", R.string.search_help_contains_anywhere),
        ),
    ),
    Section(
        R.string.search_help_modifiers,
        listOf(
            Example("has:attachment", R.string.search_help_has_attachment),
            Example("created:>2024-01-01", R.string.search_help_created_after),
            Example("created:<2024-06-30", R.string.search_help_created_before),
            Example("created:2024-01-01..2024-06-30", R.string.search_help_created_range),
            Example("modified:>2024-01-01", R.string.search_help_modified_after),
            Example("words:>100", R.string.search_help_words_more),
            Example("words:<50", R.string.search_help_words_less),
            Example("words:50..200", R.string.search_help_words_range),
        ),
    ),
)

/** What can be searched for. Tapping an example puts it in the search bar. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SearchHelpSheet(
    onChoose: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState()) {
        LazyColumn(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp),
        ) {
            item {
                Text(stringResource(R.string.search_help_title), style = MaterialTheme.typography.titleMedium)
                Text(
                    stringResource(R.string.search_help_try),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(bottom = 8.dp),
                )
            }
            for (section in sections) {
                item {
                    Text(
                        stringResource(section.title),
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(top = 12.dp, bottom = 4.dp),
                    )
                }
                items(section.examples) { example ->
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { onChoose(example.query) }
                            .padding(vertical = 8.dp),
                    ) {
                        Text(example.query, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodyMedium)
                        Text(
                            stringResource(example.label),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
        }
    }
}

/** Tags that would finish the word being typed, or recent searches before anything is. */
@Composable
fun SearchSuggestionChips(
    query: String,
    tags: List<String>,
    recent: List<String>,
    onChoose: (String) -> Unit,
) {
    val completions = SearchSuggestions.tagCompletions(query, tags)
    val offers = when {
        completions.isNotEmpty() -> completions.map { "#$it" to SearchSuggestions.complete(query, it) }
        query.isEmpty() -> recent.map { it to it }
        else -> emptyList()
    }
    if (offers.isEmpty()) return

    Row(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 16.dp),
    ) {
        offers.forEach { (label, result) ->
            AssistChip(onClick = { onChoose(result) }, label = { Text(label, maxLines = 1) })
        }
    }
}
