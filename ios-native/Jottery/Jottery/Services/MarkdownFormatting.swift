import Foundation

/// A change to a note's text, in UTF-16 offsets, as the text view counts.
struct TextEdit: Equatable {
    let range: NSRange
    let replacement: String
    /// What should be selected afterwards.
    let selection: NSRange
}

/// What the editor's bar does to a note's markdown. Each one undoes itself when
/// used again on what it did.
enum MarkdownFormat: CaseIterable {
    case bold
    case italic
    case bulletList
    case link

    func edit(of text: String, selection: NSRange) -> TextEdit {
        let text = text as NSString
        switch self {
        case .bold: return Self.toggleEmphasis(width: 2, in: text, selection: selection)
        case .italic: return Self.toggleEmphasis(width: 1, in: text, selection: selection)
        case .bulletList: return Self.toggleBullets(in: text, selection: selection)
        case .link: return Self.link(in: text, selection: selection)
        }
    }

    // MARK: - Bold and Italic

    /// `**` is bold, `*` italic and `***` both, so bold is any run of two or more
    /// and italic any odd run.
    private static func toggleEmphasis(width: Int, in text: NSString, selection: NSRange) -> TextEdit {
        let words = excludingStars(selection, in: text)
        let run = min(starsBefore(words.location, in: text), starsAfter(words.upperBound, in: text))
        let isOn = width == 2 ? run >= 2 : run % 2 == 1
        let selected = text.substring(with: words)

        if isOn {
            let start = words.location - width
            return TextEdit(
                range: NSRange(location: start, length: words.length + width * 2),
                replacement: selected,
                selection: NSRange(location: start, length: words.length)
            )
        }
        let marker = String(repeating: "*", count: width)
        return TextEdit(
            range: words,
            replacement: marker + selected + marker,
            selection: NSRange(location: words.location + width, length: words.length)
        )
    }

    /// `selection` without any stars matched at both of its ends, which count as
    /// around the words whether or not they were selected too.
    private static func excludingStars(_ selection: NSRange, in text: NSString) -> NSRange {
        var range = selection
        while range.length >= 2,
              text.character(at: range.location) == star,
              text.character(at: range.upperBound - 1) == star {
            range = NSRange(location: range.location + 1, length: range.length - 2)
        }
        return range
    }

    private static func starsBefore(_ location: Int, in text: NSString) -> Int {
        var count = 0
        while count < 3, location - count > 0, text.character(at: location - count - 1) == star {
            count += 1
        }
        return count
    }

    private static func starsAfter(_ location: Int, in text: NSString) -> Int {
        var count = 0
        while count < 3, location + count < text.length, text.character(at: location + count) == star {
            count += 1
        }
        return count
    }

    private static let star = ("*" as NSString).character(at: 0)

    // MARK: - Lists

    /// Bullets on every line the selection touches, or off if they all have one.
    private static func toggleBullets(in text: NSString, selection: NSRange) -> TextEdit {
        let block = linesTouched(by: selection, in: text)
        let lines = splitLines(text.substring(with: block))
        let written = lines.filter { !$0.content.allSatisfy(\.isWhitespace) }
        let removing = !written.isEmpty && written.allSatisfy { bullet(of: $0.content) != nil }

        var replacement = ""
        var caretShift = 0
        for line in lines {
            let isBlank = line.content.allSatisfy(\.isWhitespace)
            var content = line.content
            if removing, let found = bullet(of: content) {
                content.removeSubrange(found)
                caretShift = -2
            } else if !removing, bullet(of: content) == nil, !isBlank || written.isEmpty {
                content.insert(contentsOf: "- ", at: firstNonSpace(of: content))
                caretShift = 2
            }
            replacement += content + line.ending
        }

        let result: NSRange
        if selection.length == 0 {
            let location = max(block.location, selection.location + caretShift)
            result = NSRange(location: location, length: 0)
        } else {
            let ending = lines.last?.ending.utf16.count ?? 0
            result = NSRange(location: block.location, length: replacement.utf16.count - ending)
        }
        return TextEdit(range: block, replacement: replacement, selection: result)
    }

    /// The whole lines under `selection`. One that ends where a line begins
    /// doesn't reach that line.
    private static func linesTouched(by selection: NSRange, in text: NSString) -> NSRange {
        var range = selection
        if range.length > 0, isLineEnding(text.character(at: range.upperBound - 1)) {
            range.length -= 1
        }
        return text.lineRange(for: range)
    }

    private static func isLineEnding(_ unit: unichar) -> Bool {
        unit == 0x0A || unit == 0x0D
    }

    private static func splitLines(_ block: String) -> [(content: String, ending: String)] {
        var lines: [(content: String, ending: String)] = []
        var rest = Substring(block)
        while let end = rest.firstIndex(where: \.isNewline) {
            lines.append((String(rest[..<end]), String(rest[end])))
            rest = rest[rest.index(after: end)...]
        }
        if !rest.isEmpty || lines.isEmpty {
            lines.append((String(rest), ""))
        }
        return lines
    }

    private static func firstNonSpace(of line: String) -> String.Index {
        line.firstIndex { !$0.isWhitespace } ?? line.endIndex
    }

    /// Where a line's bullet and the space after it are, if it has one.
    private static func bullet(of line: String) -> Range<String.Index>? {
        let start = firstNonSpace(of: line)
        guard start < line.endIndex, "-*+".contains(line[start]) else { return nil }
        let space = line.index(after: start)
        guard space < line.endIndex, line[space] == " " else { return nil }
        return start..<line.index(after: space)
    }

    // MARK: - Links

    private static let scheme = "https://"

    /// Selected words become the label, with the cursor where the address goes. A
    /// selected address becomes the target, with the cursor where the label goes.
    private static func link(in text: NSString, selection: NSRange) -> TextEdit {
        let selected = text.substring(with: selection)
        let start = selection.location

        if selected.isEmpty || isAddress(selected) {
            let target = selected.isEmpty ? scheme : selected
            return TextEdit(
                range: selection,
                replacement: "[](\(target))",
                selection: NSRange(location: start + 1, length: 0)
            )
        }
        let replacement = "[\(selected)](\(scheme))"
        return TextEdit(
            range: selection,
            replacement: replacement,
            selection: NSRange(location: start + replacement.utf16.count - 1, length: 0)
        )
    }

    private static func isAddress(_ text: String) -> Bool {
        (text.hasPrefix("https://") || text.hasPrefix("http://")) && !text.contains(where: \.isWhitespace)
    }
}
