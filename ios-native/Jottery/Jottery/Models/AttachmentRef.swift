import Foundation

/// Attachment reference (metadata only — data lives in blob store or sync payload).
struct AttachmentRef: Codable, Identifiable, Equatable, Hashable {
    let id: String
    var filename: String       // Encrypted when stored, decrypted in memory
    var mimeType: String
    var size: Int
    var data: String           // Reference ID to encrypted blob store
    var thumbnailData: String? // Optional thumbnail reference
}
