import Foundation

/// A tag and how many notes have it. Tags are encrypted in the database, so this
/// is worked out from notes that are already decrypted in memory.
struct TagCount: Identifiable, Equatable {
    let tag: String
    let count: Int
    var id: String { tag }

    static func counts(in notes: [DecryptedNote]) -> [TagCount] {
        var counts: [String: Int] = [:]
        for note in notes {
            for tag in Set(note.regularTags) {
                counts[tag, default: 0] += 1
            }
        }
        return counts
            .map { TagCount(tag: $0.key, count: $0.value) }
            .sorted { $0.tag.localizedCaseInsensitiveCompare($1.tag) == .orderedAscending }
    }
}
