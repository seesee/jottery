import SwiftUI

struct VersionHistoryView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    let note: DecryptedNote

    @State private var versions: [NoteVersionRecord] = []
    @State private var selectedVersion: NoteVersionRecord?
    @State private var previewContent: String?

    var body: some View {
        NavigationStack {
            List {
                ForEach(versions.reversed()) { version in
                    Button {
                        selectedVersion = version
                        decryptPreview(version)
                    } label: {
                        VStack(alignment: .leading, spacing: 4) {
                            HStack {
                                Text(L.versionHistoryVersion(version.version))
                                    .font(.headline)
                                Spacer()
                                Text(version.reason)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(.quaternary)
                                    .clipShape(Capsule())
                            }
                            Text(formatDate(version.createdAt))
                                .font(.caption)
                                .foregroundStyle(.tertiary)
                        }
                    }
                    .foregroundStyle(.primary)
                }
            }
            .listStyle(.insetGrouped)
            .navigationTitle(L.versionHistoryTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.versionHistoryDone) { dismiss() }
                }
            }
            .overlay {
                if versions.isEmpty {
                    ContentUnavailableView(
                        L.versionHistoryNoVersions,
                        systemImage: "clock.arrow.circlepath",
                        description: Text(L.versionHistoryNoVersionsDescription)
                    )
                }
            }
            .sheet(item: $selectedVersion) { version in
                VersionPreviewSheet(
                    version: version,
                    content: previewContent ?? "",
                    onRestore: {
                        try? appState.restoreVersion(noteId: note.id, version: version)
                        dismiss()
                    }
                )
            }
            .onAppear {
                loadVersions()
            }
        }
    }

    private func loadVersions() {
        guard let versionRepo = appState.versionRepo else { return }
        versions = (try? versionRepo.getVersions(noteId: note.id)) ?? []
    }

    private func decryptPreview(_ version: NoteVersionRecord) {
        guard let key = appState.keyManager.masterKey else { return }
        do {
            let enc = try CryptoService.parseEncryptedJSON(version.content)
            previewContent = try CryptoService.decryptText(enc, key: key)
        } catch {
            previewContent = "(Could not decrypt)"
        }
    }

    private func formatDate(_ iso: String) -> String {
        guard let date = Date(iso8601: iso) else { return iso }
        let fmt = DateFormatter()
        fmt.dateStyle = .medium
        fmt.timeStyle = .short
        return fmt.string(from: date)
    }
}

// MARK: - Version Preview Sheet

private struct VersionPreviewSheet: View {
    let version: NoteVersionRecord
    let content: String
    let onRestore: () -> Void
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            ScrollView {
                Text(content)
                    .font(.system(.body, design: .monospaced))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding()
            }
            .navigationTitle(L.versionHistoryVersion(version.version))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.versionHistoryDone) { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(L.versionHistoryRestore) {
                        onRestore()
                    }
                    .bold()
                }
            }
        }
    }
}
