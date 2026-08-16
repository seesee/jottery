import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// SyncService.resolveConflict must preserve attachments for every strategy:
/// - `.keepServer` applies the server's attachment refs (previously it applied
///   content/tags/etc but silently left the note's old attachments in place).
/// - `.keepBoth` must give the duplicate note usable copies of the local
///   attachments, not an empty list, and must not let the duplicate share blob
///   ids with the original (see SyncService.copyAttachments' doc comment for
///   why sharing is unsafe: single-attachment removal deletes the blob by
///   explicit id).
/// - `.keepLocal` must leave the local attachments untouched, including when
///   the row had to be recreated from the conflict snapshot.
struct ConflictAttachmentTests {

    private func makeServices() throws -> (SyncService, NoteRepository, AttachmentRepository, SymmetricKey) {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-conflict-attachment-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let db = try DatabaseManager(path: dir.appendingPathComponent("test.db").path)
        let versionRepo = VersionRepository(db: db)
        let noteRepo = NoteRepository(db: db, versionRepo: versionRepo)
        let syncRepo = SyncRepository(db: db)
        let attachmentRepo = AttachmentRepository(db: db)
        let savedSearchRepo = SavedSearchRepository(db: db)
        let key = SymmetricKey(data: Data(repeating: 7, count: 32))
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
        return (service, noteRepo, attachmentRepo, key)
    }

    /// Stores a blob and returns an AttachmentRef pointing at it, mirroring
    /// AppState.addAttachment's construction (id == data == blob store key).
    private func makeStoredAttachment(
        attachmentRepo: AttachmentRepository,
        key: SymmetricKey,
        filename: String,
        blobData: Data
    ) throws -> AttachmentRef {
        let attachmentId = CryptoService.generateUUID()
        try attachmentRepo.storeBlob(
            id: attachmentId, filename: filename, mimeType: "text/plain", size: blobData.count, data: blobData
        )
        let encFilename = try CryptoService.encryptText(filename, key: key)
        let encFilenameJSON = try CryptoService.serializeEncryptedJSON(encFilename)
        return AttachmentRef(
            id: attachmentId, filename: encFilenameJSON, mimeType: "text/plain", size: blobData.count,
            data: attachmentId
        )
    }

    private func makeConflict(
        noteId: String,
        localContent: String,
        localAttachments: [AttachmentRef],
        serverContent: String,
        serverAttachments: [AttachmentRef],
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
            localAttachments: localAttachments,
            serverContent: serverContent,
            serverTags: [],
            serverModifiedAt: now,
            serverEncryptedContent: serverEncryptedContent,
            serverEncryptedTags: [],
            serverVersion: 3,
            serverAttachments: serverAttachments,
            serverPinned: false,
            serverSyntaxLanguage: nil,
            serverWordWrap: nil,
            serverShowPreview: nil,
            serverContentHash: "server-hash-\(noteId)",
            serverParentHash: nil,
            serverHashChain: nil
        )
    }

    private func decodeAttachments(_ json: String) -> [AttachmentRef] {
        guard let data = json.data(using: .utf8) else { return [] }
        return (try? JSONDecoder().decode([AttachmentRef].self, from: data)) ?? []
    }

    @Test func keepBothPreservesAttachmentsOnBothNotes() async throws {
        let (service, noteRepo, attachmentRepo, key) = try makeServices()

        // Existing local note with one attachment.
        let localBlob = Data("local attachment bytes".utf8)
        let localRef = try makeStoredAttachment(
            attachmentRepo: attachmentRepo, key: key, filename: "local.txt", blobData: localBlob
        )
        let note = try noteRepo.create(content: "local content", tags: [], key: key)
        try noteRepo.addAttachment(noteId: note.id, ref: localRef)

        // Server side has a different (already-known) attachment ref.
        let serverBlob = Data("server attachment bytes".utf8)
        let serverRef = try makeStoredAttachment(
            attachmentRepo: attachmentRepo, key: key, filename: "server.txt", blobData: serverBlob
        )

        let conflict = try makeConflict(
            noteId: note.id,
            localContent: "local content kept as a duplicate",
            localAttachments: [localRef],
            serverContent: "server content kept under the original id",
            serverAttachments: [serverRef],
            key: key
        )
        await service.addPendingConflictForTesting(conflict)

        try await service.resolveConflict(noteId: note.id, strategy: .keepBoth)

        // Original id now holds the server's attachment refs.
        let originalRaw = try #require(try noteRepo.getRaw(id: note.id))
        let originalAttachments = decodeAttachments(originalRaw.attachments)
        #expect(originalAttachments.map(\.id) == [serverRef.id])

        // A duplicate note exists with a non-empty, independent copy of the local attachment.
        let allNotes = try noteRepo.listActive(key: key)
        let duplicate = try #require(allNotes.first { $0.id != note.id })
        #expect(duplicate.attachments.count == 1)
        let dupRef = try #require(duplicate.attachments.first)

        // The duplicate's blob is a real, independent copy — not a shared id.
        #expect(dupRef.id != localRef.id)
        #expect(dupRef.data != localRef.data)
        let dupBlob = try attachmentRepo.getBlob(id: dupRef.data)
        #expect(dupBlob == localBlob)

        // The original blob is still intact and independently retrievable.
        let originalBlobStillThere = try attachmentRepo.getBlob(id: localRef.data)
        #expect(originalBlobStillThere == localBlob)

        let remainingConflicts = await service.pendingConflicts
        #expect(remainingConflicts.isEmpty)
    }

    @Test func keepServerAppliesServerAttachments() async throws {
        let (service, noteRepo, attachmentRepo, key) = try makeServices()

        let localBlob = Data("local attachment bytes".utf8)
        let localRef = try makeStoredAttachment(
            attachmentRepo: attachmentRepo, key: key, filename: "local.txt", blobData: localBlob
        )
        let note = try noteRepo.create(content: "local content", tags: [], key: key)
        try noteRepo.addAttachment(noteId: note.id, ref: localRef)

        let serverBlob = Data("server attachment bytes".utf8)
        let serverRef = try makeStoredAttachment(
            attachmentRepo: attachmentRepo, key: key, filename: "server.txt", blobData: serverBlob
        )

        let conflict = try makeConflict(
            noteId: note.id,
            localContent: "local content the user rejected",
            localAttachments: [localRef],
            serverContent: "server content the user chose to keep",
            serverAttachments: [serverRef],
            key: key
        )
        await service.addPendingConflictForTesting(conflict)

        try await service.resolveConflict(noteId: note.id, strategy: .keepServer)

        let raw = try #require(try noteRepo.getRaw(id: note.id))
        let attachments = decodeAttachments(raw.attachments)
        #expect(attachments.map(\.id) == [serverRef.id])
    }

    @Test func keepLocalLeavesLocalAttachmentsUntouched() async throws {
        let (service, noteRepo, attachmentRepo, key) = try makeServices()

        let localBlob = Data("local attachment bytes".utf8)
        let localRef = try makeStoredAttachment(
            attachmentRepo: attachmentRepo, key: key, filename: "local.txt", blobData: localBlob
        )
        let note = try noteRepo.create(content: "local content", tags: [], key: key)
        try noteRepo.addAttachment(noteId: note.id, ref: localRef)

        let conflict = try makeConflict(
            noteId: note.id,
            localContent: "local content the user chose to keep",
            localAttachments: [localRef],
            serverContent: "server content the user rejected",
            serverAttachments: [],
            key: key
        )
        await service.addPendingConflictForTesting(conflict)

        try await service.resolveConflict(noteId: note.id, strategy: .keepLocal)

        let raw = try #require(try noteRepo.getRaw(id: note.id))
        let attachments = decodeAttachments(raw.attachments)
        #expect(attachments.map(\.id) == [localRef.id])
    }

    /// When the local row vanished before resolution (see ConflictResolutionSafetyTests),
    /// `.keepLocal` must recreate the note from the conflict snapshot including its
    /// attachment refs — not silently drop them.
    @Test func keepLocalRecreatesAttachmentsWhenRowMissing() async throws {
        let (service, noteRepo, attachmentRepo, key) = try makeServices()
        let noteId = CryptoService.generateUUID()

        let localBlob = Data("orphaned local attachment bytes".utf8)
        let attachmentId = CryptoService.generateUUID()
        try attachmentRepo.storeBlob(
            id: attachmentId, filename: "local.txt", mimeType: "text/plain", size: localBlob.count, data: localBlob
        )
        let encFilename = try CryptoService.encryptText("local.txt", key: key)
        let localRef = AttachmentRef(
            id: attachmentId,
            filename: try CryptoService.serializeEncryptedJSON(encFilename),
            mimeType: "text/plain",
            size: localBlob.count,
            data: attachmentId
        )

        let conflict = try makeConflict(
            noteId: noteId,
            localContent: "the local edit the user chose to keep",
            localAttachments: [localRef],
            serverContent: "server content the user rejected",
            serverAttachments: [],
            key: key
        )
        await service.addPendingConflictForTesting(conflict)

        #expect(try noteRepo.getRaw(id: noteId) == nil)

        try await service.resolveConflict(noteId: noteId, strategy: .keepLocal)

        let raw = try #require(try noteRepo.getRaw(id: noteId))
        let attachments = decodeAttachments(raw.attachments)
        #expect(attachments.map(\.id) == [localRef.id])
    }
}
