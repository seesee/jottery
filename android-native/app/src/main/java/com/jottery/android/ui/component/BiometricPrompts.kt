package com.jottery.android.ui.component

import android.content.Context
import androidx.fragment.app.FragmentActivity
import com.jottery.android.R
import com.jottery.android.crypto.KeystoreService
import com.jottery.android.util.BiometricHelper
import com.jottery.android.viewmodel.AppState

/** Compose wraps the activity in contexts of its own, so look through them. */
fun Context.fragmentActivity(): FragmentActivity? {
    var current: Context? = this
    while (current is android.content.ContextWrapper) {
        if (current is FragmentActivity) return current
        current = current.baseContext
    }
    return null
}

/**
 * Turn on biometric unlock. The key is sealed inside the prompt's success, because
 * the Keystore won't let it be used any other way.
 */
fun enableBiometricUnlock(context: Context, appState: AppState, onFailed: (String) -> Unit) {
    val activity = context.fragmentActivity() ?: return
    val cipher = try {
        KeystoreService.biometricEncryptCipher()
    } catch (e: Exception) {
        onFailed(context.getString(R.string.biometric_failed))
        return
    }
    BiometricHelper.authenticate(
        activity = activity,
        cipher = cipher,
        title = context.getString(R.string.biometric_enable_title),
        subtitle = context.getString(R.string.biometric_enable_subtitle),
        negativeButton = context.getString(R.string.action_cancel),
        onSuccess = { authorised -> appState.enableBiometric(authorised) },
        onError = onFailed,
    )
}
