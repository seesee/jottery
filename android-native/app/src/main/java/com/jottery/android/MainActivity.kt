package com.jottery.android

import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.WindowManager
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import com.jottery.android.model.DecryptedNote
import com.jottery.android.ui.navigation.JotteryNavigation
import com.jottery.android.ui.theme.JotteryTheme
import com.jottery.android.util.AttachmentFiles
import com.jottery.android.util.ShareIntake
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

// BiometricPrompt needs a FragmentActivity
class MainActivity : FragmentActivity() {

    private val appState: AppState by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        DecryptedNote.untitled = getString(R.string.note_untitled)
        // Set before the first frame, so a decrypted note is never captured
        applyScreenSecurity(appState.screenSecurity.value)
        enableEdgeToEdge()

        setContent {
            val settings by appState.settings.collectAsState()

            JotteryTheme(themeMode = settings.themeMode) {
                Surface(modifier = Modifier.fillMaxSize()) {
                    JotteryNavigation(appState = appState)
                }
            }
        }

        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.CREATED) {
                appState.screenSecurity.collect(::applyScreenSecurity)
            }
        }

        // Not after a rotation, which would deliver the same share twice
        if (savedInstanceState == null) receiveShare(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        receiveShare(intent)
    }

    /**
     * Files are read now, because permission to read them ends with this activity,
     * and held in memory until the vault opens. Nothing shared reaches the disk
     * unencrypted.
     */
    private fun receiveShare(intent: Intent) {
        val shared = ShareIntake.parse(intent, "$packageName.attachments") ?: return
        lifecycleScope.launch {
            val files = shared.files.mapNotNull { uri ->
                try {
                    AttachmentFiles.read(contentResolver, uri)
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    appState.showMessage(getString(R.string.share_file_failed))
                    null
                }
            }
            appState.receiveShare(shared.text, files)
        }
    }

    private fun applyScreenSecurity(enabled: Boolean) {
        if (enabled) {
            window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        } else {
            window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            setRecentsScreenshotEnabled(!enabled)
        }
    }

    override fun onStart() {
        super.onStart()
        appState.handleLifecycleEvent(started = true)
    }

    override fun onStop() {
        super.onStop()
        // Rotation stops and restarts the activity, but the app never left
        if (!isChangingConfigurations) {
            appState.handleLifecycleEvent(started = false)
        }
    }

    override fun dispatchTouchEvent(event: MotionEvent): Boolean {
        appState.recordActivity()
        return super.dispatchTouchEvent(event)
    }

    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        appState.recordActivity()
        return super.dispatchKeyEvent(event)
    }
}
