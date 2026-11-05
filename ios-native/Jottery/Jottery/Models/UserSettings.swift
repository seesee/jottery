import Foundation
import GRDB
import SwiftUI

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
    /// When a backup last left this device (ISO 8601). Never when nil.
    var lastBackupAt: String?
    /// Days between backup reminders. Off when nil.
    var backupReminderDays: Int?
    /// When reminders were turned on, to count from if there's no backup yet.
    var backupReminderSince: String?
    /// What a new blank note is (`NoteType`). Nil means the usual.
    var defaultSyntaxLanguage: String?

    enum CodingKeys: String, CodingKey {
        case id
        case language
        case theme
        case sortOrder = "sort_order"
        case autoLockTimeout = "auto_lock_timeout"
        case syncEnabled = "sync_enabled"
        case syncEndpoint = "sync_endpoint"
        case lastBackupAt = "last_backup_at"
        case backupReminderDays = "backup_reminder_days"
        case backupReminderSince = "backup_reminder_since"
        case defaultSyntaxLanguage = "default_syntax_language"
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

    var lastBackupDate: Date? {
        lastBackupAt.flatMap { Date(iso8601: $0) }
    }
}

enum ThemeMode: String, CaseIterable {
    case light, dark, auto

    /// What to force on the app. Nil follows the system.
    var colorScheme: ColorScheme? {
        switch self {
        case .light: return .light
        case .dark: return .dark
        case .auto: return nil
        }
    }
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
