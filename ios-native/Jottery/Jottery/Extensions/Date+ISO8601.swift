import Foundation

extension ISO8601DateFormatter {
    /// Shared formatter matching Jottery's ISO 8601 format with timezone.
    nonisolated(unsafe) static let jottery: ISO8601DateFormatter = {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter
    }()
}

extension Date {
    /// Format as ISO 8601 string with timezone.
    var iso8601: String {
        ISO8601DateFormatter.jottery.string(from: self)
    }

    /// Parse an ISO 8601 string.
    init?(iso8601 string: String) {
        // Try with fractional seconds first
        if let date = ISO8601DateFormatter.jottery.date(from: string) {
            self = date
            return
        }
        // Fall back to without fractional seconds
        let plain = ISO8601DateFormatter()
        plain.formatOptions = [.withInternetDateTime]
        if let date = plain.date(from: string) {
            self = date
            return
        }
        return nil
    }

    /// Relative time description (e.g. "2 hours ago", "Yesterday").
    var relativeDescription: String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .short
        return formatter.localizedString(for: self, relativeTo: Date())
    }
}
