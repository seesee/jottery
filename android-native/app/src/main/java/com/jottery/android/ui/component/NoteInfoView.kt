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
import androidx.compose.ui.unit.dp
import com.jottery.android.model.DecryptedNote
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
                text = "Note Info",
                style = MaterialTheme.typography.titleMedium,
            )

            Spacer(modifier = Modifier.height(16.dp))

            InfoRow("Created", dateFormatter.format(note.createdAt))
            InfoRow("Modified", dateFormatter.format(note.modifiedAt))
            note.syncedAt?.let { InfoRow("Synced", dateFormatter.format(it)) }
            InfoRow("Words", note.wordCount.toString())
            InfoRow("Characters", note.content.length.toString())
            InfoRow("Tags", note.tags.size.toString())
            InfoRow("Attachments", note.attachments.size.toString())
            InfoRow("Version", note.version.toString())
            InfoRow("Syntax", note.syntaxLanguage)
            if (note.pinned) InfoRow("Pinned", "Yes")
            if (note.archived) InfoRow("Archived", "Yes")
            if (note.locked) InfoRow("Locked", "Yes")
            note.color?.let { InfoRow("Colour", it) }
            note.contentHash?.let { InfoRow("Hash", it.take(16) + "\u2026") }

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
