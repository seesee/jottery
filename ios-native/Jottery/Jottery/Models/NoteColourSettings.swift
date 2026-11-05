import Foundation

/// What the user has called each colour, and which tags have one. Neither syncs,
/// on any client. A category or tag name says as much as a note does, so this is
/// kept encrypted (`ColourSettingsRepository`) and out of memory while locked.
struct NoteColourSettings: Equatable, Sendable {

    /// The eight colours, in the order they're offered. A note stores one of these.
    static let keys = ["red", "orange", "yellow", "green", "blue", "purple", "pink", "gray"]

    /// Colour key to category name, as typed.
    var categoryNames: [String: String] = [:]
    /// Tag, normalised, to colour key.
    var tagColours: [String: String] = [:]

    /// What to call a colour.
    func displayName(for key: String) -> String {
        let name = categoryNames[key]?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return name.isEmpty ? L.colourName(key) : name
    }

    /// The colour going by `name`. A colour that has been named goes by that
    /// name alone, as on the web. The first wins if two share one.
    func colourKey(forCategory name: String) -> String? {
        let wanted = Self.normalised(name)
        return Self.keys.first { key in
            let current = categoryNames[key].map(Self.normalised) ?? ""
            return (current.isEmpty ? key : current) == wanted
        }
    }

    func colourKey(forTag tag: String) -> String? {
        tagColours[Self.normalised(tag)]
    }

    /// Name a colour, or give it its own name back with a blank one. Stored as
    /// typed, because it's called with every keystroke.
    mutating func setCategoryName(_ name: String, for key: String) {
        guard Self.keys.contains(key) else { return }
        let isBlank = name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        categoryNames[key] = isBlank ? nil : name
    }

    mutating func setColour(_ key: String?, forTag tag: String) {
        guard key.map(Self.keys.contains) ?? true else { return }
        tagColours[Self.normalised(tag)] = key
    }

    private static func normalised(_ text: String) -> String {
        SearchService.normalise(text.trimmingCharacters(in: .whitespacesAndNewlines))
    }
}
