package com.jottery.android.ui.component

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.PushPin
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Checkbox
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.toggleableState
import androidx.compose.ui.state.ToggleableState
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.jottery.android.R
import com.jottery.android.model.DecryptedNote
import com.jottery.android.ui.theme.LocalNoteCategories
import com.jottery.android.ui.theme.NoteColors
import java.time.Duration
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter

@OptIn(ExperimentalFoundationApi::class, ExperimentalLayoutApi::class)
@Composable
fun NoteRowView(
    note: DecryptedNote,
    isSelected: Boolean,
    isMultiSelectMode: Boolean,
    isChecked: Boolean,
    onClick: () -> Unit,
    onLongClick: () -> Unit,
    modifier: Modifier = Modifier,
    hasConflict: Boolean = false,
    onTagClick: ((String) -> Unit)? = null,
    /** What a swipe does, for anyone who can't swipe. */
    accessibilityActions: List<CustomAccessibilityAction> = emptyList(),
) {
    val noteColor = NoteColors.fromString(note.color)
    val categories = LocalNoteCategories.current
    val relativeDate = remember(note.modifiedAt) {
        relativeTime(note.modifiedAt)
    }

    Surface(
        color = if (isSelected && !isMultiSelectMode) {
            MaterialTheme.colorScheme.secondaryContainer.copy(alpha = 0.5f)
        } else {
            MaterialTheme.colorScheme.surface
        },
        modifier = modifier
            .fillMaxWidth()
            .combinedClickable(
                onClick = onClick,
                onLongClick = onLongClick,
                onClickLabel = stringResource(
                    if (isMultiSelectMode) R.string.note_row_select else R.string.note_row_open
                ),
                onLongClickLabel = if (isMultiSelectMode) null else stringResource(R.string.note_row_select),
            )
            .semantics {
                if (accessibilityActions.isNotEmpty()) customActions = accessibilityActions
                if (isMultiSelectMode) {
                    role = Role.Checkbox
                    toggleableState = ToggleableState(isChecked)
                }
            },
    ) {
        Row(
            // Lets the colour stripe follow the text, however large that's set
            modifier = Modifier
                .fillMaxWidth()
                .height(IntrinsicSize.Min)
                .padding(horizontal = 12.dp, vertical = 10.dp),
            verticalAlignment = Alignment.Top,
        ) {
            // Multi-select checkbox
            if (isMultiSelectMode) {
                Checkbox(
                    checked = isChecked,
                    onCheckedChange = null, // Handled by row click
                    modifier = Modifier
                        .padding(end = 8.dp)
                        .size(20.dp),
                )
            }

            // Colour stripe on the left edge
            if (noteColor != null) {
                Box(
                    modifier = Modifier
                        .testTag("colour-stripe")
                        .width(4.dp)
                        .fillMaxHeight()
                        .clip(RoundedCornerShape(2.dp))
                        .background(noteColor),
                )
                Spacer(modifier = Modifier.width(10.dp))
            }

            // Note content column
            Column(
                modifier = Modifier.weight(1f),
            ) {
                // Title row with indicators
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    // Sync conflict indicator
                    if (hasConflict) {
                        Icon(
                            imageVector = Icons.Default.Warning,
                            contentDescription = stringResource(R.string.conflict_row_label),
                            modifier = Modifier.size(14.dp),
                            tint = MaterialTheme.colorScheme.error,
                        )
                        Spacer(modifier = Modifier.width(4.dp))
                    }

                    // Pin indicator
                    if (note.pinned) {
                        Icon(
                            imageVector = Icons.Default.PushPin,
                            contentDescription = stringResource(R.string.note_info_pinned),
                            modifier = Modifier
                                .size(14.dp)
                                .padding(end = 2.dp),
                            tint = MaterialTheme.colorScheme.primary.copy(alpha = 0.8f),
                        )
                        Spacer(modifier = Modifier.width(4.dp))
                    }

                    // Lock indicator
                    if (note.locked) {
                        Icon(
                            imageVector = Icons.Default.Lock,
                            contentDescription = stringResource(R.string.note_info_locked),
                            modifier = Modifier
                                .size(14.dp)
                                .padding(end = 2.dp),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
                        )
                        Spacer(modifier = Modifier.width(4.dp))
                    }

                    // Title
                    Text(
                        text = note.title,
                        style = MaterialTheme.typography.titleMedium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f),
                    )
                }

                // Preview snippet
                if (note.preview.isNotEmpty()) {
                    Spacer(modifier = Modifier.height(2.dp))
                    Text(
                        text = note.preview,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }

                // Bottom row: date, tags, attachment indicator
                Spacer(modifier = Modifier.height(4.dp))
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    // Relative date
                    Text(
                        text = relativeDate,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
                    )

                    // Tags
                    if (note.regularTags.isNotEmpty()) {
                        Spacer(modifier = Modifier.width(8.dp))
                        FlowRow(
                            horizontalArrangement = Arrangement.spacedBy(4.dp),
                            modifier = Modifier.weight(1f, fill = false),
                        ) {
                            note.regularTags.take(3).forEach { tag ->
                                val tagColour = NoteColors.fromString(categories.colourOfTag(tag))
                                if (tagColour != null) {
                                    Box(
                                        modifier = Modifier
                                            .align(Alignment.CenterVertically)
                                            .size(8.dp)
                                            .clip(CircleShape)
                                            .background(tagColour),
                                    )
                                }
                                Text(
                                    text = "#$tag",
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.primary,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                    modifier = if (onTagClick != null) {
                                        Modifier.clickable { onTagClick(tag) }
                                    } else {
                                        Modifier
                                    },
                                )
                            }
                            if (note.regularTags.size > 3) {
                                Text(
                                    text = "+${note.regularTags.size - 3}",
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
                                )
                            }
                        }
                    }

                    // Spacer to push attachment icon to the right
                    Spacer(modifier = Modifier.weight(1f))

                    // Attachment indicator
                    if (note.attachments.isNotEmpty()) {
                        Icon(
                            imageVector = Icons.Default.AttachFile,
                            contentDescription = stringResource(R.string.note_row_has_attachments),
                            modifier = Modifier.size(14.dp),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
                        )
                    }
                }
            }
        }
    }
}
