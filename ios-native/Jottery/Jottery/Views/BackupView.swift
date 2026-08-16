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
    @State private var pendingBackupURL: URL?
    @State private var errorMessage: String?
    @State private var exportFile: ExportFile?
    @State private var showCreateSuccess = false
    @State private var showRestoreSuccess = false
    @State private var restoredCount = 0

    var body: some View {
        NavigationStack {
            Form {
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
                }
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
                    pendingBackupURL = nil
                }
            } message: {
                Text(L.backupPasswordHint)
            }
            .alert(L.backupSuccessTitle, isPresented: $showCreateSuccess) {
                Button(L.settingsDone) {}
            } message: {
                Text(L.backupSuccessMessage)
            }
            .alert(L.backupRestoreSuccessTitle, isPresented: $showRestoreSuccess) {
                Button(L.settingsDone) {}
            } message: {
                Text(L.backupRestoreSuccessMessage)
            }
            .sheet(item: $exportFile) { file in
                ActivityView(activityItems: [file.url])
            }
        }
    }

    // MARK: - Create

    private func createBackup() {
        guard let noteRepo = appState.noteRepo,
              let versionRepo = appState.versionRepo,
              let attachmentRepo = appState.attachmentRepo,
              let savedSearchRepo = appState.savedSearchRepo,
              let encryptionRepo = appState.encryptionRepo,
              let key = appState.keyManager.masterKey else { return }

        errorMessage = nil
        isCreating = true

        Task { @MainActor in
            defer { isCreating = false }
            do {
                let data = try BackupService.createBackup(
                    noteRepo: noteRepo,
                    versionRepo: versionRepo,
                    attachmentRepo: attachmentRepo,
                    savedSearchRepo: savedSearchRepo,
                    encryptionRepo: encryptionRepo,
                    key: key
                )
                let filename = BackupService.defaultFilename()
                let tempURL = FileManager.default.temporaryDirectory.appendingPathComponent(filename)
                try data.write(to: tempURL)
                exportFile = ExportFile(url: tempURL)
                showCreateSuccess = true
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    // MARK: - Restore

    private func handleFileSelected(_ result: Result<[URL], Error>) {
        guard case .success(let urls) = result, let url = urls.first else { return }
        guard url.startAccessingSecurityScopedResource() else { return }

        // Copy to temp location so we can access it after the picker dismisses
        let tempURL = FileManager.default.temporaryDirectory.appendingPathComponent("jottery-restore-\(UUID().uuidString).jottery")
        do {
            try FileManager.default.copyItem(at: url, to: tempURL)
        } catch {
            url.stopAccessingSecurityScopedResource()
            errorMessage = error.localizedDescription
            return
        }
        url.stopAccessingSecurityScopedResource()

        pendingBackupURL = tempURL
        backupPassword = ""
        showPasswordPrompt = true
    }

    private func restoreFromPendingFile() {
        guard let url = pendingBackupURL else { return }
        guard let noteRepo = appState.noteRepo,
              let versionRepo = appState.versionRepo,
              let attachmentRepo = appState.attachmentRepo,
              let key = appState.keyManager.masterKey else { return }

        let password = backupPassword
        backupPassword = ""
        errorMessage = nil
        isRestoring = true

        Task { @MainActor in
            defer {
                isRestoring = false
                pendingBackupURL = nil
                try? FileManager.default.removeItem(at: url)
            }
            do {
                let data = try Data(contentsOf: url)
                let count = try BackupService.restoreBackup(
                    data: data,
                    password: password,
                    noteRepo: noteRepo,
                    versionRepo: versionRepo,
                    attachmentRepo: attachmentRepo,
                    currentKey: key
                )
                restoredCount = count
                try? await appState.loadNotes()
                showRestoreSuccess = true
            } catch BackupError.wrongPassword {
                errorMessage = BackupError.wrongPassword.localizedDescription
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }
}
