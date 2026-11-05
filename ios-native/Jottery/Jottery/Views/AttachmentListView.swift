import CryptoKit
import QuickLook
import SwiftUI

/// Collapsible attachment list displayed below the editor.
struct AttachmentListView: View {
    let attachments: [AttachmentRef]
    let attachmentRepo: AttachmentRepository
    let encryptionKey: SymmetricKey
    var onDelete: ((String) -> Void)?

    @State private var isExpanded = true
    @State private var previewURL: URL?
    @State private var shareFile: ExportFile?
    @State private var errorMessage: String?
    @State private var attachmentToDelete: AttachmentRef?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Header
            Button {
                withAnimation(.easeInOut(duration: 0.2)) {
                    isExpanded.toggle()
                }
            } label: {
                HStack {
                    Image(systemName: "paperclip")
                        .font(.subheadline)
                    Text(L.attachmentsHeader(attachments.count))
                        .font(.subheadline.weight(.medium))
                    Spacer()
                    Image(systemName: isExpanded ? "chevron.up" : "chevron.down")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .padding(.horizontal)
                .padding(.vertical, 10)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            if isExpanded {
                Divider()
                    .padding(.horizontal)

                ForEach(attachments) { attachment in
                    AttachmentRowView(
                        attachment: attachment,
                        attachmentRepo: attachmentRepo,
                        encryptionKey: encryptionKey,
                        onTap: { decryptAndPreview(attachment) },
                        onShare: { decryptAndShare(attachment) },
                        onCopyMarkdown: { copyMarkdown(attachment) },
                        onDelete: onDelete != nil ? { attachmentToDelete = attachment } : nil
                    )

                    if attachment.id != attachments.last?.id {
                        Divider()
                            .padding(.leading, 68)
                    }
                }
            }

            if let errorMessage {
                Text(errorMessage)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .padding(.horizontal)
                    .padding(.vertical, 4)
            }
        }
        .background(Color(.systemGroupedBackground).opacity(0.5))
        .quickLookPreview($previewURL)
        .onChange(of: previewURL) { closed, _ in
            if let closed { TemporaryFiles.shared.remove(closed) }
        }
        .shareTemporaryFile($shareFile)
        // Removing an attachment takes its data with it, and the button sits
        // right next to Share.
        .confirmationDialog(
            L.attachmentsDeleteTitle(attachmentToDelete?.filename ?? ""),
            isPresented: Binding(
                get: { attachmentToDelete != nil },
                set: { if !$0 { attachmentToDelete = nil } }
            ),
            titleVisibility: .visible,
            presenting: attachmentToDelete
        ) { attachment in
            Button(L.attachmentsDeleteAction, role: .destructive) { onDelete?(attachment.id) }
            Button(L.commonCancel, role: .cancel) {}
        } message: { _ in
            Text(L.attachmentsDeleteMessage)
        }
    }

    // MARK: - Decrypt & Preview

    private func decryptAndPreview(_ attachment: AttachmentRef) {
        errorMessage = nil
        Task { @MainActor in
            if let url = await decryptToTempFile(attachment) {
                previewURL = url
            }
        }
    }

    private func decryptAndShare(_ attachment: AttachmentRef) {
        errorMessage = nil
        Task { @MainActor in
            if let url = await decryptToTempFile(attachment) {
                shareFile = ExportFile(url: url)
            }
        }
    }

    private func copyMarkdown(_ attachment: AttachmentRef) {
        UIPasteboard.general.string = AttachmentMarkdown.reference(to: attachment)
        UIAccessibility.post(notification: .announcement, argument: L.attachmentsMarkdownCopied)
    }

    /// Decrypt an attachment blob and write to a temporary file, off the
    /// main thread. Blob fetch, decrypt, and file write are the expensive
    /// steps, so they all run inside `Task.detached` — only plain,
    /// `Sendable` values (repo, key, filename, blob id) are captured, matching
    /// the pattern in `AppState.loadNotes`. Returns the file URL on success,
    /// nil on failure (setting `errorMessage`, hopped back to MainActor
    /// explicitly since this function itself is not actor-isolated).
    private func decryptToTempFile(_ attachment: AttachmentRef) async -> URL? {
        let repo = attachmentRepo
        let key = encryptionKey
        let filename = attachment.filename
        let blobId = attachment.data

        do {
            return try await Task.detached(priority: .userInitiated) {
                // Fetch the encrypted blob from the database
                guard let blobData = try repo.getBlob(id: blobId) else {
                    throw AttachmentDecryptError.dataNotAvailable
                }

                // The blob is UTF-8 of {"ciphertext":"...","iv":"..."}
                guard let blobString = String(data: blobData, encoding: .utf8) else {
                    throw AttachmentDecryptError.invalidData
                }

                let encrypted = try CryptoService.parseEncryptedJSON(blobString)
                let decryptedData = try CryptoService.decrypt(encrypted, key: key)

                // Write to temp file with the correct filename
                return try TemporaryFiles.shared.write(decryptedData, named: filename)
            }.value
        } catch AttachmentDecryptError.dataNotAvailable {
            await MainActor.run { errorMessage = L.attachmentsDataNotAvailable }
            return nil
        } catch AttachmentDecryptError.invalidData {
            await MainActor.run { errorMessage = L.attachmentsInvalidData }
            return nil
        } catch {
            await MainActor.run { errorMessage = L.attachmentsDecryptFailed }
            return nil
        }
    }
}

/// Errors surfaced by `AttachmentListView.decryptToTempFile` so the specific
/// localized message can be chosen back on MainActor.
private enum AttachmentDecryptError: Error {
    case dataNotAvailable
    case invalidData
}

// MARK: - Attachment Row

private struct AttachmentRowView: View {
    let attachment: AttachmentRef
    let attachmentRepo: AttachmentRepository
    let encryptionKey: SymmetricKey
    let onTap: () -> Void
    let onShare: () -> Void
    let onCopyMarkdown: () -> Void
    var onDelete: (() -> Void)?

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 12) {
                AttachmentThumbnailView(
                    attachment: attachment,
                    attachmentRepo: attachmentRepo,
                    encryptionKey: encryptionKey
                )

                VStack(alignment: .leading, spacing: 2) {
                    Text(attachment.filename)
                        .font(.subheadline)
                        .lineLimit(1)
                        .truncationMode(.middle)

                    Text(formattedSize(attachment.size))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Spacer()

                Button(action: onShare) {
                    Image(systemName: "square.and.arrow.up")
                        .font(.subheadline)
                        .foregroundStyle(.accent)
                        .frame(width: 44, height: 44)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(L.attachmentsShare)

                if let onDelete {
                    Button(role: .destructive, action: onDelete) {
                        Image(systemName: "trash")
                            .font(.subheadline)
                            .foregroundStyle(.red)
                            .frame(width: 44, height: 44)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(L.attachmentsDeleteAction)
                }
            }
            .padding(.horizontal)
            .padding(.vertical, 8)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .contextMenu {
            Button(action: onCopyMarkdown) {
                Label(L.attachmentsCopyMarkdown, systemImage: "doc.on.clipboard")
            }
            Button(action: onShare) {
                Label(L.attachmentsShare, systemImage: "square.and.arrow.up")
            }
            if let onDelete {
                Button(role: .destructive, action: onDelete) {
                    Label(L.attachmentsDeleteAction, systemImage: "trash")
                }
            }
        }
        .accessibilityAction(named: L.attachmentsCopyMarkdown, onCopyMarkdown)
    }

    private func formattedSize(_ bytes: Int) -> String {
        let formatter = ByteCountFormatter()
        formatter.countStyle = .file
        return formatter.string(fromByteCount: Int64(bytes))
    }
}

// MARK: - Thumbnail

/// A small picture of the attachment, or an icon for its type until there is one
/// and for anything that can't be drawn. The row's filename says what it is, so
/// VoiceOver skips this.
private struct AttachmentThumbnailView: View {
    let attachment: AttachmentRef
    let attachmentRepo: AttachmentRepository
    let encryptionKey: SymmetricKey

    @ScaledMetric(relativeTo: .subheadline) private var side: CGFloat = 40
    @State private var image: UIImage?

    var body: some View {
        Group {
            if let image {
                Image(uiImage: image)
                    .resizable()
                    .scaledToFill()
            } else {
                Image(systemName: iconName(for: attachment.mimeType))
                    .font(.title3)
                    .foregroundStyle(.accent)
            }
        }
        .frame(width: side, height: side)
        .clipShape(RoundedRectangle(cornerRadius: 6))
        .accessibilityHidden(true)
        .task(id: attachment.id) {
            let repo = attachmentRepo
            let key = encryptionKey
            let blobId = attachment.data
            image = await AttachmentThumbnails.shared.image(for: attachment) {
                guard let blob = try repo.getBlob(id: blobId),
                      let json = String(data: blob, encoding: .utf8) else { return nil }
                return try CryptoService.decrypt(CryptoService.parseEncryptedJSON(json), key: key)
            }
        }
    }

    private func iconName(for mimeType: String) -> String {
        if mimeType.hasPrefix("image/") { return "photo" }
        if mimeType == "application/pdf" { return "doc.richtext" }
        if mimeType.hasPrefix("text/") { return "doc.text" }
        if mimeType.hasPrefix("video/") { return "film" }
        if mimeType.hasPrefix("audio/") { return "waveform" }
        return "doc"
    }
}
