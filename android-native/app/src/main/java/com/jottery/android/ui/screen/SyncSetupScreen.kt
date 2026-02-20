package com.jottery.android.ui.screen

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
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
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SyncSetupScreen(
    appState: AppState,
    onBack: () -> Unit,
) {
    var mode by remember { mutableStateOf<SyncSetupMode>(SyncSetupMode.Register) }
    var serverUrl by remember { mutableStateOf("") }
    var email by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var deviceName by remember { mutableStateOf("") }
    var importApiKey by remember { mutableStateOf("") }
    var importClientId by remember { mutableStateOf("") }
    var isLoading by remember { mutableStateOf(false) }
    var errorMessage by remember { mutableStateOf<String?>(null) }
    var successMessage by remember { mutableStateOf<String?>(null) }

    val scope = rememberCoroutineScope()
    val snackbarHostState = remember { SnackbarHostState() }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Sync Setup") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back",
                        )
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(snackbarHostState) },
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(horizontal = 16.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            // Mode selector
            SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
                SegmentedButton(
                    selected = mode == SyncSetupMode.Register,
                    onClick = { mode = SyncSetupMode.Register },
                    shape = SegmentedButtonDefaults.itemShape(index = 0, count = 2),
                ) { Text("Register") }
                SegmentedButton(
                    selected = mode == SyncSetupMode.Import,
                    onClick = { mode = SyncSetupMode.Import },
                    shape = SegmentedButtonDefaults.itemShape(index = 1, count = 2),
                ) { Text("Import") }
            }

            // Server URL (common)
            OutlinedTextField(
                value = serverUrl,
                onValueChange = { serverUrl = it },
                label = { Text("Server URL") },
                placeholder = { Text("https://sync.example.com") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
            )

            when (mode) {
                SyncSetupMode.Register -> {
                    OutlinedTextField(
                        value = email,
                        onValueChange = { email = it },
                        label = { Text("Email") },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                    )
                    OutlinedTextField(
                        value = password,
                        onValueChange = { password = it },
                        label = { Text("Password") },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                        visualTransformation = PasswordVisualTransformation(),
                    )
                    OutlinedTextField(
                        value = deviceName,
                        onValueChange = { deviceName = it },
                        label = { Text("Device name") },
                        placeholder = { Text("My Android phone") },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                    )

                    Text(
                        text = "Register this device with your Jottery sync server. " +
                            "An administrator must approve your account before sync begins.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }

                SyncSetupMode.Import -> {
                    OutlinedTextField(
                        value = importApiKey,
                        onValueChange = { importApiKey = it },
                        label = { Text("API Key") },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                    )
                    OutlinedTextField(
                        value = importClientId,
                        onValueChange = { importClientId = it },
                        label = { Text("Client ID") },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                    )

                    Text(
                        text = "Import credentials from another device. " +
                            "You can find these in Settings > Sync on your other device.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }

            errorMessage?.let { error ->
                Text(
                    text = error,
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }

            successMessage?.let { success ->
                Text(
                    text = success,
                    color = MaterialTheme.colorScheme.primary,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }

            Button(
                onClick = {
                    isLoading = true
                    errorMessage = null
                    successMessage = null
                    val trimmedUrl = serverUrl.trim().trimEnd('/')

                    scope.launch {
                        when (mode) {
                            SyncSetupMode.Register -> {
                                val result = appState.registerDevice(
                                    serverUrl = trimmedUrl,
                                    email = email.trim(),
                                    password = password,
                                    deviceName = deviceName.trim(),
                                )
                                isLoading = false
                                result.onSuccess { msg ->
                                    successMessage = msg
                                }.onFailure { e ->
                                    errorMessage = e.message ?: "Registration failed"
                                }
                            }
                            SyncSetupMode.Import -> {
                                val result = appState.importCredentials(
                                    serverUrl = trimmedUrl,
                                    apiKey = importApiKey.trim(),
                                    clientId = importClientId.trim(),
                                )
                                isLoading = false
                                result.onSuccess { msg ->
                                    successMessage = msg
                                }.onFailure { e ->
                                    errorMessage = e.message ?: "Import failed"
                                }
                            }
                        }
                    }
                },
                modifier = Modifier.fillMaxWidth(),
                enabled = !isLoading && serverUrl.isNotBlank() && when (mode) {
                    SyncSetupMode.Register -> email.isNotBlank() && password.isNotBlank() && deviceName.isNotBlank()
                    SyncSetupMode.Import -> importApiKey.isNotBlank() && importClientId.isNotBlank()
                },
            ) {
                if (isLoading) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(20.dp),
                        strokeWidth = 2.dp,
                    )
                    Spacer(Modifier.width(8.dp))
                }
                Text(
                    when (mode) {
                        SyncSetupMode.Register -> "Register Device"
                        SyncSetupMode.Import -> "Import Credentials"
                    }
                )
            }

            Spacer(Modifier.height(32.dp))
        }
    }
}

private enum class SyncSetupMode { Register, Import }
