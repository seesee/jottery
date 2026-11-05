import CryptoKit
import UIKit
import WebKit

/// Getting one note out of the app, through the system's own share sheet and
/// print controller. The names follow the web's `noteExport.ts`, so a note saved
/// from either client looks the same.
enum NoteSharing {

    /// What the web's Copy Link produces, which every client resolves to the note.
    static func link(to noteId: String) -> String {
        "[](link:\(noteId))"
    }

    private static let extensions = [
        "plain": "txt", "javascript": "js", "typescript": "ts", "python": "py", "markdown": "md",
        "json": "json", "html": "html", "css": "css", "sql": "sql", "bash": "sh", "shell": "sh",
        "perl": "pl", "ruby": "rb", "rust": "rs", "go": "go", "java": "java", "c": "c", "cpp": "cpp",
        "csharp": "cs", "php": "php", "xml": "xml", "yaml": "yaml", "toml": "toml",
    ]

    static func fileExtension(for language: String) -> String {
        extensions[language] ?? "txt"
    }

    /// A file name from the note's first line, or its date if that's empty.
    static func filename(for note: DecryptedNote) -> String {
        var firstLine = (note.content.components(separatedBy: "\n").first ?? "")
            .trimmingCharacters(in: .whitespaces)
        if note.syntaxLanguage == "markdown" {
            firstLine = firstLine.replacingOccurrences(of: #"^#+\s*"#, with: "", options: .regularExpression)
                .replacingOccurrences(of: #"\*\*|__|\*|`"#, with: "", options: .regularExpression)
        }
        let safe = String(firstLine.prefix(50))
            .replacingOccurrences(of: "[^A-Za-z0-9_.-]", with: "_", options: .regularExpression)
            .replacingOccurrences(of: "_{2,}", with: "_", options: .regularExpression)
            .trimmingCharacters(in: CharacterSet(charactersIn: "_"))
        let name = safe.isEmpty ? String(note.createdAt.iso8601.prefix(10)) : safe
        return "\(name).\(fileExtension(for: note.syntaxLanguage))"
    }

    /// The note's text as a file ready to share, removed when the sheet closes.
    static func shareFile(for note: DecryptedNote, files: TemporaryFiles = .shared) throws -> ExportFile {
        ExportFile(
            url: try files.write(Data(note.content.utf8), named: filename(for: note)),
            text: note.content
        )
    }
}

/// Prints a note. Markdown goes through the preview bundle, in a web view nobody
/// sees, so it comes out rendered and with its images. Anything else is printed
/// as the text it is.
@MainActor
final class NotePrinter: NSObject, WKScriptMessageHandler, WKNavigationDelegate {

    /// The one in progress. It has to outlive the print sheet.
    private static var current: NotePrinter?

    private let note: DecryptedNote
    private let noteTitles: [String: String]
    private let loader = PreviewAttachmentLoader()
    private var webView: WKWebView?
    private var onRendered: ((WKWebView?) -> Void)?

    private init(note: DecryptedNote, noteTitles: [String: String], attachmentRepo: AttachmentRepository?, key: SymmetricKey?) {
        self.note = note
        self.noteTitles = noteTitles
        loader.attachments = note.attachments
        loader.repo = attachmentRepo
        loader.key = key
    }

    /// Show the system's print sheet for `note`.
    static func print(
        _ note: DecryptedNote,
        noteTitles: [String: String],
        attachmentRepo: AttachmentRepository?,
        key: SymmetricKey?
    ) {
        let printer = NotePrinter(note: note, noteTitles: noteTitles, attachmentRepo: attachmentRepo, key: key)
        current = printer
        printer.render { webView in
            printer.present(formatter: webView?.viewPrintFormatter() ?? UISimpleTextPrintFormatter(text: note.content))
        }
    }

    /// Whether this note prints from the preview, rather than as text.
    static func printsRendered(_ note: DecryptedNote) -> Bool {
        note.syntaxLanguage == "markdown"
    }

    /// Render `note` in a hidden page, for tests as much as for printing.
    static func render(
        _ note: DecryptedNote,
        noteTitles: [String: String] = [:],
        attachmentRepo: AttachmentRepository? = nil,
        key: SymmetricKey? = nil
    ) async -> (printer: NotePrinter, webView: WKWebView?) {
        let printer = NotePrinter(note: note, noteTitles: noteTitles, attachmentRepo: attachmentRepo, key: key)
        return await withCheckedContinuation { continuation in
            printer.render { continuation.resume(returning: (printer, $0)) }
        }
    }

    private func render(then finished: @escaping (WKWebView?) -> Void) {
        guard Self.printsRendered(note), let bundle = MarkdownPreviewView.bundleURL else {
            finished(nil)
            return
        }
        onRendered = finished

        let contentController = WKUserContentController()
        contentController.add(self, name: "bridge")
        let configuration = WKWebViewConfiguration()
        configuration.userContentController = contentController
        configuration.websiteDataStore = .nonPersistent()

        // A page's size, so the layout is the one that gets printed.
        let webView = WKWebView(frame: CGRect(x: 0, y: 0, width: 612, height: 792), configuration: configuration)
        webView.navigationDelegate = self
        self.webView = webView
        webView.loadFileURL(bundle, allowingReadAccessTo: bundle)
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard message.frameInfo.isMainFrame,
              let body = message.body as? [String: Any],
              let type = body["type"] as? String else { return }

        switch type {
        case "ready":
            guard let webView else { return }
            let titles = (try? JSONEncoder().encode(noteTitles)).map { String(decoding: $0, as: UTF8.self) }
            callJS(webView, "bridge.setNoteTitles(\(titles ?? "{}"))")
            // Paper is white, whatever the app's theme.
            callJS(webView, "bridge.setContent(\(escapeForJS(note.content)), false)")
            Task { await waitForImages(in: webView) }
        case "requestAttachment":
            if let id = body["id"] as? String { loader.load(id, into: webView) }
        default:
            break
        }
    }

    /// Images arrive one at a time. Bounded by attempts, and a page that's short
    /// of an image still beats no page.
    private func waitForImages(in webView: WKWebView) async {
        let pending = "document.querySelectorAll('img[data-attachment-id]:not([src^=\"data:\"])').length"
        for _ in 0..<500 {
            let remaining = try? await webView.evaluateJavaScript(pending) as? Int
            if remaining == 0 { break }
            try? await Task.sleep(for: .milliseconds(20))
        }
        onRendered?(webView)
        onRendered = nil
    }

    /// The print formatter has to come from the same web view, so nothing here
    /// may navigate. The page's own links do nothing on paper.
    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void
    ) {
        let allowed = PreviewPolicy.allowsNavigation(
            to: navigationAction.request.url,
            bundle: MarkdownPreviewView.bundleURL,
            isMainFrame: navigationAction.targetFrame?.isMainFrame ?? false
        )
        decisionHandler(allowed && navigationAction.navigationType != .linkActivated ? .allow : .cancel)
    }

    private func present(formatter: UIPrintFormatter) {
        let info = UIPrintInfo(dictionary: nil)
        info.jobName = note.title
        info.outputType = .general

        let controller = UIPrintInteractionController.shared
        controller.printInfo = info
        controller.printFormatter = formatter
        controller.present(animated: true) { _, _, _ in
            Self.current = nil
        }
    }
}
