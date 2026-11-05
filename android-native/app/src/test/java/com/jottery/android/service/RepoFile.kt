package com.jottery.android.service

import java.io.File

/** A file shared with the other clients, found by walking up from the test's directory. */
object RepoFile {
    fun at(path: String): File =
        generateSequence(File("").absoluteFile) { it.parentFile }
            .map { File(it, path) }
            .firstOrNull { it.exists() }
            ?: error("$path not found: this test needs the whole repository")
}
