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
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
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
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.launch
import android.os.Build

@Composable
fun SetupScreen(
    appState: AppState,
    onVaultCreated: () -> Unit,
) {
    var password by remember { mutableStateOf("") }
    var confirmPassword by remember { mutableStateOf("") }
    var passwordVisible by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var isCreating by remember { mutableStateOf(false) }
    var showServerConnect by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    // Server connect fields
    var serverUrl by remember { mutableStateOf("") }
    var email by remember { mutableStateOf("") }
    var serverPassword by remember { mutableStateOf("") }

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
            text = "Create a password to encrypt your notes.\nThis cannot be recovered if lost.",
            style = MaterialTheme.typography.bodyMedium,
            textAlign = TextAlign.Center,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        Spacer(modifier = Modifier.height(32.dp))

        if (!showServerConnect) {
            OutlinedTextField(
                value = password,
                onValueChange = { password = it; error = null },
                label = { Text("Password") },
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
                            contentDescription = "Toggle password visibility",
                        )
                    }
                },
                modifier = Modifier.fillMaxWidth(),
            )

            Spacer(modifier = Modifier.height(12.dp))

            OutlinedTextField(
                value = confirmPassword,
                onValueChange = { confirmPassword = it; error = null },
                label = { Text("Confirm Password") },
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(
                    keyboardType = KeyboardType.Password,
                    imeAction = ImeAction.Done,
                ),
                keyboardActions = KeyboardActions(
                    onDone = {
                        if (password == confirmPassword && password.isNotEmpty()) {
                            isCreating = true
                            scope.launch {
                                appState.createVault(password)
                                onVaultCreated()
                            }
                        }
                    }
                ),
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
                onClick = {
                    when {
                        password.isEmpty() -> error = "Password is required"
                        password.length < 8 -> error = "Password must be at least 8 characters"
                        password != confirmPassword -> error = "Passwords do not match"
                        else -> {
                            isCreating = true
                            scope.launch {
                                appState.createVault(password)
                                onVaultCreated()
                            }
                        }
                    }
                },
                enabled = !isCreating,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(if (isCreating) "Creating vault\u2026" else "Create Vault")
            }

            Spacer(modifier = Modifier.height(12.dp))

            OutlinedButton(
                onClick = { showServerConnect = true },
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text("Connect to Server")
            }
        } else {
            // Server connect form
            OutlinedTextField(
                value = serverUrl,
                onValueChange = { serverUrl = it; error = null },
                label = { Text("Server URL") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )

            Spacer(modifier = Modifier.height(12.dp))

            OutlinedTextField(
                value = email,
                onValueChange = { email = it; error = null },
                label = { Text("Email") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                modifier = Modifier.fillMaxWidth(),
            )

            Spacer(modifier = Modifier.height(12.dp))

            OutlinedTextField(
                value = serverPassword,
                onValueChange = { serverPassword = it; error = null },
                label = { Text("Password") },
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
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
                onClick = {
                    when {
                        serverUrl.isBlank() -> error = "Server URL is required"
                        email.isBlank() -> error = "Email is required"
                        serverPassword.isEmpty() -> error = "Password is required"
                        serverPassword.length < 8 -> error = "Password must be at least 8 characters"
                        else -> {
                            isCreating = true
                            error = null
                            scope.launch {
                                try {
                                    // 1. Create local vault first (derives encryption key)
                                    appState.createVault(serverPassword)

                                    // 2. Register device with sync server
                                    val result = appState.registerDevice(
                                        serverUrl = serverUrl.trim().trimEnd('/'),
                                        email = email.trim(),
                                        password = serverPassword,
                                        deviceName = Build.MODEL,
                                    )
                                    result.onSuccess {
                                        onVaultCreated()
                                    }.onFailure { e ->
                                        // Vault is created but sync failed —
                                        // user can retry from Settings > Sync
                                        error = "Registered locally. Sync failed: ${e.message}"
                                        isCreating = false
                                    }
                                } catch (e: Exception) {
                                    error = e.message ?: "Setup failed"
                                    isCreating = false
                                }
                            }
                        }
                    }
                },
                enabled = !isCreating,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(if (isCreating) "Registering\u2026" else "Register Device")
            }

            Spacer(modifier = Modifier.height(12.dp))

            OutlinedButton(
                onClick = { showServerConnect = false },
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text("Back to New Vault")
            }
        }
    }
}
