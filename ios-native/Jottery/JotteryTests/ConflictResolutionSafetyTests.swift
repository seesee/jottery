import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// SyncService.resolveConflict must honour the user's explicit choice even when
/// the local row backing a pending conflict has vanished (e.g. a remote deletion
/// or another sync path raced the conflict). Previously each strategy started
/// with `guard let record = try noteRepo.getRaw(id: noteId) else { break }`, so a
/// missing row silently no-opped while the conflict was still removed from
/// `pendingConflicts` — the user's choice was discarded without a trace.
struct ConflictResolutionSafetyTests {

    private func makeServices() throws -> (SyncService, NoteRepository, SymmetricKey) {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-conflict-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let db = try DatabaseManager(path: dir.appendingPathComponent("test.db").path)
        let versionRepo = VersionRepository(db: db)
        let noteRepo = NoteRepository(db: db, versionRepo: versionRepo)
        let syncRepo = SyncRepository(db: db)
        let attachmentRepo = AttachmentRepository(db: db)
        let savedSearchRepo = SavedSearchRepository(db: db)
        let key = SymmetricKey(data: Data(repeating: 9, count: 32))
        // resolveConflict makes no network calls; point at an endpoint nothing listens on.
        let syncClient = SyncClient(endpoint: "http://localhost:9")
        let service = SyncService(
            syncClient: syncClient,
            noteRepo: noteRepo,
            syncRepo: syncRepo,
            versionRepo: versionRepo,
            attachmentRepo: attachmentRepo,
            savedSearchRepo: savedSearchRepo,
            key: key
        )
        return (service, noteRepo, key)
    }

    /// Builds a conflict for a note id that has (deliberately) never been inserted
    /// into the repository, simulating a row that vanished before resolution.
    private func makeMissingRowConflict(
        noteId: String,
        localContent: String,
        serverContent: String,
        key: SymmetricKey
    ) throws -> ConflictInfo {
        let encServer = try CryptoService.encryptText(serverContent, key: key)
        let serverEncryptedContent = try CryptoService.serializeEncryptedJSON(encServer)
        let now = Date().iso8601
        return ConflictInfo(
            id: noteId,
            localContent: localContent,
            localTags: [],
            localModifiedAt: now,
            serverContent: serverContent,
            serverTags: [],
            serverModifiedAt: now,
            serverEncryptedContent: serverEncryptedContent,
            serverEncryptedTags: [],
            serverVersion: 3,
            serverAttachments: [],
            serverPinned: false,
            serverSyntaxLanguage: nil,
            serverWordWrap: nil,
            serverShowPreview: nil,
            serverContentHash: "server-hash-\(noteId)",
            serverParentHash: nil,
            serverHashChain: nil
        )
    }

    @Test func keepLocalRecreatesNoteFromLocalSnapshotWhenRowMissing() async throws {
        let (service, noteRepo, key) = try makeServices()
        let noteId = CryptoService.generateUUID()
        let conflict = try makeMissingRowConflict(
            noteId: noteId,
            localContent: "the local edit the user chose to keep",
            serverContent: "server content the user rejected",
            key: key
        )
        await service.addPendingConflictForTesting(conflict)

        // Sanity: no row exists yet.
        #expect(try noteRepo.getRaw(id: noteId) == nil)

        try await service.resolveConflict(noteId: noteId, strategy: .keepLocal)

        let restored = try #require(try noteRepo.get(id: noteId, key: key))
        #expect(restored.content == "the local edit the user chose to keep")
        #expect(restored.needsSync == true)

        let remainingConflicts = await service.pendingConflicts
        #expect(remainingConflicts.isEmpty)
    }

    @Test func keepServerRecreatesNoteFromServerPayloadWhenRowMissing() async throws {
        let (service, noteRepo, key) = try makeServices()
        let noteId = CryptoService.generateUUID()
        let conflict = try makeMissingRowConflict(
            noteId: noteId,
            localContent: "local content the user rejected",
            serverContent: "server content the user chose to keep",
            key: key
        )
        await service.addPendingConflictForTesting(conflict)

        #expect(try noteRepo.getRaw(id: noteId) == nil)

        try await service.resolveConflict(noteId: noteId, strategy: .keepServer)

        let restoredRaw = try #require(try noteRepo.getRaw(id: noteId))
        #expect(restoredRaw.content == conflict.serverEncryptedContent)
        #expect(restoredRaw.needsSync == false)
        #expect(restoredRaw.syncedAt != nil)

        let restored = try #require(try noteRepo.get(id: noteId, key: key))
        #expect(restored.content == "server content the user chose to keep")

        let remainingConflicts = await service.pendingConflicts
        #expect(remainingConflicts.isEmpty)
    }

    @Test func keepBothRecreatesServerNoteAndDuplicateWhenRowMissing() async throws {
        let (service, noteRepo, key) = try makeServices()
        let noteId = CryptoService.generateUUID()
        let conflict = try makeMissingRowConflict(
            noteId: noteId,
            localContent: "local content kept as a duplicate",
            serverContent: "server content kept under the original id",
            key: key
        )
        await service.addPendingConflictForTesting(conflict)

        #expect(try noteRepo.getRaw(id: noteId) == nil)

        try await service.resolveConflict(noteId: noteId, strategy: .keepBoth)

        // Original id now holds the server content.
        let serverNote = try #require(try noteRepo.get(id: noteId, key: key))
        #expect(serverNote.content == "server content kept under the original id")
        #expect(serverNote.needsSync == false)

        // A duplicate note exists with the local content under a new id.
        let allNotes = try noteRepo.listActive(key: key)
        let duplicate = allNotes.first { $0.id != noteId && $0.content == "local content kept as a duplicate" }
        #expect(duplicate != nil)

        let remainingConflicts = await service.pendingConflicts
        #expect(remainingConflicts.isEmpty)
    }
}
