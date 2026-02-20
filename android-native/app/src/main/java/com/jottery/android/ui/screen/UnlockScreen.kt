package com.jottery.android.ui.screen

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Fingerprint
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.jottery.android.ui.component.ConfirmDialog
import com.jottery.android.viewmodel.AppState
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

    fun attemptUnlock() {
        if (password.isEmpty()) {
            error = "Password is required"
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
                    error = "Incorrect password"
                    password = ""
                }
            } catch (_: Exception) {
                isUnlocking = false
                failedAttempts++
                error = "Incorrect password"
                password = ""
            }
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
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
            text = "Enter your password to unlock",
            style = MaterialTheme.typography.bodyMedium,
            textAlign = TextAlign.Center,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        Spacer(modifier = Modifier.height(32.dp))

        OutlinedTextField(
            value = password,
            onValueChange = { password = it; error = null },
            label = { Text("Password") },
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
                        contentDescription = "Toggle password visibility",
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
            Text(if (isUnlocking) "Unlocking\u2026" else "Unlock")
        }

        // Biometric button placeholder
        Spacer(modifier = Modifier.height(12.dp))

        OutlinedButton(
            onClick = {
                // TODO: Biometric unlock
            },
            modifier = Modifier.fillMaxWidth(),
        ) {
            Icon(
                Icons.Default.Fingerprint,
                contentDescription = null,
                modifier = Modifier.size(20.dp),
            )
            Spacer(modifier = Modifier.padding(4.dp))
            Text("Unlock with Biometrics")
        }

        // Wipe button after 3 failed attempts
        if (failedAttempts >= 3) {
            Spacer(modifier = Modifier.height(24.dp))

            Text(
                text = "Forgotten your password?",
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
                Text("Wipe All Data & Start Fresh")
            }
        }
    }

    if (showWipeConfirm) {
        ConfirmDialog(
            title = "Wipe All Data?",
            message = "This will permanently delete all notes, settings, and encryption keys. This action cannot be undone.",
            confirmText = "Wipe Everything",
            onConfirm = {
                showWipeConfirm = false
                appState.wipeAllData()
            },
            onDismiss = { showWipeConfirm = false },
        )
    }
}
