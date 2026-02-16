import UIKit
import Runestone

/// Runestone editor theme that adapts to light/dark mode.
///
/// Highlight names come from tree-sitter capture groups (e.g. `keyword`,
/// `string`, `comment`, `function`, `type`, `number`, `property`, `operator`,
/// `punctuation`, `variable.builtin`, `constant.builtin`).
final class EditorTheme: @unchecked Sendable, Theme {

    let isDark: Bool
    let fontSize: CGFloat

    init(isDark: Bool, fontSize: CGFloat = EditorTheme.defaultFontSize) {
        self.isDark = isDark
        self.fontSize = fontSize
    }

    /// System body font size — respects Dynamic Type.
    static var defaultFontSize: CGFloat {
        UIFont.preferredFont(forTextStyle: .body).pointSize
    }

    // MARK: - Typography

    var font: UIFont { .monospacedSystemFont(ofSize: fontSize, weight: .regular) }

    var textColor: UIColor {
        isDark ? UIColor(white: 0.92, alpha: 1) : UIColor(white: 0.10, alpha: 1)
    }

    // MARK: - Gutter

    var gutterBackgroundColor: UIColor {
        isDark ? UIColor(white: 0.12, alpha: 1) : UIColor(white: 0.96, alpha: 1)
    }

    var gutterHairlineColor: UIColor {
        isDark ? UIColor(white: 0.22, alpha: 1) : UIColor(white: 0.85, alpha: 1)
    }

    var gutterHairlineWidth: CGFloat { 0.5 }

    var lineNumberColor: UIColor {
        isDark ? UIColor(white: 0.45, alpha: 1) : UIColor(white: 0.55, alpha: 1)
    }

    var lineNumberFont: UIFont { .monospacedSystemFont(ofSize: max(9, fontSize * 0.8), weight: .regular) }

    // MARK: - Selected Line

    var selectedLineBackgroundColor: UIColor {
        isDark ? UIColor(white: 0.16, alpha: 1) : UIColor(white: 0.94, alpha: 1)
    }

    var selectedLinesLineNumberColor: UIColor {
        isDark ? UIColor(white: 0.70, alpha: 1) : UIColor(white: 0.30, alpha: 1)
    }

    var selectedLinesGutterBackgroundColor: UIColor {
        isDark ? UIColor(white: 0.16, alpha: 1) : UIColor(white: 0.93, alpha: 1)
    }

    // MARK: - Misc

    var invisibleCharactersColor: UIColor {
        isDark ? UIColor(white: 0.30, alpha: 1) : UIColor(white: 0.75, alpha: 1)
    }

    var pageGuideHairlineColor: UIColor {
        isDark ? UIColor(white: 0.22, alpha: 1) : UIColor(white: 0.85, alpha: 1)
    }

    var pageGuideBackgroundColor: UIColor {
        isDark ? UIColor(white: 0.14, alpha: 1) : UIColor(white: 0.96, alpha: 1)
    }

    var markedTextBackgroundColor: UIColor {
        isDark ? UIColor(white: 1, alpha: 0.08) : UIColor(white: 0, alpha: 0.06)
    }

    // MARK: - Syntax Highlighting

    // swiftlint:disable cyclomatic_complexity
    func textColor(for highlightName: String) -> UIColor? {
        if isDark {
            return darkColor(for: highlightName)
        } else {
            return lightColor(for: highlightName)
        }
    }
    // swiftlint:enable cyclomatic_complexity

    func fontTraits(for highlightName: String) -> FontTraits {
        switch highlightName {
        case "keyword", "type": return .bold
        case "text.title": return .bold
        case "text.strong": return .bold
        case "text.emphasis": return .italic
        default: return []
        }
    }

    func font(for highlightName: String) -> UIFont? {
        nil
    }

    func shadow(for highlightName: String) -> NSShadow? {
        nil
    }

    // MARK: - Dark Palette

    private func darkColor(for name: String) -> UIColor? {
        switch name {
        case "keyword":                                     // purple
            return UIColor(red: 0.78, green: 0.56, blue: 0.89, alpha: 1)
        case "string":                                      // green
            return UIColor(red: 0.60, green: 0.80, blue: 0.40, alpha: 1)
        case "comment":                                     // grey
            return UIColor(red: 0.46, green: 0.52, blue: 0.56, alpha: 1)
        case "number", "constant.builtin":                  // orange
            return UIColor(red: 0.85, green: 0.65, blue: 0.35, alpha: 1)
        case "function", "function.method":                 // blue
            return UIColor(red: 0.40, green: 0.65, blue: 0.90, alpha: 1)
        case "type", "constructor":                         // yellow
            return UIColor(red: 0.90, green: 0.75, blue: 0.40, alpha: 1)
        case "property":                                    // cyan
            return UIColor(red: 0.40, green: 0.80, blue: 0.80, alpha: 1)
        case "variable.builtin":                            // red
            return UIColor(red: 0.90, green: 0.45, blue: 0.45, alpha: 1)
        case "operator", "punctuation", "punctuation.bracket", "punctuation.delimiter":
            return UIColor(white: 0.65, alpha: 1)
        case "constant.character", "constant.character.escape":
            return UIColor(red: 0.85, green: 0.55, blue: 0.35, alpha: 1)
        case "tag":                                         // red for HTML tags
            return UIColor(red: 0.90, green: 0.45, blue: 0.45, alpha: 1)
        case "attribute":                                   // orange for HTML attributes
            return UIColor(red: 0.85, green: 0.65, blue: 0.35, alpha: 1)
        case "heading", "title", "text.title":                // bold blue for markdown headings
            return UIColor(red: 0.40, green: 0.65, blue: 0.90, alpha: 1)
        case "emphasis", "text.emphasis":
            return UIColor(white: 0.85, alpha: 1)
        case "strong", "text.strong":
            return UIColor(white: 0.95, alpha: 1)
        case "link", "uri", "text.uri":                     // blue for links
            return UIColor(red: 0.40, green: 0.65, blue: 0.90, alpha: 1)
        case "text.literal":                                // grey-green for code spans / fenced blocks
            return UIColor(red: 0.55, green: 0.75, blue: 0.55, alpha: 1)
        case "text.reference":                              // cyan for link text / labels
            return UIColor(red: 0.40, green: 0.80, blue: 0.80, alpha: 1)
        case "punctuation.special":                         // dimmed for markdown markers (# - * etc.)
            return UIColor(white: 0.50, alpha: 1)
        case "string.escape":                               // orange for escape sequences
            return UIColor(red: 0.85, green: 0.55, blue: 0.35, alpha: 1)
        case "none":                                        // tree-sitter @none — keep default
            return nil
        default:
            return nil
        }
    }

    // MARK: - Light Palette

    private func lightColor(for name: String) -> UIColor? {
        switch name {
        case "keyword":                                     // purple
            return UIColor(red: 0.55, green: 0.20, blue: 0.70, alpha: 1)
        case "string":                                      // green
            return UIColor(red: 0.20, green: 0.55, blue: 0.15, alpha: 1)
        case "comment":                                     // grey
            return UIColor(red: 0.44, green: 0.50, blue: 0.56, alpha: 1)
        case "number", "constant.builtin":                  // orange
            return UIColor(red: 0.75, green: 0.45, blue: 0.10, alpha: 1)
        case "function", "function.method":                 // blue
            return UIColor(red: 0.15, green: 0.40, blue: 0.75, alpha: 1)
        case "type", "constructor":                         // teal
            return UIColor(red: 0.00, green: 0.50, blue: 0.55, alpha: 1)
        case "property":                                    // dark cyan
            return UIColor(red: 0.10, green: 0.50, blue: 0.55, alpha: 1)
        case "variable.builtin":                            // red
            return UIColor(red: 0.75, green: 0.15, blue: 0.15, alpha: 1)
        case "operator", "punctuation", "punctuation.bracket", "punctuation.delimiter":
            return UIColor(white: 0.40, alpha: 1)
        case "constant.character", "constant.character.escape":
            return UIColor(red: 0.75, green: 0.35, blue: 0.10, alpha: 1)
        case "tag":                                         // red for HTML tags
            return UIColor(red: 0.70, green: 0.15, blue: 0.15, alpha: 1)
        case "attribute":                                   // orange for HTML attributes
            return UIColor(red: 0.75, green: 0.45, blue: 0.10, alpha: 1)
        case "heading", "title", "text.title":              // bold blue for markdown headings
            return UIColor(red: 0.15, green: 0.40, blue: 0.75, alpha: 1)
        case "emphasis", "text.emphasis":
            return UIColor(white: 0.25, alpha: 1)
        case "strong", "text.strong":
            return UIColor(white: 0.05, alpha: 1)
        case "link", "uri", "text.uri":                     // blue for links
            return UIColor(red: 0.15, green: 0.40, blue: 0.75, alpha: 1)
        case "text.literal":                                // dark green for code spans / fenced blocks
            return UIColor(red: 0.15, green: 0.50, blue: 0.25, alpha: 1)
        case "text.reference":                              // teal for link text / labels
            return UIColor(red: 0.10, green: 0.50, blue: 0.55, alpha: 1)
        case "punctuation.special":                         // dimmed for markdown markers
            return UIColor(white: 0.55, alpha: 1)
        case "string.escape":                               // orange for escape sequences
            return UIColor(red: 0.75, green: 0.35, blue: 0.10, alpha: 1)
        case "none":
            return nil
        default:
            return nil
        }
    }
}
