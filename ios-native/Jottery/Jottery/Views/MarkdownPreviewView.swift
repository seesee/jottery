import CryptoKit
import SwiftUI
import WebKit

// MARK: - Preview Policy

/// What a note's preview may do. A note is untrusted: it can come from the
/// server, or from a pasted or imported file, and this web view has a bridge to
/// native code that can open URLs and read decrypted attachments. The bundle
/// sanitises what it renders and has a content security policy; this is the
/// native half, which holds even if those were ever bypassed.
enum PreviewPolicy {

    /// `UIApplication.open` would otherwise follow any scheme a note names,
    /// including `tel:` and other apps' own.
    static func canOpenExternally(_ url: URL) -> Bool {
        guard let scheme = url.scheme?.lowercased() else { return false }
        return ["http", "https", "mailto"].contains(scheme)
    }

    /// The web view only ever shows the bundle itself.
    static func allowsNavigation(to url: URL?, bundle: URL?, isMainFrame: Bool) -> Bool {
        guard isMainFrame, let url, let bundle, url.isFileURL else { return false }
        return url.standardizedFileURL.path == bundle.standardizedFileURL.path
    }
}

// MARK: - Markdown Preview (WKWebView Bundle)

/// Renders markdown content as HTML using a marked.js + highlight.js bundle in WKWebView.
/// Supports syntax-highlighted code blocks, GFM tables, task lists, and inline attachment resolution.
struct MarkdownPreviewView: UIViewRepresentable {
    let content: String
    var fontSize: CGFloat = EditorTheme.defaultFontSize
    var attachments: [AttachmentRef] = []
    var attachmentRepo: AttachmentRepository?
    var encryptionKey: SymmetricKey?

    @Environment(\.colorScheme) private var colorScheme

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    func makeUIView(context: Context) -> WKWebView {
        let contentController = WKUserContentController()
        contentController.add(context.coordinator, name: "bridge")

        let config = WKWebViewConfiguration()
        config.userContentController = contentController
        // Nothing a note does may outlive the preview, or reach another note's.
        config.websiteDataStore = .nonPersistent()

        let webView = WKWebView(frame: .zero, configuration: config)
        webView.isOpaque = false
        webView.backgroundColor = .clear
        webView.scrollView.backgroundColor = .clear
        webView.navigationDelegate = context.coordinator

        context.coordinator.webView = webView

        // Load the preview bundle HTML from app resources. It's a single file,
        // so it needs to read nothing else from the app.
        if let url = Self.bundleURL {
            webView.loadFileURL(url, allowingReadAccessTo: url)
        }

        return webView
    }

    static var bundleURL: URL? {
        Bundle.main.url(forResource: "preview", withExtension: "html")
    }

    func updateUIView(_ webView: WKWebView, context: Context) {
        let coordinator = context.coordinator
        let isDark = colorScheme == .dark

        // Update theme if changed
        if coordinator.currentIsDark != isDark {
            coordinator.currentIsDark = isDark
            if coordinator.isReady {
                callJS(webView, "bridge.setTheme(\(isDark))")
            }
        }

        // Update font size if changed
        if coordinator.currentFontSize != fontSize {
            coordinator.currentFontSize = fontSize
            if coordinator.isReady {
                callJS(webView, "bridge.setFontSize(\(fontSize))")
            }
        }

        // Update content if changed
        if content != coordinator.lastSentContent {
            coordinator.lastSentContent = content
            coordinator.parentContent = content
            if coordinator.isReady {
                let escaped = escapeForJS(content)
                callJS(webView, "bridge.setContent(\(escaped), \(isDark))")
            }
        }

        // Update attachment context
        coordinator.attachments = attachments
        coordinator.attachmentRepo = attachmentRepo
        coordinator.encryptionKey = encryptionKey
    }

    // MARK: - Coordinator

    class Coordinator: NSObject, WKScriptMessageHandler, WKNavigationDelegate {
        var parent: MarkdownPreviewView
        weak var webView: WKWebView?
        var isReady = false
        var currentIsDark: Bool?
        var currentFontSize: CGFloat = 0
        var lastSentContent: String?
        var parentContent: String
        var attachments: [AttachmentRef]
        var attachmentRepo: AttachmentRepository?
        var encryptionKey: SymmetricKey?

        init(parent: MarkdownPreviewView) {
            self.parent = parent
            self.parentContent = parent.content
            self.attachments = parent.attachments
            self.attachmentRepo = parent.attachmentRepo
            self.encryptionKey = parent.encryptionKey
        }

        func userContentController(
            _ userContentController: WKUserContentController,
            didReceive message: WKScriptMessage
        ) {
            guard message.frameInfo.isMainFrame,
                  let body = message.body as? [String: Any],
                  let type = body["type"] as? String else { return }

            switch type {
            case "ready":
                isReady = true
                let isDark = parent.colorScheme == .dark
                currentIsDark = isDark
                currentFontSize = parent.fontSize
                lastSentContent = parentContent
                if let webView {
                    callJS(webView, "bridge.setFontSize(\(parent.fontSize))")
                    let escaped = escapeForJS(parentContent)
                    callJS(webView, "bridge.setContent(\(escaped), \(isDark))")
                }

            case "requestAttachment":
                guard let id = body["id"] as? String else { return }
                resolveAttachment(id: id)

            case "openLink":
                guard let urlString = body["url"] as? String,
                      let url = URL(string: urlString),
                      PreviewPolicy.canOpenExternally(url) else { return }
                UIApplication.shared.open(url)

            default:
                break
            }
        }

        // Prevent navigation away from the loaded bundle. A link the user taps
        // opens outside the app, if it's one that may.
        func webView(
            _ webView: WKWebView,
            decidePolicyFor navigationAction: WKNavigationAction,
            decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void
        ) {
            let url = navigationAction.request.url
            if navigationAction.navigationType == .linkActivated {
                if let url, PreviewPolicy.canOpenExternally(url) {
                    UIApplication.shared.open(url)
                }
                decisionHandler(.cancel)
            } else if PreviewPolicy.allowsNavigation(
                to: url,
                bundle: MarkdownPreviewView.bundleURL,
                isMainFrame: navigationAction.targetFrame?.isMainFrame ?? false
            ) {
                decisionHandler(.allow)
            } else {
                decisionHandler(.cancel)
            }
        }

        // MARK: - Attachment Resolution

        /// Find the AttachmentRef matching a requested ID (could match id, data, or filename).
        private func findAttachment(id: String) -> AttachmentRef? {
            // An empty id is a prefix of everything.
            guard !id.isEmpty else { return nil }
            return attachments.first { ref in
                ref.id == id ||
                ref.data == id ||
                ref.id.replacingOccurrences(of: "-", with: "").hasPrefix(id) ||
                ref.data.replacingOccurrences(of: "-", with: "").hasPrefix(id)
            }
        }

        private func resolveAttachment(id: String) {
            guard let repo = attachmentRepo, let key = encryptionKey else { return }
            guard let ref = findAttachment(id: id) else { return }

            // Capture plain, Sendable values up front so the fetch + decrypt
            // can run inside `Task.detached`, off the main thread — matches
            // the pattern in `AppState.loadNotes` and
            // `AttachmentListView.decryptToTempFile`.
            let blobId = ref.data
            let mimeType = ref.mimeType

            Task {
                do {
                    let dataUrl = try await Task.detached(priority: .userInitiated) {
                        // Fetch the encrypted blob from the database
                        guard let blobData = try repo.getBlob(id: blobId) else {
                            throw AttachmentResolveError.silent
                        }
                        guard let blobString = String(data: blobData, encoding: .utf8) else {
                            throw AttachmentResolveError.silent
                        }

                        let encrypted = try CryptoService.parseEncryptedJSON(blobString)
                        let decryptedData = try CryptoService.decrypt(encrypted, key: key)

                        let base64 = decryptedData.base64EncodedString()
                        return "data:\(mimeType);base64,\(base64)"
                    }.value

                    await MainActor.run {
                        guard let webView else { return }
                        let escapedId = escapeForJS(id)
                        let escapedUrl = escapeForJS(dataUrl)
                        callJS(webView, "bridge.resolveAttachment(\(escapedId), \(escapedUrl))")
                    }
                } catch AttachmentResolveError.silent {
                    // Matches original behaviour: missing blob / invalid
                    // encoding fails quietly (no log, no JS call).
                } catch {
                    Log.debug("[MarkdownPreview] Failed to resolve attachment \(id): \(error)")
                }
            }
        }
    }
}

/// Signals a deliberately-quiet failure in `resolveAttachment` (missing blob
/// or non-UTF8 blob) so it's distinguishable from a genuine decrypt error,
/// which is logged.
private enum AttachmentResolveError: Error {
    case silent
}

// MARK: - JS Helpers

/// Escape a string for safe embedding in a JavaScript string literal.
/// Returns a JSON-encoded string (including quotes).
func escapeForJS(_ string: String) -> String {
    guard let data = try? JSONSerialization.data(withJSONObject: string, options: .fragmentsAllowed),
          let json = String(data: data, encoding: .utf8) else {
        return "\"\""
    }
    return json
}

/// Evaluate JavaScript in the web view, ignoring the result.
@MainActor func callJS(_ webView: WKWebView, _ js: String) {
    webView.evaluateJavaScript(js) { _, error in
        if let error {
            Log.debug("[WebView] JS error: \(error.localizedDescription)")
        }
    }
}
