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

struct CloneDeviceRequest: Codable {
    let apiKey: String
    let deviceName: String
    let deviceType: String  // "ios"
}

// MARK: - Sync Push

struct SyncPushRequest: Codable {
    let notes: [SyncNote]
    let attachments: [SyncAttachment]
    let versions: [SyncNoteVersion]
    var deletions: [SyncDeletion]?
    var savedSearches: [SyncSavedSearch]?
}

struct SyncPushResponse: Codable {
    let accepted: [SyncAccepted]
    let rejected: [SyncRejected]
    var errors: [String]?
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
    var serverSyntaxLanguage: String?
    var serverWordWrap: Bool?
    var serverShowPreview: Bool?
    var serverContentHash: String?
    var serverParentHash: String?
    var serverHashChain: [String]?
    var ancestorHash: String?
    var ancestorContent: String?
    var ancestorTags: [String]?
}

// MARK: - Sync Pull

struct SyncPullRequest: Codable {
    var lastSyncAt: String?
    let knownNoteIds: [String]
    let knownAttachmentIds: [String]
    var limit: Int?
    var offset: Int?
}

struct SyncPullResponse: Codable {
    let notes: [SyncNote]
    var deletions: [SyncDeletion]?
    let attachments: [SyncAttachment]
    let versions: [SyncNoteVersion]
    var savedSearches: [SyncSavedSearch]?
    var syncedAt: String?
    var totalCount: Int?
    var hasMore: Bool?
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
