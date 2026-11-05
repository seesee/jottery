package com.jottery.android.ui.screen

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.jottery.android.R
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Create an account on a server. New accounts wait for an administrator, so this
 * stays open and watches for approval, then hands the details to the form that
 * registers this device.
 */
@Composable
fun CreateAccountDialog(
    appState: AppState,
    initialServer: String,
    onApproved: (server: String, email: String, password: String) -> Unit,
    onDismiss: () -> Unit,
) {
    var server by remember { mutableStateOf(initialServer) }
    var email by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var confirmation by remember { mutableStateOf("") }
    var error by remember { mutableStateOf<String?>(null) }
    var isWorking by remember { mutableStateOf(false) }
    var isWaiting by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val mismatch = stringResource(R.string.password_mismatch)

    suspend fun checkApproval() {
        try {
            if (appState.isAccountApproved(server.trim().trimEnd('/'), email.trim(), password)) {
                onApproved(server.trim().trimEnd('/'), email.trim(), password)
            }
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            error = appState.describeAccountFailure(e)
        }
    }

    if (isWaiting) {
        LaunchedEffect(Unit) {
            repeat(MAX_APPROVAL_CHECKS) {
                delay(APPROVAL_CHECK_MILLIS)
                checkApproval()
            }
        }
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.create_account_title)) },
        text = {
            Column(
                verticalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.verticalScroll(rememberScrollState()),
            ) {
                if (isWaiting) {
                    Text(stringResource(R.string.create_account_waiting))
                } else {
                    OutlinedTextField(
                        value = server,
                        onValueChange = { server = it },
                        label = { Text(stringResource(R.string.connect_server_url)) },
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
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
                        keyboardOptions = PASSWORD_KEYBOARD,
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                    OutlinedTextField(
                        value = confirmation,
                        onValueChange = { confirmation = it },
                        label = { Text(stringResource(R.string.create_account_confirm)) },
                        visualTransformation = PasswordVisualTransformation(),
                        keyboardOptions = PASSWORD_KEYBOARD,
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                    Text(
                        stringResource(R.string.create_account_help),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                error?.let {
                    Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                }
            }
        },
        confirmButton = {
            if (isWaiting) {
                TextButton(onClick = { scope.launch { checkApproval() } }) {
                    Text(stringResource(R.string.create_account_check))
                }
            } else {
                TextButton(
                    enabled = !isWorking && server.isNotBlank() && email.isNotBlank() && password.isNotEmpty(),
                    onClick = {
                        if (password != confirmation) {
                            error = mismatch
                            return@TextButton
                        }
                        error = null
                        isWorking = true
                        scope.launch {
                            try {
                                appState.createAccount(server.trim().trimEnd('/'), email.trim(), password)
                                isWaiting = true
                                checkApproval()
                            } catch (e: kotlinx.coroutines.CancellationException) {
                                throw e
                            } catch (e: Exception) {
                                error = appState.describeAccountFailure(e)
                            } finally {
                                isWorking = false
                            }
                        }
                    },
                ) { Text(stringResource(R.string.create_account_action)) }
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.action_cancel)) }
        },
    )
}

private val PASSWORD_KEYBOARD = KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false)

private const val APPROVAL_CHECK_MILLIS = 15_000L

/** An hour. After that it's the button, so a forgotten dialog doesn't poll for ever. */
private const val MAX_APPROVAL_CHECKS = 240
