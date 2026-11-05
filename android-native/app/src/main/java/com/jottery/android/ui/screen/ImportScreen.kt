package com.jottery.android.ui.screen

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.jottery.android.R
import com.jottery.android.service.ImportStrategy
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ImportScreen(
    appState: AppState,
    onBack: () -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var isImporting by remember { mutableStateOf(false) }
    var statusMessage by remember { mutableStateOf<String?>(null) }
    var errorMessage by remember { mutableStateOf<String?>(null) }
    var strategy by remember { mutableStateOf(ImportStrategy.SKIP) }

    val launcher = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument()
    ) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch {
            isImporting = true
            errorMessage = null
            try {
                val json = withContext(Dispatchers.IO) {
                    context.contentResolver.openInputStream(uri)?.use { input ->
                        input.bufferedReader().readText()
                    } ?: throw Exception(context.getString(R.string.import_could_not_read_file))
                }
                val result = appState.importNotes(json, strategy)
                statusMessage = buildString {
                    append(context.getString(R.string.import_done, result.imported, result.attachments))
                    if (result.skipped > 0) {
                        append(' ')
                        append(context.getString(R.string.import_skipped, result.skipped))
                    }
                }
                errorMessage = if (result.failed > 0 || result.attachmentsFailed > 0) {
                    context.getString(R.string.import_failures, result.failed, result.attachmentsFailed)
                } else {
                    null
                }
            } catch (e: Exception) {
                errorMessage = context.getString(R.string.import_failed, e.message.orEmpty())
            } finally {
                isImporting = false
            }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.import_import_notes)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.back))
                    }
                },
            )
        },
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(
                stringResource(R.string.import_import_notes_from_a_json),
                style = MaterialTheme.typography.bodyMedium,
            )

            Text(
                stringResource(R.string.import_duplicates),
                style = MaterialTheme.typography.labelLarge,
            )
            SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
                val choices = listOf(
                    ImportStrategy.SKIP to R.string.import_skip,
                    ImportStrategy.REPLACE to R.string.import_replace,
                    ImportStrategy.KEEP_BOTH to R.string.import_keep_both,
                )
                choices.forEachIndexed { index, (choice, label) ->
                    SegmentedButton(
                        selected = strategy == choice,
                        onClick = { strategy = choice },
                        shape = SegmentedButtonDefaults.itemShape(index = index, count = choices.size),
                    ) { Text(stringResource(label)) }
                }
            }

            Button(
                onClick = { launcher.launch(arrayOf("application/json", "*/*")) },
                modifier = Modifier.fillMaxWidth(),
                enabled = !isImporting,
            ) {
                if (isImporting) {
                    CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
                    Spacer(Modifier.width(8.dp))
                }
                Text(stringResource(R.string.import_select_json_file))
            }

            statusMessage?.let {
                Text(it, color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.bodyMedium)
            }
            errorMessage?.let {
                Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
            }
        }
    }
}
