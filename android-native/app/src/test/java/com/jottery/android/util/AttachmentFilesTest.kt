package com.jottery.android.util

import org.junit.Assert.assertEquals
import org.junit.Test

class AttachmentFilesTest {
    @Test fun `a hostile filename can't leave the folder`() {
        assertEquals("passwd", AttachmentFiles.safeName("../../../etc/passwd"))
        assertEquals("evil.exe", AttachmentFiles.safeName("C:\\temp\\evil.exe"))
        assertEquals("a.txt", AttachmentFiles.safeName("a\u0000.txt"))
    }

    @Test fun `an empty result falls back`() {
        assertEquals("attachment", AttachmentFiles.safeName(""))
        assertEquals("attachment", AttachmentFiles.safeName(".."))
        assertEquals("attachment", AttachmentFiles.safeName("/"))
    }

    @Test fun `ordinary names are kept`() =
        assertEquals("holiday photo.jpg", AttachmentFiles.safeName("holiday photo.jpg"))
}
