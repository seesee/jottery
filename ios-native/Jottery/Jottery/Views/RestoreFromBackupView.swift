import SwiftUI
import UniformTypeIdentifiers

/// Offers a backup as the way to get a vault, on the first-run screen. Picking
/// a file opens a sheet that asks for its password.
struct RestoreFromBackupButton: View {
    @State private var showFilePicker = false
    @State private var pickedBackup: PickedBackup?
    @State private var errorMessage: String?

    var body: some View {
        VStack(spacing: 8) {
            Button {
                errorMessage = nil
                showFilePicker = true
            } label: {
                Label(L.setupRestoreFromBackup, systemImage: "arrow.down.doc")
            }

            if let errorMessage {
                Text(errorMessage)
                    .foregroundStyle(.red)
                    .font(.callout)
                    .frame(maxWidth: 320)
            }
        }
        .fileImporter(
            isPresented: $showFilePicker,
            allowedContentTypes: [.data, .json],
            allowsMultipleSelection: false
        ) { result in
            switch result {
            case .success(let urls):
                guard let url = urls.first else { return }
                pickedBackup = PickedBackup(url: url, isScoped: url.startAccessingSecurityScopedResource())
            case .failure(let error):
                errorMessage = error.localizedDescription
            }
        }
        .sheet(item: $pickedBackup) { backup in
            RestoreFromBackupView(backup: backup)
        }
    }
}

/// A file from the picker, read in place. Its security scope stays open for as
/// long as the sheet that reads it, and a URL that needs no scope reports false.
struct PickedBackup: Identifiable {
    let url: URL
    let isScoped: Bool

    var id: URL { url }

    func release() {
        if isScoped { url.stopAccessingSecurityScopedResource() }
    }
}

/// Asks for a backup's password and makes the vault from it. A wrong password or
/// a damaged file is shown here, so it can be tried again. On success the app
/// moves on to the notes and this goes with the first-run screen.
private struct RestoreFromBackupView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    let backup: PickedBackup

    @State private var password = ""
    @State private var errorMessage: String?
    @State private var progress: BackupService.Progress?
    @State private var work: Task<Void, Never>?

    private var isRestoring: Bool { work != nil }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    LabeledContent(L.setupRestoreFile, value: backup.url.lastPathComponent)
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
                        .disabled(password.isEmpty || isRestoring)
                }
            }
            .interactiveDismissDisabled(isRestoring)
            .onDisappear {
                work?.cancel()
                backup.release()
            }
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
        guard !password.isEmpty, !isRestoring else { return }
        errorMessage = nil
        progress = nil

        let url = backup.url
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
