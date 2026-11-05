package com.jottery.android.service

import android.app.Application
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.jottery.android.viewmodel.AppState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

/**
 * Writes the backups in src/test/resources/backup, which other clients restore, from
 * the real app on a real device. Not part of the normal run: it wipes the app's data.
 *
 *   adb shell am instrument -w -e class com.jottery.android.service.BackupFixtureWriter \
 *       -e writeBackupFixtures true com.jottery.android.test/androidx.test.runner.AndroidJUnitRunner
 *   adb pull /sdcard/Android/data/com.jottery.android/files/android-v3-legacy.jottery-backup
 *   adb pull /sdcard/Android/data/com.jottery.android/files/android-v3-envelope.jottery-backup
 */
@RunWith(AndroidJUnit4::class)
class BackupFixtureWriter {
    private val app = ApplicationProvider.getApplicationContext<Application>()

    /** Test data, shared with the iOS fixture so that one test can read both. */
    private val password = "notes-password-correct-horse"
    private val changedPassword = "notes-password-battery-staple"

    @Test fun writeFixtures() {
        val args = InstrumentationRegistry.getArguments()
        if (args.getString("writeBackupFixtures") != "true") return

        runBlocking {
            val appState = withContext(Dispatchers.Main) { AppState(app) }
            withTimeout(30_000) { while (!appState.isInitialised.value) delay(10) }
            withContext(Dispatchers.Main) { appState.wipeAllData() }
            withTimeout(30_000) { while (!appState.isInitialised.value) delay(10) }

            withContext(Dispatchers.Main) {
                appState.createVault(password)

                val note = appState.createNote("From Android", listOf("android", "fixture"))!!
                appState.saveNote(note.copy(content = "From Android, edited once"))
                appState.addAttachment(note.id, "attachment bytes".toByteArray(), "hello.txt", "text/plain")
                appState.togglePin(note.id)
                appState.createNote("A note with no tags at all")
                appState.saveSearch("Android search", "#android")

                val target = app.getExternalFilesDir(null)!!

                File(target, "android-v3-legacy.jottery-backup").outputStream().use { appState.createBackup(it) }

                // A password change moves a legacy vault to an envelope
                appState.changePassword(password, changedPassword)
                File(target, "android-v3-envelope.jottery-backup").outputStream().use { appState.createBackup(it) }

                assertTrue(File(target, "android-v3-envelope.jottery-backup").length() > 0)
            }
        }
    }
}
