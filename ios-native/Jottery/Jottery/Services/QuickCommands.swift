import Foundation

/// Text the editor can write for you: typed as `/now`, or picked from its bar.
struct QuickCommand: Equatable, Sendable {

    enum Kind: Equatable, Sendable {
        case dateTime
        case date
        case time
        case uuid
        case text(String)
    }

    let trigger: String
    let kind: Kind

    /// What the command writes, in the web's formats. The numbers are built by
    /// hand, so a phone set to the Buddhist calendar or Arabic digits writes the
    /// same stamp as everything else.
    func expansion(now: Date = Date(), timeZone: TimeZone = .current) -> String {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = timeZone
        let parts = calendar.dateComponents([.year, .month, .day, .hour, .minute, .second], from: now)
        let date = String(format: "%04d-%02d-%02d", parts.year ?? 0, parts.month ?? 0, parts.day ?? 0)
        let time = String(format: "%02d:%02d:%02d", parts.hour ?? 0, parts.minute ?? 0, parts.second ?? 0)

        switch kind {
        case .dateTime: return "\(date) \(time)"
        case .date: return date
        case .time: return time
        case .uuid: return CryptoService.generateUUID()
        case .text(let text): return text
        }
    }
}

enum QuickCommands {

    /// `DEFAULT_QUICK_COMMANDS` in `src/lib/types/models.ts`. The web's own list can
    /// be edited, but that lives in its settings and doesn't reach other clients.
    static let all: [QuickCommand] = [
        QuickCommand(trigger: "/now", kind: .dateTime),
        QuickCommand(trigger: "/date", kind: .date),
        QuickCommand(trigger: "/time", kind: .time),
        QuickCommand(trigger: "/uuid", kind: .uuid),
        QuickCommand(trigger: "/hr", kind: .text("---")),
    ]

    /// How much text before the cursor `match` needs to see: the longest trigger,
    /// and what comes before it.
    static let lookBehind = (all.map(\.trigger.utf16.count).max() ?? 0) + 1

    /// Whether this finishes typing a command, as on the web.
    static func endsCommand(_ typed: String) -> Bool {
        typed == " " || typed == "\n" || typed == "\r\n" || typed == "\r"
    }

    /// The command that `text` ends with. It has to start a line or follow a
    /// space, so that `date/time` stays as it is.
    static func match(before text: String) -> QuickCommand? {
        guard let slash = text.lastIndex(of: "/") else { return nil }
        if slash > text.startIndex, !text[text.index(before: slash)].isWhitespace { return nil }
        let typed = text[slash...].lowercased()
        return all.first { $0.trigger == typed }
    }
}
