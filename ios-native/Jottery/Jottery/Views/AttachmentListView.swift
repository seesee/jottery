import CryptoKit
import QuickLook
import SwiftUI

/// Collapsible attachment list displayed below the editor.
struct AttachmentListView: View {
    let attachments: [AttachmentRef]
    let attachmentRepo: AttachmentRepository
    let encryptionKey: SymmetricKey

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
                        onShare: { decryptAndShare(attachment) }
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
        guard let url = decryptToTempFile(attachment) else { return }
        previewURL = url
    }

    private func decryptAndShare(_ attachment: AttachmentRef) {
        errorMessage = nil
        guard let url = decryptToTempFile(attachment) else { return }
        shareURL = url
        showingShareSheet = true
    }

    /// Decrypt an attachment blob and write to a temporary file.
    /// Returns the file URL on success, nil on failure.
    private func decryptToTempFile(_ attachment: AttachmentRef) -> URL? {
        do {
            // Fetch the encrypted blob from the database
            guard let blobData = try attachmentRepo.getBlob(id: attachment.data) else {
                errorMessage = L.attachmentsDataNotAvailable
                return nil
            }

            // The blob is UTF-8 of {"ciphertext":"...","iv":"..."}
            guard let blobString = String(data: blobData, encoding: .utf8) else {
                errorMessage = L.attachmentsInvalidData
                return nil
            }

            let encrypted = try CryptoService.parseEncryptedJSON(blobString)
            let decryptedData = try CryptoService.decrypt(encrypted, key: encryptionKey)

            // Write to temp file with the correct filename
            let tempDir = FileManager.default.temporaryDirectory
                .appendingPathComponent("jottery-attachments", isDirectory: true)
            try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)

            let fileURL = tempDir.appendingPathComponent(attachment.filename)
            try decryptedData.write(to: fileURL)

            return fileURL
        } catch {
            errorMessage = L.attachmentsDecryptFailed
            return nil
        }
    }
}

// MARK: - Attachment Row

private struct AttachmentRowView: View {
    let attachment: AttachmentRef
    let onTap: () -> Void
    let onShare: () -> Void

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
