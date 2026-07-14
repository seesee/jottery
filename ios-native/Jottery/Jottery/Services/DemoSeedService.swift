import Foundation

#if DEBUG
/// Configuration parsed from launch arguments for demo screenshot seeding.
/// See docs/superpowers/specs/2026-07-14-appstore-screenshots-design.md.
struct DemoSeedConfig: Equatable {
    enum Screen: Equatable {
        case list
        case note(titleContains: String)
        case search(query: String)
        case sync
        case lock
    }

    var notesPath: String
    var theme: String
    var screen: Screen

    static func parse(arguments: [String], environment: [String: String]) -> DemoSeedConfig? {
        guard arguments.contains("-demo-seed"),
              let path = environment["DEMO_NOTES_PATH"], !path.isEmpty else {
            return nil
        }

        var theme = "dark"
        if let i = arguments.firstIndex(of: "-demo-theme"), i + 1 < arguments.count {
            theme = arguments[i + 1]
        }

        var screen: Screen = .list
        if let i = arguments.firstIndex(of: "-demo-screen"), i + 1 < arguments.count {
            let raw = arguments[i + 1]
            switch raw {
            case "list": screen = .list
            case "sync": screen = .sync
            case "lock": screen = .lock
            default:
                if raw.hasPrefix("note:") {
                    screen = .note(titleContains: String(raw.dropFirst("note:".count)))
                } else if raw.hasPrefix("search:") {
                    screen = .search(query: String(raw.dropFirst("search:".count)))
                }
            }
        }

        return DemoSeedConfig(notesPath: path, theme: theme, screen: screen)
    }
}
#endif
