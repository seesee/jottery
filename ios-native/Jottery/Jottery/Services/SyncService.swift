import CryptoKit
import Foundation

/// Orchestrates push/pull sync cycles with the server.
actor SyncService {

    private let syncClient: SyncClient
    private let noteRepo: NoteRepository
    private let syncRepo: SyncRepository
    private let versionRepo: VersionRepository
    private let attachmentRepo: AttachmentRepository
    private let key: SymmetricKey
    private let sseClient: SSEClient

    private(set) var isSyncing = false
    private(set) var lastSyncAt: String?
    private(set) var lastError: String?
    private(set) var pendingConflicts: [ConflictInfo] = []
    private var postSyncHandler: (@Sendable () async -> Void)?

    init(syncClient: SyncClient, noteRepo: NoteRepository, syncRepo: SyncRepository, versionRepo: VersionRepository, attachmentRepo: AttachmentRepository, key: SymmetricKey) {
        self.syncClient = syncClient
        self.noteRepo = noteRepo
        self.syncRepo = syncRepo
        self.versionRepo = versionRepo
        self.attachmentRepo = attachmentRepo
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

    func push() async throws {
        let records = try noteRepo.listNeedingSync()
        print("[Sync] push: \(records.count) records need syncing")
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

        // Gather attachment blobs for pushed notes
        var syncAttachments: [SyncAttachment] = []
        for note in syncNotes {
            for ref in note.attachments {
                // ref.data is the blob store ID
                if let blobData = try? attachmentRepo.getBlob(id: ref.data) {
                    syncAttachments.append(SyncAttachment(
                        id: ref.data,
                        data: blobData.base64EncodedString()
                    ))
                }
            }
        }

        // Get pending deletions
        let deletions = try syncRepo.getPendingDeletions()
        let syncDeletions = deletions.map { SyncDeletion(id: $0.id, deletedAt: $0.deletedAt) }

        let request = SyncPushRequest(
            notes: syncNotes,
            attachments: syncAttachments,
            versions: syncVersions,
            deletions: syncDeletions.isEmpty ? nil : syncDeletions
        )

        print("[Sync] push: sending \(syncNotes.count) notes, \(syncDeletions.count) deletions")
        let response: SyncPushResponse
        do {
            response = try await syncClient.push(request)
        } catch {
            print("[Sync] push: HTTP error — \(error)")
            throw error
        }
        print("[Sync] push: accepted=\(response.accepted.count), rejected=\(response.rejected.count), errors=\(response.errors ?? [])")
        let now = Date().iso8601

        // Mark accepted notes as synced and create version snapshots
        for accepted in response.accepted {
            try noteRepo.markSynced(id: accepted.id, syncedAt: accepted.syncedAt ?? now, serverVersion: accepted.serverVersion)
            // Create a version snapshot of the accepted state
            if let record = try noteRepo.getRaw(id: accepted.id) {
                try versionRepo.createVersion(from: record, reason: "sync")
            }
        }

        // Clear successful deletions
        let acceptedIds = Set(response.accepted.map(\.id))
        let clearedDeletionIds = deletions.filter { acceptedIds.contains($0.id) }.map(\.id)
        try syncRepo.clearDeletions(ids: clearedDeletionIds)

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

        // Check for notes with missing attachment blobs.
        // The server only returns notes modified after lastSyncAt for incremental
        // pulls, and only includes attachment blobs for notes in the response.
        // So if we have notes whose attachment blobs were never stored, we must
        // do a full pull (lastSyncAt = nil) to get those notes re-delivered.
        let allRecords = try noteRepo.listActive(key: key)
        let storedBlobIds = Set(try attachmentRepo.listBlobIds())
        var hasMissingBlobs = false
        var knownIds: [String] = []
        for note in allRecords {
            if note.attachments.isEmpty || note.attachments.allSatisfy({ storedBlobIds.contains($0.data) }) {
                knownIds.append(note.id)
            } else {
                hasMissingBlobs = true
                print("[Sync] pull: note \(note.id) has missing attachment blobs — will do full pull")
            }
        }

        // If any blobs are missing, force a full pull so the server re-sends
        // those notes and their attachment data.
        let effectiveLastSyncAt = hasMissingBlobs ? nil : lastSyncAt

        var offset = 0
        var hasMore = true

        while hasMore {
            let request = SyncPullRequest(
                lastSyncAt: effectiveLastSyncAt,
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
                try processNote(syncNote, syncedAt: now)
                knownIds.append(syncNote.id)
                // Index attachment refs by their blob ID (ref.data)
                for ref in syncNote.attachments {
                    attachmentMetadata[ref.data] = ref
                }
            }

            // Store pulled attachment blobs
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

            // Process deletions
            if let deletions = response.deletions {
                for deletion in deletions {
                    try noteRepo.hardDelete(id: deletion.id)
                }
            }

            // Store pulled version snapshots
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

            hasMore = response.hasMore ?? false
            offset += response.notes.count
        }

        try syncRepo.updateLastPull(at: Date().iso8601)
    }

    /// Process a single note from a pull response.
    private func processNote(_ syncNote: SyncNote, syncedAt: String) throws {
        // Convert individually encrypted sync tags to single storage blob
        let tagsString = Self.syncTagsToStorageTags(syncNote.tags, key: key)

        let attachmentsJSON = try JSONEncoder().encode(syncNote.attachments)
        let attachmentsString = String(data: attachmentsJSON, encoding: .utf8) ?? "[]"

        let hashChainStr: String?
        if let chain = syncNote.hashChain {
            hashChainStr = String(data: try JSONEncoder().encode(chain), encoding: .utf8)
        } else {
            hashChainStr = nil
        }

        let record = NoteRecord(
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
            // Mark the local note as needing sync again — it will be pushed on next sync
            guard let record = try noteRepo.getRaw(id: noteId) else { break }
            var updated = record
            updated.needsSync = true
            // Bump version past server to win next push
            updated.version = conflict.serverVersion + 1
            try noteRepo.updateRaw(updated)

        case .keepServer:
            // Apply server version (same as the old auto-accept behaviour)
            guard let record = try noteRepo.getRaw(id: noteId) else { break }
            var updated = record
            updated.content = conflict.serverEncryptedContent
            updated.tags = Self.syncTagsToStorageTags(conflict.serverEncryptedTags, key: key)
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
            try noteRepo.updateRaw(updated)
            try versionRepo.createVersion(from: updated, reason: "sync-conflict-resolved")

        case .keepBoth:
            // Accept server version for the existing note
            guard let record = try noteRepo.getRaw(id: noteId) else { break }
            var serverRecord = record
            serverRecord.content = conflict.serverEncryptedContent
            serverRecord.tags = Self.syncTagsToStorageTags(conflict.serverEncryptedTags, key: key)
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
            try noteRepo.updateRaw(serverRecord)

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
            try noteRepo.insertOrReplace(duplicate)
        }

        pendingConflicts.remove(at: conflictIndex)
    }

    /// Clear all pending conflicts (used when locking the app).
    func clearConflicts() {
        pendingConflicts.removeAll()
    }

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
                // JSON-encode the tag string (wraps in quotes), then encrypt
                let tagJSON = String(data: try JSONEncoder().encode(tag), encoding: .utf8)!
                let encrypted = try CryptoService.encryptText(tagJSON, key: key)
                return try CryptoService.serializeEncryptedJSON(encrypted)
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

    // MARK: - SSE

    /// Start listening for real-time sync events.
    func startSSE() async {
        await sseClient.start()
        // Wire up the callback to trigger a pull
        let syncService = self
        await sseClient.setOnSyncEvent {
            try? await syncService.sync()
        }
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
