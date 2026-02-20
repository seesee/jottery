package com.jottery.android.ui.component

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Restore
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.jottery.android.model.NoteVersionRecord
import java.time.ZoneId
import java.time.format.DateTimeFormatter

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun VersionHistoryView(
    versions: List<NoteVersionRecord>,
    onRestore: (NoteVersionRecord) -> Unit,
    onPreview: ((NoteVersionRecord) -> String?)? = null,
    onDismiss: () -> Unit,
) {
    val sheetState = rememberModalBottomSheetState()
    val dateFormatter = DateTimeFormatter.ofPattern("dd MMM yyyy HH:mm")
        .withZone(ZoneId.systemDefault())

    var previewVersion by remember { mutableStateOf<NoteVersionRecord?>(null) }
    var previewContent by remember { mutableStateOf<String?>(null) }

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
                text = "Version History",
                style = MaterialTheme.typography.titleMedium,
            )

            Spacer(modifier = Modifier.height(16.dp))

            if (versions.isEmpty()) {
                Text(
                    text = "No version history available. Version history will appear after syncing.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                LazyColumn {
                    items(versions) { version ->
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable {
                                    if (onPreview != null) {
                                        previewContent = onPreview(version)
                                        previewVersion = version
                                    }
                                }
                                .padding(vertical = 8.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    text = "Version ${version.version}",
                                    style = MaterialTheme.typography.bodyMedium,
                                )
                                Text(
                                    text = "${version.reason} \u2022 ${
                                        try {
                                            dateFormatter.format(
                                                java.time.Instant.parse(version.createdAt)
                                            )
                                        } catch (_: Exception) { version.createdAt }
                                    }",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }

                            IconButton(onClick = { onRestore(version) }) {
                                Icon(
                                    Icons.Default.Restore,
                                    contentDescription = "Restore this version",
                                )
                            }
                        }
                        HorizontalDivider()
                    }
                }
            }

            Spacer(modifier = Modifier.height(24.dp))
        }
    }

    // Version content preview dialog
    val pVersion = previewVersion
    val pContent = previewContent
    if (pVersion != null && pContent != null) {
        AlertDialog(
            onDismissRequest = {
                previewVersion = null
                previewContent = null
            },
            title = {
                Text("Version ${pVersion.version}")
            },
            text = {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(400.dp)
                        .verticalScroll(rememberScrollState()),
                ) {
                    Text(
                        text = "${pVersion.reason} \u2022 ${
                            try {
                                dateFormatter.format(
                                    java.time.Instant.parse(pVersion.createdAt)
                                )
                            } catch (_: Exception) { pVersion.createdAt }
                        }",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(modifier = Modifier.height(12.dp))
                    Text(
                        text = pContent,
                        style = MaterialTheme.typography.bodySmall.copy(
                            fontFamily = FontFamily.Monospace,
                            fontSize = 13.sp,
                            lineHeight = 18.sp,
                        ),
                    )
                }
            },
            confirmButton = {
                TextButton(onClick = {
                    onRestore(pVersion)
                    previewVersion = null
                    previewContent = null
                }) {
                    Text("Restore")
                }
            },
            dismissButton = {
                TextButton(onClick = {
                    previewVersion = null
                    previewContent = null
                }) {
                    Text("Done")
                }
            },
        )
    }
}
