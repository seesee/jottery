import CryptoKit
import Foundation
import GRDB

/// Orchestrates push/pull sync cycles with the server.
actor SyncService {

    private let syncClient: SyncClient
    private let noteRepo: NoteRepository
    private let syncRepo: SyncRepository
    private let versionRepo: VersionRepository
    private let attachmentRepo: AttachmentRepository
    private let savedSearchRepo: SavedSearchRepository
    private let key: SymmetricKey
    private let sseClient: SSEClient

    private(set) var isSyncing = false
    private(set) var lastSyncAt: String?
    private(set) var lastError: String?
    private(set) var pendingConflicts: [ConflictInfo] = []
    private var postSyncHandler: (@Sendable (SyncChanges) async -> Void)?
    /// Set when the server said to wait. No sync starts until then.
    private var retryNotBefore: Date?
    /// The times on whatever the last push sent and heard nothing back about.
    private var unansweredTimes: [String] = []
    /// The server's clock, from the last pull.
    private var lastServerTime: Date?

    /// - Parameter sseClient: replaces the live-updates listener. Only used by
    ///   tests, which need to reach it once the service has gone.
    init(syncClient: SyncClient, noteRepo: NoteRepository, syncRepo: SyncRepository, versionRepo: VersionRepository, attachmentRepo: AttachmentRepository, savedSearchRepo: SavedSearchRepository, key: SymmetricKey, sseClient: SSEClient? = nil) {
        self.syncClient = syncClient
        self.noteRepo = noteRepo
        self.syncRepo = syncRepo
        self.versionRepo = versionRepo
        self.attachmentRepo = attachmentRepo
        self.savedSearchRepo = savedSearchRepo
        self.key = key
        self.sseClient = sseClient ?? SSEClient(syncClient: syncClient)
    }

    /// Set a handler called after SSE-triggered syncs complete.
    func setPostSyncHandler(_ handler: @escaping @Sendable (SyncChanges) async -> Void) {
        postSyncHandler = handler
    }

    // MARK: - Full Sync

    /// Run a full push-then-pull sync cycle. Returns the combined set of note
    /// ids the cycle touched so callers can refresh incrementally instead of
    /// reloading the whole vault.
    ///
    /// A push that fails doesn't stop the pull: otherwise one note the server
    /// refuses would keep this device from receiving anything. Its error is
    /// thrown once the pull is done.
    @discardableResult
    func sync() async throws -> SyncChanges {
        guard !isSyncing, !isShutDown else { return .skipped }
        if let retryNotBefore, Date() < retryNotBefore { return .skipped }
        isSyncing = true
        lastError = nil

        defer { isSyncing = false }

        do {
            await uploadPendingWrappedKey()
            var (changes, pushError) = await pushCapturingFailure()
            do {
                changes.merge(try await pull())
            } catch {
                throw pushError ?? error
            }
            changes.clockLooksWrong = clockLooksWrong
            try await finalise()
            await postSyncHandler?(changes)
            if let pushError { throw pushError }
            return changes
        } catch {
            noteFailure(error)
            throw error
        }
    }

    /// A push that throws has still marked earlier batches synced, without
    /// reporting which, so the caller is told to reload.
    private func pushCapturingFailure() async -> (SyncChanges, Error?) {
        do {
            return (try await push(), nil)
        } catch {
            return (.fullReload, error)
        }
    }

    /// How far ahead of its clock the server lets a time be (docs/sync-protocol.md).
    private static let clockTolerance: TimeInterval = 24 * 60 * 60

    /// The server skips anything dated over a day ahead of it, and says so only
    /// in `errors`, which aren't for parsing. So this looks at what it answered
    /// nothing about and compares those times with the server's clock, because
    /// the device's own clock is the one in doubt.
    private var clockLooksWrong: Bool {
        guard let limit = lastServerTime?.addingTimeInterval(Self.clockTolerance) else { return false }
        return unansweredTimes.contains { time in
            Date(iso8601: time).map { $0 > limit } ?? false
        }
    }

    private func noteFailure(_ error: Error) {
        lastError = error.localizedDescription
        if let wait = (error as? SyncClientError)?.requestedWait {
            retryNotBefore = Date().addingTimeInterval(wait)
        }
    }

    /// A key a password change left waiting. Failing here isn't a failed sync:
    /// it stays queued, and the rest of the cycle doesn't need it.
    private func uploadPendingWrappedKey() async {
        do {
            try await EnvelopeService.flushPendingWrappedKey(syncClient: syncClient, syncRepo: syncRepo)
        } catch {
            Log.debug("[Sync] couldn't upload the wrapped key a password change left waiting: \(error)")
        }
    }

    /// Note the time of the last completed pull. Called after push + pull.
    func finalise() async throws {
        lastSyncAt = try syncRepo.getMetadata()?.lastSyncAt
    }

    // MARK: - Push

    /// Build the attachments-only repair payload for server attachment
    /// warnings: every warned blob the lookup can supply, deduplicated by id.
    /// Pure logic split out for testability.
    static func collectRepairAttachments(
        for warnings: [AttachmentWarning],
        blobLookup: (String) -> Data?
    ) -> [SyncAttachment] {
        var repairs: [SyncAttachment] = []
        var seen = Set<String>()
        for warning in warnings {
            for attachmentId in warning.attachmentIds {
                guard seen.insert(attachmentId).inserted else { continue }
                if let blob = blobLookup(attachmentId) {
                    repairs.append(SyncAttachment(id: attachmentId, data: blob.base64EncodedString()))
                }
            }
        }
        return repairs
    }

    /// Notes per push request. Large pending batches (e.g. after being
    /// offline for days) are split into sequential chunks of this size —
    /// each chunk's acceptances are durably marked (`markSynced`) before the
    /// next chunk is sent, so a connection that drops mid-batch only loses
    /// the unsent tail instead of the whole push (jottery-l9dt).
    static let pushChunkSize = 50

    /// Most bytes of notes and blobs per push request. The server's per-request
    /// upload limit defaults to 5 MB, and a push over it fails with nothing
    /// written. A limit set lower than this is handled by `push()` splitting
    /// the batch when it's refused.
    static let pushByteBudget = 4 * 1024 * 1024

    /// Pure chunking arithmetic, split out for testability: split `records`
    /// into consecutive groups of at most `size` records and `maxBytes` bytes,
    /// preserving order. A record over `maxBytes` gets a group to itself.
    static func chunked(
        _ records: [NoteRecord],
        size: Int,
        maxBytes: Int = .max,
        bytes: (NoteRecord) -> Int = { _ in 0 }
    ) -> [[NoteRecord]] {
        guard !records.isEmpty else { return [] }
        guard size > 0 else { return [records] }
        var chunks: [[NoteRecord]] = []
        var current: [NoteRecord] = []
        var currentBytes = 0
        for record in records {
            let recordBytes = bytes(record)
            if !current.isEmpty, current.count >= size || currentBytes + recordBytes > maxBytes {
                chunks.append(current)
                current = []
                currentBytes = 0
            }
            current.append(record)
            currentBytes += recordBytes
        }
        chunks.append(current)
        return chunks
    }

    /// Roughly what pushing `record` adds to a request, as the server counts it.
    private func pushBytes(of record: NoteRecord) -> Int {
        let blobIds = Self.decodeAttachments(record.attachments).map(\.data)
        let blobBytes = (try? attachmentRepo.uploadSizes(ids: blobIds).values.reduce(0, +)) ?? 0
        return record.content.utf8.count + record.tags.utf8.count + blobBytes * 4 / 3
    }

    /// Whether every record's attachment list could be read, and so is safe
    /// to call complete. An unreadable one goes up as an empty list, and the
    /// server mustn't take that to mean the attachments are gone.
    private static func attachmentListsAreComplete(_ records: [NoteRecord]) -> Bool {
        records.allSatisfy {
            (try? JSONDecoder().decode([AttachmentRef].self, from: Data($0.attachments.utf8))) != nil
        }
    }

    /// Build sync-format notes for a batch of records (the full push set or
    /// a single chunk of it).
    private func buildSyncNotes(for records: [NoteRecord]) -> [SyncNote] {
        records.map { record in
            // Decrypt tags from single blob, then encrypt each tag individually
            // for the sync format (web expects individually encrypted tags).
            let tagsArray: [String] = Self.storageTagsToSyncTags(record.tags, key: key)

            let attachments = Self.decodeAttachments(record.attachments)

            let hashChain: [String]?
            if let chainStr = record.hashChain, let data = chainStr.data(using: .utf8) {
                hashChain = try? JSONDecoder().decode([String].self, from: data)
            } else {
                hashChain = nil
            }

            return SyncNote(
                id: record.id,
                createdAt: record.createdAt,
                modifiedAt: record.modifiedAt,
                content: record.content,
                tags: tagsArray,
                attachments: attachments,
                pinned: record.pinned,
                archived: record.archived,
                archivedAt: record.archivedAt,
                locked: record.locked,
                lockedAt: record.lockedAt,
                deleted: record.deleted,
                deletedAt: record.deletedAt,
                version: record.version,
                wordWrap: record.wordWrap,
                syntaxLanguage: record.syntaxLanguage,
                showPreview: record.showPreview,
                color: record.color,
                contentHash: record.contentHash,
                parentHash: record.parentHash,
                hashChain: hashChain
            )
        }
    }

    /// Gather version snapshots for a batch of records.
    private func buildSyncVersions(for records: [NoteRecord]) throws -> [SyncNoteVersion] {
        var syncVersions: [SyncNoteVersion] = []
        for record in records {
            let versions = try versionRepo.getVersions(noteId: record.id)
            for ver in versions {
                // Convert storage-format tags to sync-format for the version
                let verSyncTags = Self.storageTagsToSyncTags(ver.tags, key: key)
                syncVersions.append(SyncNoteVersion(
                    versionKey: ver.versionKey,
                    noteId: ver.noteId,
                    version: ver.version,
                    createdAt: ver.createdAt,
                    syncedAt: ver.syncedAt,
                    content: ver.content,
                    tags: verSyncTags,
                    attachments: (try? JSONDecoder().decode([AttachmentRef].self, from: Data(ver.attachments.utf8))) ?? [],
                    syntaxLanguage: ver.syntaxLanguage,
                    wordWrap: ver.wordWrap,
                    showPreview: ver.showPreview,
                    color: ver.color,
                    reason: ver.reason,
                    contentHash: ver.contentHash,
                    parentHash: ver.parentHash
                ))
            }
        }
        return syncVersions
    }

    /// Gather attachment blobs for a batch of sync notes (ride along with
    /// their note, deduplicated within the batch). Only blobs the server isn't
    /// known to hold: sending them all on every edit could put a note over the
    /// server's limit for good. A blob it lost anyway comes back through
    /// `attachmentWarnings`.
    /// Use ref.id as the SyncAttachment ID — the server's attachments_data.id
    /// is a FK to attachments_meta.id, so they must match.
    private func buildSyncAttachments(for notes: [SyncNote]) -> [SyncAttachment] {
        var syncAttachments: [SyncAttachment] = []
        var seenAttachmentIds = Set<String>()
        for note in notes {
            for ref in note.attachments {
                guard seenAttachmentIds.insert(ref.id).inserted else { continue }
                do {
                    if let blobData = try attachmentRepo.getBlobToUpload(id: ref.data) {
                        syncAttachments.append(SyncAttachment(
                            id: ref.id,
                            data: blobData.base64EncodedString()
                        ))
                    }
                    // A blob that's missing here too is no reason to hold the
                    // note back: the server warns if it has no copy either.
                } catch {
                    Log.debug("[Sync] push: attachment \(ref.id) (note \(note.id)) could not be read (\(error)) — pushing note without it")
                }
            }
        }
        return syncAttachments
    }

    /// Push locally-changed notes/attachments/versions/deletions/saved
    /// searches to the server. Returns the set of note ids this push wrote
    /// to locally (accepted notes get `markSynced` — new `syncedAt`/version —
    /// even though content is unchanged) so callers can refresh incrementally.
    ///
    /// Sends `listNeedingSync()` in sequential batches limited by note count
    /// and bytes (attachments/versions ride with their note). Each batch's
    /// response is fully processed — `markSynced`, version snapshots,
    /// attachment-warning healing, conflict queuing — before the next batch
    /// is sent, so a mid-batch network failure leaves already-accepted
    /// batches durably marked instead of losing the whole push. A failed
    /// batch throws and aborts the rest, except that one the server says is
    /// too large is split, so a note it can't take is left behind and the
    /// others still go. Deletions and saved searches go last, in a request of
    /// their own, so they don't wait for a note to change.
    @discardableResult
    func push() async throws -> SyncChanges {
        var changes = SyncChanges()
        unansweredTimes = []
        let records = try noteRepo.listNeedingSync()
        let deletions = try syncRepo.getPendingDeletions()
        let savedSearchRecords = try savedSearchRepo.listNeedingSync()
        Log.debug("[Sync] push: \(records.count) notes, \(deletions.count) deletions, \(savedSearchRecords.count) saved searches")
        guard !records.isEmpty || !deletions.isEmpty || !savedSearchRecords.isEmpty else { return changes }

        // Guard against clobbering an edit made while the push was in
        // flight: markSynced only clears needs_sync if modified_at still
        // matches the snapshot that was actually pushed (captured here,
        // before any chunk is sent).
        let pushed = Dictionary(uniqueKeysWithValues: records.map { ($0.id, $0) })

        let bytesById = Dictionary(uniqueKeysWithValues: records.map { ($0.id, pushBytes(of: $0)) })
        var queue = Self.chunked(
            records, size: Self.pushChunkSize, maxBytes: Self.pushByteBudget, bytes: { bytesById[$0.id] ?? 0 }
        )
        var tooLarge = 0
        var quotaReached = false

        while !queue.isEmpty, !quotaReached {
            let batch = queue.removeFirst()
            do {
                try await pushBatch(batch, pushed: pushed, changes: &changes)
            } catch SyncClientError.httpError(let status, _) where status == 413 {
                if batch.count > 1 {
                    let middle = batch.count / 2
                    queue.insert(contentsOf: [Array(batch[..<middle]), Array(batch[middle...])], at: 0)
                } else {
                    Log.debug("[Sync] push: note \(batch[0].id) is over the server's limit — leaving it behind")
                    tooLarge += 1
                }
            } catch SyncClientError.storageQuotaExceeded {
                quotaReached = true
            }
        }

        // A push that only deletes works at the quota too, which is how the
        // user makes room, so deletions go up on their own.
        if !deletions.isEmpty {
            try await pushDeletions(deletions)
        }
        if !savedSearchRecords.isEmpty, !quotaReached {
            try await pushSavedSearches(savedSearchRecords)
        }

        try syncRepo.updateLastPush(at: Date().iso8601)
        if quotaReached { throw SyncClientError.storageQuotaExceeded }
        if tooLarge > 0 { throw SyncServiceError.notesTooLarge(count: tooLarge) }
        return changes
    }

    /// Send one batch of notes and act on what the server says about it.
    private func pushBatch(
        _ batch: [NoteRecord],
        pushed: [String: NoteRecord],
        changes: inout SyncChanges
    ) async throws {
        let syncNotes = buildSyncNotes(for: batch)
        let request = SyncPushRequest(
            notes: syncNotes,
            attachments: buildSyncAttachments(for: syncNotes),
            versions: try buildSyncVersions(for: batch),
            attachmentsAuthoritative: Self.attachmentListsAreComplete(batch) ? true : nil
        )

        let response = try await syncClient.push(request)
        Log.debug("[Sync] push: \(syncNotes.count) notes: accepted=\(response.accepted.count), rejected=\(response.rejected.count), errors=\(response.errors ?? [])")
        let now = Date().iso8601

        // Mark accepted notes as synced and create version snapshots. The server
        // only gets to speak for notes this batch actually sent.
        let inBatch = Set(batch.map(\.id))
        var acceptedBlobIds: [String] = []
        for accepted in response.accepted where inBatch.contains(accepted.id) {
            do {
                let sent = pushed[accepted.id]
                try noteRepo.markSynced(
                    id: accepted.id,
                    syncedAt: accepted.syncedAt.flatMap { SyncValidation.isTimestamp($0) ? $0 : nil } ?? now,
                    serverVersion: accepted.serverVersion,
                    ifModifiedAt: sent?.modifiedAt,
                    pushedHash: sent?.contentHash
                )
                changes.updatedIds.insert(accepted.id)
                acceptedBlobIds += Self.decodeAttachments(sent?.attachments ?? "[]").map(\.data)
                // Create a version snapshot of the accepted state
                if let record = try noteRepo.getRaw(id: accepted.id) {
                    try versionRepo.createVersion(from: record, reason: "sync")
                }
            } catch {
                Log.debug("[Sync] push: failed to mark accepted note \(accepted.id): \(error)")
            }
        }
        // A blob for a note that wasn't accepted is ignored by the server.
        try attachmentRepo.markUploaded(ids: acceptedBlobIds)

        // React to server attachment warnings: re-upload blobs this device
        // holds (one attachments-only push, no in-run retry — the next regular
        // sync retries naturally); log the ones it cannot heal.
        if let warnings = response.attachmentWarnings, !warnings.isEmpty {
            // Attachment healing can touch note records in ways this method
            // doesn't individually track — be conservative and let the
            // caller fall back to a full reload.
            changes.fullReloadRequired = true
            let repairs = Self.collectRepairAttachments(for: warnings) { id in
                try? attachmentRepo.getBlob(id: id)
            }
            for warning in warnings {
                for attachmentId in warning.attachmentIds where !repairs.contains(where: { $0.id == attachmentId }) {
                    Log.debug("[Sync] push: server has no data for attachment \(attachmentId) (note \(warning.noteId)) and neither does this device")
                }
            }
            if !repairs.isEmpty {
                Log.debug("[Sync] push: re-uploading \(repairs.count) attachment(s) the server is missing")
                let repairRequest = SyncPushRequest(notes: [], attachments: repairs, versions: [])
                do {
                    _ = try await syncClient.push(repairRequest)
                } catch {
                    Log.debug("[Sync] push: attachment repair push failed — \(error)")
                }
            }
        }

        // Handle rejected (conflicts) — queue for user resolution
        for rejected in response.rejected where inBatch.contains(rejected.id) {
            guard !pendingConflicts.contains(where: { $0.id == rejected.id }),
                  let record = try noteRepo.getRaw(id: rejected.id) else { continue }

            // Decrypt local content for display
            let local = Self.decryptNoteForDisplay(
                content: record.content, tags: record.tags, key: key
            )

            // Decrypt server content for display (tags are in sync format)
            let server = Self.decryptServerNoteForDisplay(
                content: rejected.serverContent, syncTags: rejected.serverTags, key: key
            )

            pendingConflicts.append(ConflictInfo(
                id: rejected.id,
                localContent: local?.content ?? "",
                localTags: local?.tags ?? [],
                localModifiedAt: record.modifiedAt,
                localAttachments: Self.decodeAttachments(record.attachments),
                serverContent: server?.content ?? "",
                serverTags: server?.tags ?? [],
                serverModifiedAt: rejected.serverModifiedAt,
                serverEncryptedContent: rejected.serverContent,
                serverEncryptedTags: rejected.serverTags,
                serverVersion: rejected.serverVersion,
                serverAttachments: rejected.serverAttachments,
                serverPinned: rejected.serverPinned,
                serverSyntaxLanguage: rejected.serverSyntaxLanguage,
                serverWordWrap: rejected.serverWordWrap,
                serverShowPreview: rejected.serverShowPreview,
                serverContentHash: rejected.serverContentHash,
                serverParentHash: rejected.serverParentHash,
                serverHashChain: rejected.serverHashChain,
                serverArchived: rejected.serverArchived,
                serverArchivedAt: rejected.serverArchivedAt,
                serverLocked: rejected.serverLocked,
                serverLockedAt: rejected.serverLockedAt,
                serverColor: rejected.serverColor,
                serverDeleted: rejected.serverDeleted,
                serverDeletedAt: rejected.serverDeletedAt,
                ancestor: ConflictAncestor.decrypted(
                    content: rejected.ancestorContent, tags: rejected.ancestorTags, key: key
                ),
                localUnreadable: local == nil,
                serverUnreadable: server == nil
            ))
        }

        try applySkippedNotes(in: response, batch: batch, pushed: pushed, changes: &changes)
    }

    /// Act on the notes the server left out of `accepted` and `rejected`. Only
    /// what this batch sent counts: the server can't name other notes here.
    private func applySkippedNotes(
        in response: SyncPushResponse,
        batch: [NoteRecord],
        pushed: [String: NoteRecord],
        changes: inout SyncChanges
    ) throws {
        let answered = Set(response.accepted.map(\.id)).union(response.rejected.map(\.id))
        let deletedElsewhere = Set(response.skippedDeleted ?? []).subtracting(answered)

        for record in batch where !answered.contains(record.id) {
            guard deletedElsewhere.contains(record.id) else {
                unansweredTimes += [
                    record.createdAt, record.modifiedAt, record.deletedAt, record.archivedAt, record.lockedAt,
                ].compactMap { $0 }
                continue
            }
            // A conflict is an edit the user hasn't decided about yet.
            guard !pendingConflicts.contains(where: { $0.id == record.id }) else { continue }
            let result = try noteRepo.applySkippedDeletion(id: record.id, sentModifiedAt: pushed[record.id]?.modifiedAt)
            if result == .movedToBin {
                changes.deletedIds.insert(record.id)
                changes.removedByServerIds.insert(record.id)
            }
        }
    }

    /// Send pending deletions.
    ///
    /// A deletion is done with once the server has answered, whatever it said.
    /// One it refused (`rejectedDeletions`, absent on an older server) lost to a
    /// newer edit, and the note comes back through a normal pull. Sending it
    /// again would only re-announce the note to every device on every push.
    private func pushDeletions(_ deletions: [(id: String, deletedAt: String)]) async throws {
        let response = try await syncClient.push(SyncPushRequest(
            notes: [],
            attachments: [],
            versions: [],
            deletions: deletions.map { SyncDeletion(id: $0.id, deletedAt: $0.deletedAt) }
        ))
        if let refused = response.rejectedDeletions, !refused.isEmpty {
            Log.debug("[Sync] push: \(refused.count) deletion(s) refused: the notes have newer edits")
        }
        try syncRepo.clearDeletions(ids: deletions.map(\.id))
    }

    private func pushSavedSearches(_ records: [SavedSearchRepository.SavedSearchRecord]) async throws {
        let syncSavedSearches = records.map { record in
            SyncSavedSearch(
                id: record.id,
                name: record.name,
                query: record.query,
                order: record.displayOrder,
                createdAt: record.createdAt,
                modifiedAt: record.modifiedAt,
                deleted: record.deleted,
                deletedAt: record.deletedAt,
                version: record.version
            )
        }
        let response = try await syncClient.push(SyncPushRequest(
            notes: [], attachments: [], versions: [], savedSearches: syncSavedSearches
        ))

        // Searches are all this request holds, so an error means one was skipped.
        // Nothing says which, and sending them all again is harmless.
        if let errors = response.errors, !errors.isEmpty {
            Log.debug("[Sync] push: the server skipped saved searches: \(errors.count)")
            unansweredTimes += records.map(\.modifiedAt)
            return
        }

        let now = Date().iso8601
        for record in records {
            do {
                try savedSearchRepo.markSynced(id: record.id, syncedAt: now)
            } catch {
                Log.debug("[Sync] push: failed to mark saved search \(record.id) synced: \(error)")
            }
        }
    }

    // MARK: - Pull

    /// Changes per pull request.
    static let pullPageSize = 100

    /// A server that keeps saying there's more is stopped here. No real pull
    /// comes anywhere near this.
    static let maxPullPages = 100_000

    /// How far `lastSyncAt` is set back from the server's time. Only used if the
    /// server can't do cursors: a change stamped just before that time but
    /// committed just after would otherwise be missed. Applying is idempotent,
    /// so receiving something twice is harmless.
    static let pullOverlap: TimeInterval = 120

    /// Pull remote changes and apply them locally. Returns the set of note
    /// ids this pull wrote or removed, plus `fullReloadRequired` for any
    /// write this method can't attribute to a specific id (decrypt/DB
    /// failures, saved-search changes) — the conservative, always-correct
    /// fallback for `AppState.applySyncChanges`.
    @discardableResult
    func pull() async throws -> SyncChanges {
        let metadata = try syncRepo.getMetadata()
        return try await runPull(
            cursor: metadata?.pullCursor ?? "",
            lastSyncAt: metadata?.lastSyncAt,
            knownAttachmentIds: try attachmentRepo.listBlobIds(),
            // Pulling by timestamp can have skipped changes without anyone
            // knowing, so the first time cursors work, start from the top.
            fullPullIfCursorsWork: metadata?.pullCursor == nil && metadata?.lastSyncAt != nil
        )
    }

    /// Pull every page, against a server with or without cursors: one with
    /// them answers with `nextCursor` and ignores `offset`, and an older one
    /// ignores `cursor` and goes by `offset`. Checked on every response, since
    /// a server can be rolled back or replaced.
    ///
    /// The cursor is saved after its page is applied, so a pull that stops
    /// partway picks up from there. `lastSyncAt` is saved once the pull is
    /// finished, with the time from the first page: the server's clock, and
    /// from before anything that arrived during the pull.
    private func runPull(
        cursor: String,
        lastSyncAt: String?,
        knownAttachmentIds: [String],
        fullPullIfCursorsWork: Bool
    ) async throws -> SyncChanges {
        var changes = SyncChanges()
        var request = SyncPullRequest(
            cursor: cursor,
            lastSyncAt: lastSyncAt,
            knownNoteIds: [],
            knownAttachmentIds: knownAttachmentIds,
            limit: Self.pullPageSize,
            offset: 0
        )
        var fullPullPending = fullPullIfCursorsWork
        var startedAt: Date?
        let pendingDeletions = Set(try syncRepo.getPendingDeletions().map(\.id))
        lastServerTime = nil

        for _ in 0..<Self.maxPullPages {
            let requestedAt = Date()
            let page = SyncValidation.sanitised(try await syncClient.pull(request))
            try apply(page, skipping: pendingDeletions, changes: &changes)
            let serverTime = page.syncedAt.flatMap { Date(iso8601: $0) }
            lastServerTime = lastServerTime ?? serverTime
            startedAt = startedAt ?? serverTime ?? requestedAt

            let hasMore = page.hasMore ?? false
            if let nextCursor = SyncValidation.opaqueToken(page.nextCursor) {
                if fullPullPending {
                    fullPullPending = false
                    request.cursor = ""
                    request.lastSyncAt = nil
                    startedAt = nil
                    continue
                }
                try syncRepo.updatePullCursor(nextCursor)
                let advanced = nextCursor != request.cursor
                request.cursor = nextCursor
                // Only `hasMore` decides: a page can hold no notes at all and
                // still have more behind it. A cursor that doesn't move would
                // fetch the same page for ever.
                guard hasMore, advanced else { break }
            } else {
                try syncRepo.updatePullCursor(nil)
                let received = page.receivedNoteCount ?? page.notes.count
                request.offset = (request.offset ?? 0) + received
                // An older server pages by notes, so an empty page can't move on.
                guard hasMore, received > 0 else { break }
            }
        }

        let overlapped = (startedAt ?? Date()).addingTimeInterval(-Self.pullOverlap)
        try syncRepo.updateLastSync(at: overlapped.iso8601)
        try syncRepo.updateLastPull(at: Date().iso8601)
        return changes
    }

    /// Apply one validated page. Anything can arrive twice, so every step is
    /// idempotent. An entry that can't be used is skipped and the rest applied,
    /// but a database error stops the pull, so that the cursor doesn't move
    /// past a change that wasn't saved.
    private func apply(_ page: SyncPullResponse, skipping pendingDeletions: Set<String>, changes: inout SyncChanges) throws {
        let now = Date().iso8601

        // Process pulled notes and build attachment metadata lookup
        var attachmentMetadata: [String: AttachmentRef] = [:]
        for syncNote in page.notes {
            // Deleted here and not pushed yet: the server decides that one, and
            // if it refuses, the note comes back on a later pull.
            guard !pendingDeletions.contains(syncNote.id) else { continue }
            do {
                try processNote(syncNote, syncedAt: now, changes: &changes)
                // Index attachment refs by their blob ID (ref.data)
                for ref in syncNote.attachments {
                    attachmentMetadata[ref.data] = ref
                }
            } catch let error as DatabaseError {
                throw error
            } catch {
                Log.debug("[Sync] pull: failed to process note \(syncNote.id): \(error)")
                // Unclear whether processNote wrote before throwing —
                // don't risk a stale row, fall back to a full reload.
                changes.fullReloadRequired = true
            }
        }

        // Store pulled attachment blobs
        for syncAttachment in page.attachments {
            guard let blobData = Data(base64Encoded: syncAttachment.data) else { continue }
            let ref = attachmentMetadata[syncAttachment.id]
            try attachmentRepo.storeBlob(
                id: syncAttachment.id,
                mimeType: ref?.mimeType ?? "application/octet-stream",
                size: ref?.size ?? blobData.count,
                data: blobData,
                uploaded: true
            )
        }

        // Process deletions
        for deletion in page.deletions ?? [] {
            switch try noteRepo.applyServerDeletion(id: deletion.id) {
            case .movedToBin:
                changes.deletedIds.insert(deletion.id)
                changes.removedByServerIds.insert(deletion.id)
            case .keptLocalEdit:
                // The in-memory row is still current, so it isn't reported.
                Log.debug("[Sync] pull: kept note \(deletion.id) — local unsynced edit outranks remote deletion")
            case .nothingToDo:
                break
            }
        }

        // Store pulled version snapshots
        for syncVersion in page.versions {
            do {
                let attachmentsStr = String(data: (try? JSONEncoder().encode(syncVersion.attachments)) ?? Data("[]".utf8), encoding: .utf8) ?? "[]"
                // Never over one this device holds. A version key is only unique
                // per client, so a match can be someone else's revision, and the
                // server keeps whichever it heard first (R2-076).
                try versionRepo.insertIfAbsent(NoteVersionRecord(
                    versionKey: syncVersion.versionKey,
                    noteId: syncVersion.noteId,
                    version: syncVersion.version,
                    createdAt: syncVersion.createdAt,
                    syncedAt: syncVersion.syncedAt,
                    content: syncVersion.content,
                    tags: try Self.syncTagsToStorageTags(syncVersion.tags, key: key),
                    attachments: attachmentsStr,
                    syntaxLanguage: syncVersion.syntaxLanguage,
                    wordWrap: syncVersion.wordWrap,
                    showPreview: syncVersion.showPreview,
                    color: syncVersion.color,
                    reason: syncVersion.reason,
                    contentHash: syncVersion.contentHash,
                    parentHash: syncVersion.parentHash
                ))
            } catch let error as DatabaseError {
                throw error
            } catch {
                Log.debug("[Sync] pull: failed to store version \(syncVersion.versionKey): \(error)")
            }
        }

        // Process pulled saved searches. AppState.applySyncChanges only
        // refreshes the notes/archivedNotes lists, not `savedSearches` —
        // any saved-search change this pull carries needs the full
        // loadNotes() fallback to actually reach the UI.
        if let savedSearches = page.savedSearches, !savedSearches.isEmpty {
            changes.fullReloadRequired = true
            for remote in savedSearches {
                try savedSearchRepo.insertOrReplace(record: SavedSearchRepository.SavedSearchRecord(
                    id: remote.id,
                    name: remote.name,
                    query: remote.query,
                    displayOrder: remote.order,
                    createdAt: remote.createdAt,
                    modifiedAt: remote.modifiedAt,
                    syncedAt: now,
                    deleted: remote.deleted,
                    deletedAt: remote.deletedAt,
                    version: remote.version,
                    needsSync: false
                ))
            }
        }
    }

    /// Build a NoteRecord from a SyncNote for local storage.
    ///
    /// Older builds could send filenames in plaintext. Those are encrypted on
    /// the way in, and the note queued for a push so the server's copy is
    /// replaced too. (A name is also made safe to use whenever it's read: see
    /// `AttachmentFilename`.)
    ///
    /// `local` is the row being replaced, for what only this device knows.
    private func buildNoteRecord(from syncNote: SyncNote, syncedAt: String, local: NoteRecord?) throws -> NoteRecord {
        let tagsString = try Self.syncTagsToStorageTags(syncNote.tags, key: key)
        let attachments = try AttachmentFilename.encryptingPlaintext(in: syncNote.attachments, key: key)
        let attachmentsJSON = try JSONEncoder().encode(attachments.refs)
        let attachmentsString = String(data: attachmentsJSON, encoding: .utf8) ?? "[]"

        let hashChainStr: String?
        if let chain = syncNote.hashChain {
            hashChainStr = String(data: try JSONEncoder().encode(chain), encoding: .utf8)
        } else {
            hashChainStr = nil
        }

        return NoteRecord(
            id: syncNote.id,
            createdAt: syncNote.createdAt,
            modifiedAt: syncNote.modifiedAt,
            syncedAt: syncedAt,
            content: syncNote.content,
            tags: tagsString,
            attachments: attachmentsString,
            pinned: syncNote.pinned,
            archived: syncNote.archived,
            archivedAt: syncNote.archivedAt,
            locked: syncNote.locked ?? false,
            lockedAt: syncNote.lockedAt,
            deleted: syncNote.deleted,
            deletedAt: syncNote.deletedAt,
            version: syncNote.version,
            wordWrap: syncNote.wordWrap ?? true,
            syntaxLanguage: syncNote.syntaxLanguage ?? "markdown",
            // A note from the inbox was written by anyone with the inbox token,
            // and the server could otherwise switch its preview on.
            showPreview: local?.fromInbox == true ? false : syncNote.showPreview ?? false,
            color: syncNote.color,
            contentHash: syncNote.contentHash,
            parentHash: syncNote.parentHash,
            hashChain: hashChainStr,
            needsSync: attachments.changed,
            syncHash: syncNote.contentHash,
            // `deletedAt` is the server's to set: the purge goes by this instead.
            localDeletedAt: syncNote.deleted ? local?.localDeletedAt ?? syncedAt : nil,
            fromInbox: local?.fromInbox ?? false
        )
    }

    /// Build a ConflictInfo for a pull-side conflict.
    private func buildPullConflictInfo(localRecord: NoteRecord, syncNote: SyncNote) -> ConflictInfo {
        let local = Self.decryptNoteForDisplay(
            content: localRecord.content, tags: localRecord.tags, key: key
        )
        let server = Self.decryptServerNoteForDisplay(
            content: syncNote.content, syncTags: syncNote.tags, key: key
        )

        return ConflictInfo(
            id: syncNote.id,
            localContent: local?.content ?? "",
            localTags: local?.tags ?? [],
            localModifiedAt: localRecord.modifiedAt,
            localAttachments: Self.decodeAttachments(localRecord.attachments),
            serverContent: server?.content ?? "",
            serverTags: server?.tags ?? [],
            serverModifiedAt: syncNote.modifiedAt,
            serverEncryptedContent: syncNote.content,
            serverEncryptedTags: syncNote.tags,
            serverVersion: syncNote.version,
            serverAttachments: syncNote.attachments,
            serverPinned: syncNote.pinned,
            serverSyntaxLanguage: syncNote.syntaxLanguage,
            serverWordWrap: syncNote.wordWrap,
            serverShowPreview: syncNote.showPreview,
            serverContentHash: syncNote.contentHash,
            serverParentHash: syncNote.parentHash,
            serverHashChain: syncNote.hashChain,
            serverArchived: syncNote.archived,
            serverArchivedAt: syncNote.archivedAt,
            serverLocked: syncNote.locked,
            serverLockedAt: syncNote.lockedAt,
            serverColor: syncNote.color,
            serverDeleted: syncNote.deleted,
            serverDeletedAt: syncNote.deletedAt,
            localUnreadable: local == nil,
            serverUnreadable: server == nil
        )
    }

    /// Process a single note from a pull response. Any pull can deliver a note
    /// the client already has, so this has to be idempotent, and it must never
    /// overwrite a local edit that hasn't been pushed. See "Applying a pulled
    /// note" in docs/sync-protocol.md, which this follows.
    ///
    /// Every branch that writes a record inserts `syncNote.id` into
    /// `changes.updatedIds` — the conflict-detection branches (which only
    /// append to `pendingConflicts`) deliberately don't, since `resolveConflict`
    /// triggers its own full reload once the user resolves them.
    private func processNote(_ syncNote: SyncNote, syncedAt: String, changes: inout SyncChanges) throws {
        // A conflict counts as an edit that hasn't been sent, so the note here
        // stays as it is: it may be the only copy of the local side. The conflict
        // moves on to the newest server copy, though, because the pull won't
        // deliver this one again, and Keep Server has to take what's there now.
        if let waiting = pendingConflicts.firstIndex(where: { $0.id == syncNote.id }) {
            if syncNote.contentHash != pendingConflicts[waiting].serverContentHash,
               let localRecord = try noteRepo.getRaw(id: syncNote.id) {
                pendingConflicts[waiting] = buildPullConflictInfo(localRecord: localRecord, syncNote: syncNote)
            }
            return
        }

        func take(replacing local: NoteRecord?) throws {
            try noteRepo.insertOrReplace(buildNoteRecord(from: syncNote, syncedAt: syncedAt, local: local))
            changes.updatedIds.insert(syncNote.id)
        }

        // Check for existing local note
        guard let localRecord = try noteRepo.getRaw(id: syncNote.id) else {
            // New note from server — no conflict
            try take(replacing: nil)
            return
        }

        switch Self.action(forPulled: syncNote, local: localRecord) {
        case .take:
            try take(replacing: localRecord)
        case .keepLocal:
            break
        case .conflict:
            pendingConflicts.append(buildPullConflictInfo(localRecord: localRecord, syncNote: syncNote))
        }
    }

    enum PulledNoteAction: Equatable {
        case take
        case keepLocal
        case conflict
    }

    /// What to do with a pulled note the client already has.
    static func action(forPulled remote: SyncNote, local: NoteRecord) -> PulledNoteAction {
        guard let remoteHash = remote.contentHash, let localHash = local.contentHash else {
            // Notes from before hash chains: only the timestamps can say.
            guard local.needsSync else { return .take }
            return Date.isLater(remote.modifiedAt, than: local.modifiedAt) ? .conflict : .keepLocal
        }

        // Same hash. The notes usually match, but not always: see
        // `differsDespiteEqualHash`. A local change waiting to go up wins, and
        // the next push carries it.
        if localHash == remoteHash {
            return !local.needsSync && differsDespiteEqualHash(local: local, remote: remote) ? .take : .keepLocal
        }

        guard local.needsSync else { return .take }

        // Older than the local edit, which has already seen it.
        if local.parentHash == remoteHash || decodeHashChain(local.hashChain).contains(remoteHash) {
            return .keepLocal
        }

        // Newer, and includes the local edit.
        if remote.parentHash == localHash || (remote.hashChain ?? []).contains(localHash) {
            return .take
        }

        // Diverged, even if the chains share an ancestor.
        return .conflict
    }

    private static func decodeHashChain(_ json: String?) -> [String] {
        guard let json else { return [] }
        return (try? JSONDecoder().decode([String].self, from: Data(json.utf8))) ?? []
    }

    /// Whether a note with the same content hash as the local one really is a
    /// different note. Delete, archive, pin, lock and attachment changes don't
    /// recompute the hash on every client, and older web clients restore a
    /// version without doing so, yet the server stores them all. Comparing
    /// hashes alone left those changes stuck on the device that made them,
    /// until a later edit elsewhere undid them. The hash covers the ciphertext,
    /// so different ciphertext can't be the same content.
    private static func differsDespiteEqualHash(local: NoteRecord, remote: SyncNote) -> Bool {
        local.content != remote.content
            || local.pinned != remote.pinned
            || local.archived != remote.archived
            || local.locked != (remote.locked ?? local.locked)
            || local.deleted != remote.deleted
            || Set(decodeAttachments(local.attachments).map(\.id)) != Set(remote.attachments.map(\.id))
    }

    // MARK: - Conflict Resolution

    /// Resolve a conflict with the given strategy.
    func resolveConflict(noteId: String, strategy: ConflictResolutionStrategy) throws {
        guard let conflictIndex = pendingConflicts.firstIndex(where: { $0.id == noteId }) else { return }
        let conflict = pendingConflicts[conflictIndex]
        guard conflict.allows(strategy) else { throw ConflictResolutionError.unreadable }
        let now = Date().iso8601

        switch strategy {
        case .keepLocal:
            // Keep local content, rebase hash chain onto server's so next push is a fast-forward
            let record = try recreateRecordIfMissing(noteId: noteId, conflict: conflict)
            var updated = record
            updated.needsSync = true
            updated.version = conflict.serverVersion + 1
            // Rebase: set parentHash to server's hash so server sees fast-forward
            updated.parentHash = conflict.serverContentHash
            updated.syncHash = conflict.serverContentHash
            // Merge hash chains: local hash, server hash, then merged history
            var localChain: [String] = []
            if let chainStr = record.hashChain, let data = chainStr.data(using: .utf8) {
                localChain = (try? JSONDecoder().decode([String].self, from: data)) ?? []
            }
            let serverChain = conflict.serverHashChain ?? []
            var mergedChain: [String] = []
            if let localHash = record.contentHash { mergedChain.append(localHash) }
            if let serverHash = conflict.serverContentHash { mergedChain.append(serverHash) }
            var seen = Set(mergedChain)
            for hash in localChain + serverChain {
                if seen.insert(hash).inserted {
                    mergedChain.append(hash)
                }
            }
            mergedChain = Array(mergedChain.prefix(CryptoService.maxHashChainLength))
            updated.hashChain = String(data: (try? JSONEncoder().encode(mergedChain)) ?? Data("[]".utf8), encoding: .utf8)
            // insertOrReplace (upsert) rather than updateRaw: the record may have
            // just been recreated in-memory above and doesn't exist in the DB yet.
            try noteRepo.insertOrReplace(updated)

        case .keepServer:
            // Apply server version (same as the old auto-accept behaviour)
            let record = try recreateRecordIfMissing(noteId: noteId, conflict: conflict)
            let updated = try Self.applyingServerCopy(from: conflict, to: record, syncedAt: now, key: key)
            // insertOrReplace (upsert) rather than updateRaw: the record may have
            // just been recreated in-memory above and doesn't exist in the DB yet.
            try noteRepo.insertOrReplace(updated)
            try versionRepo.createVersion(from: updated, reason: "sync-conflict-resolved")

        case .keepBoth:
            // Accept server version for the existing note
            let record = try recreateRecordIfMissing(noteId: noteId, conflict: conflict)
            let serverRecord = try Self.applyingServerCopy(from: conflict, to: record, syncedAt: now, key: key)

            // Copy attachment blobs under new ids rather than sharing them with
            // the original — see copyAttachments' doc comment for why sharing
            // ids is unsafe (single-attachment removal deletes by explicit id).
            let duplicateAttachments = try copyAttachments(conflict.localAttachments)
            let newIds = Dictionary(
                zip(conflict.localAttachments.map(\.id), duplicateAttachments.map(\.id)),
                uniquingKeysWith: { first, _ in first }
            )

            // Create a duplicate note with the local content, whose references
            // have to name its own attachments.
            let localContent = NoteCombining.content(conflict.localContent, followingCopies: newIds)
            let encContent = try CryptoService.encryptText(localContent, key: key)
            let encTags = try CryptoService.encryptStringArray(conflict.localTags, key: key)
            let contentJSON = try CryptoService.serializeEncryptedJSON(encContent)
            let tagsJSON = try CryptoService.serializeEncryptedJSON(encTags)
            var duplicate = NoteRecord.new(encryptedContent: contentJSON, encryptedTags: tagsJSON)
            duplicate.pinned = record.pinned
            duplicate.syntaxLanguage = record.syntaxLanguage
            duplicate.wordWrap = record.wordWrap
            duplicate.showPreview = record.showPreview
            duplicate.color = record.color
            duplicate.attachments = Self.encodeAttachments(duplicateAttachments)
            // Together, or stopping between them would lose the local side:
            // the original overwritten, and no copy made. A copied blob left
            // behind costs space and nothing else.
            try noteRepo.insertOrReplace([serverRecord, duplicate])
        }

        pendingConflicts.remove(at: conflictIndex)
    }

    /// `record` as the server has it. Later pulls skip a note whose hash matches,
    /// so anything left out here would stay different for good.
    private static func applyingServerCopy(
        from conflict: ConflictInfo,
        to record: NoteRecord,
        syncedAt: String,
        key: SymmetricKey
    ) throws -> NoteRecord {
        var updated = record
        updated.content = conflict.serverEncryptedContent
        updated.tags = try syncTagsToStorageTags(conflict.serverEncryptedTags, key: key)
        updated.attachments = encodeAttachments(conflict.serverAttachments)
        updated.version = conflict.serverVersion
        updated.pinned = conflict.serverPinned
        // An older server's conflicts don't carry these. One that sends
        // `serverArchived` sends the rest, and no colour then means none.
        if let archived = conflict.serverArchived {
            updated.archived = archived
            updated.archivedAt = conflict.serverArchivedAt
            updated.color = conflict.serverColor
        }
        if let locked = conflict.serverLocked {
            updated.locked = locked
            updated.lockedAt = conflict.serverLockedAt
        }
        if let deleted = conflict.serverDeleted {
            updated.deleted = deleted
            updated.deletedAt = deleted ? conflict.serverDeletedAt ?? syncedAt : nil
            updated.localDeletedAt = deleted ? updated.localDeletedAt ?? syncedAt : nil
        }
        updated.syntaxLanguage = conflict.serverSyntaxLanguage ?? updated.syntaxLanguage
        updated.wordWrap = conflict.serverWordWrap ?? updated.wordWrap
        updated.showPreview = conflict.serverShowPreview ?? updated.showPreview
        updated.contentHash = conflict.serverContentHash
        updated.parentHash = conflict.serverParentHash
        updated.syncHash = conflict.serverContentHash
        if let chain = conflict.serverHashChain {
            updated.hashChain = String(data: (try? JSONEncoder().encode(chain)) ?? Data("[]".utf8), encoding: .utf8)
        }
        updated.needsSync = false
        updated.syncedAt = syncedAt
        return updated
    }

    /// Fetch the note row backing a conflict, or recreate it from the conflict's
    /// local snapshot if it has vanished (e.g. a remote deletion raced the
    /// conflict). Every resolution strategy needs a starting record to mutate;
    /// without this fallback a missing row silently discarded the user's choice
    /// while still clearing the conflict.
    private func recreateRecordIfMissing(noteId: String, conflict: ConflictInfo) throws -> NoteRecord {
        if let existing = try noteRepo.getRaw(id: noteId) {
            return existing
        }
        let encContent = try CryptoService.encryptText(conflict.localContent, key: key)
        let encTags = try CryptoService.encryptStringArray(conflict.localTags, key: key)
        var recreated = NoteRecord.new(
            encryptedContent: try CryptoService.serializeEncryptedJSON(encContent),
            encryptedTags: try CryptoService.serializeEncryptedJSON(encTags)
        )
        recreated.id = noteId
        recreated.modifiedAt = conflict.localModifiedAt
        recreated.attachments = Self.encodeAttachments(conflict.localAttachments)
        return recreated
    }

    /// Clear all pending conflicts (used when locking the app).
    func clearConflicts() {
        pendingConflicts.removeAll()
    }

    #if DEBUG
    /// Test-only hook: inject a conflict without running an actual sync cycle.
    func addPendingConflictForTesting(_ conflict: ConflictInfo) {
        pendingConflicts.append(conflict)
    }
    #endif

    // MARK: - Decryption Helpers

    /// Decrypt storage-format content and tags for display.
    private static func decryptNoteForDisplay(content: String, tags: String, key: SymmetricKey) -> (content: String, tags: [String])? {
        do {
            let encContent = try CryptoService.parseEncryptedJSON(content)
            let plainContent = try CryptoService.decryptText(encContent, key: key)
            let encTags = try CryptoService.parseEncryptedJSON(tags)
            let tagsText = try CryptoService.decryptText(encTags, key: key)
            let plainTags: [String]
            if tagsText.isEmpty {
                plainTags = []
            } else if tagsText.hasPrefix("[") {
                plainTags = (try? JSONDecoder().decode([String].self, from: Data(tagsText.utf8))) ?? []
            } else {
                plainTags = tagsText.split(separator: ",").map { String($0).trimmingCharacters(in: .whitespaces) }
            }
            return (plainContent, plainTags)
        } catch {
            return nil
        }
    }

    /// Decrypt server content and sync-format tags for display.
    private static func decryptServerNoteForDisplay(content: String, syncTags: [String], key: SymmetricKey) -> (content: String, tags: [String])? {
        do {
            let encContent = try CryptoService.parseEncryptedJSON(content)
            let plainContent = try CryptoService.decryptText(encContent, key: key)
            var tags: [String] = []
            for encTagJSON in syncTags {
                let encrypted = try CryptoService.parseEncryptedJSON(encTagJSON)
                let decryptedText = try CryptoService.decryptText(encrypted, key: key)
                if let data = decryptedText.data(using: .utf8),
                   let tag = try? JSONDecoder().decode(String.self, from: data) {
                    tags.append(tag)
                } else if !decryptedText.isEmpty {
                    tags.append(decryptedText)
                }
            }
            return (plainContent, tags)
        } catch {
            return nil
        }
    }

    // MARK: - Tag Format Conversion

    /// Convert storage format (single encrypted blob) to sync format (individually encrypted tags).
    /// Storage: `{"ciphertext":"...","iv":"..."}` → decrypts to `["tag1","tag2"]`
    /// Sync: `["enc(\"tag1\")","enc(\"tag2\")"]` — each tag JSON-encoded then encrypted separately.
    private static func storageTagsToSyncTags(_ storageTags: String, key: SymmetricKey) -> [String] {
        do {
            let encBlob = try CryptoService.parseEncryptedJSON(storageTags)
            let plainTags: [String] = try CryptoService.decryptStringArray(encBlob, key: key)
            return try plainTags.map { tag in
                try CryptoService.encryptTagForSync(tag, key: key)
            }
        } catch {
            // Fallback: wrap as single element (will trigger legacy conversion on web)
            return [storageTags]
        }
    }

    /// Convert sync format (individually encrypted tags) to storage format (single encrypted blob).
    /// Sync: `["enc(\"tag1\")","enc(\"tag2\")"]` → decrypt each → `["tag1","tag2"]` → encrypt as blob.
    /// Accepts a whole array encrypted as one blob as well, which is how web and
    /// Android write version history. Throws if anything won't decrypt.
    private static func syncTagsToStorageTags(_ syncTags: [String], key: SymmetricKey) throws -> String {
        var plainTags: [String] = []
        for encTagJSON in syncTags {
            let encrypted = try CryptoService.parseEncryptedJSON(encTagJSON)
            let decryptedText = try CryptoService.decryptText(encrypted, key: key)
            // Each tag decrypts to a JSON-encoded string like "\"tag1\""
            if let data = decryptedText.data(using: .utf8),
               let tag = try? JSONDecoder().decode(String.self, from: data) {
                plainTags.append(tag)
            } else if decryptedText.hasPrefix("[") {
                // Legacy: entire array encrypted as one blob
                if let tags = try? JSONDecoder().decode([String].self, from: Data(decryptedText.utf8)) {
                    plainTags.append(contentsOf: tags)
                }
            } else if !decryptedText.isEmpty {
                plainTags.append(decryptedText)
            }
        }
        let encrypted = try CryptoService.encryptStringArray(plainTags, key: key)
        return try CryptoService.serializeEncryptedJSON(encrypted)
    }

    // MARK: - Attachment Ref Encoding

    /// Decode a `NoteRecord.attachments` JSON string into refs. Mirrors the
    /// inline pattern used elsewhere in this file (e.g. push's SyncNote
    /// construction) so conflict-creation sites share one implementation.
    private static func decodeAttachments(_ json: String) -> [AttachmentRef] {
        guard let data = json.data(using: .utf8) else { return [] }
        return (try? JSONDecoder().decode([AttachmentRef].self, from: data)) ?? []
    }

    /// Encode attachment refs back into the JSON string stored on `NoteRecord.attachments`.
    private static func encodeAttachments(_ refs: [AttachmentRef]) -> String {
        let data = (try? JSONEncoder().encode(refs)) ?? Data("[]".utf8)
        return String(data: data, encoding: .utf8) ?? "[]"
    }

    /// Copy each attachment's blob under a freshly generated id and return refs
    /// pointing at the copies, leaving the originals untouched.
    ///
    /// Blob-sharing investigation (see task-5-report.md): `AttachmentRepository`
    /// has no cascade delete when a note is hard-deleted (`NoteRepository.hardDelete`
    /// only issues `DELETE FROM notes`), but `AppState.removeAttachment` — the
    /// normal single-attachment-removal path — deletes the blob by explicit id
    /// (`attachmentRepo.deleteBlob(id: ref.data)`) after stripping the ref from
    /// just one note. If a "Keep Both" duplicate shared blob ids with the
    /// original, removing an attachment from either note would delete a blob the
    /// other note's ref still points at. Copying under new ids avoids that.
    private func copyAttachments(_ refs: [AttachmentRef]) throws -> [AttachmentRef] {
        var copied: [AttachmentRef] = []
        copied.reserveCapacity(refs.count)
        for ref in refs {
            let newId = CryptoService.generateUUID()
            if let blobData = try attachmentRepo.getBlob(id: ref.data) {
                try attachmentRepo.storeBlob(
                    id: newId,
                    mimeType: ref.mimeType,
                    size: ref.size,
                    data: blobData
                )
            }
            copied.append(AttachmentRef(
                id: newId,
                filename: ref.filename,
                mimeType: ref.mimeType,
                size: ref.size,
                data: newId,
                // Would name the original's blob, the sharing this exists to avoid.
                thumbnailData: nil
            ))
        }
        return copied
    }

    // MARK: - Force Full Sync

    /// Run a full sync with no incremental state — re-downloads all notes from
    /// the server. Callers always fall back to a full reload after this (it's
    /// the "force-resync" case from the incremental-refresh contract), so
    /// this discards the per-note change tracking `pull()` does.
    func forceFullSync() async throws {
        guard !isSyncing, !isShutDown else { return }
        isSyncing = true
        lastError = nil
        defer { isSyncing = false }

        do {
            // Push local changes first
            let (_, pushError) = await pushCapturingFailure()

            // Pull everything: no position, and no blobs taken as known
            do {
                _ = try await runPull(
                    cursor: "", lastSyncAt: nil, knownAttachmentIds: [], fullPullIfCursorsWork: false
                )
            } catch {
                throw pushError ?? error
            }

            try await finalise()
            // Single reload (jottery-vn6f): forceFullSync()'s only caller,
            // AppState.forceFullSync(), always performs its own explicit
            // full reload right after this method returns. Firing
            // postSyncHandler(.fullReload) here as well used to trigger a
            // *second* one via handlePostSyncCompletion →
            // applySyncChanges(.fullReload) — redundant, and its errors were
            // silently swallowed (`try?`) unlike the caller's reload. Do not
            // reinstate this call without also removing the caller's reload.
            if let pushError { throw pushError }
        } catch {
            noteFailure(error)
            throw error
        }
    }

    // MARK: - SSE

    /// Start listening for real-time sync events.
    func startSSE() async {
        // Wire up the callback BEFORE starting so no early events are missed
        // Weak, or the service, and the key it holds, would never be released.
        await sseClient.setOnSyncEvent { [weak self] in
            _ = try? await self?.sync()
        }
        await sseClient.start()
    }

    /// Stop SSE listener.
    func stopSSE() async {
        await sseClient.stop()
    }

    /// Set by `shutDown(timeout:)` and `stop()`: no new sync starts on this service.
    private(set) var isShutDown = false

    /// Stop this service for good, without waiting for a sync in progress. Used
    /// when the app locks or the account goes away.
    func stop() async {
        isShutDown = true
        await sseClient.stop()
    }

    /// Stop this service for good and wait for a sync in progress to finish.
    /// Used before the vault's master key changes: the service encrypts and
    /// decrypts with the key it was created with, so anything it pushed or
    /// pulled after the re-key would be under the wrong key. Returns false if
    /// a sync was still running when `timeout` ran out.
    func shutDown(timeout: Duration = .seconds(30)) async -> Bool {
        isShutDown = true
        await sseClient.stop()
        let deadline = ContinuousClock.now + timeout
        while isSyncing {
            guard ContinuousClock.now < deadline else { return false }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return true
    }
}

/// When to try a failed sync again without being asked.
enum SyncRetry {
    static let maxAttempts = 5

    /// Seconds to wait before attempt number `attempt` (from zero), or nil if
    /// this failure isn't one that trying again can help. At least as long as
    /// the server asked for, and longer each time, so a struggling server isn't
    /// made worse.
    static func delay(after error: Error, attempt: Int) -> TimeInterval? {
        guard attempt < maxAttempts, let error = error as? SyncClientError else { return nil }
        let backoff = 5 * pow(2, Double(attempt))
        switch error {
        case .requestTimedOut: return backoff
        case .rateLimited(let wait), .serverBusy(let wait): return max(wait, backoff)
        default: return nil
        }
    }
}

enum SyncServiceError: LocalizedError, Equatable {
    /// Notes the server refused as too large, even one at a time.
    case notesTooLarge(count: Int)

    var errorDescription: String? {
        switch self {
        case .notesTooLarge(let count): return L.syncNotesTooLarge(count)
        }
    }
}

// Extension to set the callback
extension SSEClient {
    func setOnSyncEvent(_ handler: @escaping @Sendable () async -> Void) {
        self.onSyncEvent = handler
    }
}
