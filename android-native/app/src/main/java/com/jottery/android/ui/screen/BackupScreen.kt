package com.jottery.android.ui.screen

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import com.jottery.android.R
import com.jottery.android.service.BackupReminder
import com.jottery.android.service.backup.BackupException
import com.jottery.android.service.backup.BackupSource
import com.jottery.android.service.backup.RestorePlan
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.FileNotFoundException
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle

/** The message for a failed restore. */
fun Context.backupErrorText(error: Throwable): String = when (error) {
    is BackupException.WrongPassword -> getString(R.string.backup_error_wrong_password)
    is BackupException.NotABackup -> getString(R.string.backup_error_not_backup)
    is BackupException.UnsupportedVersion -> getString(R.string.backup_error_unsupported, error.version)
    is BackupException.Corrupt -> getString(R.string.backup_error_corrupt)
    is BackupException.KeyNotInBackup -> getString(R.string.backup_error_no_key)
    else -> getString(R.string.backup_error_generic, error.message ?: error.javaClass.simpleName)
}

private fun Context.sourceFor(uri: Uri) = BackupSource {
    contentResolver.openInputStream(uri) ?: throw FileNotFoundException()
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BackupScreen(
    appState: AppState,
    onBack: () -> Unit,
    onOpenVaultHealth: () -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var lastBackup by remember { mutableStateOf(BackupReminder.lastBackupAt(context)) }
    var remindersOn by remember { mutableStateOf(BackupReminder.isEnabled(context)) }
    val deniedMessage = stringResource(R.string.backup_reminders_denied)
    val notificationPermission = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        remindersOn = granted
        BackupReminder.setEnabled(context, granted)
        if (!granted) appState.showMessage(deniedMessage)
    }
    var isBusy by remember { mutableStateOf(false) }
    var statusMessage by remember { mutableStateOf<String?>(null) }
    var errorMessage by remember { mutableStateOf<String?>(null) }

    var uriToRestore by remember { mutableStateOf<Uri?>(null) }
    var plan by remember { mutableStateOf<RestorePlan?>(null) }

    val createLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/octet-stream")
    ) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch {
            isBusy = true
            errorMessage = null
            statusMessage = null
            try {
                val summary = withContext(Dispatchers.IO) {
                    val stream = context.contentResolver.openOutputStream(uri)
                        ?: throw FileNotFoundException()
                    stream.use { appState.createBackup(it) }
                }
                statusMessage = context.getString(R.string.backup_created, summary.notes, summary.attachments)
                lastBackup = BackupReminder.lastBackupAt(context)
            } catch (e: Exception) {
                errorMessage = context.getString(R.string.backup_failed, e.message ?: e.javaClass.simpleName)
            } finally {
                isBusy = false
            }
        }
    }

    val restoreLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument()
    ) { uri ->
        if (uri != null) {
            errorMessage = null
            statusMessage = null
            uriToRestore = uri
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.backup_title)) },
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
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(stringResource(R.string.backup_intro), style = MaterialTheme.typography.bodyMedium)

            Text(
                stringResource(
                    R.string.backup_last,
                    lastBackup?.let(::backupDate) ?: stringResource(R.string.backup_never),
                ),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            Button(
                onClick = { createLauncher.launch("jottery-backup-${LocalDate.now()}.jottery-backup") },
                modifier = Modifier.fillMaxWidth(),
                enabled = !isBusy,
            ) {
                Text(stringResource(R.string.backup_create))
            }

            OutlinedButton(
                onClick = { restoreLauncher.launch(arrayOf("*/*")) },
                modifier = Modifier.fillMaxWidth(),
                enabled = !isBusy,
            ) {
                Text(stringResource(R.string.backup_restore))
            }

            ListItem(
                headlineContent = { Text(stringResource(R.string.backup_reminders)) },
                supportingContent = { Text(stringResource(R.string.backup_reminders_detail)) },
                trailingContent = {
                    Switch(
                        checked = remindersOn,
                        onCheckedChange = { on ->
                            val needsAsking = on && Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
                                ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) !=
                                PackageManager.PERMISSION_GRANTED
                            if (needsAsking) {
                                notificationPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
                            } else {
                                remindersOn = on
                                BackupReminder.setEnabled(context, on)
                            }
                        },
                    )
                },
            )

            OutlinedButton(onClick = onOpenVaultHealth, modifier = Modifier.fillMaxWidth()) {
                Text(stringResource(R.string.health_title))
            }

            if (isBusy) {
                LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
            }
            statusMessage?.let {
                Text(it, color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.bodyMedium)
            }
            errorMessage?.let {
                Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
            }
        }
    }

    val pendingUri = uriToRestore
    if (pendingUri != null && plan == null) {
        BackupPasswordDialog(
            isChecking = isBusy,
            onConfirm = { password ->
                scope.launch {
                    isBusy = true
                    try {
                        plan = appState.inspectBackup(context.sourceFor(pendingUri), password)
                    } catch (e: Exception) {
                        errorMessage = context.backupErrorText(e)
                        uriToRestore = null
                    } finally {
                        isBusy = false
                    }
                }
            },
            onDismiss = { uriToRestore = null },
        )
    }

    val pendingPlan = plan
    if (pendingUri != null && pendingPlan != null) {
        RestoreConfirmDialog(
            plan = pendingPlan,
            onConfirm = {
                plan = null
                uriToRestore = null
                scope.launch {
                    isBusy = true
                    try {
                        val result = appState.restoreBackup(context.sourceFor(pendingUri), pendingPlan)
                        statusMessage = buildString {
                            append(context.getString(R.string.backup_restored, result.notesRestored, result.attachmentsRestored))
                            if (result.keptNewer > 0) {
                                append(' ')
                                append(context.getString(R.string.backup_restored_kept, result.keptNewer))
                            }
                            if (result.unreadable > 0) {
                                append(' ')
                                append(context.getString(R.string.backup_restored_unreadable, result.unreadable))
                            }
                        }
                    } catch (e: Exception) {
                        errorMessage = context.backupErrorText(e)
                    } finally {
                        isBusy = false
                    }
                }
            },
            onDismiss = {
                plan = null
                uriToRestore = null
            },
        )
    }
}

@Composable
private fun BackupPasswordDialog(
    isChecking: Boolean,
    onConfirm: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    var password by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.backup_password_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(stringResource(R.string.backup_password_message))
                OutlinedTextField(
                    value = password,
                    onValueChange = { password = it },
                    label = { Text(stringResource(R.string.backup_password_label)) },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Password,
                        autoCorrectEnabled = false,
                    ),
                    enabled = !isChecking,
                    modifier = Modifier.fillMaxWidth(),
                )
                if (isChecking) {
                    Text(stringResource(R.string.backup_checking), style = MaterialTheme.typography.bodySmall)
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { onConfirm(password) }, enabled = !isChecking && password.isNotEmpty()) {
                Text(stringResource(R.string.backup_confirm_button))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss, enabled = !isChecking) {
                Text(stringResource(R.string.cancel))
            }
        },
    )
}

@Composable
private fun RestoreConfirmDialog(
    plan: RestorePlan,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.backup_confirm_title)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    stringResource(
                        R.string.backup_confirm_summary,
                        plan.notes, plan.attachments, plan.versions, plan.savedSearches,
                    )
                )
                if (plan.replaced > 0) {
                    Text(stringResource(R.string.backup_confirm_replaced, plan.replaced))
                }
                if (plan.keptNewer > 0) {
                    Text(stringResource(R.string.backup_confirm_kept, plan.keptNewer))
                }
                if (plan.unreadable > 0) {
                    Text(
                        stringResource(R.string.backup_confirm_unreadable, plan.unreadable),
                        color = MaterialTheme.colorScheme.error,
                    )
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onConfirm) { Text(stringResource(R.string.backup_confirm_button)) }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) }
        },
    )
}

private fun backupDate(at: Instant): String =
    DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM).format(at.atZone(ZoneId.systemDefault()))
