import CryptoKit
import ImageIO
import SwiftUI
import UniformTypeIdentifiers
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

// MARK: - Attachments in a Preview

/// Getting a note's images into a preview, without running out of memory. The
/// web content process is killed if it takes too much, which leaves a blank page.
enum PreviewAttachments {

    /// The longest side of an image in a preview. More than a phone can show.
    static let maxImagePixels = 2048
    /// Images smaller than this go through as they are.
    static let scaleAboveBytes = 1_000_000

    /// The attachment a note's reference names: its id or data field, in full,
    /// with or without the hyphens, as on the web.
    static func find(_ reference: String, in attachments: [AttachmentRef]) -> AttachmentRef? {
        guard !reference.isEmpty else { return nil }
        return attachments.first { ref in
            ref.id == reference
                || ref.data == reference
                || reference.hasPrefix(ref.id.replacingOccurrences(of: "-", with: ""))
                || reference.hasPrefix(ref.data.replacingOccurrences(of: "-", with: ""))
        }
    }

    /// A data URL for a decrypted image, scaled down if it's large.
    static func dataURL(for data: Data, mimeType: String) -> String {
        if mimeType != "image/svg+xml", data.count > scaleAboveBytes, let scaled = scaledDown(data) {
            return "data:\(scaled.mimeType);base64,\(scaled.data.base64EncodedString())"
        }
        let type = SyncValidation.isMimeType(mimeType) ? mimeType : "application/octet-stream"
        return "data:\(type);base64,\(data.base64EncodedString())"
    }

    private static func scaledDown(_ data: Data) -> (data: Data, mimeType: String)? {
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: maxImagePixels,
        ]
        guard let source = CGImageSourceCreateWithData(data as CFData, nil),
              let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else {
            return nil
        }
        let hasAlpha = ![.none, .noneSkipFirst, .noneSkipLast].contains(image.alphaInfo)
        let type = hasAlpha ? UTType.png : UTType.jpeg
        let output = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(output, type.identifier as CFString, 1, nil) else {
            return nil
        }
        CGImageDestinationAddImage(destination, image, [kCGImageDestinationLossyCompressionQuality: 0.8] as CFDictionary)
        guard CGImageDestinationFinalize(destination), let mimeType = type.preferredMIMEType else { return nil }
        return (output as Data, mimeType)
    }
}

/// Decrypts a note's images for a web view, one at a time, so that a note with
/// many photos never holds more than one in memory here.
@MainActor
final class PreviewAttachmentLoader {
    var attachments: [AttachmentRef] = []
    var repo: AttachmentRepository?
    var key: SymmetricKey?
    private var requested = Set<String>()
    private var lastLoad: Task<Void, Never>?

    /// Forget what's been sent, for a page that has started again.
    func reset() {
        requested = []
    }

    func load(_ reference: String, into webView: WKWebView?) {
        guard let repo, let key, requested.insert(reference).inserted,
              let ref = PreviewAttachments.find(reference, in: attachments),
              ref.mimeType.hasPrefix("image/") else { return }

        // Capture plain, Sendable values up front so the fetch + decrypt can run
        // inside `Task.detached`, off the main thread.
        let blobId = ref.data
        let mimeType = ref.mimeType
        let previous = lastLoad
        lastLoad = Task { [weak webView] in
            await previous?.value
            do {
                let dataUrl = try await Task.detached(priority: .userInitiated) {
                    // A blob that's missing or isn't text fails quietly.
                    guard let blobData = try repo.getBlob(id: blobId),
                          let blobString = String(data: blobData, encoding: .utf8) else { return String?.none }
                    let encrypted = try CryptoService.parseEncryptedJSON(blobString)
                    let decryptedData = try CryptoService.decrypt(encrypted, key: key)
                    return PreviewAttachments.dataURL(for: decryptedData, mimeType: mimeType)
                }.value
                if let dataUrl, let webView {
                    callJS(webView, "bridge.resolveAttachment(\(escapeForJS(reference)), \(escapeForJS(dataUrl)))")
                }
            } catch {
                Log.debug("[MarkdownPreview] Failed to resolve attachment \(reference): \(error)")
            }
        }
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
    /// Note ids and their titles, for the note links in this content.
    var noteTitles: [String: String] = [:]
    /// A note link was tapped.
    var onOpenNote: ((String) -> Void)?

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

        // Attachment context, before any content that needs it
        coordinator.parent = self
        coordinator.loader.attachments = attachments
        coordinator.loader.repo = attachmentRepo
        coordinator.loader.key = encryptionKey

        // Update content if changed, or if what its links lead to has
        if content != coordinator.lastSentContent || noteTitles != coordinator.sentNoteTitles {
            coordinator.lastSentContent = content
            coordinator.parentContent = content
            if coordinator.isReady {
                coordinator.sendContent(to: webView, isDark: isDark)
            }
        }
    }

    // MARK: - Coordinator

    @MainActor
    class Coordinator: NSObject, WKScriptMessageHandler, WKNavigationDelegate {
        var parent: MarkdownPreviewView
        weak var webView: WKWebView?
        var isReady = false
        var currentIsDark: Bool?
        var currentFontSize: CGFloat = 0
        var lastSentContent: String?
        var sentNoteTitles: [String: String]?
        var parentContent: String
        let loader = PreviewAttachmentLoader()
        /// How many times the web content process has died on this preview.
        private var restarts = 0

        init(parent: MarkdownPreviewView) {
            self.parent = parent
            self.parentContent = parent.content
            loader.attachments = parent.attachments
            loader.repo = parent.attachmentRepo
            loader.key = parent.encryptionKey
        }

        func sendContent(to webView: WKWebView, isDark: Bool) {
            if parent.noteTitles != sentNoteTitles {
                sentNoteTitles = parent.noteTitles
                let titles = (try? JSONEncoder().encode(parent.noteTitles)).map { String(decoding: $0, as: UTF8.self) }
                callJS(webView, "bridge.setNoteTitles(\(titles ?? "{}"))")
            }
            loader.reset()
            callJS(webView, "bridge.setContent(\(escapeForJS(parentContent)), \(isDark))")
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
                sentNoteTitles = nil
                if let webView {
                    callJS(webView, "bridge.setFontSize(\(parent.fontSize))")
                    sendContent(to: webView, isDark: isDark)
                }

            case "requestAttachment":
                guard let id = body["id"] as? String else { return }
                loader.load(id, into: webView)

            case "openNote":
                guard let id = body["id"] as? String, SyncValidation.isId(id) else { return }
                parent.onOpenNote?(id)

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

        /// The web content process went away, usually for using too much
        /// memory, which leaves the page blank. Bring it back, and `ready` sends
        /// the content again. A note that kills it every time is left alone.
        func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
            isReady = false
            restarts += 1
            guard restarts <= 2 else { return }
            webView.reload()
        }
    }
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
