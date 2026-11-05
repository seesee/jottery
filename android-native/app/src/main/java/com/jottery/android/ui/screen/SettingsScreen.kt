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
import com.jottery.android.BuildConfig
import com.jottery.android.R
import com.jottery.android.model.SortOrder
import com.jottery.android.model.ThemeMode
import com.jottery.android.model.tagCounts
import com.jottery.android.ui.component.autoLockLabel
import com.jottery.android.ui.component.enableBiometricUnlock
import com.jottery.android.ui.component.relativeTime
import com.jottery.android.ui.component.sortOrderLabel
import com.jottery.android.ui.component.syntaxLanguages
import com.jottery.android.ui.component.themeModeLabel
import com.jottery.android.util.BiometricHelper
import com.jottery.android.viewmodel.AppState
import com.jottery.android.viewmodel.defaultExportName
import java.text.NumberFormat
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
    onNavigateToColours: () -> Unit,
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
    var showNoteTypeDialog by remember { mutableStateOf(false) }
    val defaultNoteType by appState.defaultNoteType.collectAsState()
    var showAutoLockDialog by remember { mutableStateOf(false) }
    var showWipeConfirm by remember { mutableStateOf(false) }

    val scope = rememberCoroutineScope()
    val notes by appState.notes.collectAsState()
    val archivedNotes by appState.archivedNotes.collectAsState()
    var binCount by remember { mutableIntStateOf(0) }
    LaunchedEffect(Unit) { binCount = appState.loadDeletedNotes().size }
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
                title = { Text(stringResource(R.string.settings_title)) },
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
                .verticalScroll(rememberScrollState()),
        ) {
            // General Section
            SettingsSectionHeader(stringResource(R.string.settings_general))
            SettingsRow(
                title = stringResource(R.string.settings_theme),
                subtitle = themeModeLabel(settings.themeMode),
                onClick = { showThemeDialog = true },
            )
            SettingsRow(
                title = stringResource(R.string.settings_sort_order2),
                subtitle = sortOrderLabel(settings.sort),
                onClick = { showSortDialog = true },
            )
            SettingsRow(
                title = stringResource(R.string.settings_auto_lock_timeout),
                subtitle = autoLockLabel(settings.autoLockTimeout),
                onClick = { showAutoLockDialog = true },
            )
            SettingsRow(
                title = stringResource(R.string.settings_default_note_type),
                subtitle = defaultNoteType.replaceFirstChar { it.uppercase() },
                onClick = { showNoteTypeDialog = true },
            )
            SettingsRow(
                title = stringResource(R.string.settings_colours),
                onClick = onNavigateToColours,
            )
            SettingsRow(
                title = stringResource(R.string.saved_searches_title),
                onClick = onNavigateToSavedSearches,
            )

            HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))

            // Security Section
            SettingsSectionHeader(stringResource(R.string.settings_security))
            SettingsRow(
                title = stringResource(R.string.settings_lock_now),
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
            SettingsSectionHeader(stringResource(R.string.settings_sync))
            if (syncEnabled) {
                SettingsRow(
                    title = stringResource(R.string.settings_sync_status),
                    subtitle = syncError
                        ?: lastSyncAt?.let { stringResource(R.string.sync_account_last_synced, relativeTime(it)) }
                        ?: stringResource(R.string.sync_account_not_synced),
                )
                SettingsRow(
                    title = stringResource(R.string.settings_force_full_sync),
                    onClick = {
                        scope.launch { appState.forceFullSync() }
                    },
                )
            }
            SettingsRow(
                title = if (syncEnabled) stringResource(R.string.settings_sync_settings) else stringResource(R.string.settings_set_up_sync),
                onClick = onNavigateToSyncSetup,
            )

            HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))

            // Data Section
            SettingsSectionHeader(stringResource(R.string.settings_data))
            SettingsRow(title = stringResource(R.string.settings_backup), onClick = onNavigateToBackup)
            SettingsRow(title = stringResource(R.string.settings_import_notes), onClick = onNavigateToImport)
            SettingsRow(
                title = stringResource(R.string.export_all),
                subtitle = exportStatus ?: stringResource(R.string.export_all_detail),
                onClick = { exportLauncher.launch(defaultExportName()) },
            )
            SettingsRow(
                title = stringResource(R.string.settings_wipe_all_data),
                subtitle = stringResource(R.string.settings_delete_everything_and_start_fresh),
                onClick = { showWipeConfirm = true },
                isDestructive = true,
            )

            HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))

            // About Section
            SettingsSectionHeader(stringResource(R.string.settings_about2))
            SettingsRow(
                title = stringResource(R.string.settings_about_app),
                subtitle = stringResource(R.string.settings_about_version, BuildConfig.VERSION_NAME),
            )
            val numbers = remember { NumberFormat.getInstance() }
            val counts = listOf(
                R.string.settings_about_notes to notes.size,
                R.string.note_info_archived to archivedNotes.size,
                R.string.recycle_bin_title to binCount,
                R.string.note_info_tags to (notes + archivedNotes).tagCounts().size,
                R.string.note_info_attachments to (notes + archivedNotes).sumOf { it.attachments.size },
            )
            for ((label, count) in counts) {
                SettingsRow(title = stringResource(label), subtitle = numbers.format(count))
            }

            Spacer(Modifier.height(32.dp))
        }
    }

    // Theme dialog
    if (showThemeDialog) {
        SingleChoiceDialog(
            title = stringResource(R.string.settings_theme),
            options = listOf(ThemeMode.AUTO, ThemeMode.LIGHT, ThemeMode.DARK).map { themeModeLabel(it) to it },
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

    if (showNoteTypeDialog) {
        SingleChoiceDialog(
            title = stringResource(R.string.settings_default_note_type),
            options = syntaxLanguages.map { it.replaceFirstChar { first -> first.uppercase() } to it },
            selected = defaultNoteType,
            onSelect = { type ->
                showNoteTypeDialog = false
                appState.setDefaultNoteType(type)
            },
            onDismiss = { showNoteTypeDialog = false },
        )
    }

    // Sort dialog
    if (showSortDialog) {
        SingleChoiceDialog(
            title = stringResource(R.string.settings_sort_order2),
            options = SortOrder.entries.map { sortOrderLabel(it) to it },
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
            title = stringResource(R.string.settings_auto_lock_timeout),
            options = listOf(0, 1, 5, 15, 30, 60).map { autoLockLabel(it) to it },
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
            title = { Text(stringResource(R.string.settings_wipe_all_data2)) },
            text = { Text(stringResource(R.string.unlock_this_will_permanently_delete_all)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        showWipeConfirm = false
                        appState.wipeAllData()
                    },
                    colors = ButtonDefaults.textButtonColors(
                        contentColor = MaterialTheme.colorScheme.error,
                    ),
                ) { Text(stringResource(R.string.settings_wipe_everything)) }
            },
            dismissButton = {
                TextButton(onClick = { showWipeConfirm = false }) { Text(stringResource(R.string.dialog_cancel)) }
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
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.dialog_cancel)) }
        },
    )
}
