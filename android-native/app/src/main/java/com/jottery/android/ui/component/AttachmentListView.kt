package com.jottery.android.ui.component

import android.graphics.Bitmap
import android.text.format.Formatter
import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Image
import androidx.compose.material.icons.filled.InsertDriveFile
import androidx.compose.material.icons.filled.Share
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.jottery.android.R
import com.jottery.android.model.AttachmentRef

/**
 * A note's attachments: tap to open, share, and remove. Removing is only offered
 * when [onRemove] is set, so a locked note can still be read.
 */
@Composable
fun AttachmentListView(
    attachments: List<AttachmentRef>,
    onOpen: (AttachmentRef) -> Unit,
    onShare: (AttachmentRef) -> Unit,
    onRemove: ((AttachmentRef) -> Unit)?,
    loadThumbnail: suspend (AttachmentRef) -> Bitmap?,
    modifier: Modifier = Modifier,
) {
    Column(
        modifier = modifier
            .heightIn(max = 176.dp)
            .verticalScroll(rememberScrollState()),
    ) {
        attachments.forEach { attachment ->
            val name = attachment.filename.ifEmpty { stringResource(R.string.attachment_unnamed) }
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { onOpen(attachment) }
                    .padding(horizontal = 12.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                AttachmentIcon(attachment, loadThumbnail)

                Spacer(modifier = Modifier.width(10.dp))

                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = name,
                        style = MaterialTheme.typography.bodySmall,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        text = Formatter.formatShortFileSize(LocalContext.current, (attachment.size).toLong()),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }

                IconButton(onClick = { onShare(attachment) }) {
                    Icon(
                        Icons.Default.Share,
                        contentDescription = stringResource(R.string.attachment_share, name),
                        modifier = Modifier.size(18.dp),
                    )
                }
                if (onRemove != null) {
                    IconButton(onClick = { onRemove(attachment) }) {
                        Icon(
                            Icons.Default.Close,
                            contentDescription = stringResource(R.string.attachment_remove, name),
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun AttachmentIcon(attachment: AttachmentRef, loadThumbnail: suspend (AttachmentRef) -> Bitmap?) {
    val isImage = attachment.mimeType.startsWith("image/")
    val thumbnail by produceState<Bitmap?>(initialValue = null, attachment.id) {
        if (isImage) value = loadThumbnail(attachment)
    }

    Box(modifier = Modifier.size(36.dp), contentAlignment = Alignment.Center) {
        val bitmap = thumbnail
        if (bitmap != null) {
            Image(
                bitmap = bitmap.asImageBitmap(),
                contentDescription = null,
                contentScale = ContentScale.Crop,
                modifier = Modifier
                    .size(36.dp)
                    .clip(RoundedCornerShape(6.dp)),
            )
        } else {
            Icon(
                imageVector = if (isImage) Icons.Default.Image else Icons.Default.InsertDriveFile,
                contentDescription = null,
                modifier = Modifier.size(20.dp),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

