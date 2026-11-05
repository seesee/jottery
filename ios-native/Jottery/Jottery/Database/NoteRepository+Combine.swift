import CryptoKit
import Foundation
import GRDB

extension NoteRepository {

    /// Make one note from `notes`, given oldest first, and move them to the bin.
    /// Returns the new note's id.
    ///
    /// One transaction, so it either happens or it doesn't: a failure part way
    /// leaves no half-made note and no original in the bin. The new note gets its
    /// own copy of every attachment, because a note in the bin can be purged and
    /// removing an attachment deletes its file (jottery-kxxx).
    func combine(_ notes: [DecryptedNote], mergeTags: Bool, key: SymmetricKey) throws -> String {
        // As stored, not as decrypted, so that filenames stay encrypted.
        var originals: [AttachmentRef] = []
        var seen = Set<String>()
        for note in notes {
            let stored = try getRaw(id: note.id)?.attachments ?? "[]"
            let refs = (try? JSONDecoder().decode([AttachmentRef].self, from: Data(stored.utf8))) ?? []
            originals += refs.filter { seen.insert($0.id).inserted }
        }

        var newIds: [String: String] = [:]
        let copies = originals.map { ref -> AttachmentRef in
            let newId = CryptoService.generateUUID()
            newIds[ref.id] = newId
            return AttachmentRef(
                id: newId, filename: ref.filename, mimeType: ref.mimeType, size: ref.size,
                data: newId, thumbnailData: ref.thumbnailData
            )
        }

        let content = NoteCombining.content(NoteCombining.content(of: notes), followingCopies: newIds)
        let tags = NoteCombining.tags(of: notes, merging: mergeTags)

        let contentJSON = try CryptoService.serializeEncryptedJSON(CryptoService.encryptText(content, key: key))
        let tagsJSON = try CryptoService.serializeEncryptedJSON(CryptoService.encryptStringArray(tags, key: key))
        var record = NoteRecord.new(encryptedContent: contentJSON, encryptedTags: tagsJSON)
        record.attachments = String(decoding: try JSONEncoder().encode(copies), as: UTF8.self)

        let contentHash = HashChainService.computeContentHash(
            encryptedContent: contentJSON,
            encryptedTags: try tags.map { try CryptoService.encryptTagForSync($0, key: key) },
            attachmentIds: copies.map(\.id).sorted(),
            pinned: record.pinned,
            archived: record.archived,
            locked: record.locked,
            syntaxLanguage: record.syntaxLanguage,
            wordWrap: record.wordWrap,
            showPreview: record.showPreview,
            color: record.color
        )
        record.contentHash = contentHash
        let hashChain = HashChainService.updateHashChain(currentHash: contentHash, parentChain: [])
        record.hashChain = String(decoding: try JSONEncoder().encode(hashChain), as: UTF8.self)

        let finished = record
        try db.dbPool.write { db in
            for (original, copy) in zip(originals, copies) {
                // Copied inside the database, so a large file never passes through
                // memory. The ciphertext doesn't depend on the attachment's id.
                try db.execute(sql: """
                    INSERT INTO attachments (id, filename, mime_type, size, data, uploaded)
                    SELECT ?, '', mime_type, size, data, 0 FROM attachments WHERE id = ?
                """, arguments: [copy.id, original.data])
                guard db.changesCount > 0 else { throw NoteCombining.Failure.attachmentNotOnDevice }
            }
            try finished.insert(db)
            try Self.softDelete(ids: notes.map(\.id), at: Date().iso8601, in: db)
        }
        return finished.id
    }
}
