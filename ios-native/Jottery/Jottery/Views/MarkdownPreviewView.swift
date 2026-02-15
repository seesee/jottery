import SwiftUI
import WebKit

// MARK: - Markdown Preview (WKWebView)

/// Renders markdown content as HTML in a WKWebView with proper block-level formatting.
struct MarkdownPreviewView: UIViewRepresentable {
    let content: String
    @Environment(\.colorScheme) private var colorScheme

    func makeUIView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        let webView = WKWebView(frame: .zero, configuration: config)
        webView.isOpaque = false
        webView.backgroundColor = .clear
        webView.scrollView.backgroundColor = .clear
        return webView
    }

    func updateUIView(_ webView: WKWebView, context: Context) {
        let html = Self.buildHTML(markdown: content, isDark: colorScheme == .dark)
        webView.loadHTMLString(html, baseURL: nil)
    }

    // MARK: - HTML Generation

    static func buildHTML(markdown: String, isDark: Bool) -> String {
        let body = MarkdownToHTML.convert(markdown)
        let bg = isDark ? "#1c1c1e" : "#ffffff"
        let fg = isDark ? "#e5e5e7" : "#1c1c1e"
        let codeBg = isDark ? "#2c2c2e" : "#f2f2f7"
        let codeFg = isDark ? "#e5e5e7" : "#1c1c1e"
        let linkColor = isDark ? "#64d2ff" : "#007aff"
        let hrColor = isDark ? "#48484a" : "#d1d1d6"
        let blockquoteBorder = isDark ? "#636366" : "#c7c7cc"
        let blockquoteBg = isDark ? "#2c2c2e" : "#f9f9f9"
        let tableBorder = isDark ? "#48484a" : "#d1d1d6"
        let tableStripeBg = isDark ? "#2c2c2e" : "#f8f8f8"

        return """
        <!DOCTYPE html>
        <html>
        <head>
        <meta charset="utf-8">
        <meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no">
        <style>
            * { box-sizing: border-box; }
            body {
                font-family: -apple-system, BlinkMacSystemFont, sans-serif;
                font-size: 16px;
                line-height: 1.6;
                color: \(fg);
                background-color: \(bg);
                padding: 16px;
                margin: 0;
                word-wrap: break-word;
                overflow-wrap: break-word;
                -webkit-text-size-adjust: 100%;
            }
            h1, h2, h3, h4, h5, h6 {
                margin-top: 1.2em;
                margin-bottom: 0.4em;
                font-weight: 600;
                line-height: 1.3;
            }
            h1 { font-size: 1.8em; border-bottom: 1px solid \(hrColor); padding-bottom: 0.3em; }
            h2 { font-size: 1.5em; border-bottom: 1px solid \(hrColor); padding-bottom: 0.3em; }
            h3 { font-size: 1.25em; }
            h4 { font-size: 1.1em; }
            h5 { font-size: 1em; }
            h6 { font-size: 0.9em; opacity: 0.8; }
            p { margin: 0.6em 0; }
            a { color: \(linkColor); text-decoration: none; }
            a:hover { text-decoration: underline; }
            code {
                font-family: 'SF Mono', Menlo, monospace;
                font-size: 0.88em;
                background: \(codeBg);
                color: \(codeFg);
                padding: 0.15em 0.35em;
                border-radius: 4px;
            }
            pre {
                background: \(codeBg);
                color: \(codeFg);
                padding: 12px 16px;
                border-radius: 8px;
                overflow-x: auto;
                -webkit-overflow-scrolling: touch;
                margin: 0.8em 0;
                line-height: 1.45;
            }
            pre code {
                background: none;
                padding: 0;
                border-radius: 0;
                font-size: 0.85em;
            }
            blockquote {
                margin: 0.8em 0;
                padding: 0.4em 1em;
                border-left: 4px solid \(blockquoteBorder);
                background: \(blockquoteBg);
                border-radius: 0 4px 4px 0;
            }
            blockquote p { margin: 0.3em 0; }
            ul, ol { padding-left: 1.5em; margin: 0.6em 0; }
            li { margin: 0.2em 0; }
            li > ul, li > ol { margin: 0.1em 0; }
            hr {
                border: none;
                border-top: 1px solid \(hrColor);
                margin: 1.5em 0;
            }
            table {
                border-collapse: collapse;
                width: 100%;
                margin: 0.8em 0;
                font-size: 0.95em;
            }
            th, td {
                border: 1px solid \(tableBorder);
                padding: 8px 12px;
                text-align: left;
            }
            th {
                font-weight: 600;
                background: \(codeBg);
            }
            tr:nth-child(even) { background: \(tableStripeBg); }
            img { max-width: 100%; height: auto; border-radius: 4px; }
            del { opacity: 0.6; }
            input[type="checkbox"] { margin-right: 0.4em; }
        </style>
        </head>
        <body>\(body)</body>
        </html>
        """
    }
}

// MARK: - Markdown to HTML Converter

enum MarkdownToHTML {

    static func convert(_ markdown: String) -> String {
        let lines = markdown.components(separatedBy: "\n")
        var html = ""
        var i = 0
        var paragraphLines: [String] = []

        // Fenced code block state
        var inCodeBlock = false
        var codeBlockContent = ""
        var codeBlockLanguage = ""
        var codeFence = ""

        func flushParagraph() {
            guard !paragraphLines.isEmpty else { return }
            let text = paragraphLines.map { processInline($0) }.joined(separator: "<br>")
            html += "<p>\(text)</p>\n"
            paragraphLines = []
        }

        while i < lines.count {
            let line = lines[i]
            let trimmed = line.trimmingCharacters(in: .whitespaces)

            // --- Fenced code blocks ---
            if !inCodeBlock, (trimmed.hasPrefix("```") || trimmed.hasPrefix("~~~")) {
                flushParagraph()
                codeFence = String(trimmed.prefix(3))
                codeBlockLanguage = trimmed.dropFirst(3).trimmingCharacters(in: .whitespaces)
                inCodeBlock = true
                codeBlockContent = ""
                i += 1
                continue
            }

            if inCodeBlock {
                if trimmed.hasPrefix(codeFence) && trimmed.dropFirst(codeFence.count).allSatisfy({ $0 == codeFence.first }) {
                    let langAttr = codeBlockLanguage.isEmpty ? "" : " class=\"language-\(escapeAttr(codeBlockLanguage))\""
                    html += "<pre><code\(langAttr)>\(escapeHTML(codeBlockContent))</code></pre>\n"
                    inCodeBlock = false
                    codeBlockContent = ""
                    codeBlockLanguage = ""
                } else {
                    if !codeBlockContent.isEmpty { codeBlockContent += "\n" }
                    codeBlockContent += line
                }
                i += 1
                continue
            }

            // --- Blank line ---
            if trimmed.isEmpty {
                flushParagraph()
                i += 1
                continue
            }

            // --- ATX headings ---
            if let level = atxHeadingLevel(trimmed) {
                flushParagraph()
                let text = String(trimmed.drop(while: { $0 == "#" }).dropFirst()) // drop "# "
                    .replacingOccurrences(of: #"\s+#+\s*$"#, with: "", options: .regularExpression) // trailing #s
                html += "<h\(level)>\(processInline(text))</h\(level)>\n"
                i += 1
                continue
            }

            // --- Horizontal rule ---
            if isHorizontalRule(trimmed) {
                flushParagraph()
                html += "<hr>\n"
                i += 1
                continue
            }

            // --- Blockquote ---
            if trimmed.hasPrefix(">") {
                flushParagraph()
                var quoteLines: [String] = []
                while i < lines.count {
                    let ql = lines[i]
                    let qt = ql.trimmingCharacters(in: .whitespaces)
                    if qt.hasPrefix(">") {
                        let content = String(qt.dropFirst())
                        quoteLines.append(content.hasPrefix(" ") ? String(content.dropFirst()) : content)
                    } else if qt.isEmpty {
                        break
                    } else {
                        // Lazy continuation
                        quoteLines.append(ql)
                    }
                    i += 1
                }
                let inner = convert(quoteLines.joined(separator: "\n"))
                html += "<blockquote>\(inner)</blockquote>\n"
                continue
            }

            // --- Unordered list ---
            if isUnorderedListItem(line) {
                flushParagraph()
                i = parseUnorderedList(lines: lines, startIndex: i, html: &html)
                continue
            }

            // --- Ordered list ---
            if isOrderedListItem(line) {
                flushParagraph()
                i = parseOrderedList(lines: lines, startIndex: i, html: &html)
                continue
            }

            // --- Table ---
            if trimmed.contains("|"), i + 1 < lines.count, isTableSeparator(lines[i + 1].trimmingCharacters(in: .whitespaces)) {
                flushParagraph()
                i = parseTable(lines: lines, startIndex: i, html: &html)
                continue
            }

            // --- Regular text → paragraph ---
            paragraphLines.append(line)
            i += 1
        }

        // Flush remaining state
        if inCodeBlock {
            let langAttr = codeBlockLanguage.isEmpty ? "" : " class=\"language-\(escapeAttr(codeBlockLanguage))\""
            html += "<pre><code\(langAttr)>\(escapeHTML(codeBlockContent))</code></pre>\n"
        }
        flushParagraph()

        return html
    }

    // MARK: - Block Detection

    private static func atxHeadingLevel(_ line: String) -> Int? {
        var count = 0
        for ch in line {
            if ch == "#" { count += 1 } else { break }
        }
        guard count >= 1 && count <= 6 else { return nil }
        // Must be followed by a space or end of line
        let rest = line.dropFirst(count)
        guard rest.isEmpty || rest.first == " " else { return nil }
        return count
    }

    private static func isHorizontalRule(_ line: String) -> Bool {
        let stripped = line.replacingOccurrences(of: " ", with: "")
        guard stripped.count >= 3 else { return false }
        let ch = stripped.first!
        guard ch == "-" || ch == "*" || ch == "_" else { return false }
        return stripped.allSatisfy({ $0 == ch })
    }

    private static func isUnorderedListItem(_ line: String) -> Bool {
        let pattern = #"^(\s*)[*+\-]\s"#
        return line.range(of: pattern, options: .regularExpression) != nil
    }

    private static func isOrderedListItem(_ line: String) -> Bool {
        let pattern = #"^(\s*)\d+\.\s"#
        return line.range(of: pattern, options: .regularExpression) != nil
    }

    private static func isTableSeparator(_ line: String) -> Bool {
        let stripped = line.trimmingCharacters(in: .whitespaces)
        guard stripped.contains("-"), stripped.contains("|") else { return false }
        // Should look like |---|---|  or ---|---  or :---:|:---:
        let cells = stripped.split(separator: "|").map { $0.trimmingCharacters(in: .whitespaces) }
        return cells.allSatisfy { cell in
            let c = cell.replacingOccurrences(of: ":", with: "").replacingOccurrences(of: "-", with: "").trimmingCharacters(in: .whitespaces)
            return c.isEmpty
        }
    }

    private static func listItemIndent(_ line: String) -> Int {
        var count = 0
        for ch in line {
            if ch == " " { count += 1 }
            else if ch == "\t" { count += 4 }
            else { break }
        }
        return count
    }

    // MARK: - List Parsing

    private static func parseUnorderedList(lines: [String], startIndex: Int, html: inout String) -> Int {
        var i = startIndex
        let baseIndent = listItemIndent(lines[i])
        html += "<ul>\n"

        while i < lines.count {
            let line = lines[i]
            let trimmed = line.trimmingCharacters(in: .whitespaces)

            if trimmed.isEmpty {
                // Check if next line is still part of the list
                if i + 1 < lines.count && (isUnorderedListItem(lines[i + 1]) || listItemIndent(lines[i + 1]) > baseIndent) {
                    i += 1
                    continue
                }
                break
            }

            let indent = listItemIndent(line)

            if indent > baseIndent && (isUnorderedListItem(line) || isOrderedListItem(line)) {
                // Nested list — recurse
                if isOrderedListItem(line) {
                    i = parseOrderedList(lines: lines, startIndex: i, html: &html)
                } else {
                    i = parseUnorderedList(lines: lines, startIndex: i, html: &html)
                }
                html += "</li>\n"
                continue
            }

            if indent < baseIndent || (!isUnorderedListItem(line) && indent <= baseIndent) {
                break
            }

            // Parse the list item content
            let content: String
            if let range = trimmed.range(of: #"^[*+\-]\s"#, options: .regularExpression) {
                content = String(trimmed[range.upperBound...])
            } else {
                content = trimmed
            }

            // Check for task list items
            if content.hasPrefix("[ ] ") {
                html += "<li><input type=\"checkbox\" disabled> \(processInline(String(content.dropFirst(4))))"
            } else if content.hasPrefix("[x] ") || content.hasPrefix("[X] ") {
                html += "<li><input type=\"checkbox\" checked disabled> \(processInline(String(content.dropFirst(4))))"
            } else {
                html += "<li>\(processInline(content))"
            }

            // Check if next line is a nested list
            if i + 1 < lines.count {
                let nextLine = lines[i + 1]
                let nextIndent = listItemIndent(nextLine)
                let nextTrimmed = nextLine.trimmingCharacters(in: .whitespaces)
                if nextIndent > baseIndent && (isUnorderedListItem(nextLine) || isOrderedListItem(nextLine) || !nextTrimmed.isEmpty) {
                    if isUnorderedListItem(nextLine) || isOrderedListItem(nextLine) {
                        html += "\n"
                        i += 1
                        continue
                    }
                }
            }

            html += "</li>\n"
            i += 1
        }

        html += "</ul>\n"
        return i
    }

    private static func parseOrderedList(lines: [String], startIndex: Int, html: inout String) -> Int {
        var i = startIndex
        let baseIndent = listItemIndent(lines[i])
        html += "<ol>\n"

        while i < lines.count {
            let line = lines[i]
            let trimmed = line.trimmingCharacters(in: .whitespaces)

            if trimmed.isEmpty {
                if i + 1 < lines.count && (isOrderedListItem(lines[i + 1]) || listItemIndent(lines[i + 1]) > baseIndent) {
                    i += 1
                    continue
                }
                break
            }

            let indent = listItemIndent(line)

            if indent > baseIndent && (isUnorderedListItem(line) || isOrderedListItem(line)) {
                if isUnorderedListItem(line) {
                    i = parseUnorderedList(lines: lines, startIndex: i, html: &html)
                } else {
                    i = parseOrderedList(lines: lines, startIndex: i, html: &html)
                }
                html += "</li>\n"
                continue
            }

            if indent < baseIndent || (!isOrderedListItem(line) && indent <= baseIndent) {
                break
            }

            let content: String
            if let range = trimmed.range(of: #"^\d+\.\s"#, options: .regularExpression) {
                content = String(trimmed[range.upperBound...])
            } else {
                content = trimmed
            }

            html += "<li>\(processInline(content))"

            if i + 1 < lines.count {
                let nextLine = lines[i + 1]
                let nextIndent = listItemIndent(nextLine)
                if nextIndent > baseIndent && (isUnorderedListItem(nextLine) || isOrderedListItem(nextLine)) {
                    html += "\n"
                    i += 1
                    continue
                }
            }

            html += "</li>\n"
            i += 1
        }

        html += "</ol>\n"
        return i
    }

    // MARK: - Table Parsing

    private static func parseTable(lines: [String], startIndex: Int, html: inout String) -> Int {
        var i = startIndex
        guard i + 1 < lines.count else { return i + 1 }

        let headerLine = lines[i]
        let separatorLine = lines[i + 1]

        // Parse alignment from separator
        let sepCells = parsePipeCells(separatorLine)
        let alignments: [String] = sepCells.map { cell in
            let t = cell.trimmingCharacters(in: .whitespaces)
            let left = t.hasPrefix(":")
            let right = t.hasSuffix(":")
            if left && right { return "center" }
            if right { return "right" }
            return "left"
        }

        // Header
        let headerCells = parsePipeCells(headerLine)
        html += "<table>\n<thead><tr>\n"
        for (j, cell) in headerCells.enumerated() {
            let align = j < alignments.count ? alignments[j] : "left"
            html += "<th style=\"text-align:\(align)\">\(processInline(cell.trimmingCharacters(in: .whitespaces)))</th>\n"
        }
        html += "</tr></thead>\n<tbody>\n"

        // Body rows
        i += 2
        while i < lines.count {
            let line = lines[i].trimmingCharacters(in: .whitespaces)
            guard !line.isEmpty, line.contains("|") else { break }

            let cells = parsePipeCells(lines[i])
            html += "<tr>\n"
            for (j, cell) in cells.enumerated() {
                let align = j < alignments.count ? alignments[j] : "left"
                html += "<td style=\"text-align:\(align)\">\(processInline(cell.trimmingCharacters(in: .whitespaces)))</td>\n"
            }
            // Pad missing cells
            if cells.count < headerCells.count {
                for j in cells.count..<headerCells.count {
                    let align = j < alignments.count ? alignments[j] : "left"
                    html += "<td style=\"text-align:\(align)\"></td>\n"
                }
            }
            html += "</tr>\n"
            i += 1
        }

        html += "</tbody>\n</table>\n"
        return i
    }

    private static func parsePipeCells(_ line: String) -> [String] {
        var s = line.trimmingCharacters(in: .whitespaces)
        if s.hasPrefix("|") { s = String(s.dropFirst()) }
        if s.hasSuffix("|") { s = String(s.dropLast()) }
        return s.components(separatedBy: "|")
    }

    // MARK: - Inline Processing

    static func processInline(_ text: String) -> String {
        var result = escapeHTML(text)

        // Images: ![alt](url) — must come before links
        result = result.replacingOccurrences(
            of: #"!\[([^\]]*)\]\(([^)]+)\)"#,
            with: "<img src=\"$2\" alt=\"$1\">",
            options: .regularExpression
        )

        // Links: [text](url)
        result = result.replacingOccurrences(
            of: #"\[([^\]]+)\]\(([^)]+)\)"#,
            with: "<a href=\"$2\">$1</a>",
            options: .regularExpression
        )

        // Inline code: `code` — process before bold/italic to avoid conflicts
        result = result.replacingOccurrences(
            of: #"`([^`]+)`"#,
            with: "<code>$1</code>",
            options: .regularExpression
        )

        // Bold+italic: ***text*** or ___text___
        result = result.replacingOccurrences(
            of: #"\*\*\*(.+?)\*\*\*"#,
            with: "<strong><em>$1</em></strong>",
            options: .regularExpression
        )
        result = result.replacingOccurrences(
            of: #"___(.+?)___"#,
            with: "<strong><em>$1</em></strong>",
            options: .regularExpression
        )

        // Bold: **text** or __text__
        result = result.replacingOccurrences(
            of: #"\*\*(.+?)\*\*"#,
            with: "<strong>$1</strong>",
            options: .regularExpression
        )
        result = result.replacingOccurrences(
            of: #"__(.+?)__"#,
            with: "<strong>$1</strong>",
            options: .regularExpression
        )

        // Italic: *text* or _text_
        result = result.replacingOccurrences(
            of: #"\*(.+?)\*"#,
            with: "<em>$1</em>",
            options: .regularExpression
        )
        result = result.replacingOccurrences(
            of: #"\b_(.+?)_\b"#,
            with: "<em>$1</em>",
            options: .regularExpression
        )

        // Strikethrough: ~~text~~
        result = result.replacingOccurrences(
            of: #"~~(.+?)~~"#,
            with: "<del>$1</del>",
            options: .regularExpression
        )

        return result
    }

    // MARK: - HTML Escaping

    static func escapeHTML(_ text: String) -> String {
        text.replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
            .replacingOccurrences(of: "\"", with: "&quot;")
    }

    private static func escapeAttr(_ text: String) -> String {
        escapeHTML(text).replacingOccurrences(of: "'", with: "&#39;")
    }
}
