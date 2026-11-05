package com.jottery.android.util

import android.content.ContentResolver
import android.content.Intent
import android.net.Uri
import androidx.core.content.IntentCompat

/** What another app handed over through the share sheet. */
data class SharedContent(
    val text: String?,
    val files: List<Uri>,
)

object ShareIntake {
    /**
     * Read a share. Any app can send one, so only content the sender actually holds
     * is accepted: file: URIs and this app's own provider would let it reach into
     * Jottery's private files, the database included.
     */
    fun parse(intent: Intent, ownAuthority: String): SharedContent? {
        if (intent.action != Intent.ACTION_SEND && intent.action != Intent.ACTION_SEND_MULTIPLE) return null

        val subject = intent.getStringExtra(Intent.EXTRA_SUBJECT)?.takeIf { it.isNotBlank() }
        val body = intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString()?.takeIf { it.isNotBlank() }
        val text = listOfNotNull(subject, body).joinToString("\n\n").ifEmpty { null }

        val uris = if (intent.action == Intent.ACTION_SEND_MULTIPLE) {
            IntentCompat.getParcelableArrayListExtra(intent, Intent.EXTRA_STREAM, Uri::class.java).orEmpty()
        } else {
            listOfNotNull(IntentCompat.getParcelableExtra(intent, Intent.EXTRA_STREAM, Uri::class.java))
        }
        val files = uris.filter { it.scheme == ContentResolver.SCHEME_CONTENT && it.authority != ownAuthority }

        return if (text == null && files.isEmpty()) null else SharedContent(text, files)
    }
}
