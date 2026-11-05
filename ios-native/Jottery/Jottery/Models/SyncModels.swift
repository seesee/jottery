import Foundation

// MARK: - Device Registration

struct RegisterDeviceRequest: Codable {
    let email: String
    let password: String
    let deviceName: String
    let deviceType: String  // "ios"
}

struct RegisterDeviceResponse: Codable {
    let clientId: String
    let apiKey: String
    let userId: String
    let deviceName: String
}

// MARK: - Account Registration

struct RegisterUserResponse: Codable {
    let userId: String
    let email: String
    let status: String  // "pending_approval" or "approved"

    var isApproved: Bool { status == "approved" }
}

/// Where an account has got to. Whether it can connect a device is `isApproved`.
struct UserStatusResponse: Codable {
    let exists: Bool
    let isApproved: Bool
    let isActive: Bool
}

// MARK: - Sync Push

struct SyncPushRequest: Codable {
    let notes: [SyncNote]
    let attachments: [SyncAttachment]
    let versions: [SyncNoteVersion]
    var deletions: [SyncDeletion]?
    var savedSearches: [SyncSavedSearch]?
    /// Each note's `attachments` is its complete list, so the server can drop
    /// the references that are gone. A server that doesn't know it ignores it.
    var attachmentsAuthoritative: Bool?
}

struct SyncPushResponse: Codable {
    let accepted: [SyncAccepted]
    let rejected: [SyncRejected]
    var errors: [String]?
    /// Accepted notes referencing attachment data the server does not hold.
    /// Absent on older servers; the client re-pushes blobs it has (self-heal).
    var attachmentWarnings: [AttachmentWarning]?
    /// Deletions refused because the note has a newer edit. Absent on older
    /// servers. The note comes back through a normal pull.
    var rejectedDeletions: [String]?
    /// Pushed notes skipped because they were permanently deleted after this
    /// edit. Absent on older servers.
    var skippedDeleted: [String]?
}

/// One accepted note whose attachment references have no data on the server.
struct AttachmentWarning: Codable {
    let noteId: String
    let attachmentIds: [String]
}

struct SyncAccepted: Codable {
    let id: String
    var serverVersion: Int?
    var syncedAt: String?
}

struct SyncRejected: Codable {
    let id: String
    let reason: String
    let serverModifiedAt: String
    let serverContent: String
    let serverTags: [String]
    let serverVersion: Int
    let serverAttachments: [AttachmentRef]
    let serverPinned: Bool
    var serverArchived: Bool?
    var serverArchivedAt: String?
    var serverLocked: Bool?
    var serverLockedAt: String?
    /// Whether the server's copy is in the bin. Absent on older servers.
    var serverDeleted: Bool?
    var serverDeletedAt: String?
    var serverSyntaxLanguage: String?
    var serverWordWrap: Bool?
    var serverShowPreview: Bool?
    var serverColor: String?
    var serverContentHash: String?
    var serverParentHash: String?
    var serverHashChain: [String]?
    var ancestorHash: String?
    var ancestorContent: String?
    var ancestorTags: [String]?
}

// MARK: - Sync Pull

struct SyncPullRequest: Codable {
    /// Selects cursor mode, even when empty. An older server ignores it and
    /// goes by `lastSyncAt` and `offset`.
    var cursor: String?
    var lastSyncAt: String?
    let knownNoteIds: [String]
    let knownAttachmentIds: [String]
    var limit: Int?
    var offset: Int?
}

struct SyncPullResponse: Codable {
    var notes: [SyncNote]
    var deletions: [SyncDeletion]?
    var attachments: [SyncAttachment]
    var versions: [SyncNoteVersion]
    var savedSearches: [SyncSavedSearch]?
    var syncedAt: String?
    var totalCount: Int?
    var hasMore: Bool?
    /// Only in reply to a request with a `cursor`, from a server that has them.
    var nextCursor: String?
    /// How many notes the server sent, including any that were malformed: what
    /// an offset has to advance by.
    var receivedNoteCount: Int?

    enum CodingKeys: String, CodingKey {
        case notes, deletions, attachments, versions, savedSearches
        case syncedAt, totalCount, hasMore, nextCursor
    }
}

extension SyncPullResponse {
    /// One malformed entry mustn't lose the rest of the page.
    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let notes = try container.decodeLossily([SyncNote].self, forKey: .notes)
        self.init(
            notes: notes.elements,
            deletions: try container.decodeLossilyIfPresent([SyncDeletion].self, forKey: .deletions),
            attachments: try container.decodeLossily([SyncAttachment].self, forKey: .attachments).elements,
            versions: try container.decodeLossily([SyncNoteVersion].self, forKey: .versions).elements,
            savedSearches: try container.decodeLossilyIfPresent([SyncSavedSearch].self, forKey: .savedSearches),
            syncedAt: try? container.decodeIfPresent(String.self, forKey: .syncedAt),
            totalCount: try? container.decodeIfPresent(Int.self, forKey: .totalCount),
            hasMore: try? container.decodeIfPresent(Bool.self, forKey: .hasMore),
            nextCursor: try? container.decodeIfPresent(String.self, forKey: .nextCursor),
            receivedNoteCount: notes.received
        )
    }
}

// MARK: - Sync Note

struct SyncNote: Codable {
    let id: String
    let createdAt: String
    let modifiedAt: String
    let content: String
    let tags: [String]
    let attachments: [AttachmentRef]
    let pinned: Bool
    var archived: Bool
    var archivedAt: String?
    var locked: Bool?
    var lockedAt: String?
    let deleted: Bool
    var deletedAt: String?
    let version: Int
    var wordWrap: Bool?
    var syntaxLanguage: String?
    var showPreview: Bool?
    var color: String?
    var contentHash: String?
    var parentHash: String?
    var hashChain: [String]?
}

struct SyncAttachment: Codable {
    let id: String
    let data: String  // Base64 encoded encrypted blob
}

struct SyncDeletion: Codable {
    let id: String
    let deletedAt: String
}

struct SyncNoteVersion: Codable {
    let versionKey: String
    let noteId: String
    let version: Int
    let createdAt: String
    let syncedAt: String
    let content: String
    let tags: [String]
    let attachments: [AttachmentRef]
    var syntaxLanguage: String?
    var wordWrap: Bool?
    var showPreview: Bool?
    var color: String?
    let reason: String
    var contentHash: String?
    var parentHash: String?
}

// MARK: - Inbox

struct InboxItem: Codable, Identifiable {
    let id: String
    let content: String
    let tags: [String]
    let createdAt: String
    var source: String?
    var sizeBytes: Int?
}

struct InboxStatusResponse: Codable {
    let count: Int
    let totalSizeBytes: Int
    let maxItems: Int
    let maxSizeMb: Int
}

struct InboxTokenResponse: Codable {
    let token: String
}

struct InboxTokenStatusResponse: Codable {
    let hasToken: Bool
}

// MARK: - Sync Status

struct SyncStatusResponse: Codable {
    let clientId: String
    let deviceName: String
    let serverLastModified: String
    let noteCount: Int
    var lastSyncedAt: String?
}

// MARK: - SSE Event Token

struct SSETokenResponse: Codable {
    let token: String
    let expiresIn: Int

    enum CodingKeys: String, CodingKey {
        case token
        case expiresIn = "expires_in"
    }
}

// MARK: - Wrapped Key (Envelope Encryption)

struct WrappedKeyResponse: Codable {
    let blob: String           // JSON {"ciphertext":"...","iv":"..."}
    let kdfVersion: Int
    let kdfIterations: Int
    /// Opaque, and changes on every write. Its presence means the server
    /// supports conditional writes.
    var updatedAt: String?
}

struct PutWrappedKeyRequest: Codable {
    let blob: String
    let kdfVersion: Int
    let kdfIterations: Int
    /// Set by `SyncClient.putWrappedKey` from what the write is for.
    var replace: Bool?
}

/// What a wrapped key upload is for, which decides when the server refuses it.
/// There's one key per account, and overwriting it with the wrong one locks out
/// every new device, so every upload has to say.
enum WrappedKeyWrite: Equatable, Sendable {
    /// The server had no key when asked. Refused if it has one now.
    case firstUpload
    /// Replacing a blob this device read and couldn't open. Refused if it has
    /// changed since. `updatedAt` is from that read, and nil on an older server.
    case repair(updatedAt: String?)
    /// The user changed the notes password. Never refused.
    case passwordChange
}

// MARK: - Sync Saved Search

struct SyncSavedSearch: Codable {
    let id: String
    let name: String          // Encrypted JSON string
    let query: String         // Encrypted JSON string
    let order: Int
    let createdAt: String
    let modifiedAt: String
    let deleted: Bool
    var deletedAt: String?
    let version: Int
}

// MARK: - Account Management

struct DeleteAccountRequest: Codable {
    let currentPassword: String
}

struct UserLoginRequest: Codable {
    let email: String
    let password: String
}

struct UserLoginResponse: Codable {
    let sessionId: String
    let expiresAt: String
}

/// How the server should treat the account.
/// Mirrors the server's `DeleteAccountQuery.mode`.
enum AccountDeletionMode: String {
    /// Soft delete — deactivates, re-registration needs admin approval.
    case deactivate
    /// Hard delete — removes the account and its data.
    case delete
}

// MARK: - Incremental Sync Refresh

/// The set of note ids a sync cycle touched, so `AppState` can refresh only
/// those rows instead of decrypting the whole vault. `updatedIds` covers
/// notes that were inserted or replaced (new from server, fast-forwarded,
/// auto-accepted, or marked synced after push); `deletedIds` covers notes
/// hard-deleted locally in response to a server deletion. When a write can't
/// be attributed to a specific id — or the change set is large — the caller
/// should fall back to a full reload via `fullReloadRequired`.
struct SyncChanges: Sendable {
    var updatedIds: Set<String> = []
    var deletedIds: Set<String> = []
    /// Notes that were in use until the server listed them as permanently
    /// deleted, and are now in the bin. Normally a note reaches the bin first,
    /// so a lot of these at once is suspicious.
    var removedByServerIds: Set<String> = []
    var fullReloadRequired: Bool = false
    /// Something this cycle sent went unanswered, and was dated over a day
    /// ahead of the server's clock. Never set by `merge`: `sync()` decides it.
    var clockLooksWrong = false
    /// Nothing ran: a sync was in progress, or the server has said to wait.
    var wasSkipped = false

    /// No changes, no reload needed — the default/no-op value.
    static let none = SyncChanges()

    /// No cycle ran at all, which isn't the same as finding nothing to do.
    static let skipped = SyncChanges(wasSkipped: true)

    /// Ambiguous or oversized change set — always safe, just not incremental.
    static let fullReload = SyncChanges(fullReloadRequired: true)

    mutating func merge(_ other: SyncChanges) {
        updatedIds.formUnion(other.updatedIds)
        deletedIds.formUnion(other.deletedIds)
        removedByServerIds.formUnion(other.removedByServerIds)
        fullReloadRequired = fullReloadRequired || other.fullReloadRequired
    }
}
