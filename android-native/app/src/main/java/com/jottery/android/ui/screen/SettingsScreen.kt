package com.jottery.android.ui.screen

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.jottery.android.R
import com.jottery.android.model.SortOrder
import com.jottery.android.model.ThemeMode
import com.jottery.android.ui.component.enableBiometricUnlock
import com.jottery.android.util.BiometricHelper
import com.jottery.android.viewmodel.AppState
import com.jottery.android.viewmodel.defaultExportName
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    appState: AppState,
    onBack: () -> Unit,
    onNavigateToSyncSetup: () -> Unit,
    onNavigateToBackup: () -> Unit,
    onNavigateToImport: () -> Unit,
    onNavigateToSavedSearches: () -> Unit,
    onNavigateToChangePassword: () -> Unit,
) {
    val screenSecurity by appState.screenSecurity.collectAsState()
    val biometricEnabled by appState.biometricEnabled.collectAsState()
    val context = LocalContext.current
    val biometricsAvailable = remember { BiometricHelper.isBiometricAvailable(context) }
    val settings by appState.settings.collectAsState()
    val syncEnabled by appState.syncEnabled.collectAsState()
    val lastSyncAt by appState.lastSyncAt.collectAsState()
    val syncError by appState.syncError.collectAsState()
    var showThemeDialog by remember { mutableStateOf(false) }
    var showSortDialog by remember { mutableStateOf(false) }
    var showAutoLockDialog by remember { mutableStateOf(false) }
    var showWipeConfirm by remember { mutableStateOf(false) }

    val scope = rememberCoroutineScope()
    var exportStatus by remember { mutableStateOf<String?>(null) }
    val exportLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/json")
    ) { uri ->
        if (uri != null) {
            scope.launch { exportStatus = appState.exportTo(context, uri, ids = null) }
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Settings") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
            )
        },
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState()),
        ) {
            // General Section
            SettingsSectionHeader("General")
            SettingsRow(
                title = "Theme",
                subtitle = when (settings.themeMode) {
                    ThemeMode.LIGHT -> "Light"
                    ThemeMode.DARK -> "Dark"
                    ThemeMode.AUTO -> "System"
                },
                onClick = { showThemeDialog = true },
            )
            SettingsRow(
                title = "Sort order",
                subtitle = when (settings.sort) {
                    SortOrder.RECENT -> "Most recent"
                    SortOrder.OLDEST -> "Oldest first"
                    SortOrder.ALPHA -> "Alphabetical"
                    SortOrder.CREATED -> "Created date"
                },
                onClick = { showSortDialog = true },
            )
            SettingsRow(
                title = "Auto-lock timeout",
                subtitle = if (settings.autoLockTimeout == 0) "Never" else "${settings.autoLockTimeout} minutes",
                onClick = { showAutoLockDialog = true },
            )
            SettingsRow(
                title = "Saved searches",
                onClick = onNavigateToSavedSearches,
            )

            HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))

            // Security Section
            SettingsSectionHeader("Security")
            SettingsRow(
                title = "Lock now",
                onClick = { appState.lock() },
            )
            SettingsRow(
                title = stringResource(R.string.settings_change_password),
                onClick = onNavigateToChangePassword,
            )
            if (biometricsAvailable) {
                ListItem(
                    headlineContent = { Text(stringResource(R.string.biometric_setting)) },
                    supportingContent = { Text(stringResource(R.string.biometric_setting_detail)) },
                    trailingContent = {
                        Switch(
                            checked = biometricEnabled,
                            onCheckedChange = { enable ->
                                if (enable) {
                                    enableBiometricUnlock(context, appState) { appState.showMessage(it) }
                                } else {
                                    appState.disableBiometric()
                                }
                            },
                        )
                    },
                )
            }
            ListItem(
                headlineContent = { Text(stringResource(R.string.settings_screen_security)) },
                supportingContent = {
                    Text(
                        stringResource(R.string.settings_screen_security_detail),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                },
                trailingContent = {
                    Switch(checked = screenSecurity, onCheckedChange = appState::setScreenSecurity)
                },
            )

            HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))

            // Sync Section
            SettingsSectionHeader("Sync")
            if (syncEnabled) {
                SettingsRow(
                    title = "Sync status",
                    subtitle = syncError ?: lastSyncAt?.let { "Last synced: $it" } ?: "Not yet synced",
                )
                SettingsRow(
                    title = "Force full sync",
                    onClick = {
                        scope.launch { appState.forceFullSync() }
                    },
                )
            }
            SettingsRow(
                title = if (syncEnabled) "Sync settings" else "Set up sync",
                onClick = onNavigateToSyncSetup,
            )

            HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))

            // Data Section
            SettingsSectionHeader("Data")
            SettingsRow(title = "Backup & Restore", onClick = onNavigateToBackup)
            SettingsRow(title = "Import notes", onClick = onNavigateToImport)
            SettingsRow(
                title = stringResource(R.string.export_all),
                subtitle = exportStatus ?: stringResource(R.string.export_all_detail),
                onClick = { exportLauncher.launch(defaultExportName()) },
            )
            SettingsRow(
                title = "Wipe all data",
                subtitle = "Delete everything and start fresh",
                onClick = { showWipeConfirm = true },
                isDestructive = true,
            )

            HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))

            // About Section
            SettingsSectionHeader("About")
            SettingsRow(title = "Jottery for Android", subtitle = "Version 1.0.0")

            Spacer(Modifier.height(32.dp))
        }
    }

    // Theme dialog
    if (showThemeDialog) {
        SingleChoiceDialog(
            title = "Theme",
            options = listOf("System" to ThemeMode.AUTO, "Light" to ThemeMode.LIGHT, "Dark" to ThemeMode.DARK),
            selected = settings.themeMode,
            onSelect = { mode ->
                showThemeDialog = false
                scope.launch {
                    appState.updateSettings(settings.copy(theme = mode.value))
                }
            },
            onDismiss = { showThemeDialog = false },
        )
    }

    // Sort dialog
    if (showSortDialog) {
        SingleChoiceDialog(
            title = "Sort order",
            options = listOf(
                "Most recent" to SortOrder.RECENT,
                "Oldest first" to SortOrder.OLDEST,
                "Alphabetical" to SortOrder.ALPHA,
                "Created date" to SortOrder.CREATED,
            ),
            selected = settings.sort,
            onSelect = { order ->
                showSortDialog = false
                scope.launch {
                    appState.updateSettings(settings.copy(sortOrder = order.value))
                }
            },
            onDismiss = { showSortDialog = false },
        )
    }

    // Auto-lock dialog
    if (showAutoLockDialog) {
        SingleChoiceDialog(
            title = "Auto-lock timeout",
            options = listOf(
                "Never" to 0,
                "1 minute" to 1,
                "5 minutes" to 5,
                "15 minutes" to 15,
                "30 minutes" to 30,
                "1 hour" to 60,
            ),
            selected = settings.autoLockTimeout,
            onSelect = { timeout ->
                showAutoLockDialog = false
                scope.launch {
                    appState.updateSettings(settings.copy(autoLockTimeout = timeout))
                }
            },
            onDismiss = { showAutoLockDialog = false },
        )
    }

    // Wipe confirm
    if (showWipeConfirm) {
        AlertDialog(
            onDismissRequest = { showWipeConfirm = false },
            title = { Text("Wipe all data?") },
            text = {
                Text(
                    "This will permanently delete all notes, settings, and encryption keys. " +
                    "This action cannot be undone."
                )
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        showWipeConfirm = false
                        appState.wipeAllData()
                    },
                    colors = ButtonDefaults.textButtonColors(
                        contentColor = MaterialTheme.colorScheme.error,
                    ),
                ) { Text("Wipe Everything") }
            },
            dismissButton = {
                TextButton(onClick = { showWipeConfirm = false }) { Text("Cancel") }
            },
        )
    }
}

@Composable
private fun SettingsSectionHeader(title: String) {
    Text(
        text = title,
        style = MaterialTheme.typography.titleSmall,
        fontWeight = FontWeight.Bold,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

@Composable
private fun SettingsRow(
    title: String,
    subtitle: String? = null,
    onClick: (() -> Unit)? = null,
    isDestructive: Boolean = false,
) {
    ListItem(
        headlineContent = {
            Text(
                text = title,
                color = if (isDestructive) MaterialTheme.colorScheme.error
                    else MaterialTheme.colorScheme.onSurface,
            )
        },
        supportingContent = subtitle?.let {
            { Text(it, color = MaterialTheme.colorScheme.onSurfaceVariant) }
        },
        modifier = if (onClick != null) Modifier.clickable(onClick = onClick) else Modifier,
    )
}

@Composable
private fun <T> SingleChoiceDialog(
    title: String,
    options: List<Pair<String, T>>,
    selected: T,
    onSelect: (T) -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            Column {
                for ((label, value) in options) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { onSelect(value) }
                            .padding(vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(
                            selected = value == selected,
                            onClick = { onSelect(value) },
                        )
                        Spacer(Modifier.width(8.dp))
                        Text(label)
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        },
    )
}
