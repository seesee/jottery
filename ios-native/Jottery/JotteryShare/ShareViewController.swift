import UIKit
import UniformTypeIdentifiers

/// Receives shared content from the system share sheet and stages it in
/// the app-group container. The main app converts staged items into
/// encrypted notes on its next unlocked run (`AppState.importSharedInboxItems`).
final class ShareViewController: UIViewController {

    private let label = UILabel()
    private let closeButton = UIButton(configuration: .borderedProminent())

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

        label.text = String(localized: "share.savingToJottery", defaultValue: "Saving to Jottery…")
        label.font = .preferredFont(forTextStyle: .headline)
        label.adjustsFontForContentSizeCategory = true
        label.numberOfLines = 0
        label.textAlignment = .center

        closeButton.configuration?.title = String(localized: "share.close", defaultValue: "Close")
        closeButton.isHidden = true
        closeButton.addAction(UIAction { [weak self] _ in self?.finish() }, for: .touchUpInside)

        let stack = UIStackView(arrangedSubviews: [label, closeButton])
        stack.axis = .vertical
        stack.spacing = 12
        stack.alignment = .center
        stack.translatesAutoresizingMaskIntoConstraints = false
        card.contentView.addSubview(stack)

        NSLayoutConstraint.activate([
            card.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            card.centerYAnchor.constraint(equalTo: view.centerYAnchor),
            card.widthAnchor.constraint(lessThanOrEqualTo: view.widthAnchor, constant: -48),
            stack.topAnchor.constraint(equalTo: card.contentView.topAnchor, constant: 20),
            stack.bottomAnchor.constraint(equalTo: card.contentView.bottomAnchor, constant: -20),
            stack.leadingAnchor.constraint(equalTo: card.contentView.leadingAnchor, constant: 24),
            stack.trailingAnchor.constraint(equalTo: card.contentView.trailingAnchor, constant: -24),
        ])

        Task { await ingestAndComplete() }
    }

    private func finish() {
        extensionContext?.completeRequest(returningItems: nil)
    }

    /// Stage what was shared and say how it went. A failure stays on screen, so
    /// nobody walks away thinking something was saved when it wasn't.
    private func ingestAndComplete() async {
        let staging: SharedInboxStore.Staging
        do {
            staging = try SharedInboxStore.begin()
        } catch {
            show(String(localized: "share.failed", defaultValue: "This couldn't be saved to Jottery."))
            return
        }

        var text: String?
        var urls: [String] = []
        var files: [SharedInboxStore.Manifest.File] = []
        var failures = 0

        let providers = (extensionContext?.inputItems ?? [])
            .compactMap { $0 as? NSExtensionItem }
            .flatMap { $0.attachments ?? [] }

        for provider in providers {
            if provider.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
                await stageFile(from: provider, type: .image, fallbackName: "shared-image", in: staging, files: &files, failures: &failures)
            } else if provider.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
                await stageFile(from: provider, type: .data, fallbackName: "shared-file", in: staging, files: &files, failures: &failures)
            } else if provider.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
                if let url = await loadURL(from: provider) {
                    urls.append(url.absoluteString)
                } else {
                    failures += 1
                }
            } else if provider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
                if let string = await loadText(from: provider) {
                    text = [text, string].compactMap { $0 }.joined(separator: "\n")
                } else {
                    failures += 1
                }
            }
        }

        guard text != nil || !urls.isEmpty || !files.isEmpty else {
            staging.discard()
            if failures > 0 {
                show(String(localized: "share.failed", defaultValue: "This couldn't be saved to Jottery."))
            } else {
                finish()
            }
            return
        }

        do {
            try staging.commit(text: text, urls: urls, files: files)
        } catch {
            staging.discard()
            show(String(localized: "share.failed", defaultValue: "This couldn't be saved to Jottery."))
            return
        }

        if failures > 0 {
            show(String(localized: "share.partlySaved", defaultValue: "Some of this couldn't be saved. The rest will be added next time you unlock Jottery."))
        } else {
            // It's unencrypted until the app next runs, which the user should know.
            label.text = String(localized: "share.saved", defaultValue: "Saved. It stays unencrypted on this device until you next unlock Jottery.")
            try? await Task.sleep(for: .seconds(2))
            finish()
        }
    }

    private func show(_ message: String) {
        label.text = message
        closeButton.isHidden = false
    }

    /// Copy a provider's file into the staged item. The file is copied, never
    /// read: extensions get very little memory, and a video would be fatal.
    private func stageFile(
        from provider: NSItemProvider,
        type: UTType,
        fallbackName: String,
        in staging: SharedInboxStore.Staging,
        files: inout [SharedInboxStore.Manifest.File],
        failures: inout Int
    ) async {
        let index = files.count
        let staged: SharedInboxStore.Manifest.File? = await withCheckedContinuation { continuation in
            provider.loadFileRepresentation(forTypeIdentifier: type.identifier) { url, _ in
                // The temporary file is only valid inside this closure.
                guard let url else {
                    continuation.resume(returning: nil)
                    return
                }
                let filename = url.lastPathComponent.isEmpty ? fallbackName : url.lastPathComponent
                let mimeType = UTType(filenameExtension: url.pathExtension)?.preferredMIMEType
                    ?? "application/octet-stream"
                continuation.resume(returning: try? staging.add(
                    copying: url, index: index, filename: filename, mimeType: mimeType
                ))
            }
        }
        if let staged {
            files.append(staged)
        } else {
            failures += 1
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
