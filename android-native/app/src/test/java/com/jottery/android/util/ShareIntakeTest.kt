package com.jottery.android.util

import android.content.Intent
import android.net.Uri
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class ShareIntakeTest {
    private val own = "com.jottery.android.attachments"

    @Test fun `text and a subject become the note's text`() {
        val intent = Intent(Intent.ACTION_SEND)
            .putExtra(Intent.EXTRA_SUBJECT, "An article")
            .putExtra(Intent.EXTRA_TEXT, "https://example.com")

        assertEquals("An article\n\nhttps://example.com", ShareIntake.parse(intent, own)?.text)
    }

    @Test fun `files arrive one or many`() {
        val one = Uri.parse("content://photos/1")
        val two = Uri.parse("content://photos/2")

        assertEquals(listOf(one), ShareIntake.parse(Intent(Intent.ACTION_SEND).putExtra(Intent.EXTRA_STREAM, one), own)?.files)
        val many = Intent(Intent.ACTION_SEND_MULTIPLE)
            .putParcelableArrayListExtra(Intent.EXTRA_STREAM, arrayListOf(one, two))
        assertEquals(listOf(one, two), ShareIntake.parse(many, own)?.files)
    }

    @Test fun `another app can't point this one at its own private files`() {
        // Sharing is open to every app. These would read the database into a note.
        val files = listOf(
            "file:///data/data/com.jottery.android/databases/jottery.db",
            "content://$own/attachments/secret.pdf",
        ).map { Uri.parse(it) }

        for (uri in files) {
            assertNull(ShareIntake.parse(Intent(Intent.ACTION_SEND).putExtra(Intent.EXTRA_STREAM, uri), own))
        }
        val mixed = Intent(Intent.ACTION_SEND_MULTIPLE)
            .putParcelableArrayListExtra(Intent.EXTRA_STREAM, ArrayList(files + Uri.parse("content://photos/1")))
        assertEquals(listOf(Uri.parse("content://photos/1")), ShareIntake.parse(mixed, own)?.files)
    }

    @Test fun `anything else is ignored`() {
        assertNull(ShareIntake.parse(Intent(Intent.ACTION_MAIN), own))
        assertNull(ShareIntake.parse(Intent(Intent.ACTION_SEND), own))
        assertTrue(ShareIntake.parse(Intent(Intent.ACTION_SEND).putExtra(Intent.EXTRA_TEXT, "x"), own)!!.files.isEmpty())
    }
}
