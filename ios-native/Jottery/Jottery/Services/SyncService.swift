import CryptoKit
import Foundation

/// Orchestrates push/pull sync cycles with the server.
actor SyncService {

    private let syncClient: SyncClient
    private let noteRepo: NoteRepository
    private let syncRepo: SyncRepository
    private let key: SymmetricKey
    private let sseClient: SSEClient

    private(set) var isSyncing = false
    private(set) var lastSyncAt: String?
    private(set) var lastError: String?

    init(syncClient: SyncClient, noteRepo: NoteRepository, syncRepo: SyncRepository, key: SymmetricKey) {
        self.syncClient = syncClient
        self.noteRepo = noteRepo
        self.syncRepo = syncRepo
        self.key = key
        self.sseClient = SSEClient(syncClient: syncClient)
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
            let now = Date().iso8601
            try syncRepo.updateLastSync(at: now)
            lastSyncAt = now
        } catch {
            lastError = error.localizedDescription
            throw error
        }
    }

    // MARK: - Push

    private func push() async throws {
        let records = try noteRepo.listNeedingSync()
        guard !records.isEmpty else { return }

        let syncNotes: [SyncNote] = records.map { record in
            // Parse tags from encrypted JSON to get the array format the server expects.
            // Tags are stored as a single encrypted blob; server expects array with one element.
            let tagsArray: [String]
            if record.tags.hasPrefix("{") {
                tagsArray = [record.tags]
            } else {
                tagsArray = (try? JSONDecoder().decode([String].self, from: Data(record.tags.utf8))) ?? [record.tags]
            }

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

        // Get pending deletions
        let deletions = try syncRepo.getPendingDeletions()
        let syncDeletions = deletions.map { SyncDeletion(id: $0.id, deletedAt: $0.deletedAt) }

        let request = SyncPushRequest(
            notes: syncNotes,
            attachments: [],
            versions: [],
            deletions: syncDeletions.isEmpty ? nil : syncDeletions
        )

        let response = try await syncClient.push(request)
        let now = Date().iso8601

        // Mark accepted notes as synced
        for accepted in response.accepted {
            try noteRepo.markSynced(id: accepted.id, syncedAt: accepted.syncedAt ?? now)
        }

        // Clear successful deletions
        let acceptedIds = Set(response.accepted.map(\.id))
        let clearedDeletionIds = deletions.filter { acceptedIds.contains($0.id) }.map(\.id)
        try syncRepo.clearDeletions(ids: clearedDeletionIds)

        // Handle rejected (conflicts) — last-write-wins for now
        for rejected in response.rejected {
            // Accept server version (last-write-wins)
            if let record = try noteRepo.getRaw(id: rejected.id) {
                var updated = record
                updated.content = rejected.serverContent
                // Server tags are an array with one encrypted element
                if let firstTag = rejected.serverTags.first {
                    updated.tags = firstTag
                }
                updated.version = rejected.serverVersion
                updated.pinned = rejected.serverPinned
                updated.syntaxLanguage = rejected.serverSyntaxLanguage ?? updated.syntaxLanguage
                updated.wordWrap = rejected.serverWordWrap ?? updated.wordWrap
                updated.showPreview = rejected.serverShowPreview ?? updated.showPreview
                updated.needsSync = false
                updated.syncedAt = now
                try noteRepo.updateRaw(updated)
            }
        }

        try syncRepo.updateLastPush(at: now)
    }

    // MARK: - Pull

    private func pull() async throws {
        let metadata = try syncRepo.getMetadata()
        let lastSyncAt = metadata?.lastSyncAt

        // Get known note IDs
        let activeNotes = try noteRepo.listNeedingSync() // We need all IDs actually
        let allRecords = try noteRepo.listActive(key: key)
        let knownIds = allRecords.map(\.id)

        let request = SyncPullRequest(
            lastSyncAt: lastSyncAt,
            knownNoteIds: knownIds,
            knownAttachmentIds: []
        )

        let response = try await syncClient.pull(request)
        let now = Date().iso8601

        // Process pulled notes
        for syncNote in response.notes {
            // Convert tags array back to single encrypted string
            let tagsString: String
            if let firstTag = syncNote.tags.first {
                tagsString = firstTag
            } else {
                // Empty tags — encrypt empty array
                let emptyEncrypted = try CryptoService.encryptStringArray([], key: key)
                tagsString = try CryptoService.serializeEncryptedJSON(emptyEncrypted)
            }

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
                syncedAt: now,
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

        // Process deletions
        if let deletions = response.deletions {
            for deletion in deletions {
                try noteRepo.hardDelete(id: deletion.id)
            }
        }

        try syncRepo.updateLastPull(at: now)
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
