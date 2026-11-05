import SwiftUI
import UniformTypeIdentifiers

struct BackupView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    @State private var isCreating = false
    @State private var isRestoring = false
    @State private var showFilePicker = false
    @State private var showPasswordPrompt = false
    @State private var backupPassword = ""
    /// The picked backup, read in place. Its security scope stays open until
    /// the restore ends or the password prompt is cancelled.
    @State private var pendingBackupURL: URL?
    @State private var errorMessage: String?
    @State private var exportFile: ExportFile?
    @State private var showRestoreSuccess = false
    @State private var skippedCount = 0
    @State private var keptNewerCount = 0
    @State private var progress: BackupService.Progress?
    @State private var work: Task<Void, Never>?

    var body: some View {
        NavigationStack {
            Form {
                BackupReminderSection()

                // Create Backup
                Section {
                    Button {
                        createBackup()
                    } label: {
                        HStack {
                            Label(L.backupCreate, systemImage: "arrow.up.doc")
                            Spacer()
                            if isCreating {
                                ProgressView()
                                    .controlSize(.small)
                            }
                        }
                    }
                    .disabled(isCreating || isRestoring)
                } footer: {
                    Text(L.backupCreateDescription)
                }

                // Restore Backup
                Section {
                    Button {
                        showFilePicker = true
                    } label: {
                        HStack {
                            Label(L.backupRestore, systemImage: "arrow.down.doc")
                            Spacer()
                            if isRestoring {
                                ProgressView()
                                    .controlSize(.small)
                            }
                        }
                    }
                    .disabled(isCreating || isRestoring)
                } footer: {
                    Text(L.backupRestoreDescription)
                }

                if let progress {
                    Section {
                        ProgressView(value: Double(progress.completed), total: Double(max(progress.total, 1))) {
                            Text(isCreating
                                ? L.backupProgress(progress.completed, progress.total)
                                : L.restoreProgress(progress.completed, progress.total))
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
            .navigationTitle(L.backupTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button(L.settingsDone) { dismiss() }
                        .disabled(isCreating || isRestoring)
                }
            }
            .interactiveDismissDisabled(isCreating || isRestoring)
            .onDisappear {
                // The app can lock while this runs, which takes the view away.
                work?.cancel()
                if !isRestoring { releasePendingBackup() }
            }
            .fileImporter(
                isPresented: $showFilePicker,
                allowedContentTypes: [.data, .json],
                allowsMultipleSelection: false
            ) { result in
                handleFileSelected(result)
            }
            .alert(L.backupPassword, isPresented: $showPasswordPrompt) {
                SecureField(L.backupPassword, text: $backupPassword)
                Button(L.backupRestore) {
                    restoreFromPendingFile()
                }
                Button(L.commonCancel, role: .cancel) {
                    backupPassword = ""
                    releasePendingBackup()
                }
            } message: {
                Text(L.backupPasswordHint)
            }
            .alert(L.backupRestoreSuccessTitle, isPresented: $showRestoreSuccess) {
                Button(L.settingsDone) {}
            } message: {
                Text(restoreSummary)
            }
            .shareTemporaryFile($exportFile, onShared: { appState.recordBackupMade() })
        }
    }

    private var restoreSummary: String {
        var lines: [String] = []
        if skippedCount > 0 { lines.append(L.backupRestoreSkipped(skippedCount)) }
        if keptNewerCount > 0 { lines.append(L.backupRestoreKeptNewer(keptNewerCount)) }
        return lines.isEmpty ? L.backupRestoreSuccessMessage : lines.joined(separator: "\n\n")
    }

    // MARK: - Create

    private func createBackup() {
        guard let noteRepo = appState.noteRepo,
              let attachmentRepo = appState.attachmentRepo,
              let savedSearchRepo = appState.savedSearchRepo,
              let encryptionRepo = appState.encryptionRepo,
              let key = appState.keyManager.masterKey else { return }

        errorMessage = nil
        isCreating = true

        work = Task { @MainActor in
            defer {
                isCreating = false
                progress = nil
            }
            var file: URL?
            do {
                let url = try TemporaryFiles.shared.createFile(named: BackupService.defaultFilename())
                file = url
                try await offMainActor {
                    try BackupService.writeBackup(
                        to: url,
                        noteRepo: noteRepo,
                        attachmentRepo: attachmentRepo,
                        savedSearchRepo: savedSearchRepo,
                        encryptionRepo: encryptionRepo,
                        key: key,
                        progress: { update in Task { @MainActor in progress = update } }
                    )
                }
                exportFile = ExportFile(url: url)
            } catch {
                if let file { TemporaryFiles.shared.remove(file) }
                if !(error is CancellationError) {
                    errorMessage = error.localizedDescription
                }
            }
        }
    }

    // MARK: - Restore

    private func handleFileSelected(_ result: Result<[URL], Error>) {
        guard case .success(let urls) = result, let url = urls.first else { return }
        guard url.startAccessingSecurityScopedResource() else { return }

        pendingBackupURL = url
        backupPassword = ""
        showPasswordPrompt = true
    }

    private func releasePendingBackup() {
        pendingBackupURL?.stopAccessingSecurityScopedResource()
        pendingBackupURL = nil
    }

    private func restoreFromPendingFile() {
        guard let url = pendingBackupURL else { return }
        guard let noteRepo = appState.noteRepo,
              let versionRepo = appState.versionRepo,
              let attachmentRepo = appState.attachmentRepo,
              let savedSearchRepo = appState.savedSearchRepo,
              let key = appState.keyManager.masterKey else {
            releasePendingBackup()
            return
        }

        let password = backupPassword
        backupPassword = ""
        errorMessage = nil
        isRestoring = true

        work = Task { @MainActor in
            defer {
                isRestoring = false
                progress = nil
                releasePendingBackup()
            }
            do {
                let result = try await offMainActor {
                    try BackupService.restoreBackup(
                        from: url,
                        password: password,
                        noteRepo: noteRepo,
                        versionRepo: versionRepo,
                        attachmentRepo: attachmentRepo,
                        savedSearchRepo: savedSearchRepo,
                        currentKey: key,
                        progress: { update in Task { @MainActor in progress = update } }
                    )
                }
                skippedCount = result.skipped
                keptNewerCount = result.keptNewer
                try? await appState.loadNotes()
                showRestoreSuccess = true
            } catch is CancellationError {
                // The view went away, so there's nobody to tell.
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }
}
