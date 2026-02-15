import Foundation
import UIKit

extension RelativeDateTimeFormatter {
    /// Shared formatter — creating `RelativeDateTimeFormatter` is expensive
    /// on first use (ICU locale data loading). A shared instance avoids
    /// per-row allocation in note lists and amortises the cold-start cost.
    nonisolated(unsafe) static let shared: RelativeDateTimeFormatter = {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .short
        return formatter
    }()
}

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
        RelativeDateTimeFormatter.shared.localizedString(for: self, relativeTo: Date())
    }

    /// Pre-warm expensive subsystems so the first search-bar tap feels instant.
    /// Call once after unlock on the main thread — the heavy work (ICU locale
    /// loading, keyboard framework init) is done before the user interacts.
    @MainActor static func warmUpForSearch() {
        // 1. Trigger RelativeDateTimeFormatter ICU locale loading
        _ = RelativeDateTimeFormatter.shared.localizedString(for: Date(), relativeTo: Date())

        // 2. Pre-warm the iOS keyboard subsystem.
        //    When the user unlocks via Face ID the keyboard has never been loaded
        //    in this process. Briefly making a hidden field first responder forces
        //    the system to initialise the input stack without showing the keyboard.
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = scene.windows.first else { return }
        let field = UITextField(frame: CGRect(x: -1, y: -1, width: 1, height: 1))
        field.autocorrectionType = .no
        window.addSubview(field)
        field.becomeFirstResponder()
        field.resignFirstResponder()
        field.removeFromSuperview()
    }
}
