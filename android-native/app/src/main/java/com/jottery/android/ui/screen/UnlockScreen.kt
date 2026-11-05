package com.jottery.android.ui.screen

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
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
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
import com.jottery.android.ui.component.ConfirmDialog
import com.jottery.android.ui.component.fragmentActivity
import com.jottery.android.util.BiometricHelper
import com.jottery.android.R
import com.jottery.android.viewmodel.AppState
import com.jottery.android.viewmodel.AppStateError
import kotlinx.coroutines.launch

@Composable
fun UnlockScreen(
    appState: AppState,
    onUnlocked: () -> Unit,
) {
    var password by remember { mutableStateOf("") }
    var passwordVisible by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var isUnlocking by remember { mutableStateOf(false) }
    var failedAttempts by remember { mutableIntStateOf(0) }
    var showWipeConfirm by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val context = LocalContext.current

    fun attemptUnlock() {
        if (password.isEmpty()) {
            error = context.getString(R.string.setup_password_is_required)
            return
        }
        isUnlocking = true
        error = null
        scope.launch {
            try {
                val success = appState.unlock(password)
                isUnlocking = false
                if (success) {
                    failedAttempts = 0
                    onUnlocked()
                } else {
                    failedAttempts++
                    error = context.getString(R.string.unlock_incorrect)
                    password = ""
                }
            } catch (_: AppStateError.WrongPassword) {
                isUnlocking = false
                failedAttempts++
                error = context.getString(R.string.unlock_incorrect)
                password = ""
            } catch (e: Exception) {
                // Not a password problem, so it must not count towards the wipe offer
                isUnlocking = false
                error = context.getString(R.string.unlock_failed, e.message ?: e.javaClass.simpleName)
            }
        }
    }

    val biometricEnabled by appState.biometricEnabled.collectAsState()
    val keyChangedMessage = stringResource(R.string.biometric_key_changed)

    fun attemptBiometric() {
        val activity = context.fragmentActivity() ?: return
        val cipher = appState.biometricUnlockCipher()
        if (cipher == null) {
            // Enrolments changed, so the key is gone. The password still works.
            error = keyChangedMessage
            return
        }
        BiometricHelper.authenticate(
            activity = activity,
            cipher = cipher,
            title = context.getString(R.string.biometric_unlock_title),
            subtitle = context.getString(R.string.biometric_unlock_subtitle),
            negativeButton = context.getString(R.string.biometric_use_password),
            onSuccess = { authorised ->
                isUnlocking = true
                scope.launch {
                    try {
                        appState.unlockWithBiometric(authorised)
                        onUnlocked()
                    } catch (_: AppStateError.BiometricUnavailable) {
                        error = keyChangedMessage
                    } catch (e: Exception) {
                        error = context.getString(R.string.unlock_failed, e.message ?: e.javaClass.simpleName)
                    } finally {
                        isUnlocking = false
                    }
                }
            },
            onError = { message -> error = message },
        )
    }

    // Offered straight away, as on iOS. Cancelling leaves the password field.
    LaunchedEffect(Unit) {
        if (biometricEnabled && BiometricHelper.isBiometricAvailable(context)) attemptBiometric()
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

        Text(
            text = stringResource(R.string.unlock_title),
            style = MaterialTheme.typography.bodyMedium,
            textAlign = TextAlign.Center,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        Spacer(modifier = Modifier.height(32.dp))

        OutlinedTextField(
            value = password,
            onValueChange = { password = it; error = null },
            label = { Text(stringResource(R.string.setup_password)) },
            singleLine = true,
            visualTransformation = if (passwordVisible) VisualTransformation.None
                else PasswordVisualTransformation(),
            keyboardOptions = KeyboardOptions(
                keyboardType = KeyboardType.Password,
                imeAction = ImeAction.Done,
            ),
            keyboardActions = KeyboardActions(
                onDone = { attemptUnlock() }
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

        if (error != null) {
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = error!!,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodySmall,
            )
        }

        Spacer(modifier = Modifier.height(24.dp))

        Button(
            onClick = { attemptUnlock() },
            enabled = !isUnlocking,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text(if (isUnlocking) stringResource(R.string.setup_unlocking) else stringResource(R.string.unlock_button))
        }

        if (biometricEnabled && BiometricHelper.isBiometricAvailable(context)) {
            Spacer(modifier = Modifier.height(8.dp))
            OutlinedButton(
                onClick = { attemptBiometric() },
                enabled = !isUnlocking,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(stringResource(R.string.biometric_unlock_button))
            }
        }

        // Wipe button after 3 failed attempts
        if (failedAttempts >= 3) {
            Spacer(modifier = Modifier.height(24.dp))

            Text(
                text = stringResource(R.string.unlock_forgotten_your_password),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            Spacer(modifier = Modifier.height(8.dp))

            OutlinedButton(
                onClick = { showWipeConfirm = true },
                modifier = Modifier.fillMaxWidth(),
                colors = ButtonDefaults.outlinedButtonColors(
                    contentColor = MaterialTheme.colorScheme.error,
                ),
            ) {
                Text(stringResource(R.string.unlock_wipe_all_data_start_fresh))
            }
        }
    }

    if (showWipeConfirm) {
        ConfirmDialog(
            title = stringResource(R.string.unlock_wipe_all_data),
            message = stringResource(R.string.unlock_this_will_permanently_delete_all),
            confirmText = stringResource(R.string.settings_wipe_everything),
            onConfirm = {
                showWipeConfirm = false
                appState.wipeAllData()
            },
            onDismiss = { showWipeConfirm = false },
        )
    }
}
