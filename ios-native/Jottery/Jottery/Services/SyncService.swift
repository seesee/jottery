import CryptoKit
import Foundation

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
    private var postSyncHandler: (@Sendable () async -> Void)?

    init(syncClient: SyncClient, noteRepo: NoteRepository, syncRepo: SyncRepository, versionRepo: VersionRepository, attachmentRepo: AttachmentRepository, savedSearchRepo: SavedSearchRepository, key: SymmetricKey) {
        self.syncClient = syncClient
        self.noteRepo = noteRepo
        self.syncRepo = syncRepo
        self.versionRepo = versionRepo
        self.attachmentRepo = attachmentRepo
        self.savedSearchRepo = savedSearchRepo
        self.key = key
        self.sseClient = SSEClient(syncClient: syncClient)
    }

    /// Set a handler called after SSE-triggered syncs complete.
    func setPostSyncHandler(_ handler: @escaping @Sendable () async -> Void) {
        postSyncHandler = handler
    }

    // MARK: - Full Sync

    /// Run a full push-then-pull sync cycle.
    func sync() async throws {
        guard !isSyncing else { return }
        isSyncing = true
        lastError = nil

        defer { isSyncing = false }

        do {
            try await push()
            try await pull()
            try await finalise()
            await postSyncHandler?()
        } catch {
            lastError = error.localizedDescription
            throw error
        }
    }

    /// Record the sync timestamp. Called after push + pull complete.
    func finalise() async throws {
        let now = Date().iso8601
        try syncRepo.updateLastSync(at: now)
        lastSyncAt = now
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

    func push() async throws {
        let records = try noteRepo.listNeedingSync()
        Log.debug("[Sync] push: \(records.count) records need syncing")
        guard !records.isEmpty else { return }

        var syncNotes: [SyncNote] = []
        for record in records {
            // Decrypt tags from single blob, then encrypt each tag individually
            // for the sync format (web expects individually encrypted tags).
            let tagsArray: [String] = Self.storageTagsToSyncTags(record.tags, key: key)

            let attachments: [AttachmentRef]
            if let data = record.attachments.data(using: .utf8) {
                attachments = (try? JSONDecoder().decode([AttachmentRef].self, from: data)) ?? []
            } else {
                attachments = []
            }

            let hashChain: [String]?
            if let chainStr = record.hashChain, let data = chainStr.data(using: .utf8) {
                hashChain = try? JSONDecoder().decode([String].self, from: data)
            } else {
                hashChain = nil
            }

            syncNotes.append(SyncNote(
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
            ))
        }

        // Gather version snapshots for pushed notes
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

        // Gather attachment blobs for pushed notes.
        // Use ref.id as the SyncAttachment ID — the server's attachments_data.id
        // is a FK to attachments_meta.id, so they must match.
        var syncAttachments: [SyncAttachment] = []
        var seenAttachmentIds = Set<String>()
        for note in syncNotes {
            for ref in note.attachments {
                guard seenAttachmentIds.insert(ref.id).inserted else { continue }
                do {
                    if let blobData = try attachmentRepo.getBlob(id: ref.data) {
                        syncAttachments.append(SyncAttachment(
                            id: ref.id,
                            data: blobData.base64EncodedString()
                        ))
                    } else {
                        // Never skip silently: the note still pushes without the
                        // blob and the server warns back if it has no copy either
                        Log.debug("[Sync] push: attachment \(ref.id) (note \(note.id)) has no local blob — pushing note without it")
                    }
                } catch {
                    Log.debug("[Sync] push: attachment \(ref.id) (note \(note.id)) could not be read (\(error)) — pushing note without it")
                }
            }
        }

        // Get pending deletions
        let deletions = try syncRepo.getPendingDeletions()
        let syncDeletions = deletions.map { SyncDeletion(id: $0.id, deletedAt: $0.deletedAt) }

        // Collect saved searches needing sync
        let savedSearchRecords = try savedSearchRepo.listNeedingSync()
        let syncSavedSearches: [SyncSavedSearch] = savedSearchRecords.map { record in
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

        let request = SyncPushRequest(
            notes: syncNotes,
            attachments: syncAttachments,
            versions: syncVersions,
            deletions: syncDeletions.isEmpty ? nil : syncDeletions,
            savedSearches: syncSavedSearches.isEmpty ? nil : syncSavedSearches
        )

        Log.debug("[Sync] push: sending \(syncNotes.count) notes, \(syncDeletions.count) deletions, \(syncSavedSearches.count) saved searches")
        let response: SyncPushResponse
        do {
            response = try await syncClient.push(request)
        } catch {
            Log.debug("[Sync] push: HTTP error — \(error)")
            throw error
        }
        Log.debug("[Sync] push: accepted=\(response.accepted.count), rejected=\(response.rejected.count), errors=\(response.errors ?? [])")
        let now = Date().iso8601

        // Mark accepted notes as synced and create version snapshots. Guard
        // against clobbering an edit made while the push was in flight: only
        // clear needs_sync if modified_at still matches the snapshot that was
        // actually pushed (records, captured at the top of this method).
        let pushedModifiedAtById = Dictionary(uniqueKeysWithValues: records.map { ($0.id, $0.modifiedAt) })
        for accepted in response.accepted {
            do {
                try noteRepo.markSynced(
                    id: accepted.id,
                    syncedAt: accepted.syncedAt ?? now,
                    serverVersion: accepted.serverVersion,
                    ifModifiedAt: pushedModifiedAtById[accepted.id]
                )
                // Create a version snapshot of the accepted state
                if let record = try noteRepo.getRaw(id: accepted.id) {
                    try versionRepo.createVersion(from: record, reason: "sync")
                }
            } catch {
                Log.debug("[Sync] push: failed to mark accepted note \(accepted.id): \(error)")
            }
        }

        // React to server attachment warnings: re-upload blobs this device
        // holds (one attachments-only push, no in-run retry — the next regular
        // sync retries naturally); log the ones it cannot heal.
        if let warnings = response.attachmentWarnings, !warnings.isEmpty {
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
                let repairRequest = SyncPushRequest(
                    notes: [],
                    attachments: repairs,
                    versions: [],
                    deletions: nil,
                    savedSearches: nil
                )
                do {
                    _ = try await syncClient.push(repairRequest)
                } catch {
                    Log.debug("[Sync] push: attachment repair push failed — \(error)")
                }
            }
        }

        // Clear successful deletions
        let acceptedIds = Set(response.accepted.map(\.id))
        let clearedDeletionIds = deletions.filter { acceptedIds.contains($0.id) }.map(\.id)
        try syncRepo.clearDeletions(ids: clearedDeletionIds)

        // Mark pushed saved searches as synced
        for record in savedSearchRecords {
            do {
                try savedSearchRepo.markSynced(id: record.id, syncedAt: now)
            } catch {
                Log.debug("[Sync] push: failed to mark saved search \(record.id) synced: \(error)")
            }
        }

        // Handle rejected (conflicts) — queue for user resolution
        for rejected in response.rejected {
            if let record = try noteRepo.getRaw(id: rejected.id) {
                // Decrypt local content for display
                let (localContent, localTags) = Self.decryptNoteForDisplay(
                    content: record.content, tags: record.tags, key: key
                )

                // Decrypt server content for display (tags are in sync format)
                let (serverContent, serverTags) = Self.decryptServerNoteForDisplay(
                    content: rejected.serverContent, syncTags: rejected.serverTags, key: key
                )

                let conflict = ConflictInfo(
                    id: rejected.id,
                    localContent: localContent,
                    localTags: localTags,
                    localModifiedAt: record.modifiedAt,
                    localAttachments: Self.decodeAttachments(record.attachments),
                    serverContent: serverContent,
                    serverTags: serverTags,
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
                    serverHashChain: rejected.serverHashChain
                )
                pendingConflicts.append(conflict)
            }
        }

        try syncRepo.updateLastPush(at: now)
    }

    // MARK: - Pull

    func pull() async throws {
        let metadata = try syncRepo.getMetadata()
        let lastSyncAt = metadata?.lastSyncAt

        // Check for notes with missing attachment blobs using a raw DB query
        // (no decryption needed — just parse the attachments JSON for blob IDs).
        // Notes with missing blobs are excluded from knownIds so the server
        // re-sends them on the next incremental pull. We never force a full pull
        // for missing blobs — if the server had the blob we'd already have it.
        let noteAttachments = try noteRepo.listIdsAndAttachmentBlobIds()
        let storedBlobIds = Set(try attachmentRepo.listBlobIds())
        var knownIds: [String] = []
        for (id, blobIds) in noteAttachments {
            if blobIds.isEmpty || blobIds.allSatisfy({ storedBlobIds.contains($0) }) {
                knownIds.append(id)
            } else {
                // Exclude from knownIds so server re-sends if modified
            }
        }

        var offset = 0
        var hasMore = true

        while hasMore {
            let request = SyncPullRequest(
                lastSyncAt: lastSyncAt,
                knownNoteIds: knownIds,
                knownAttachmentIds: Array(storedBlobIds),
                limit: 100,
                offset: offset
            )

            let response = try await syncClient.pull(request)
            let now = Date().iso8601

            // Process pulled notes and build attachment metadata lookup
            var attachmentMetadata: [String: AttachmentRef] = [:]
            for syncNote in response.notes {
                do {
                    try processNote(syncNote, syncedAt: now)
                    knownIds.append(syncNote.id)
                    // Index attachment refs by their blob ID (ref.data)
                    for ref in syncNote.attachments {
                        attachmentMetadata[ref.data] = ref
                    }
                } catch {
                    Log.debug("[Sync] pull: failed to process note \(syncNote.id): \(error)")
                    knownIds.append(syncNote.id) // still track as known to avoid re-pull loops
                }
            }

            // Store pulled attachment blobs
            for syncAttachment in response.attachments {
                guard let blobData = Data(base64Encoded: syncAttachment.data) else { continue }
                do {
                    let ref = attachmentMetadata[syncAttachment.id]
                    try attachmentRepo.storeBlob(
                        id: syncAttachment.id,
                        filename: ref?.filename ?? "",
                        mimeType: ref?.mimeType ?? "application/octet-stream",
                        size: ref?.size ?? blobData.count,
                        data: blobData
                    )
                } catch {
                    Log.debug("[Sync] pull: failed to store attachment \(syncAttachment.id): \(error)")
                }
            }

            // Process deletions
            if let deletions = response.deletions {
                for deletion in deletions {
                    do {
                        let deleted = try noteRepo.hardDeleteIfSynced(id: deletion.id)
                        if !deleted {
                            Log.debug("[Sync] pull: kept note \(deletion.id) — local unsynced edit outranks remote deletion")
                        }
                    } catch {
                        Log.debug("[Sync] pull: failed to delete note \(deletion.id): \(error)")
                    }
                }
            }

            // Store pulled version snapshots
            for syncVersion in response.versions {
                do {
                    let storageTags = Self.syncTagsToStorageTags(syncVersion.tags, key: key)
                    let attachmentsStr = String(data: (try? JSONEncoder().encode(syncVersion.attachments)) ?? Data("[]".utf8), encoding: .utf8) ?? "[]"
                    let versionRecord = NoteVersionRecord(
                        versionKey: syncVersion.versionKey,
                        noteId: syncVersion.noteId,
                        version: syncVersion.version,
                        createdAt: syncVersion.createdAt,
                        syncedAt: syncVersion.syncedAt,
                        content: syncVersion.content,
                        tags: storageTags,
                        attachments: attachmentsStr,
                        syntaxLanguage: syncVersion.syntaxLanguage,
                        wordWrap: syncVersion.wordWrap,
                        showPreview: syncVersion.showPreview,
                        color: syncVersion.color,
                        reason: syncVersion.reason,
                        contentHash: syncVersion.contentHash,
                        parentHash: syncVersion.parentHash
                    )
                    try versionRepo.insertOrReplace(versionRecord)
                } catch {
                    Log.debug("[Sync] pull: failed to store version \(syncVersion.versionKey): \(error)")
                }
            }

            // Process pulled saved searches
            if let savedSearches = response.savedSearches {
                for remote in savedSearches {
                    do {
                        let record = SavedSearchRepository.SavedSearchRecord(
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
                        )
                        try savedSearchRepo.insertOrReplace(record: record)
                    } catch {
                        Log.debug("[Sync] pull: failed to store saved search \(remote.id): \(error)")
                    }
                }
            }

            hasMore = response.hasMore ?? false
            offset += response.notes.count

            // If the server says there's more but returned zero notes this page,
            // break to avoid an infinite loop (offset would never advance).
            if hasMore && response.notes.isEmpty {
                break
            }
        }

        try syncRepo.updateLastPull(at: Date().iso8601)
    }

    /// Build a NoteRecord from a SyncNote for local storage.
    private func buildNoteRecord(from syncNote: SyncNote, syncedAt: String) throws -> NoteRecord {
        let tagsString = Self.syncTagsToStorageTags(syncNote.tags, key: key)
        let attachmentsJSON = try JSONEncoder().encode(syncNote.attachments)
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
            showPreview: syncNote.showPreview ?? false,
            color: syncNote.color,
            contentHash: syncNote.contentHash,
            parentHash: syncNote.parentHash,
            hashChain: hashChainStr,
            needsSync: false
        )
    }

    /// Build a ConflictInfo for a pull-side conflict.
    private func buildPullConflictInfo(localRecord: NoteRecord, syncNote: SyncNote) -> ConflictInfo {
        let (localContent, localTags) = Self.decryptNoteForDisplay(
            content: localRecord.content, tags: localRecord.tags, key: key
        )
        let (serverContent, serverTags) = Self.decryptServerNoteForDisplay(
            content: syncNote.content, syncTags: syncNote.tags, key: key
        )

        return ConflictInfo(
            id: syncNote.id,
            localContent: localContent,
            localTags: localTags,
            localModifiedAt: localRecord.modifiedAt,
            localAttachments: Self.decodeAttachments(localRecord.attachments),
            serverContent: serverContent,
            serverTags: serverTags,
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
            serverHashChain: syncNote.hashChain
        )
    }

    /// Process a single note from a pull response with hash-chain conflict detection.
    private func processNote(_ syncNote: SyncNote, syncedAt: String) throws {
        // Skip notes with pending conflicts from push — preserve local version
        if pendingConflicts.contains(where: { $0.id == syncNote.id }) {
            return
        }

        // Check for existing local note
        guard let localRecord = try noteRepo.getRaw(id: syncNote.id) else {
            // New note from server — no conflict
            let record = try buildNoteRecord(from: syncNote, syncedAt: syncedAt)
            try noteRepo.insertOrReplace(record)
            return
        }

        // Hash-based conflict detection (git-like merkle chain)
        if let remoteHash = syncNote.contentHash, let localHash = localRecord.contentHash {
            // CASE 1: Fast-forward — local is remote's parent
            if localHash == syncNote.parentHash {
                let record = try buildNoteRecord(from: syncNote, syncedAt: syncedAt)
                try noteRepo.insertOrReplace(record)
                return
            }

            // CASE 2: Identical content — no update needed
            if localHash == remoteHash {
                return
            }

            // CASE 3: Local has unsynced changes — always a conflict
            if localRecord.needsSync {
                let conflict = buildPullConflictInfo(localRecord: localRecord, syncNote: syncNote)
                pendingConflicts.append(conflict)
                return
            }

            // Local is synced — check chain relationships
            let localChain: [String]
            if let chainStr = localRecord.hashChain, let data = chainStr.data(using: .utf8) {
                localChain = (try? JSONDecoder().decode([String].self, from: data)) ?? []
            } else {
                localChain = []
            }

            // CASE 4: Remote's parent in local chain — local is ahead, skip
            if let remoteParent = syncNote.parentHash, localChain.contains(remoteParent) {
                return
            }

            // CASE 5: Local hash in remote chain — remote is ahead, fast-forward
            let remoteChain = syncNote.hashChain ?? []
            if remoteChain.contains(localHash) {
                let record = try buildNoteRecord(from: syncNote, syncedAt: syncedAt)
                try noteRepo.insertOrReplace(record)
                return
            }

            // No clear relationship and local is synced — accept remote
            let record = try buildNoteRecord(from: syncNote, syncedAt: syncedAt)
            try noteRepo.insertOrReplace(record)
            return
        }

        // Fallback for legacy notes without hash chains
        if localRecord.needsSync && syncNote.modifiedAt > localRecord.modifiedAt {
            let conflict = buildPullConflictInfo(localRecord: localRecord, syncNote: syncNote)
            pendingConflicts.append(conflict)
            return
        }

        // Server is newer or local is synced — accept remote
        let record = try buildNoteRecord(from: syncNote, syncedAt: syncedAt)
        try noteRepo.insertOrReplace(record)
    }

    // MARK: - Conflict Resolution

    /// Resolve a conflict with the given strategy.
    func resolveConflict(noteId: String, strategy: ConflictResolutionStrategy) throws {
        guard let conflictIndex = pendingConflicts.firstIndex(where: { $0.id == noteId }) else { return }
        let conflict = pendingConflicts[conflictIndex]
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
            var updated = record
            updated.content = conflict.serverEncryptedContent
            updated.tags = Self.syncTagsToStorageTags(conflict.serverEncryptedTags, key: key)
            updated.attachments = Self.encodeAttachments(conflict.serverAttachments)
            updated.version = conflict.serverVersion
            updated.pinned = conflict.serverPinned
            updated.syntaxLanguage = conflict.serverSyntaxLanguage ?? updated.syntaxLanguage
            updated.wordWrap = conflict.serverWordWrap ?? updated.wordWrap
            updated.showPreview = conflict.serverShowPreview ?? updated.showPreview
            updated.contentHash = conflict.serverContentHash
            updated.parentHash = conflict.serverParentHash
            if let chain = conflict.serverHashChain {
                updated.hashChain = String(data: (try? JSONEncoder().encode(chain)) ?? Data("[]".utf8), encoding: .utf8)
            }
            updated.needsSync = false
            updated.syncedAt = now
            // insertOrReplace (upsert) rather than updateRaw: the record may have
            // just been recreated in-memory above and doesn't exist in the DB yet.
            try noteRepo.insertOrReplace(updated)
            try versionRepo.createVersion(from: updated, reason: "sync-conflict-resolved")

        case .keepBoth:
            // Accept server version for the existing note
            let record = try recreateRecordIfMissing(noteId: noteId, conflict: conflict)
            var serverRecord = record
            serverRecord.content = conflict.serverEncryptedContent
            serverRecord.tags = Self.syncTagsToStorageTags(conflict.serverEncryptedTags, key: key)
            serverRecord.attachments = Self.encodeAttachments(conflict.serverAttachments)
            serverRecord.version = conflict.serverVersion
            serverRecord.pinned = conflict.serverPinned
            serverRecord.syntaxLanguage = conflict.serverSyntaxLanguage ?? serverRecord.syntaxLanguage
            serverRecord.wordWrap = conflict.serverWordWrap ?? serverRecord.wordWrap
            serverRecord.showPreview = conflict.serverShowPreview ?? serverRecord.showPreview
            serverRecord.contentHash = conflict.serverContentHash
            serverRecord.parentHash = conflict.serverParentHash
            if let chain = conflict.serverHashChain {
                serverRecord.hashChain = String(data: (try? JSONEncoder().encode(chain)) ?? Data("[]".utf8), encoding: .utf8)
            }
            serverRecord.needsSync = false
            serverRecord.syncedAt = now
            // insertOrReplace (upsert) rather than updateRaw: the record may have
            // just been recreated in-memory above and doesn't exist in the DB yet.
            try noteRepo.insertOrReplace(serverRecord)

            // Create a duplicate note with the local content
            let encContent = try CryptoService.encryptText(conflict.localContent, key: key)
            let encTags = try CryptoService.encryptStringArray(conflict.localTags, key: key)
            let contentJSON = try CryptoService.serializeEncryptedJSON(encContent)
            let tagsJSON = try CryptoService.serializeEncryptedJSON(encTags)
            var duplicate = NoteRecord.new(encryptedContent: contentJSON, encryptedTags: tagsJSON)
            duplicate.pinned = record.pinned
            duplicate.syntaxLanguage = record.syntaxLanguage
            duplicate.wordWrap = record.wordWrap
            duplicate.showPreview = record.showPreview
            duplicate.color = record.color
            // Copy attachment blobs under new ids rather than sharing them with
            // the original — see copyAttachments' doc comment for why sharing
            // ids is unsafe (single-attachment removal deletes by explicit id).
            let duplicateAttachments = try copyAttachments(conflict.localAttachments)
            duplicate.attachments = Self.encodeAttachments(duplicateAttachments)
            try noteRepo.insertOrReplace(duplicate)
        }

        pendingConflicts.remove(at: conflictIndex)
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
    private static func decryptNoteForDisplay(content: String, tags: String, key: SymmetricKey) -> (String, [String]) {
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
            return ("", [])
        }
    }

    /// Decrypt server content and sync-format tags for display.
    private static func decryptServerNoteForDisplay(content: String, syncTags: [String], key: SymmetricKey) -> (String, [String]) {
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
            return ("", [])
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
    private static func syncTagsToStorageTags(_ syncTags: [String], key: SymmetricKey) -> String {
        do {
            if syncTags.isEmpty {
                let encrypted = try CryptoService.encryptStringArray([], key: key)
                return try CryptoService.serializeEncryptedJSON(encrypted)
            }
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
        } catch {
            // Fallback: return first tag as-is (best effort)
            return syncTags.first ?? "{}"
        }
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
                    filename: ref.filename,
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
                thumbnailData: ref.thumbnailData
            ))
        }
        return copied
    }

    // MARK: - Force Full Sync

    /// Run a full sync with no incremental state — re-downloads all notes from the server.
    func forceFullSync() async throws {
        guard !isSyncing else { return }
        isSyncing = true
        lastError = nil
        defer { isSyncing = false }

        do {
            // Push local changes first
            try await push()

            // Pull everything: no lastSyncAt, no knownNoteIds
            var offset = 0
            var hasMore = true

            while hasMore {
                let request = SyncPullRequest(
                    lastSyncAt: nil,
                    knownNoteIds: [],
                    knownAttachmentIds: [],
                    limit: 100,
                    offset: offset
                )

                let response = try await syncClient.pull(request)
                let now = Date().iso8601

                var attachmentMetadata: [String: AttachmentRef] = [:]
                for syncNote in response.notes {
                    try processNote(syncNote, syncedAt: now)
                    for ref in syncNote.attachments {
                        attachmentMetadata[ref.data] = ref
                    }
                }

                for syncAttachment in response.attachments {
                    guard let blobData = Data(base64Encoded: syncAttachment.data) else { continue }
                    let ref = attachmentMetadata[syncAttachment.id]
                    try attachmentRepo.storeBlob(
                        id: syncAttachment.id,
                        filename: ref?.filename ?? "",
                        mimeType: ref?.mimeType ?? "application/octet-stream",
                        size: ref?.size ?? blobData.count,
                        data: blobData
                    )
                }

                if let deletions = response.deletions {
                    for deletion in deletions {
                        let deleted = try noteRepo.hardDeleteIfSynced(id: deletion.id)
                        if !deleted {
                            Log.debug("[Sync] forceFullSync: kept note \(deletion.id) — local unsynced edit outranks remote deletion")
                        }
                    }
                }

                for syncVersion in response.versions {
                    let storageTags = Self.syncTagsToStorageTags(syncVersion.tags, key: key)
                    let attachmentsStr = String(data: (try? JSONEncoder().encode(syncVersion.attachments)) ?? Data("[]".utf8), encoding: .utf8) ?? "[]"
                    let versionRecord = NoteVersionRecord(
                        versionKey: syncVersion.versionKey,
                        noteId: syncVersion.noteId,
                        version: syncVersion.version,
                        createdAt: syncVersion.createdAt,
                        syncedAt: syncVersion.syncedAt,
                        content: syncVersion.content,
                        tags: storageTags,
                        attachments: attachmentsStr,
                        syntaxLanguage: syncVersion.syntaxLanguage,
                        wordWrap: syncVersion.wordWrap,
                        showPreview: syncVersion.showPreview,
                        color: syncVersion.color,
                        reason: syncVersion.reason,
                        contentHash: syncVersion.contentHash,
                        parentHash: syncVersion.parentHash
                    )
                    try versionRepo.insertOrReplace(versionRecord)
                }

                if let savedSearches = response.savedSearches {
                    for remote in savedSearches {
                        let record = SavedSearchRepository.SavedSearchRecord(
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
                        )
                        try savedSearchRepo.insertOrReplace(record: record)
                    }
                }

                hasMore = response.hasMore ?? false
                offset += response.notes.count
                if hasMore && response.notes.isEmpty { break }
            }

            try await finalise()
            await postSyncHandler?()
        } catch {
            lastError = error.localizedDescription
            throw error
        }
    }

    // MARK: - SSE

    /// Start listening for real-time sync events.
    func startSSE() async {
        // Wire up the callback BEFORE starting so no early events are missed
        let syncService = self
        await sseClient.setOnSyncEvent {
            try? await syncService.sync()
        }
        await sseClient.start()
    }

    /// Stop SSE listener.
    func stopSSE() async {
        await sseClient.stop()
    }
}

// Extension to set the callback
private extension SSEClient {
    func setOnSyncEvent(_ handler: @escaping @Sendable () async -> Void) {
        self.onSyncEvent = handler
    }
}
