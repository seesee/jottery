package com.jottery.android.ui.screen

import android.net.Uri
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.jottery.android.R
import com.jottery.android.service.backup.BackupSource
import com.jottery.android.viewmodel.AppState

private enum class SetupMode { NEW_VAULT, REGISTER, IMPORT, RESTORE }

@Composable
fun SetupScreen(
    appState: AppState,
) {
    var mode by rememberSaveable { mutableStateOf(SetupMode.NEW_VAULT) }
    var password by remember { mutableStateOf("") }
    var confirmPassword by remember { mutableStateOf("") }
    var passwordVisible by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var isCreating by remember { mutableStateOf(false) }

    // Register fields
    var serverUrl by rememberSaveable { mutableStateOf("") }
    var email by rememberSaveable { mutableStateOf("") }
    var serverPassword by remember { mutableStateOf("") }
    var encryptionPassword by remember { mutableStateOf("") }

    // Owned by AppState, so a rotation can't lose a registered device or an
    // action in flight, and leaving this screen can't cancel it
    val pendingConnection by appState.pendingConnection.collectAsState()
    val connectState by appState.connectState.collectAsState()
    val isRegistered = pendingConnection != null
    val isConnecting = connectState is AppState.ConnectState.Working
    var showCreateAccount by rememberSaveable { mutableStateOf(false) }
    val context = LocalContext.current
    val connectError = (connectState as? AppState.ConnectState.Failed)?.message
    var confirmationPassword by remember { mutableStateOf("") }
    var askForConfirmation by remember { mutableStateOf(false) }
    LaunchedEffect(connectState) {
        if (connectState is AppState.ConnectState.ConfirmNewPassword) askForConfirmation = true
    }

    // Import fields
    var credentialString by remember { mutableStateOf("") }
    var importPassword by remember { mutableStateOf("") }
    var deviceName by rememberSaveable { mutableStateOf(Build.MODEL) }

    fun submitNewVault() {
        when {
            password.isEmpty() -> error = context.getString(R.string.setup_password_is_required)
            password.length < AppState.MIN_NOTES_PASSWORD_LENGTH -> error = context.getString(R.string.setup_password_must_be_at_least)
            password != confirmPassword -> error = context.getString(R.string.setup_passwords_do_not_match)
            else -> appState.createNewVault(password)
        }
    }

    // Restore fields
    var backupUri by rememberSaveable { mutableStateOf<Uri?>(null) }
    var backupPassword by remember { mutableStateOf("") }
    val backupPicker = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocument()
    ) { uri ->
        if (uri != null) {
            backupUri = uri
            error = null
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .systemBarsPadding()
            .imePadding()
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Icon(
            imageVector = Icons.Default.Lock,
            contentDescription = null,
            modifier = Modifier.size(64.dp),
            tint = MaterialTheme.colorScheme.primary,
        )

        Spacer(modifier = Modifier.height(16.dp))

        Text(
            text = "Jottery",
            style = MaterialTheme.typography.headlineMedium,
        )

        Spacer(modifier = Modifier.height(8.dp))

        val subtitle = when (mode) {
            SetupMode.NEW_VAULT -> stringResource(R.string.setup_subtitle)
            SetupMode.REGISTER -> stringResource(R.string.setup_register_as_the_first_device)
            SetupMode.IMPORT -> stringResource(R.string.setup_import_credentials_from_an_existing)
            SetupMode.RESTORE -> stringResource(R.string.setup_restore_subtitle)
        }
        Text(
            text = subtitle,
            style = MaterialTheme.typography.bodyMedium,
            textAlign = TextAlign.Center,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        Spacer(modifier = Modifier.height(32.dp))

        when (mode) {
            // ── New Vault ────────────────────────────────────────────
            SetupMode.NEW_VAULT -> {
                OutlinedTextField(
                    value = password,
                    onValueChange = { password = it; error = null },
                    label = { Text(stringResource(R.string.setup_password)) },
                    singleLine = true,
                    visualTransformation = if (passwordVisible) VisualTransformation.None
                        else PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Password,
                        imeAction = ImeAction.Next,
                    ),
                    trailingIcon = {
                        IconButton(onClick = { passwordVisible = !passwordVisible }) {
                            Icon(
                                if (passwordVisible) Icons.Default.VisibilityOff
                                else Icons.Default.Visibility,
                                contentDescription = stringResource(R.string.setup_toggle_password_visibility),
                            )
                        }
                    },
                    modifier = Modifier.fillMaxWidth(),
                )

                Spacer(modifier = Modifier.height(12.dp))

                OutlinedTextField(
                    value = confirmPassword,
                    onValueChange = { confirmPassword = it; error = null },
                    label = { Text(stringResource(R.string.setup_confirm_password)) },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Password,
                        imeAction = ImeAction.Done,
                    ),
                    keyboardActions = KeyboardActions(onDone = { submitNewVault() }),
                    modifier = Modifier.fillMaxWidth(),
                )

                ErrorText(error ?: connectError)

                Spacer(modifier = Modifier.height(24.dp))

                Button(
                    onClick = { submitNewVault() },
                    enabled = !isConnecting,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(if (isConnecting) stringResource(R.string.setup_creating_vault) else stringResource(R.string.setup_title))
                }

                Spacer(modifier = Modifier.height(12.dp))

                OutlinedButton(
                    onClick = { mode = SetupMode.REGISTER; error = null },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(stringResource(R.string.setup_connect_server))
                }

                Spacer(modifier = Modifier.height(8.dp))

                OutlinedButton(
                    onClick = { mode = SetupMode.IMPORT; error = null },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(stringResource(R.string.setup_import_from_another_device))
                }

                Spacer(modifier = Modifier.height(8.dp))

                OutlinedButton(
                    onClick = { mode = SetupMode.RESTORE; error = null },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(stringResource(R.string.setup_restore_button))
                }
            }

            // ── Restore (fresh install, from a backup file) ─────────
            SetupMode.RESTORE -> {
                OutlinedButton(
                    onClick = { backupPicker.launch(arrayOf("*/*")) },
                    enabled = !isCreating,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(
                        stringResource(
                            if (backupUri == null) R.string.setup_restore_choose
                            else R.string.setup_restore_chosen
                        )
                    )
                }

                Spacer(modifier = Modifier.height(12.dp))

                OutlinedTextField(
                    value = backupPassword,
                    onValueChange = { backupPassword = it; error = null },
                    label = { Text(stringResource(R.string.backup_password_label)) },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                    enabled = !isCreating,
                    modifier = Modifier.fillMaxWidth(),
                )

                ErrorText(error)

                Spacer(modifier = Modifier.height(24.dp))

                Button(
                    onClick = {
                        val uri = backupUri ?: return@Button
                        isCreating = true
                        error = null
                        val source = BackupSource {
                            context.contentResolver.openInputStream(uri)
                                ?: throw java.io.FileNotFoundException()
                        }
                        appState.restoreAsNewVault(source, backupPassword) { failure ->
                            error = context.backupErrorText(failure)
                            isCreating = false
                        }
                    },
                    enabled = !isCreating && backupUri != null && backupPassword.isNotEmpty(),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(
                        stringResource(
                            if (isCreating) R.string.setup_restore_working else R.string.setup_restore_action
                        )
                    )
                }

                Spacer(modifier = Modifier.height(12.dp))

                OutlinedButton(
                    onClick = { mode = SetupMode.NEW_VAULT; error = null },
                    enabled = !isCreating,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(stringResource(R.string.back))
                }
            }

            // ── Register (first device) ──────────────────────────────
            SetupMode.REGISTER -> {
                if (!isRegistered) {
                    // Step 1: Server registration
                    OutlinedTextField(
                        value = serverUrl,
                        onValueChange = { serverUrl = it; error = null },
                        label = { Text(stringResource(R.string.settings_sync_endpoint)) },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )

                    Spacer(modifier = Modifier.height(12.dp))

                    OutlinedTextField(
                        value = email,
                        onValueChange = { email = it; error = null },
                        label = { Text(stringResource(R.string.connect_email)) },
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                        modifier = Modifier.fillMaxWidth(),
                    )

                    Spacer(modifier = Modifier.height(12.dp))

                    OutlinedTextField(
                        value = serverPassword,
                        onValueChange = { serverPassword = it; error = null },
                        label = { Text(stringResource(R.string.setup_password)) },
                        singleLine = true,
                        visualTransformation = PasswordVisualTransformation(),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false),
                        modifier = Modifier.fillMaxWidth(),
                    )

                    TextButton(onClick = { showCreateAccount = true }) {
                        Text(stringResource(R.string.create_account_title))
                    }

                    ErrorText(error ?: connectError)

                    Spacer(modifier = Modifier.height(24.dp))

                    Button(
                        onClick = {
                            when {
                                serverUrl.isBlank() -> error = context.getString(R.string.setup_server_url_is_required)
                                email.isBlank() -> error = context.getString(R.string.setup_email_is_required)
                                serverPassword.isEmpty() -> error = context.getString(R.string.setup_password_is_required)
                                serverPassword.length < 8 -> error = context.getString(R.string.setup_password_must_be_at_least)
                                else -> {
                                    error = null
                                    appState.beginRegistration(
                                        serverUrl = serverUrl.trim().trimEnd('/'),
                                        email = email.trim(),
                                        password = serverPassword,
                                        deviceName = Build.MODEL,
                                    )
                                }
                            }
                        },
                        enabled = !isConnecting,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(if (isConnecting) stringResource(R.string.setup_registering) else stringResource(R.string.setup_register_device))
                    }

                    Spacer(modifier = Modifier.height(12.dp))

                    OutlinedButton(
                        onClick = { mode = SetupMode.NEW_VAULT; error = null },
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(stringResource(R.string.back))
                    }
                } else {
                    // Step 2: Encryption password
                    Text(
                        text = stringResource(R.string.setup_connected),
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.primary,
                    )

                    Spacer(modifier = Modifier.height(16.dp))

                    OutlinedTextField(
                        value = encryptionPassword,
                        onValueChange = { encryptionPassword = it; appState.clearConnectState() },
                        label = { Text(stringResource(R.string.setup_encryption_password)) },
                        supportingText = { Text(stringResource(R.string.setup_enter_the_same_encryption_password)) },
                        singleLine = true,
                        visualTransformation = PasswordVisualTransformation(),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                        enabled = !isConnecting,
                        modifier = Modifier.fillMaxWidth(),
                    )

                    if (askForConfirmation) {
                        Spacer(modifier = Modifier.height(12.dp))
                        OutlinedTextField(
                            value = confirmationPassword,
                            onValueChange = { confirmationPassword = it },
                            label = { Text(stringResource(R.string.connect_confirm_password)) },
                            supportingText = { Text(stringResource(R.string.connect_confirm_password_help)) },
                            singleLine = true,
                            visualTransformation = PasswordVisualTransformation(),
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                            enabled = !isConnecting,
                            modifier = Modifier.fillMaxWidth(),
                        )
                    }

                    ErrorText(connectError)

                    Spacer(modifier = Modifier.height(24.dp))

                    Button(
                        onClick = {
                            appState.connectNewVault(
                                encryptionPassword,
                                confirmationPassword.takeIf { askForConfirmation },
                            )
                        },
                        enabled = !isConnecting && encryptionPassword.isNotEmpty(),
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(if (isConnecting) stringResource(R.string.setup_unlocking) else stringResource(R.string.setup_unlock_sync))
                    }

                    Spacer(modifier = Modifier.height(12.dp))

                    OutlinedButton(
                        onClick = {
                            appState.abandonPendingConnection()
                            appState.clearConnectState()
                            mode = SetupMode.NEW_VAULT
                        },
                        enabled = !isConnecting,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(stringResource(R.string.back))
                    }
                }
            }

            // ── Import Credentials (second+ device) ─────────────────
            SetupMode.IMPORT -> {
                OutlinedTextField(
                    value = credentialString,
                    onValueChange = { credentialString = it; error = null },
                    label = { Text(stringResource(R.string.setup_credential_string)) },
                    placeholder = { Text(stringResource(R.string.setup_jottery_v1_or_base64_json)) },
                    minLines = 3,
                    maxLines = 5,
                    modifier = Modifier.fillMaxWidth(),
                )

                Spacer(modifier = Modifier.height(12.dp))

                OutlinedTextField(
                    value = importPassword,
                    onValueChange = { importPassword = it; error = null },
                    label = { Text(stringResource(R.string.setup_encryption_password)) },
                    supportingText = { Text(stringResource(R.string.setup_same_password_used_on_your)) },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                    modifier = Modifier.fillMaxWidth(),
                )

                Spacer(modifier = Modifier.height(12.dp))

                OutlinedTextField(
                    value = deviceName,
                    onValueChange = { deviceName = it; error = null },
                    label = { Text(stringResource(R.string.setup_device_name)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )

                ErrorText(error ?: connectError)

                Spacer(modifier = Modifier.height(24.dp))

                Button(
                    onClick = {
                        when {
                            credentialString.isBlank() -> error = context.getString(R.string.setup_credential_string_is_required)
                            importPassword.isEmpty() -> error = context.getString(R.string.setup_password_is_required)
                            deviceName.isBlank() -> error = context.getString(R.string.setup_device_name_is_required)
                            else -> {
                                error = null
                                appState.importFromDevice(
                                    credentialString = credentialString,
                                    password = importPassword,
                                    deviceName = deviceName.trim(),
                                )
                            }
                        }
                    },
                    enabled = !isConnecting,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(if (isConnecting) stringResource(R.string.setup_importing) else stringResource(R.string.setup_import_sync))
                }

                Spacer(modifier = Modifier.height(12.dp))

                OutlinedButton(
                    onClick = { mode = SetupMode.NEW_VAULT; error = null },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(stringResource(R.string.back))
                }
            }
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
                serverPassword = password
            },
            onDismiss = { showCreateAccount = false },
        )
    }
}

@Composable
private fun ErrorText(error: String?) {
    if (error != null) {
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = error,
            color = MaterialTheme.colorScheme.error,
            style = MaterialTheme.typography.bodySmall,
        )
    }
}
