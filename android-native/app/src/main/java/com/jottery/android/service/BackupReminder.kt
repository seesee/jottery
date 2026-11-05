package com.jottery.android.service

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import com.jottery.android.MainActivity
import com.jottery.android.R
import java.time.Duration
import java.time.Instant
import java.util.concurrent.TimeUnit

/**
 * A nudge to back up, for someone with no sync and no other copy. It reads two dates
 * and nothing else: it can't reach the vault, and a notification shows on a lock
 * screen, so it says nothing about any note.
 */
object BackupReminder {
    private const val PREFS = "jottery_backup_reminder"
    private const val KEY_ENABLED = "enabled"
    private const val KEY_LAST_BACKUP = "lastBackupAt"
    private const val KEY_LAST_REMINDER = "lastReminderAt"
    private const val WORK_NAME = "backup-reminder"
    private const val CHANNEL = "backup_reminders"
    private const val NOTIFICATION_ID = 1

    /** The web's default. */
    val INTERVAL: Duration = Duration.ofDays(7)

    private fun prefs(context: Context) = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    fun isEnabled(context: Context): Boolean = prefs(context).getBoolean(KEY_ENABLED, false)

    fun lastBackupAt(context: Context): Instant? =
        prefs(context).getLong(KEY_LAST_BACKUP, 0L).takeIf { it > 0 }?.let(Instant::ofEpochMilli)

    fun recordBackup(context: Context, at: Instant = Instant.now()) {
        prefs(context).edit().putLong(KEY_LAST_BACKUP, at.toEpochMilli()).apply()
    }

    fun setEnabled(context: Context, enabled: Boolean, now: Instant = Instant.now()) {
        val prefs = prefs(context)
        val edit = prefs.edit().putBoolean(KEY_ENABLED, enabled)
        // The week starts here: nobody turning this on needs telling that very moment
        if (enabled && !prefs.contains(KEY_LAST_REMINDER)) edit.putLong(KEY_LAST_REMINDER, now.toEpochMilli())
        edit.apply()

        val work = WorkManager.getInstance(context)
        if (enabled) {
            work.enqueueUniquePeriodicWork(
                WORK_NAME,
                ExistingPeriodicWorkPolicy.KEEP,
                PeriodicWorkRequestBuilder<BackupReminderWorker>(1, TimeUnit.DAYS)
                    .setInitialDelay(1, TimeUnit.DAYS)
                    .build(),
            )
        } else {
            work.cancelUniqueWork(WORK_NAME)
        }
    }

    /** A wiped vault has nothing to remind anyone about. */
    fun forget(context: Context) {
        setEnabled(context, false)
        prefs(context).edit().clear().apply()
    }

    /** Due once it's been long enough since either the last backup or the last reminder. */
    fun isDue(lastBackup: Instant?, lastReminder: Instant?, now: Instant): Boolean {
        val latest = listOfNotNull(lastBackup, lastReminder).maxOrNull() ?: return true
        return Duration.between(latest, now) >= INTERVAL
    }

    /** Called once a day. */
    internal fun remindIfDue(context: Context, now: Instant = Instant.now()) {
        if (!isEnabled(context)) return
        val prefs = prefs(context)
        val lastReminder = prefs.getLong(KEY_LAST_REMINDER, 0L).takeIf { it > 0 }?.let(Instant::ofEpochMilli)
        if (!isDue(lastBackupAt(context), lastReminder, now)) return

        prefs.edit().putLong(KEY_LAST_REMINDER, now.toEpochMilli()).apply()
        post(context)
    }

    private fun post(context: Context) {
        val allowed = Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            ContextCompat.checkSelfPermission(context, android.Manifest.permission.POST_NOTIFICATIONS) ==
            PackageManager.PERMISSION_GRANTED
        if (!allowed) return

        context.getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(
                CHANNEL,
                context.getString(R.string.backup_reminder_channel),
                NotificationManager.IMPORTANCE_DEFAULT,
            )
        )
        val open = PendingIntent.getActivity(
            context,
            0,
            Intent(context, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val notification = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_sys_upload_done)
            .setContentTitle(context.getString(R.string.backup_reminder_title))
            .setContentIntent(open)
            .setAutoCancel(true)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .build()
        NotificationManagerCompat.from(context).notify(NOTIFICATION_ID, notification)
    }
}

class BackupReminderWorker(context: Context, params: WorkerParameters) : Worker(context, params) {
    override fun doWork(): Result {
        BackupReminder.remindIfDue(applicationContext)
        return Result.success()
    }
}
