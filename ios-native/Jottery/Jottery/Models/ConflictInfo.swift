import Foundation

/// Holds both local and server versions of a conflicting note for resolution.
/// Decrypted content is stored for display; raw encrypted server data is stored for applying resolutions.
struct ConflictInfo: Identifiable, Sendable {
    let id: String  // noteId

    // Decrypted for display
    let localContent: String
    let localTags: [String]
    let localModifiedAt: String
    let localAttachments: [AttachmentRef]

    let serverContent: String
    let serverTags: [String]
    let serverModifiedAt: String

    // Encrypted server data for applying "Keep Server" / "Keep Both"
    let serverEncryptedContent: String
    let serverEncryptedTags: [String]
    let serverVersion: Int
    let serverAttachments: [AttachmentRef]
    let serverPinned: Bool
    let serverSyntaxLanguage: String?
    let serverWordWrap: Bool?
    let serverShowPreview: Bool?
    let serverContentHash: String?
    let serverParentHash: String?
    let serverHashChain: [String]?
    // Nil when the server didn't say, and the local value then stands.
    var serverArchived: Bool?
    var serverArchivedAt: String?
    var serverLocked: Bool?
    var serverLockedAt: String?
    var serverColor: String?
    var serverDeleted: Bool?
    var serverDeletedAt: String?
}

enum ConflictResolutionStrategy: Sendable {
    case keepLocal
    case keepServer
    case keepBoth
}
