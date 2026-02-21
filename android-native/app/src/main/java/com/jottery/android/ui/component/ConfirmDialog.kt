package com.jottery.android.ui.component

import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable

/**
 * Reusable confirmation dialog using Material3 AlertDialog.
 *
 * Used throughout the app to replace native browser `confirm()` dialogs,
 * following the project's architecture principle of never using native
 * browser controls.
 *
 * @param title Dialog title text.
 * @param message Dialog message/body text.
 * @param confirmText Text for the confirm action button.
 * @param onConfirm Callback invoked when the user confirms the action.
 * @param onDismiss Callback invoked when the user cancels or dismisses the dialog.
 * @param confirmDestructive When true, the confirm button uses the error colour
 *   to indicate a destructive action (default: true).
 * @param cancelText Text for the cancel button (default: "Cancel").
 */
@Composable
fun ConfirmDialog(
    title: String,
    message: String,
    confirmText: String,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
    confirmDestructive: Boolean = true,
    cancelText: String = "Cancel",
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(
                text = title,
                style = MaterialTheme.typography.titleMedium,
            )
        },
        text = {
            Text(
                text = message,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        },
        confirmButton = {
            TextButton(
                onClick = onConfirm,
                colors = if (confirmDestructive) {
                    ButtonDefaults.textButtonColors(
                        contentColor = MaterialTheme.colorScheme.error,
                    )
                } else {
                    ButtonDefaults.textButtonColors()
                },
            ) {
                Text(confirmText)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(cancelText)
            }
        },
    )
}
