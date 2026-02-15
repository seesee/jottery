import Foundation
import GRDB

/// User application settings — stored in the `settings` table.
struct UserSettings: Codable, FetchableRecord, PersistableRecord {
    static let databaseTableName = "settings"

    var id: Int = 1
    var language: String
    var theme: String            // "light", "dark", "auto"
    var sortOrder: String        // "recent", "oldest", "alpha", "created"
    var autoLockTimeout: Int     // Minutes
    var syncEnabled: Bool
    var syncEndpoint: String?

    enum CodingKeys: String, CodingKey {
        case id
        case language
        case theme
        case sortOrder = "sort_order"
        case autoLockTimeout = "auto_lock_timeout"
        case syncEnabled = "sync_enabled"
        case syncEndpoint = "sync_endpoint"
    }

    static let defaults = UserSettings(
        language: "system",
        theme: "auto",
        sortOrder: "recent",
        autoLockTimeout: 15,
        syncEnabled: false,
        syncEndpoint: nil
    )

    var themeMode: ThemeMode {
        get { ThemeMode(rawValue: theme) ?? .auto }
        set { theme = newValue.rawValue }
    }

    var sort: SortOrder {
        get { SortOrder(rawValue: sortOrder) ?? .recent }
        set { sortOrder = newValue.rawValue }
    }
}

enum ThemeMode: String, CaseIterable {
    case light, dark, auto
}

enum SortOrder: String, CaseIterable {
    case recent, oldest, alpha, created

    var displayName: String {
        switch self {
        case .recent: return L.sortRecentlyModified
        case .oldest: return L.sortOldestModified
        case .alpha: return L.sortAlphabetical
        case .created: return L.sortDateCreated
        }
    }
}
