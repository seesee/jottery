import SwiftUI
import WebKit

// MARK: - WebView Editor Base

/// Shared UIViewRepresentable for loading self-contained HTML bundles into WKWebView
/// and communicating via the bridge message handler protocol.
///
/// Used by WebCalcEditorView and WebOutlinerEditorView for interactive editing modes.
struct WebEditorView: UIViewRepresentable {
    let bundleName: String
    @Binding var content: String

    @Environment(\.colorScheme) private var colorScheme

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    func makeUIView(context: Context) -> WKWebView {
        let contentController = WKUserContentController()
        contentController.add(context.coordinator, name: "bridge")

        let config = WKWebViewConfiguration()
        config.userContentController = contentController

        let webView = WKWebView(frame: .zero, configuration: config)
        webView.isOpaque = false
        webView.backgroundColor = .clear
        webView.scrollView.backgroundColor = .clear
        webView.navigationDelegate = context.coordinator

        context.coordinator.webView = webView

        // Load the bundle HTML from app resources
        if let url = Bundle.main.url(forResource: bundleName, withExtension: "html") {
            webView.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
        }

        return webView
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

        // Update content if changed externally (not from JS callback)
        if !coordinator.isUpdatingFromJS && content != coordinator.lastSentContent {
            coordinator.lastSentContent = content
            if coordinator.isReady {
                let escaped = escapeForJS(content)
                callJS(webView, "bridge.setContent(\(escaped), \(isDark))")
            }
        }
    }

    // MARK: - Coordinator

    class Coordinator: NSObject, WKScriptMessageHandler, WKNavigationDelegate {
        var parent: WebEditorView
        weak var webView: WKWebView?
        var isReady = false
        var isUpdatingFromJS = false
        var currentIsDark: Bool?
        var lastSentContent: String?

        init(parent: WebEditorView) {
            self.parent = parent
        }

        func userContentController(
            _ userContentController: WKUserContentController,
            didReceive message: WKScriptMessage
        ) {
            guard let body = message.body as? [String: Any],
                  let type = body["type"] as? String else { return }

            switch type {
            case "ready":
                isReady = true
                // Send initial content now that JS is ready
                let isDark = parent.colorScheme == .dark
                currentIsDark = isDark
                lastSentContent = parent.content
                let escaped = escapeForJS(parent.content)
                if let webView {
                    callJS(webView, "bridge.setContent(\(escaped), \(isDark))")
                }

            case "contentChanged":
                guard let content = body["content"] as? String else { return }
                isUpdatingFromJS = true
                lastSentContent = content
                parent.content = content
                isUpdatingFromJS = false

            default:
                break
            }
        }

        // Prevent navigation away from the loaded bundle
        func webView(
            _ webView: WKWebView,
            decidePolicyFor navigationAction: WKNavigationAction,
            decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void
        ) {
            if navigationAction.navigationType == .linkActivated {
                decisionHandler(.cancel)
            } else {
                decisionHandler(.allow)
            }
        }
    }
}
