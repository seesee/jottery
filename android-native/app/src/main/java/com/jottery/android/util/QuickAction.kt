package com.jottery.android.util

import android.net.Uri

/** What a launcher shortcut asks for. Any app can send the activity a link, so only these are read. */
enum class QuickAction(private val host: String) {
    NEW_NOTE("new_note"),
    SEARCH("search");

    companion object {
        fun from(uri: Uri?): QuickAction? {
            if (uri?.scheme != "jottery") return null
            return entries.firstOrNull { it.host == uri.host }
        }
    }
}
