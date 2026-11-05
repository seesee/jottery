import SwiftUI
import UniformTypeIdentifiers

/// Offers a backup as the way to get a vault, on the first-run screen.
struct RestoreFromBackupButton: View {
    @State private var showRestore = false

    var body: some View {
        Button {
            showRestore = true
        } label: {
            Label(L.setupRestoreFromBackup, systemImage: "arrow.down.doc")
        }
        .sheet(isPresented: $showRestore) {
            RestoreFromBackupView()
        }
    }
}

/// A file from the picker, read in place. Its security scope stays open until
/// it's released, and a URL that needs no scope reports false.
private struct PickedBackup {
    let url: URL
    let isScoped: Bool

    func release() {
        if isScoped { url.stopAccessingSecurityScopedResource() }
    }
}

/// Asks for a backup and its password, and makes the vault from them. A wrong
/// password or a damaged file is shown here, where either can be tried again. On
/// success the app moves on to the notes and this goes with the first-run screen.
private struct RestoreFromBackupView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    @State private var backup: PickedBackup?
    @State private var showFilePicker = false
    @State private var password = ""
    @State private var errorMessage: String?
    @State private var progress: BackupService.Progress?
    @State private var work: Task<Void, Never>?

    private var isRestoring: Bool { work != nil }
    private var canRestore: Bool { backup != nil && !password.isEmpty && !isRestoring }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Button {
                        showFilePicker = true
                    } label: {
                        LabeledContent(
                            L.setupRestoreFile,
                            value: backup?.url.lastPathComponent ?? L.setupRestoreChooseFile
                        )
                    }
                    .foregroundStyle(.primary)
                    .disabled(isRestoring)

                    PasswordField(L.backupPassword, text: $password, contentType: .password)
                        .disabled(isRestoring)
                        .onSubmit(restore)
                } footer: {
                    Text(L.setupRestoreHint)
                }

                if isRestoring {
                    Section {
                        ProgressView(
                            value: Double(progress?.completed ?? 0),
                            total: Double(max(progress?.total ?? 1, 1))
                        ) {
                            Text(progress.map { L.restoreProgress($0.completed, $0.total) } ?? L.backupRestoring)
                        }
                    }
                }

                if let errorMessage {
                    Section {
                        Text(errorMessage)
                            .foregroundStyle(.red)
                    }
                }
            }
            .navigationTitle(L.setupRestoreTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.commonCancel) { cancel() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(L.setupRestoreAction) { restore() }
                        .disabled(!canRestore)
                }
            }
            .interactiveDismissDisabled(isRestoring)
            .fileImporter(
                isPresented: $showFilePicker,
                allowedContentTypes: [.data, .json],
                allowsMultipleSelection: false
            ) { result in
                handleFileSelected(result)
            }
            .onDisappear {
                work?.cancel()
                backup?.release()
            }
        }
    }

    private func handleFileSelected(_ result: Result<[URL], Error>) {
        switch result {
        case .success(let urls):
            guard let url = urls.first else { return }
            backup?.release()
            backup = PickedBackup(url: url, isScoped: url.startAccessingSecurityScopedResource())
            errorMessage = nil
        case .failure(let error):
            errorMessage = error.localizedDescription
        }
    }

    private func cancel() {
        guard let work else {
            dismiss()
            return
        }
        // The sheet closes when the restore has stopped and cleared up after itself.
        work.cancel()
    }

    private func restore() {
        guard canRestore, let url = backup?.url else { return }
        errorMessage = nil
        progress = nil

        let password = password
        work = Task { @MainActor in
            defer { work = nil }
            do {
                let result = try await appState.restoreVault(fromBackup: url, password: password) { update in
                    Task { @MainActor in progress = update }
                }
                if result.skipped > 0 {
                    appState.reportError(L.backupRestoreSkipped(result.skipped), duration: .seconds(10))
                }
            } catch is CancellationError {
                dismiss()
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }
}
