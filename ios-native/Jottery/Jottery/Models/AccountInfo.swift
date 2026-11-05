import Foundation

/// `GET /user/account`. Everything's optional, because an older server sends
/// less. Figures come from the server, so they're kept to what makes sense
/// wherever they're used.
struct AccountInfo: Decodable, Equatable, Sendable {
    var email: String?
    var noteCount: Int?
    var attachmentCount: Int?
    var storageUsedBytes: Int64?
    var storageQuotaMb: Int64?
    var inbox: Inbox?

    struct Inbox: Decodable, Equatable, Sendable {
        var itemCount: Int?
        var maxItems: Int?
        var hasToken: Bool?
    }

    /// The quota in bytes, if there is one and it could be real.
    var quotaBytes: Int64? {
        guard let storageQuotaMb, storageQuotaMb > 0 else { return nil }
        let (bytes, overflowed) = storageQuotaMb.multipliedReportingOverflow(by: 1_048_576)
        return overflowed ? nil : bytes
    }

    /// How much of the quota is used, from 0 to 1.
    var usedFraction: Double? {
        guard let quotaBytes else { return nil }
        return min(max(Double(storageUsedBytes ?? 0) / Double(quotaBytes), 0), 1)
    }
}
