package com.jottery.android.service

import android.app.Application
import android.app.NotificationManager
import android.content.Context
import androidx.test.core.app.ApplicationProvider
import androidx.work.ListenableWorker
import androidx.work.testing.TestWorkerBuilder
import androidx.work.testing.WorkManagerTestInitHelper
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import java.time.Duration
import java.time.Instant
import java.util.concurrent.Executors

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class BackupReminderTest {
    private val context: Context = ApplicationProvider.getApplicationContext()
    private val now = Instant.parse("2026-06-15T12:00:00Z")
    private val week = Duration.ofDays(7)

    @Before fun setUp() {
        WorkManagerTestInitHelper.initializeTestWorkManager(context)
        shadowOf(context as Application).grantPermissions(android.Manifest.permission.POST_NOTIFICATIONS)
    }

    private fun posted() = shadowOf(context.getSystemService(NotificationManager::class.java)).allNotifications

    @Test fun `a week after a backup it's due, and not before`() {
        assertFalse(BackupReminder.isDue(now.minus(week).plusSeconds(1), null, now))
        assertTrue(BackupReminder.isDue(now.minus(week), null, now))
    }

    @Test fun `a reminder holds off the next one as a backup does`() {
        assertFalse(BackupReminder.isDue(null, now.minus(Duration.ofDays(1)), now))
        assertTrue(BackupReminder.isDue(null, now.minus(week), now))
    }

    @Test fun `never backed up and never reminded is due`() {
        assertTrue(BackupReminder.isDue(null, null, now))
    }

    @Test fun `nothing is posted unless it's turned on`() {
        BackupReminder.remindIfDue(context, now)

        assertTrue(posted().isEmpty())
    }

    @Test fun `turning it on doesn't send one that very moment`() {
        BackupReminder.setEnabled(context, true, now)

        BackupReminder.remindIfDue(context, now)
        BackupReminder.remindIfDue(context, now.plus(week).minusSeconds(1))

        assertTrue(posted().isEmpty())
    }

    @Test fun `it says it's time, and nothing about any note`() {
        BackupReminder.setEnabled(context, true, now)

        BackupReminder.remindIfDue(context, now.plus(week))

        val extras = posted().single().extras
        assertEquals("Time to back up your notes", extras.getCharSequence("android.title").toString())
        assertNull(extras.getCharSequence("android.text"))
    }

    @Test fun `once a week is enough, however often it runs`() {
        BackupReminder.setEnabled(context, true, now)

        BackupReminder.remindIfDue(context, now.plus(week))
        context.getSystemService(NotificationManager::class.java).cancelAll()
        BackupReminder.remindIfDue(context, now.plus(week).plus(Duration.ofDays(1)))

        assertTrue(posted().isEmpty())
    }

    @Test fun `a recent backup means no reminder`() {
        BackupReminder.setEnabled(context, true, now)
        BackupReminder.recordBackup(context, now.plus(Duration.ofDays(5)))

        BackupReminder.remindIfDue(context, now.plus(week))

        assertTrue(posted().isEmpty())
    }

    @Test fun `the daily job runs and succeeds`() {
        val worker = TestWorkerBuilder<BackupReminderWorker>(context, Executors.newSingleThreadExecutor()).build()

        assertEquals(ListenableWorker.Result.success(), worker.doWork())
    }

    @Test fun `a wipe forgets it all`() {
        BackupReminder.setEnabled(context, true, now)
        BackupReminder.recordBackup(context, now)

        BackupReminder.forget(context)

        assertFalse(BackupReminder.isEnabled(context))
        assertNull(BackupReminder.lastBackupAt(context))
    }
}
