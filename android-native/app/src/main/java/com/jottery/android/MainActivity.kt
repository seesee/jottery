package com.jottery.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.jottery.android.ui.navigation.JotteryNavigation
import com.jottery.android.ui.theme.JotteryTheme
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        setContent {
            val appState: AppState = viewModel()
            val settings by appState.settings.collectAsState()

            JotteryTheme(themeMode = settings.themeMode) {
                Surface(modifier = Modifier.fillMaxSize()) {
                    JotteryNavigation(appState = appState)
                }
            }
        }

        // Handle lifecycle events for auto-lock and SSE management
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) {
                // Foreground
                val appState = (application as JotteryApp).let {
                    // AppState handles its own lifecycle via handleLifecycleEvent
                }
            }
        }
    }
}
