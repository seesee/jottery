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
    @State private var shareURL: URL?
    @State private var showingShareSheet = false
    @State private var errorMessage: String?

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
                        onTap: { decryptAndPreview(attachment) },
                        onShare: { decryptAndShare(attachment) },
                        onDelete: onDelete != nil ? { onDelete?(attachment.id) } : nil
                    )

                    if attachment.id != attachments.last?.id {
                        Divider()
                            .padding(.leading, 52)
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
        .sheet(isPresented: $showingShareSheet) {
            if let shareURL {
                ShareSheet(url: shareURL)
            }
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
                shareURL = url
                showingShareSheet = true
            }
        }
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
                let tempDir = FileManager.default.temporaryDirectory
                    .appendingPathComponent("jottery-attachments", isDirectory: true)
                try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)

                let fileURL = tempDir.appendingPathComponent(filename)
                try decryptedData.write(to: fileURL)

                return fileURL
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
    let onTap: () -> Void
    let onShare: () -> Void
    var onDelete: (() -> Void)?

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 12) {
                Image(systemName: iconName(for: attachment.mimeType))
                    .font(.title3)
                    .foregroundStyle(.accent)
                    .frame(width: 28)

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
                }
                .buttonStyle(.plain)

                if let onDelete {
                    Button(role: .destructive, action: onDelete) {
                        Image(systemName: "trash")
                            .font(.subheadline)
                            .foregroundStyle(.red)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal)
            .padding(.vertical, 8)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func iconName(for mimeType: String) -> String {
        if mimeType.hasPrefix("image/") { return "photo" }
        if mimeType == "application/pdf" { return "doc.richtext" }
        if mimeType.hasPrefix("text/") { return "doc.text" }
        if mimeType.hasPrefix("video/") { return "film" }
        if mimeType.hasPrefix("audio/") { return "waveform" }
        return "doc"
    }

    private func formattedSize(_ bytes: Int) -> String {
        let formatter = ByteCountFormatter()
        formatter.countStyle = .file
        return formatter.string(fromByteCount: Int64(bytes))
    }
}

// MARK: - Share Sheet

private struct ShareSheet: UIViewControllerRepresentable {
    let url: URL

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: [url], applicationActivities: nil)
    }

    func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}
