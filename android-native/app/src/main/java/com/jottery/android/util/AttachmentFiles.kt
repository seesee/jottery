package com.jottery.android.util

import android.content.ContentResolver
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.provider.OpenableColumns
import android.util.Log
import androidx.core.content.FileProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

/**
 * Moving attachments between the vault and the rest of the system. A decrypted
 * copy exists only so another app can read it, in a cache folder that is cleared
 * on lock and at the next launch.
 */
object AttachmentFiles {
    /** The same limit as web. */
    const val MAX_ATTACHMENT_BYTES = 10 * 1024 * 1024

    private const val TAG = "AttachmentFiles"
    private const val FOLDER = "attachments"
    private const val THUMBNAIL_SIZE = 96

    class TooLarge : Exception("Attachment is over the size limit")

    data class Picked(val name: String, val mimeType: String, val data: ByteArray)

    /** Read a picked file, refusing one over the limit without loading it all. */
    suspend fun read(resolver: ContentResolver, uri: Uri): Picked = withContext(Dispatchers.IO) {
        val name = resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst()) cursor.getString(0) else null
        } ?: uri.lastPathSegment ?: "attachment"

        val stream = resolver.openInputStream(uri) ?: throw java.io.FileNotFoundException()
        val data = stream.use { input ->
            val out = java.io.ByteArrayOutputStream()
            val buffer = ByteArray(64 * 1024)
            while (true) {
                val count = input.read(buffer)
                if (count < 0) break
                out.write(buffer, 0, count)
                if (out.size() > MAX_ATTACHMENT_BYTES) throw TooLarge()
            }
            out.toByteArray()
        }
        Picked(name, resolver.getType(uri) ?: "application/octet-stream", data)
    }

    /** A filename from a note or the server, made safe to use in a path. */
    fun safeName(name: String): String {
        val cleaned = name.substringAfterLast('/').substringAfterLast('\\')
            .filter { !it.isISOControl() }
            .trim('.', ' ')
            .take(120)
        return cleaned.ifEmpty { "attachment" }
    }

    /** Write a decrypted copy where another app can be granted access to it. */
    private fun export(context: Context, name: String, data: ByteArray): Uri {
        val folder = File(context.cacheDir, FOLDER).apply { mkdirs() }
        val file = File(File(folder, java.util.UUID.randomUUID().toString()).apply { mkdirs() }, safeName(name))
        file.writeBytes(data)
        return FileProvider.getUriForFile(context, "${context.packageName}.attachments", file)
    }

    suspend fun viewIntent(context: Context, name: String, mimeType: String, data: ByteArray): Intent =
        withContext(Dispatchers.IO) {
            Intent(Intent.ACTION_VIEW)
                .setDataAndType(export(context, name, data), mimeType.ifEmpty { "application/octet-stream" })
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }

    suspend fun shareIntent(context: Context, name: String, mimeType: String, data: ByteArray): Intent =
        withContext(Dispatchers.IO) {
            val send = Intent(Intent.ACTION_SEND)
                .setType(mimeType.ifEmpty { "application/octet-stream" })
                .putExtra(Intent.EXTRA_STREAM, export(context, name, data))
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            Intent.createChooser(send, null)
        }

    fun clear(context: Context) {
        try {
            File(context.cacheDir, FOLDER).deleteRecursively()
        } catch (e: Exception) {
            Log.w(TAG, "Couldn't clear decrypted copies: ${e.javaClass.simpleName}")
        }
    }

    /** A small bitmap, decoded at reduced size so a large photo stays cheap. */
    suspend fun thumbnail(data: ByteArray): Bitmap? = withContext(Dispatchers.Default) {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(data, 0, data.size, bounds)
        if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return@withContext null

        var sample = 1
        while (bounds.outWidth / (sample * 2) >= THUMBNAIL_SIZE && bounds.outHeight / (sample * 2) >= THUMBNAIL_SIZE) {
            sample *= 2
        }
        BitmapFactory.decodeByteArray(data, 0, data.size, BitmapFactory.Options().apply { inSampleSize = sample })
    }
}
