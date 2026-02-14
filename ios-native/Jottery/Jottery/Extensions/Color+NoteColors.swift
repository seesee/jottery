import SwiftUI

/// 8-colour palette matching the web UI's `DEFAULT_COLOR_PALETTE`.
extension Color {
    struct NoteColors {
        let light: Color
        let dark: Color
    }

    static let noteColorPalette: [String: NoteColors] = [
        "red":    NoteColors(light: Color(hex: 0xFFE5E5), dark: Color(hex: 0x5C1A1A)),
        "orange": NoteColors(light: Color(hex: 0xFFF0E0), dark: Color(hex: 0x5C3A1A)),
        "yellow": NoteColors(light: Color(hex: 0xFFFACD), dark: Color(hex: 0x5C5520)),
        "green":  NoteColors(light: Color(hex: 0xE5F5E5), dark: Color(hex: 0x1A4D1A)),
        "blue":   NoteColors(light: Color(hex: 0xE5F0FF), dark: Color(hex: 0x1A3A5C)),
        "purple": NoteColors(light: Color(hex: 0xF0E5FF), dark: Color(hex: 0x3A1A5C)),
        "pink":   NoteColors(light: Color(hex: 0xFFE5F0), dark: Color(hex: 0x5C1A3A)),
        "gray":   NoteColors(light: Color(hex: 0xF0F0F0), dark: Color(hex: 0x2A2A2A)),
    ]

    static let noteColorNames = ["red", "orange", "yellow", "green", "blue", "purple", "pink", "gray"]

    /// Get the note color for the current colour scheme.
    static func noteColor(named name: String, scheme: ColorScheme) -> Color? {
        guard let colors = noteColorPalette[name] else { return nil }
        return scheme == .dark ? colors.dark : colors.light
    }

    init(hex: UInt, opacity: Double = 1.0) {
        self.init(
            red: Double((hex >> 16) & 0xFF) / 255.0,
            green: Double((hex >> 8) & 0xFF) / 255.0,
            blue: Double(hex & 0xFF) / 255.0,
            opacity: opacity
        )
    }
}
