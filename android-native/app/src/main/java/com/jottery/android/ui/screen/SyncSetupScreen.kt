package com.jottery.android.ui.screen

import android.os.Build
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.jottery.android.R
import com.jottery.android.viewmodel.AppState

private val passwordKeyboard = KeyboardOptions(
    keyboardType = KeyboardType.Password,
    autoCorrectEnabled = false,
)

/**
 * Connect the open vault to a server. Goes through the same key checks as first
 * setup: sync starts only once this vault and the account agree on a key.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SyncSetupScreen(
    appState: AppState,
    onBack: () -> Unit,
) {
    var mode by rememberSaveable { mutableStateOf(SyncSetupMode.Register) }
    var serverUrl by rememberSaveable { mutableStateOf("") }
    var email by rememberSaveable { mutableStateOf("") }
    var accountPassword by remember { mutableStateOf("") }
    var notesPassword by remember { mutableStateOf("") }
    var deviceName by rememberSaveable { mutableStateOf(Build.MODEL ?: "") }
    var credentialString by remember { mutableStateOf("") }
    var showCreateAccount by rememberSaveable { mutableStateOf(false) }

    val connectState by appState.connectState.collectAsState()

    val isBusy = connectState is AppState.ConnectState.Working
    val failure = (connectState as? AppState.ConnectState.Failed)?.message
    val isDone = connectState is AppState.ConnectState.Done

    // A connect in progress carries on in AppState. Anything else ends with the
    // screen, and an earlier result mustn't greet the next visit.
    DisposableEffect(Unit) {
        if (appState.connectState.value !is AppState.ConnectState.Working) appState.clearConnectState()
        onDispose {
            when (appState.connectState.value) {
                AppState.ConnectState.Working -> Unit
                AppState.ConnectState.NeedsMerge -> appState.cancelMerge()
                else -> appState.clearConnectState()
            }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.connect_title)) },
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
                .consumeWindowInsets(padding)
                .imePadding()
                .padding(horizontal = 16.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
                SegmentedButton(
                    selected = mode == SyncSetupMode.Register,
                    onClick = { mode = SyncSetupMode.Register },
                    shape = SegmentedButtonDefaults.itemShape(index = 0, count = 2),
                ) { Text(stringResource(R.string.connect_register)) }
                SegmentedButton(
                    selected = mode == SyncSetupMode.Import,
                    onClick = { mode = SyncSetupMode.Import },
                    shape = SegmentedButtonDefaults.itemShape(index = 1, count = 2),
                ) { Text(stringResource(R.string.connect_import)) }
            }

            when (mode) {
                SyncSetupMode.Register -> {
                    OutlinedTextField(
                        value = serverUrl,
                        onValueChange = { serverUrl = it },
                        label = { Text(stringResource(R.string.connect_server_url)) },
                        placeholder = { Text("https://sync.example.com") },
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                    )
                    OutlinedTextField(
                        value = email,
                        onValueChange = { email = it },
                        label = { Text(stringResource(R.string.connect_email)) },
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                    )
                    OutlinedTextField(
                        value = accountPassword,
                        onValueChange = { accountPassword = it },
                        label = { Text(stringResource(R.string.connect_account_password)) },
                        visualTransformation = PasswordVisualTransformation(),
                        keyboardOptions = passwordKeyboard,
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                    )
                    TextButton(onClick = { showCreateAccount = true }) {
                        Text(stringResource(R.string.create_account_title))
                    }
                }

                SyncSetupMode.Import -> {
                    OutlinedTextField(
                        value = credentialString,
                        onValueChange = { credentialString = it },
                        label = { Text(stringResource(R.string.connect_credentials)) },
                        supportingText = { Text(stringResource(R.string.connect_credentials_help)) },
                        visualTransformation = PasswordVisualTransformation(),
                        keyboardOptions = passwordKeyboard,
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                    )
                }
            }

            OutlinedTextField(
                value = notesPassword,
                onValueChange = { notesPassword = it },
                label = { Text(stringResource(R.string.connect_notes_password)) },
                supportingText = { Text(stringResource(R.string.connect_notes_password_help)) },
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = passwordKeyboard,
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )
            OutlinedTextField(
                value = deviceName,
                onValueChange = { deviceName = it },
                label = { Text(stringResource(R.string.connect_device_name)) },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )

            if (mode == SyncSetupMode.Register) {
                Text(
                    text = stringResource(R.string.connect_register_help),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }

            failure?.let { error ->
                Text(
                    text = error,
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
            if (isDone) {
                Text(
                    text = stringResource(R.string.connect_done),
                    color = MaterialTheme.colorScheme.primary,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }

            Button(
                onClick = {
                    appState.clearConnectState()
                    when (mode) {
                        SyncSetupMode.Register -> {
                            val password = notesPassword
                            appState.beginRegistration(
                                serverUrl = serverUrl.trim().trimEnd('/'),
                                email = email.trim(),
                                password = accountPassword,
                                deviceName = deviceName.trim(),
                                onRegistered = { appState.connectExistingVault(it, password) },
                            )
                        }
                        SyncSetupMode.Import -> appState.importIntoExistingVault(
                            credentialString = credentialString,
                            password = notesPassword,
                            deviceName = deviceName.trim(),
                        )
                    }
                },
                modifier = Modifier.fillMaxWidth(),
                enabled = !isBusy && !isDone && notesPassword.isNotEmpty() && deviceName.isNotBlank() && when (mode) {
                    SyncSetupMode.Register -> serverUrl.isNotBlank() && email.isNotBlank() && accountPassword.isNotEmpty()
                    SyncSetupMode.Import -> credentialString.isNotBlank()
                },
            ) {
                if (isBusy) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(20.dp),
                        strokeWidth = 2.dp,
                    )
                    Spacer(Modifier.width(8.dp))
                }
                Text(
                    stringResource(
                        when (mode) {
                            SyncSetupMode.Register -> R.string.connect_button_register
                            SyncSetupMode.Import -> R.string.connect_button_import
                        }
                    )
                )
            }

            Spacer(Modifier.height(32.dp))
        }
    }

    if (showCreateAccount) {
        CreateAccountDialog(
            appState = appState,
            initialServer = serverUrl,
            onApproved = { server, newEmail, password ->
                showCreateAccount = false
                serverUrl = server
                email = newEmail
                accountPassword = password
            },
            onDismiss = { showCreateAccount = false },
        )
    }

    if (connectState is AppState.ConnectState.NeedsMerge) {
        AlertDialog(
            onDismissRequest = { appState.cancelMerge() },
            title = { Text(stringResource(R.string.connect_merge_title)) },
            text = { Text(stringResource(R.string.connect_merge_message)) },
            confirmButton = {
                TextButton(onClick = { appState.confirmMerge() }) {
                    Text(stringResource(R.string.connect_merge_confirm))
                }
            },
            dismissButton = {
                TextButton(onClick = { appState.cancelMerge() }) {
                    Text(stringResource(R.string.cancel))
                }
            },
        )
    }
}

private enum class SyncSetupMode { Register, Import }
