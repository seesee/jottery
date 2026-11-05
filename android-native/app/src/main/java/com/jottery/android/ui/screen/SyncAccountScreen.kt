package com.jottery.android.ui.screen

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import android.text.format.Formatter
import com.jottery.android.R
import com.jottery.android.model.UserAccountInfo
import com.jottery.android.ui.component.ConfirmDialog
import com.jottery.android.viewmodel.AccountDeletionMode
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.launch

/**
 * The connected state of Sync: where this device syncs, the way to unlink it, and
 * the account itself. Account actions need the account password, not the device's
 * key, so that a stolen phone can't delete the account behind it. The session
 * lives only as long as this screen.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SyncAccountScreen(
    appState: AppState,
    onBack: () -> Unit,
) {
    val settings by appState.settings.collectAsState()
    val isSyncing by appState.isSyncing.collectAsState()
    val syncError by appState.syncError.collectAsState()
    val lastSyncAt by appState.lastSyncAt.collectAsState()
    val scope = rememberCoroutineScope()
    val context = LocalContext.current
    val clipboard = LocalClipboardManager.current

    var deviceName by remember { mutableStateOf<String?>(null) }
    var email by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var session by remember { mutableStateOf<String?>(null) }
    var account by remember { mutableStateOf<UserAccountInfo?>(null) }
    var inboxToken by remember { mutableStateOf<String?>(null) }
    var error by remember { mutableStateOf<String?>(null) }
    var isWorking by remember { mutableStateOf(false) }
    var deletionMode by remember { mutableStateOf(AccountDeletionMode.DEACTIVATE) }
    var showDisconnect by remember { mutableStateOf(false) }
    var showDeleteNotes by remember { mutableStateOf(false) }
    var showDeleteAccount by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) { deviceName = appState.syncDeviceName() }

    /** Runs an account action, showing any failure on this screen. */
    fun working(action: suspend () -> Unit) {
        error = null
        isWorking = true
        scope.launch {
            try {
                action()
            } catch (e: kotlinx.coroutines.CancellationException) {
                throw e
            } catch (e: Exception) {
                error = appState.describeAccountFailure(e)
            } finally {
                isWorking = false
            }
        }
    }

    suspend fun refresh(sessionId: String) {
        account = appState.accountInfo(sessionId)
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.sync_account_title)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = stringResource(R.string.action_back),
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
                .consumeWindowInsets(padding)
                .imePadding()
                .verticalScroll(rememberScrollState()),
        ) {
            ListItem(
                headlineContent = { Text(stringResource(R.string.sync_account_server)) },
                supportingContent = { Text(settings.syncEndpoint.orEmpty()) },
            )
            deviceName?.let { name ->
                ListItem(
                    headlineContent = { Text(stringResource(R.string.connect_device_name)) },
                    supportingContent = { Text(name) },
                )
            }
            ListItem(
                headlineContent = { Text(stringResource(R.string.sync_account_status)) },
                supportingContent = {
                    Text(
                        syncError
                            ?: lastSyncAt?.let { stringResource(R.string.sync_account_last_synced, formatTime(it)) }
                            ?: stringResource(R.string.sync_account_not_synced),
                    )
                },
            )

            Column(
                modifier = Modifier.padding(horizontal = 16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Button(
                    onClick = { scope.launch { appState.triggerSync() } },
                    enabled = !isSyncing,
                    modifier = Modifier.fillMaxWidth(),
                ) { Text(stringResource(R.string.sync_account_sync_now)) }
                OutlinedButton(
                    onClick = { showDisconnect = true },
                    modifier = Modifier.fillMaxWidth(),
                ) { Text(stringResource(R.string.sync_account_disconnect)) }
            }

            HorizontalDivider(modifier = Modifier.padding(vertical = 16.dp))

            Column(
                modifier = Modifier.padding(horizontal = 16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text(stringResource(R.string.sync_account_section), style = MaterialTheme.typography.titleMedium)

                val sessionId = session
                val info = account
                if (sessionId == null || info == null) {
                    Text(
                        stringResource(R.string.sync_account_sign_in_help),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    OutlinedTextField(
                        value = email,
                        onValueChange = { email = it },
                        label = { Text(stringResource(R.string.connect_email)) },
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                    OutlinedTextField(
                        value = password,
                        onValueChange = { password = it },
                        label = { Text(stringResource(R.string.connect_account_password)) },
                        visualTransformation = PasswordVisualTransformation(),
                        keyboardOptions = KeyboardOptions(
                            keyboardType = KeyboardType.Password,
                            autoCorrectEnabled = false,
                        ),
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                    Button(
                        enabled = !isWorking && email.isNotBlank() && password.isNotEmpty(),
                        onClick = {
                            working {
                                val id = appState.signInToAccount(email.trim(), password)
                                password = ""
                                refresh(id)
                                session = id
                            }
                        },
                        modifier = Modifier.fillMaxWidth(),
                    ) { Text(stringResource(R.string.sync_account_sign_in)) }
                } else {
                    val used = Formatter.formatShortFileSize(context, info.storageUsedBytes)
                    val quota = Formatter.formatShortFileSize(context, info.storageQuotaMb * 1024 * 1024)
                    Text(
                        pluralStringResource(R.plurals.sync_account_notes, info.noteCount, info.noteCount),
                        style = MaterialTheme.typography.bodyMedium,
                    )
                    Text(
                        stringResource(R.string.sync_account_storage, used, quota),
                        style = MaterialTheme.typography.bodyMedium,
                    )

                    Text(stringResource(R.string.sync_account_inbox), style = MaterialTheme.typography.titleSmall)
                    Text(
                        stringResource(
                            if (info.inbox.hasToken) R.string.sync_account_inbox_on else R.string.sync_account_inbox_off
                        ),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    inboxToken?.let { token ->
                        Text(token, style = MaterialTheme.typography.bodySmall)
                        Text(
                            stringResource(R.string.sync_account_inbox_shown_once),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        OutlinedButton(
                            onClick = { clipboard.setText(AnnotatedString(token)) },
                            modifier = Modifier.fillMaxWidth(),
                        ) { Text(stringResource(R.string.sync_account_inbox_copy)) }
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        OutlinedButton(
                            enabled = !isWorking,
                            onClick = {
                                working {
                                    inboxToken = appState.generateInboxToken(sessionId)
                                    refresh(sessionId)
                                }
                            },
                            modifier = Modifier.weight(1f),
                        ) { Text(stringResource(R.string.sync_account_inbox_new)) }
                        if (info.inbox.hasToken) {
                            OutlinedButton(
                                enabled = !isWorking,
                                onClick = {
                                    working {
                                        appState.revokeInboxToken(sessionId)
                                        inboxToken = null
                                        refresh(sessionId)
                                    }
                                },
                                modifier = Modifier.weight(1f),
                            ) { Text(stringResource(R.string.sync_account_inbox_revoke)) }
                        }
                    }

                    OutlinedButton(
                        enabled = !isWorking,
                        onClick = { showDeleteNotes = true },
                        colors = ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.error),
                        modifier = Modifier.fillMaxWidth(),
                    ) { Text(stringResource(R.string.sync_account_delete_notes)) }

                    Text(stringResource(R.string.sync_account_close), style = MaterialTheme.typography.titleSmall)
                    AccountDeletionMode.entries.forEach { mode ->
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier
                                .fillMaxWidth()
                                .selectable(selected = deletionMode == mode, onClick = { deletionMode = mode }),
                        ) {
                            RadioButton(selected = deletionMode == mode, onClick = null)
                            Spacer(Modifier.padding(start = 8.dp))
                            Text(
                                stringResource(
                                    when (mode) {
                                        AccountDeletionMode.DEACTIVATE -> R.string.sync_account_mode_deactivate
                                        AccountDeletionMode.DELETE -> R.string.sync_account_mode_delete
                                    }
                                ),
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        }
                    }
                    Button(
                        enabled = !isWorking,
                        onClick = { showDeleteAccount = true },
                        colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.error),
                        modifier = Modifier.fillMaxWidth(),
                    ) { Text(stringResource(R.string.sync_account_close_action)) }
                }

                error?.let {
                    Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
                }
            }
            Spacer(Modifier.height(32.dp))
        }
    }

    if (showDisconnect) {
        ConfirmDialog(
            title = stringResource(R.string.sync_account_disconnect),
            message = stringResource(R.string.sync_account_disconnect_message),
            confirmText = stringResource(R.string.sync_account_disconnect_confirm),
            onConfirm = {
                showDisconnect = false
                val stillRegistered = context.getString(R.string.sync_account_still_registered)
                scope.launch {
                    val revoked = appState.disconnectSync()
                    if (!revoked) appState.showMessage(stillRegistered)
                    onBack()
                }
            },
            onDismiss = { showDisconnect = false },
        )
    }

    if (showDeleteNotes) {
        ConfirmDialog(
            title = stringResource(R.string.sync_account_delete_notes),
            message = stringResource(R.string.sync_account_delete_notes_message),
            confirmText = stringResource(R.string.action_delete),
            onConfirm = {
                showDeleteNotes = false
                val id = session ?: return@ConfirmDialog
                working {
                    appState.deleteServerNotes(id)
                    refresh(id)
                }
            },
            onDismiss = { showDeleteNotes = false },
        )
    }

    if (showDeleteAccount) {
        ConfirmDialog(
            title = stringResource(R.string.sync_account_close),
            message = stringResource(
                when (deletionMode) {
                    AccountDeletionMode.DEACTIVATE -> R.string.sync_account_deactivate_message
                    AccountDeletionMode.DELETE -> R.string.sync_account_delete_message
                }
            ),
            confirmText = stringResource(R.string.sync_account_close_action),
            onConfirm = {
                showDeleteAccount = false
                val id = session ?: return@ConfirmDialog
                working {
                    appState.closeAccount(id, deletionMode)
                    onBack()
                }
            },
            onDismiss = { showDeleteAccount = false },
        )
    }
}

private fun formatTime(instant: java.time.Instant): String =
    java.time.format.DateTimeFormatter.ofLocalizedDateTime(java.time.format.FormatStyle.SHORT)
        .withZone(java.time.ZoneId.systemDefault())
        .format(instant)
