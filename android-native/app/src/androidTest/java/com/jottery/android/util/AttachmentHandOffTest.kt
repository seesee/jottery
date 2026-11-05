package com.jottery.android.util

import android.app.Application
import android.content.Intent
import android.net.Uri
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

@RunWith(AndroidJUnit4::class)
class AttachmentHandOffTest {
    private val context = ApplicationProvider.getApplicationContext<Application>()
    private val folder get() = File(context.cacheDir, "attachments")

    @Test fun openingGoesThroughTheFileProviderAndGrantsReadOnly() = runBlocking {
        val intent = AttachmentFiles.viewIntent(context, "../../secret.pdf", "application/pdf", ByteArray(3))

        val uri = intent.data!!
        assertEquals("content", uri.scheme)
        assertEquals("${context.packageName}.attachments", uri.authority)
        assertEquals("application/pdf", intent.type)
        assertTrue(uri.path!!.endsWith("/secret.pdf"))
        assertTrue(intent.flags and Intent.FLAG_GRANT_READ_URI_PERMISSION != 0)
        assertEquals(0, intent.flags and Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
    }

    @Test fun sharingWrapsTheSameKindOfLink() = runBlocking {
        val chooser = AttachmentFiles.shareIntent(context, "a.txt", "text/plain", ByteArray(1))

        val send = chooser.getParcelableExtra(Intent.EXTRA_INTENT, Intent::class.java)!!
        val uri = send.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)!!
        assertEquals("${context.packageName}.attachments", uri.authority)
    }

    @Test fun decryptedCopiesAreCleared() = runBlocking {
        AttachmentFiles.viewIntent(context, "a.txt", "text/plain", ByteArray(1))
        assertTrue(folder.exists())

        AttachmentFiles.clear(context)

        assertFalse(folder.exists())
    }
}
