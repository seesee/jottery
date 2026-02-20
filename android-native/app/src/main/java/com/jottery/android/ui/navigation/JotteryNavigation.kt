package com.jottery.android.ui.navigation

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.jottery.android.ui.screen.BackupScreen
import com.jottery.android.ui.screen.ChangePasswordScreen
import com.jottery.android.ui.screen.ConflictResolutionScreen
import com.jottery.android.ui.screen.ImportScreen
import com.jottery.android.ui.screen.InboxScreen
import com.jottery.android.ui.screen.MainScreen
import com.jottery.android.ui.screen.RecycleBinScreen
import com.jottery.android.ui.screen.SavedSearchesScreen
import com.jottery.android.ui.screen.SettingsScreen
import com.jottery.android.ui.screen.SetupScreen
import com.jottery.android.ui.screen.SyncSetupScreen
import com.jottery.android.ui.screen.UnlockScreen
import com.jottery.android.viewmodel.AppState

/** Navigation routes. */
object Routes {
    const val SETUP = "setup"
    const val UNLOCK = "unlock"
    const val MAIN = "main"
    const val RECYCLE_BIN = "recycle_bin"
    const val SETTINGS = "settings"
    const val SYNC_SETUP = "sync_setup"
    const val BACKUP = "backup"
    const val IMPORT = "import"
    const val SAVED_SEARCHES = "saved_searches"
    const val CHANGE_PASSWORD = "change_password"
    const val CONFLICT_RESOLUTION = "conflict_resolution"
    const val INBOX = "inbox"
}

@Composable
fun JotteryNavigation(
    appState: AppState,
    navController: NavHostController = rememberNavController(),
) {
    val isFirstLaunch by appState.isFirstLaunch.collectAsState()
    val isLocked by appState.isLocked.collectAsState()

    val startDestination = when {
        isFirstLaunch -> Routes.SETUP
        isLocked -> Routes.UNLOCK
        else -> Routes.MAIN
    }

    // React to state changes (e.g. wipeAllData sets isFirstLaunch=true while on
    // the unlock screen). NavHost's startDestination only affects initial composition,
    // so we must navigate explicitly when the auth state changes.
    LaunchedEffect(isFirstLaunch, isLocked) {
        val currentRoute = navController.currentBackStackEntry?.destination?.route
        when {
            isFirstLaunch && currentRoute != null && currentRoute != Routes.SETUP -> {
                navController.navigate(Routes.SETUP) {
                    popUpTo(0) { inclusive = true }
                }
            }
            !isFirstLaunch && isLocked && currentRoute != null && currentRoute != Routes.UNLOCK -> {
                navController.navigate(Routes.UNLOCK) {
                    popUpTo(0) { inclusive = true }
                }
            }
        }
    }

    NavHost(
        navController = navController,
        startDestination = startDestination,
    ) {
        composable(Routes.SETUP) {
            SetupScreen(
                appState = appState,
                onVaultCreated = {
                    navController.navigate(Routes.MAIN) {
                        popUpTo(Routes.SETUP) { inclusive = true }
                    }
                },
            )
        }

        composable(Routes.UNLOCK) {
            UnlockScreen(
                appState = appState,
                onUnlocked = {
                    navController.navigate(Routes.MAIN) {
                        popUpTo(Routes.UNLOCK) { inclusive = true }
                    }
                },
            )
        }

        composable(Routes.MAIN) {
            MainScreen(
                appState = appState,
                onNavigateToRecycleBin = { navController.navigate(Routes.RECYCLE_BIN) },
                onNavigateToSettings = { navController.navigate(Routes.SETTINGS) },
            )
        }

        composable(Routes.RECYCLE_BIN) {
            RecycleBinScreen(
                appState = appState,
                onNavigateBack = { navController.popBackStack() },
            )
        }

        composable(Routes.SETTINGS) {
            SettingsScreen(
                appState = appState,
                onBack = { navController.popBackStack() },
                onNavigateToSyncSetup = { navController.navigate(Routes.SYNC_SETUP) },
                onNavigateToChangePassword = { navController.navigate(Routes.CHANGE_PASSWORD) },
                onNavigateToBackup = { navController.navigate(Routes.BACKUP) },
                onNavigateToImport = { navController.navigate(Routes.IMPORT) },
                onNavigateToSavedSearches = { navController.navigate(Routes.SAVED_SEARCHES) },
            )
        }

        composable(Routes.SYNC_SETUP) {
            SyncSetupScreen(
                appState = appState,
                onBack = { navController.popBackStack() },
            )
        }

        composable(Routes.BACKUP) {
            BackupScreen(
                appState = appState,
                onBack = { navController.popBackStack() },
            )
        }

        composable(Routes.IMPORT) {
            ImportScreen(
                appState = appState,
                onBack = { navController.popBackStack() },
            )
        }

        composable(Routes.SAVED_SEARCHES) {
            SavedSearchesScreen(
                appState = appState,
                onBack = { navController.popBackStack() },
                onApplySearch = {
                    navController.popBackStack()
                },
            )
        }

        composable(Routes.CHANGE_PASSWORD) {
            ChangePasswordScreen(
                appState = appState,
                onBack = { navController.popBackStack() },
            )
        }

        composable(Routes.CONFLICT_RESOLUTION) {
            ConflictResolutionScreen(
                appState = appState,
                onBack = { navController.popBackStack() },
            )
        }

        composable(Routes.INBOX) {
            InboxScreen(
                appState = appState,
                onBack = { navController.popBackStack() },
            )
        }
    }
}
