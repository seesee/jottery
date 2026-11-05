import Foundation

/// The rules for combining notes, which are the web client's `combineNotes`.
/// `NoteRepository.combine` applies them.
enum NoteCombining {

    enum Failure: Error, Equatable {
        /// It takes two notes that can still be changed.
        case tooFewNotes
        /// A file hasn't reached this device, so it can't be copied.
        case attachmentNotOnDevice
    }

    static let separator = "\n\n---\n\n"

    /// The notes that can be combined, oldest first. Locked and archived notes
    /// are read-only, so they aren't taken apart and put in the bin.
    static func combinable(_ notes: [DecryptedNote]) -> [DecryptedNote] {
        notes
            .filter { !$0.locked && !$0.archived && !$0.deleted }
            .sorted { ($0.createdAt, $0.id) < ($1.createdAt, $1.id) }
    }

    static func content(of notes: [DecryptedNote]) -> String {
        notes.map(\.content).joined(separator: separator)
    }

    /// Every tag once in the order they're met, or only the oldest note's.
    static func tags(of notes: [DecryptedNote], merging: Bool) -> [String] {
        guard merging else { return notes.first?.tags ?? [] }
        var seen = Set<String>()
        return notes.flatMap(\.tags).filter { seen.insert($0).inserted }
    }

    /// `content` with its attachment references moved to the copies. Ids are
    /// UUIDs, so one can't be part of another.
    static func content(_ content: String, followingCopies newIds: [String: String]) -> String {
        newIds.reduce(content) { text, id in
            text.replacingOccurrences(of: "attachment:\(id.key)", with: "attachment:\(id.value)")
        }
    }
}
