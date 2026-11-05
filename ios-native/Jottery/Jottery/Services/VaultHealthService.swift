import CryptoKit
import Foundation

/// What a vault health scan found. Held in memory while the screen is open.
/// Nothing here came from a note that couldn't be read, beyond its dates.
struct VaultHealthReport: Sendable, Equatable {

    /// A note whose content or tags won't decrypt. The lists leave these out, so
    /// this is the only place one can be seen or removed.
    struct UnreadableNote: Identifiable, Sendable, Equatable {
        let id: String
        let modifiedAt: Date?
        let inBin: Bool
    }

    /// An attachment a note lists, with no blob on this device.
    struct MissingAttachment: Identifiable, Sendable, Equatable {
        let attachmentId: String
        let blobId: String
        let noteId: String
        /// Nil if the note can't be read.
        let noteTitle: String?
        let filename: String
        let mimeType: String
        let size: Int

        var id: String { attachmentId }
    }

    /// An attachment this device has the blob for, on a note the server has.
    struct HeldAttachment: Identifiable, Sendable, Equatable {
        let attachmentId: String
        let blobId: String
        let noteId: String
        let noteTitle: String?
        let filename: String

        var id: String { attachmentId }
    }

    var notesChecked = 0
    var unreadableNotes: [UnreadableNote] = []
    var missingAttachments: [MissingAttachment] = []
    var heldAttachments: [HeldAttachment] = []

    var isHealthy: Bool { unreadableNotes.isEmpty && missingAttachments.isEmpty }
}

/// Finds what's wrong with a vault, and repairs what can be repaired. It follows
/// `vaultHealthService.ts`, with one addition. Push warnings only cover notes in
/// the current push, so a synced note that's never edited again would never hear
/// that the server lost its blob (jottery-m3wg). Here that's asked directly.
///
/// The server is untrusted. A blob it sends is stored only if it answers to the
/// right id and opens with the vault's key: anything else is an error, and in
/// particular plaintext is never kept because it didn't look encrypted.
enum VaultHealthService {

    typealias Progress = BackupService.Progress

    private static let batchSize = 250

    enum RepairResult: Sendable, Equatable {
        case repaired
        /// The server has no blob either, so only removing the reference is left.
        case notOnServer
        /// What came back wasn't this attachment, and it wasn't stored.
        case unreadable
        /// The server couldn't be asked. Trying again may work.
        case failed
    }

    enum UploadResult: Sendable, Equatable {
        case uploaded
        /// The server took the request, and still doesn't have the blob.
        case notKept
        /// The blob here doesn't open, so it isn't worth sending.
        case unreadable
        case failed
    }

    // MARK: - Scanning

    /// Look through every note, a batch at a time. Run this off the main actor.
    /// Stops with `CancellationError` if the current task is cancelled.
    static func scan(
        noteRepo: NoteRepository,
        attachmentRepo: AttachmentRepository,
        key: SymmetricKey,
        progress: (@Sendable (Progress) -> Void)? = nil
    ) throws -> VaultHealthReport {
        let stored = Set(try attachmentRepo.listBlobIds())
        let ids = try noteRepo.allNoteIds()
        var report = VaultHealthReport()
        var seen = Set<String>()

        for start in stride(from: 0, to: ids.count, by: batchSize) {
            try Task.checkCancellation()
            try autoreleasepool {
                let batch = Array(ids[start..<min(start + batchSize, ids.count)])
                for record in try noteRepo.rawRecords(ids: batch) {
                    check(record, stored: stored, seen: &seen, into: &report, noteRepo: noteRepo, key: key)
                }
                report.notesChecked += batch.count
            }
            progress?(Progress(completed: report.notesChecked, total: ids.count))
        }

        report.unreadableNotes.sort { ($0.modifiedAt ?? .distantPast) > ($1.modifiedAt ?? .distantPast) }
        return report
    }

    private static func check(
        _ record: NoteRecord,
        stored: Set<String>,
        seen: inout Set<String>,
        into report: inout VaultHealthReport,
        noteRepo: NoteRepository,
        key: SymmetricKey
    ) {
        let note = try? noteRepo.decrypt(record, key: key)
        if note == nil {
            report.unreadableNotes.append(VaultHealthReport.UnreadableNote(
                id: record.id,
                modifiedAt: Date(iso8601: record.modifiedAt),
                inBin: record.deleted
            ))
        }
        // A note in the bin is on its way out, so its blobs don't matter.
        guard !record.deleted else { return }

        let refs = (try? JSONDecoder().decode([AttachmentRef].self, from: Data(record.attachments.utf8))) ?? []
        for ref in refs where seen.insert(ref.id).inserted {
            let filename = AttachmentFilename.display(stored: ref.filename, key: key)
            if !stored.contains(ref.data) {
                report.missingAttachments.append(VaultHealthReport.MissingAttachment(
                    attachmentId: ref.id,
                    blobId: ref.data,
                    noteId: record.id,
                    noteTitle: note?.title,
                    filename: filename,
                    mimeType: ref.mimeType,
                    size: ref.size
                ))
            } else if !record.needsSync {
                // A note still to be pushed takes its blob with it.
                report.heldAttachments.append(VaultHealthReport.HeldAttachment(
                    attachmentId: ref.id,
                    blobId: ref.data,
                    noteId: record.id,
                    noteTitle: note?.title,
                    filename: filename
                ))
            }
        }
    }

    // MARK: - Repairing This Device

    /// Fetch a missing blob from the server. Never throws: the caller shows
    /// the result beside the attachment.
    static func repair(
        _ missing: VaultHealthReport.MissingAttachment,
        client: SyncClient,
        attachmentRepo: AttachmentRepository,
        key: SymmetricKey
    ) async -> RepairResult {
        // Notes come from the server, and this one goes into a URL.
        guard SyncValidation.isId(missing.attachmentId), SyncValidation.isId(missing.blobId) else {
            return .unreadable
        }

        let response: SyncAttachment?
        do {
            response = try await client.fetchAttachment(id: missing.attachmentId)
        } catch {
            Log.debug("[VaultHealth] Couldn't fetch attachment \(missing.attachmentId): \(error)")
            return .failed
        }
        guard let response else { return .notOnServer }

        guard response.id == missing.attachmentId,
              let blob = Data(base64Encoded: response.data),
              await opens(blob, key: key) else {
            return .unreadable
        }

        do {
            try attachmentRepo.storeBlob(
                id: missing.blobId,
                mimeType: missing.mimeType,
                size: missing.size,
                data: blob,
                uploaded: true
            )
            return .repaired
        } catch {
            Log.debug("[VaultHealth] Couldn't store attachment \(missing.attachmentId): \(error)")
            return .failed
        }
    }

    /// Whether `blob` is really something encrypted with this vault's key. It's
    /// authenticated, so nothing but the key's holder can pass this.
    private static func opens(_ blob: Data, key: SymmetricKey) async -> Bool {
        let opened = try? await offMainActor { () -> Bool in
            let encrypted = try CryptoService.parseEncryptedJSON(String(decoding: blob, as: UTF8.self))
            _ = try CryptoService.decrypt(encrypted, key: key)
            return true
        }
        return opened == true
    }

    // MARK: - Repairing the Server

    /// Which of `held` the server has lost, asking about each one without
    /// downloading it. Throws if the server can't answer: not knowing whether it
    /// has a blob is no reason to say it doesn't.
    static func missingFromServer(
        _ held: [VaultHealthReport.HeldAttachment],
        client: SyncClient,
        progress: (@Sendable (Progress) -> Void)? = nil
    ) async throws -> [VaultHealthReport.HeldAttachment] {
        var missing: [VaultHealthReport.HeldAttachment] = []
        for (index, attachment) in held.enumerated() {
            try Task.checkCancellation()
            if SyncValidation.isId(attachment.attachmentId),
               try await !client.hasAttachment(id: attachment.attachmentId) {
                missing.append(attachment)
            }
            progress?(Progress(completed: index + 1, total: held.count))
        }
        return missing
    }

    /// Send a blob the server has lost back to it, one at a time so that each
    /// fits the upload limit. The server then re-announces the notes that use
    /// it, which is what repairs other devices. It ignores a blob it can't
    /// place without saying so, so it's asked afterwards.
    static func reupload(
        _ held: VaultHealthReport.HeldAttachment,
        client: SyncClient,
        attachmentRepo: AttachmentRepository,
        key: SymmetricKey
    ) async -> UploadResult {
        guard SyncValidation.isId(held.attachmentId),
              let blob = try? attachmentRepo.getBlob(id: held.blobId),
              await opens(blob, key: key) else {
            return .unreadable
        }

        do {
            _ = try await client.push(SyncPushRequest(
                notes: [],
                attachments: [SyncAttachment(id: held.attachmentId, data: blob.base64EncodedString())],
                versions: []
            ))
            guard try await client.hasAttachment(id: held.attachmentId) else { return .notKept }
        } catch {
            Log.debug("[VaultHealth] Couldn't send attachment \(held.attachmentId): \(error)")
            return .failed
        }
        try? attachmentRepo.markUploaded(ids: [held.blobId])
        return .uploaded
    }
}
