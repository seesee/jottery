import Foundation

/// Checks on what the server sends, made as it arrives. The server stores
/// ciphertext and can't read notes, so nothing it says is trusted: an entry
/// that fails is dropped, and the rest of the page is still applied. A local
/// copy stays as it was, and the entry returns if it's ever sent correctly.
enum SyncValidation {

    /// Nothing the server sends for one attachment can be larger than this.
    static let maxAttachmentSize = 2_000_000_000
    static let maxMimeTypeLength = 255
    static let maxSyntaxLanguageLength = 64
    static let maxColorLength = 64
    /// Cursors and the like are "a short ASCII string". Anything else isn't one.
    static let maxTokenLength = 256

    static func isId(_ value: String) -> Bool {
        UUID(uuidString: value) != nil
    }

    static func isTimestamp(_ value: String) -> Bool {
        Date(iso8601: value) != nil
    }

    private static func isOptionalTimestamp(_ value: String?) -> Bool {
        value.map(isTimestamp) ?? true
    }

    /// `type/subtype`, with optional parameters like `charset=utf-8`.
    static func isMimeType(_ value: String) -> Bool {
        guard !value.isEmpty, value.utf8.count <= maxMimeTypeLength else { return false }
        let token = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "!#$&^_.+-"))
        let pieces = value.split(separator: ";", omittingEmptySubsequences: false)
        let parameters = token.union(CharacterSet(charactersIn: "= "))
        guard pieces.dropFirst().allSatisfy({ $0.unicodeScalars.allSatisfy(parameters.contains) }) else { return false }
        let parts = pieces[0].trimmingCharacters(in: .whitespaces).split(separator: "/", omittingEmptySubsequences: false)
        return parts.count == 2 && parts.allSatisfy { !$0.isEmpty && $0.unicodeScalars.allSatisfy(token.contains) }
    }

    static func isValid(_ ref: AttachmentRef) -> Bool {
        isId(ref.id)
            && isId(ref.data)
            && isMimeType(ref.mimeType)
            && (0...maxAttachmentSize).contains(ref.size)
    }

    static func isValid(_ note: SyncNote) -> Bool {
        isId(note.id)
            && isTimestamp(note.createdAt)
            && isTimestamp(note.modifiedAt)
            && isOptionalTimestamp(note.deletedAt)
            && isOptionalTimestamp(note.archivedAt)
            && isOptionalTimestamp(note.lockedAt)
            && note.attachments.allSatisfy(isValid)
            && (note.syntaxLanguage?.utf8.count ?? 0) <= maxSyntaxLanguageLength
            && (note.color?.utf8.count ?? 0) <= maxColorLength
    }

    static func isValid(_ deletion: SyncDeletion) -> Bool {
        isId(deletion.id) && isTimestamp(deletion.deletedAt)
    }

    static func isValid(_ attachment: SyncAttachment) -> Bool {
        isId(attachment.id)
    }

    /// A version's key is `<noteId>:<version>`. Checking the prefix stops one
    /// note's version being written over another's.
    static func isValid(_ version: SyncNoteVersion) -> Bool {
        isId(version.noteId)
            && version.versionKey.hasPrefix("\(version.noteId):")
            && isTimestamp(version.createdAt)
            && isTimestamp(version.syncedAt)
            && version.attachments.allSatisfy(isValid)
            && (version.syntaxLanguage?.utf8.count ?? 0) <= maxSyntaxLanguageLength
            && (version.color?.utf8.count ?? 0) <= maxColorLength
    }

    static func isValid(_ search: SyncSavedSearch) -> Bool {
        isId(search.id)
            && isTimestamp(search.createdAt)
            && isTimestamp(search.modifiedAt)
            && isOptionalTimestamp(search.deletedAt)
    }

    /// An opaque value to hand back to the server, or nil when it isn't usable,
    /// which is treated as a server that doesn't support the feature. This also
    /// keeps anything that could break out of a header from reaching one.
    static func opaqueToken(_ value: String?) -> String? {
        guard let value, value.utf8.count <= maxTokenLength,
              value.unicodeScalars.allSatisfy({ (0x20...0x7E).contains($0.value) }) else {
            return nil
        }
        return value
    }

    static let maxDisplayNameLength = 100

    /// A name from the server, fit to show: no control characters or the
    /// overrides that reorder text, and of a sensible length. Nil if nothing's left.
    static func displayName(_ raw: String) -> String? {
        let kept = raw.unicodeScalars.filter { scalar in
            scalar.properties.generalCategory != .control
                && !(0x202A...0x202E).contains(scalar.value)
                && !(0x2066...0x2069).contains(scalar.value)
        }
        let text = String(String.UnicodeScalarView(kept)).trimmingCharacters(in: .whitespacesAndNewlines)
        return text.isEmpty ? nil : String(text.prefix(maxDisplayNameLength))
    }

    /// `page` with everything that failed removed.
    static func sanitised(_ page: SyncPullResponse) -> SyncPullResponse {
        var clean = page
        clean.notes = page.notes.filter(isValid)
        clean.deletions = page.deletions?.filter(isValid)
        clean.attachments = page.attachments.filter(isValid)
        clean.versions = page.versions.filter(isValid)
        clean.savedSearches = page.savedSearches?.filter(isValid)

        let dropped = page.notes.count - clean.notes.count
            + (page.deletions?.count ?? 0) - (clean.deletions?.count ?? 0)
            + page.attachments.count - clean.attachments.count
            + page.versions.count - clean.versions.count
            + (page.savedSearches?.count ?? 0) - (clean.savedSearches?.count ?? 0)
        if dropped > 0 {
            Log.debug("[Sync] pull: dropped \(dropped) invalid item(s) from a page")
        }
        return clean
    }
}

/// Decodes one element of an array, keeping the rest if this one is malformed.
private struct Lossy<Element: Decodable>: Decodable {
    let value: Element?

    init(from decoder: Decoder) throws {
        value = try? Element(from: decoder)
    }
}

extension KeyedDecodingContainer {
    /// An array that survives malformed elements. Returns how many there were
    /// before any were dropped, which an offset has to count.
    func decodeLossily<Element: Decodable>(
        _ type: [Element].Type,
        forKey key: Key
    ) throws -> (elements: [Element], received: Int) {
        let lossy = try decode([Lossy<Element>].self, forKey: key)
        return (lossy.compactMap(\.value), lossy.count)
    }

    func decodeLossilyIfPresent<Element: Decodable>(_ type: [Element].Type, forKey key: Key) throws -> [Element]? {
        try decodeIfPresent([Lossy<Element>].self, forKey: key)?.compactMap(\.value)
    }
}
