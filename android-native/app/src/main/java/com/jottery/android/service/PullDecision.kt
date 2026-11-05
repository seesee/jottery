package com.jottery.android.service

import com.jottery.android.util.DateUtils

/** What a pull should do with a note the server sent. */
enum class PullAction {
    /** The local row is already current. */
    SKIP,

    /** The remote row descends from the local one: replace it. */
    FAST_FORWARD,

    /** The remote row descends from the local one, but flags are still queued here. */
    FAST_FORWARD_KEEP_FLAGS,

    /** A synced local row with no provable link to the remote: snapshot, then replace. */
    REPLACE_WITH_SNAPSHOT,

    /** Both sides changed. Nothing is written until the user decides. */
    CONFLICT,
}

/**
 * The hash-chain rules that keep a pull from destroying local work, as set out
 * under "Applying a pulled note" in docs/sync-protocol.md: a row with unsent
 * changes is never overwritten unless the remote provably descends from it, and a
 * row with none takes whatever the server says is current.
 */
object PullDecision {

    data class Side(
        val contentHash: String?,
        val parentHash: String?,
        val hashChain: List<String>,
        val modifiedAt: String,
    )

    /**
     * @param flagsDiffer the rows disagree on pinned, archived, locked, deleted or
     *   a display setting. Clients change these without advancing the hash, so
     *   equal hashes alone don't prove the rows match.
     */
    fun decide(local: Side, remote: Side, localNeedsSync: Boolean, flagsDiffer: Boolean): PullAction {
        val localHash = local.contentHash
        val remoteHash = remote.contentHash

        if (localHash != null && remoteHash != null) {
            if (localHash == remoteHash) {
                return if (flagsDiffer && !localNeedsSync) PullAction.FAST_FORWARD else PullAction.SKIP
            }
            // The remote is an older state of this note: an echo of a push that
            // has since been overtaken by a local edit. Checked before the test
            // for a descendant, so a queued edit can never lose to its own past.
            val isOlderThanLocal = remoteHash == local.parentHash || remoteHash in local.hashChain
            if (localNeedsSync && isOlderThanLocal) return PullAction.SKIP

            // Every text edit advances the hash, so if the remote descends from
            // the current local hash the only thing still queued here is flags.
            if (localHash == remote.parentHash || localHash in remote.hashChain) {
                return if (localNeedsSync) PullAction.FAST_FORWARD_KEEP_FLAGS else PullAction.FAST_FORWARD
            }
            return if (localNeedsSync) PullAction.CONFLICT else PullAction.REPLACE_WITH_SNAPSHOT
        }

        // Notes from before hash chains: fall back to timestamps.
        if (!localNeedsSync) return PullAction.FAST_FORWARD
        val localTime = DateUtils.parseISO8601(local.modifiedAt)
        val remoteTime = DateUtils.parseISO8601(remote.modifiedAt)
        val remoteIsNewer = localTime != null && remoteTime != null && remoteTime.isAfter(localTime)
        return if (remoteIsNewer) PullAction.CONFLICT else PullAction.SKIP
    }
}
