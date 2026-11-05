import Foundation

/// How a batch of attachments went.
struct AttachmentBatchResult {
    /// What went in, in the order it came, with names in the clear.
    var added: [AttachmentRef] = []
    var tooLarge: [String] = []
    var empty: [String] = []
    var failedCount = 0

    /// What to tell the user about whatever didn't go in.
    var problem: String? {
        if tooLarge.count > 1 { return L.attachmentsTooLargeCount(tooLarge.count) }
        if let filename = tooLarge.first { return L.attachmentsTooLarge(filename) }
        if failedCount > 0 { return L.errorCouldntAddAttachment }
        if let filename = empty.first { return L.attachmentsEmptyFile(filename) }
        return nil
    }
}

extension AppState {

    /// Add what was picked, pasted or dropped. One that can't go in doesn't stop
    /// the rest, and any copy made on the way in is gone when this returns.
    ///
    /// The 10 MB limit is here and not in `addAttachment`, which also finishes
    /// what the share extension staged: refusing that there would leave it staged
    /// for good, with no way to tell anyone.
    func addAttachments(_ incoming: [IncomingAttachment], to noteId: String) async -> AttachmentBatchResult {
        var result = AttachmentBatchResult()
        for item in incoming {
            defer { item.discard() }
            do {
                let data = try await Task.detached(priority: .userInitiated) { try item.read() }.value
                // Looked up after the read: the note may have been locked meanwhile.
                guard let note = displayedNote(id: noteId), !note.locked, !note.archived,
                      let added = try addAttachment(
                        to: noteId,
                        data: data,
                        filename: item.filename,
                        mimeType: item.mimeType
                      ) else {
                    result.failedCount += 1
                    continue
                }
                result.added.append(added)
            } catch AttachmentImportError.tooLarge {
                result.tooLarge.append(item.filename)
            } catch AttachmentImportError.empty {
                result.empty.append(item.filename)
            } catch {
                result.failedCount += 1
            }
        }
        return result
    }
}
