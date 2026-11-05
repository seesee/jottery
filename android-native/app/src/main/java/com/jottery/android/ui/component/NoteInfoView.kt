package com.jottery.android.ui.component

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.jottery.android.R
import com.jottery.android.model.DecryptedNote
import com.jottery.android.ui.theme.noteColourLabel
import java.time.ZoneId
import java.time.format.DateTimeFormatter

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NoteInfoView(
    note: DecryptedNote,
    onDismiss: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState()
    val dateFormatter = DateTimeFormatter.ofPattern("dd MMM yyyy HH:mm")
        .withZone(ZoneId.systemDefault())

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = sheetState,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp, vertical = 16.dp),
        ) {
            Text(
                text = stringResource(R.string.editor_info),
                style = MaterialTheme.typography.titleMedium,
            )

            Spacer(modifier = Modifier.height(16.dp))

            InfoRow(stringResource(R.string.note_info_created), dateFormatter.format(note.createdAt))
            InfoRow(stringResource(R.string.note_info_modified), dateFormatter.format(note.modifiedAt))
            note.syncedAt?.let { InfoRow(stringResource(R.string.sync_synced), dateFormatter.format(it)) }
            InfoRow(stringResource(R.string.note_info_words), note.wordCount.toString())
            InfoRow(stringResource(R.string.note_info_characters), note.content.length.toString())
            InfoRow(stringResource(R.string.note_info_tags), note.tags.size.toString())
            InfoRow(stringResource(R.string.note_info_attachments), note.attachments.size.toString())
            InfoRow(stringResource(R.string.settings_version), note.version.toString())
            InfoRow(stringResource(R.string.note_info_syntax), note.syntaxLanguage)
            if (note.pinned) InfoRow(stringResource(R.string.note_info_pinned), stringResource(R.string.note_info_yes))
            if (note.archived) InfoRow(stringResource(R.string.note_info_archived), stringResource(R.string.note_info_yes))
            if (note.locked) InfoRow(stringResource(R.string.note_info_locked), stringResource(R.string.note_info_yes))
            note.color?.let { InfoRow(stringResource(R.string.info_colour), noteColourLabel(it)) }
            note.contentHash?.let { InfoRow(stringResource(R.string.info_hash), it.take(16) + "\u2026") }

            Spacer(modifier = Modifier.height(24.dp))
        }
    }
}

@Composable
private fun InfoRow(label: String, value: String) {
    Column(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(vertical = 6.dp),
        ) {
            Text(
                text = label,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f),
            )
            Text(
                text = value,
                style = MaterialTheme.typography.bodyMedium,
            )
        }
        HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.3f))
    }
}
