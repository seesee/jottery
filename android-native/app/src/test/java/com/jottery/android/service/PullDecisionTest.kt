package com.jottery.android.service

import org.junit.Assert.assertEquals
import org.junit.Test

class PullDecisionTest {
    private fun side(hash: String?, parent: String? = null, chain: List<String> = emptyList(), at: String = "2026-01-01T00:00:00.000Z") =
        PullDecision.Side(hash, parent, chain, at)

    private fun decide(local: PullDecision.Side, remote: PullDecision.Side, needsSync: Boolean, flagsDiffer: Boolean = false) =
        PullDecision.decide(local, remote, needsSync, flagsDiffer)

    @Test fun `equal hashes are skipped`() =
        assertEquals(PullAction.SKIP, decide(side("a"), side("a"), needsSync = false))

    @Test fun `equal hashes with different flags take the remote when clean`() =
        assertEquals(PullAction.FAST_FORWARD, decide(side("a"), side("a"), needsSync = false, flagsDiffer = true))

    @Test fun `equal hashes never overwrite queued flags`() =
        assertEquals(PullAction.SKIP, decide(side("a"), side("a"), needsSync = true, flagsDiffer = true))

    @Test fun `a descendant of a clean note fast-forwards`() =
        assertEquals(PullAction.FAST_FORWARD, decide(side("a"), side("b", "a", listOf("b", "a")), needsSync = false))

    @Test fun `a descendant found only through the chain fast-forwards`() =
        assertEquals(PullAction.FAST_FORWARD, decide(side("a"), side("c", "b", listOf("c", "b", "a")), needsSync = false))

    @Test fun `a descendant of a note with queued flags keeps them`() =
        assertEquals(PullAction.FAST_FORWARD_KEEP_FLAGS, decide(side("a"), side("b", "a", listOf("b", "a")), needsSync = true))

    @Test fun `an older remote state is skipped, even when the local note is queued`() =
        assertEquals(PullAction.SKIP, decide(side("c", "b", listOf("c", "b", "a")), side("b"), needsSync = true))

    @Test fun `the parent of a queued edit is older, even if the chain was trimmed`() =
        assertEquals(PullAction.SKIP, decide(side("c", "b", listOf("c")), side("b"), needsSync = true))

    @Test fun `a shared ancestor alone doesn't make the remote newer`() =
        assertEquals(
            PullAction.CONFLICT,
            decide(side("b", "a", listOf("b", "a")), side("c", "a", listOf("c", "a")), needsSync = true),
        )

    @Test fun `an older remote with no queued edit is what the server now holds`() =
        assertEquals(
            PullAction.REPLACE_WITH_SNAPSHOT,
            decide(side("c", "b", listOf("c", "b", "a")), side("b"), needsSync = false),
        )

    @Test fun `diverged with queued edits is a conflict`() =
        assertEquals(PullAction.CONFLICT, decide(side("b", "a", listOf("b", "a")), side("c", "a", listOf("c", "a")), needsSync = true))

    @Test fun `diverged but clean takes the remote behind a snapshot`() =
        assertEquals(PullAction.REPLACE_WITH_SNAPSHOT, decide(side("b", "a", listOf("b", "a")), side("c", "a", listOf("c", "a")), needsSync = false))

    @Test fun `no hashes and clean takes the remote`() =
        assertEquals(PullAction.FAST_FORWARD, decide(side(null), side(null), needsSync = false))

    @Test fun `no hashes with a newer remote is a conflict when queued`() =
        assertEquals(
            PullAction.CONFLICT,
            decide(side(null), side(null, at = "2026-02-01T00:00:00+00:00"), needsSync = true),
        )

    @Test fun `no hashes with a newer local keeps the local note`() =
        assertEquals(
            PullAction.SKIP,
            decide(side(null, at = "2026-03-01T00:00:00.000Z"), side(null, at = "2026-02-01T00:00:00.000Z"), needsSync = true),
        )
}
