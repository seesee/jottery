import UIKit
import UniformTypeIdentifiers

/// Receives shared content from the system share sheet and stages it in
/// the app-group container. The main app converts staged items into
/// encrypted notes on its next unlocked run (`AppState.importSharedInboxItems`).
final class ShareViewController: UIViewController {

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .clear

        // Minimal confirmation chrome — the work is quick, so a small
        // translucent card is shown while providers load.
        let card = UIVisualEffectView(effect: UIBlurEffect(style: .systemMaterial))
        card.layer.cornerRadius = 16
        card.clipsToBounds = true
        card.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(card)

        let label = UILabel()
        label.text = String(localized: "share.savingToJottery", defaultValue: "Saving to Jottery…")
        label.font = .preferredFont(forTextStyle: .headline)
        label.translatesAutoresizingMaskIntoConstraints = false
        card.contentView.addSubview(label)

        NSLayoutConstraint.activate([
            card.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            card.centerYAnchor.constraint(equalTo: view.centerYAnchor),
            label.topAnchor.constraint(equalTo: card.contentView.topAnchor, constant: 20),
            label.bottomAnchor.constraint(equalTo: card.contentView.bottomAnchor, constant: -20),
            label.leadingAnchor.constraint(equalTo: card.contentView.leadingAnchor, constant: 24),
            label.trailingAnchor.constraint(equalTo: card.contentView.trailingAnchor, constant: -24),
        ])

        Task { await ingestAndComplete() }
    }

    private func ingestAndComplete() async {
        var text: String?
        var urls: [String] = []
        var files: [(data: Data, filename: String, mimeType: String)] = []

        let providers = (extensionContext?.inputItems ?? [])
            .compactMap { $0 as? NSExtensionItem }
            .flatMap { $0.attachments ?? [] }

        for provider in providers {
            if provider.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
                if let file = await loadFile(from: provider, type: .image, fallbackName: "shared-image") {
                    files.append(file)
                }
            } else if provider.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
                if let file = await loadFile(from: provider, type: .data, fallbackName: "shared-file") {
                    files.append(file)
                }
            } else if provider.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
                if let url = await loadURL(from: provider) {
                    urls.append(url.absoluteString)
                }
            } else if provider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
                if let string = await loadText(from: provider) {
                    text = [text, string].compactMap { $0 }.joined(separator: "\n")
                }
            }
        }

        if text != nil || !urls.isEmpty || !files.isEmpty {
            try? SharedInboxStore.write(text: text, urls: urls, files: files)
        }
        extensionContext?.completeRequest(returningItems: nil)
    }

    /// Load a provider as an on-disk file representation and read it.
    private func loadFile(
        from provider: NSItemProvider, type: UTType, fallbackName: String
    ) async -> (data: Data, filename: String, mimeType: String)? {
        await withCheckedContinuation { continuation in
            provider.loadFileRepresentation(forTypeIdentifier: type.identifier) { url, _ in
                // The temporary file is only valid inside this closure —
                // read it before returning.
                guard let url, let data = try? Data(contentsOf: url) else {
                    continuation.resume(returning: nil)
                    return
                }
                let filename = url.lastPathComponent.isEmpty ? fallbackName : url.lastPathComponent
                let mimeType = UTType(filenameExtension: url.pathExtension)?.preferredMIMEType
                    ?? "application/octet-stream"
                continuation.resume(returning: (data, filename, mimeType))
            }
        }
    }

    /// Load a provider's item as a URL.
    private func loadURL(from provider: NSItemProvider) async -> URL? {
        await withCheckedContinuation { continuation in
            _ = provider.loadObject(ofClass: URL.self) { url, _ in
                continuation.resume(returning: url)
            }
        }
    }

    /// Load a provider's item as plain text.
    private func loadText(from provider: NSItemProvider) async -> String? {
        await withCheckedContinuation { continuation in
            _ = provider.loadObject(ofClass: String.self) { string, _ in
                continuation.resume(returning: string)
            }
        }
    }
}
