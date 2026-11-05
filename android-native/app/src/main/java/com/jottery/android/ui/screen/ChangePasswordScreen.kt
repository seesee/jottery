package com.jottery.android.ui.screen

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.jottery.android.R
import com.jottery.android.viewmodel.AppState
import com.jottery.android.viewmodel.AppStateError
import kotlinx.coroutines.launch

private val passwordKeyboard = KeyboardOptions(
    keyboardType = KeyboardType.Password,
    autoCorrectEnabled = false,
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChangePasswordScreen(
    appState: AppState,
    onBack: () -> Unit,
) {
    var currentPassword by remember { mutableStateOf("") }
    var newPassword by remember { mutableStateOf("") }
    var confirmPassword by remember { mutableStateOf("") }
    var isChanging by remember { mutableStateOf(false) }
    var errorMessage by remember { mutableStateOf<String?>(null) }
    var successMessage by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    val tooShortMessage = stringResource(R.string.password_too_short, AppState.MIN_NOTES_PASSWORD_LENGTH)
    val mismatchMessage = stringResource(R.string.password_mismatch)
    val doneMessage = stringResource(R.string.change_password_done)
    val failedMessage = stringResource(R.string.change_password_failed)
    val wrongPasswordMessage = stringResource(R.string.unlock_wrong_password)

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.settings_change_password)) },
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
                .consumeWindowInsets(padding)
                .imePadding()
                .verticalScroll(rememberScrollState())
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(
                stringResource(R.string.change_password_detail),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                stringResource(R.string.change_password_old_backups),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            OutlinedTextField(
                value = currentPassword,
                onValueChange = { currentPassword = it; errorMessage = null },
                label = { Text(stringResource(R.string.change_password_current_password)) },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = passwordKeyboard,
            )

            OutlinedTextField(
                value = newPassword,
                onValueChange = { newPassword = it; errorMessage = null },
                label = { Text(stringResource(R.string.change_password_new_password)) },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = passwordKeyboard,
            )

            OutlinedTextField(
                value = confirmPassword,
                onValueChange = { confirmPassword = it; errorMessage = null },
                label = { Text(stringResource(R.string.change_password_confirm_new_password)) },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = passwordKeyboard,
            )

            errorMessage?.let {
                Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium)
            }

            successMessage?.let {
                Text(it, color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.bodyMedium)
            }

            Button(
                onClick = {
                    if (newPassword != confirmPassword) {
                        errorMessage = mismatchMessage
                        return@Button
                    }
                    if (newPassword.length < AppState.MIN_NOTES_PASSWORD_LENGTH) {
                        errorMessage = tooShortMessage
                        return@Button
                    }
                    isChanging = true
                    errorMessage = null
                    scope.launch {
                        try {
                            appState.changePassword(currentPassword, newPassword)
                            successMessage = doneMessage
                            currentPassword = ""
                            newPassword = ""
                            confirmPassword = ""
                        } catch (_: AppStateError.WrongPassword) {
                            errorMessage = wrongPasswordMessage
                        } catch (e: kotlinx.coroutines.CancellationException) {
                            throw e
                        } catch (e: Exception) {
                            errorMessage = failedMessage
                        } finally {
                            isChanging = false
                        }
                    }
                },
                modifier = Modifier.fillMaxWidth(),
                enabled = !isChanging && currentPassword.isNotBlank() &&
                    newPassword.isNotBlank() && confirmPassword.isNotBlank(),
            ) {
                if (isChanging) {
                    CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
                    Spacer(Modifier.width(8.dp))
                }
                Text(stringResource(R.string.settings_change_password))
            }
        }
    }
}
