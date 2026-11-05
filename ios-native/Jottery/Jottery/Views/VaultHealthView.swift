import SwiftUI

/// Finds notes that can't be decrypted and attachments that have gone missing,
/// here or on the server, and repairs what it can. Pushed from Settings.
struct VaultHealthView: View {
    @Environment(AppState.self) private var appState
    @State private var model = VaultHealthModel()
    @State private var work: Task<Void, Never>?
    @State private var noteToDelete: VaultHealthReport.UnreadableNote?

    var body: some View {
        Form {
            statusSection

            if let report = model.report {
                if !report.unreadableNotes.isEmpty {
                    unreadableSection(report.unreadableNotes)
                }
                if !report.missingAttachments.isEmpty {
                    missingSection(report.missingAttachments)
                }
                if appState.syncClient != nil, !report.heldAttachments.isEmpty {
                    serverSection
                }
            }
        }
        .navigationTitle(L.healthTitle)
        .navigationBarTitleDisplayMode(.inline)
        .task { await model.scan(appState: appState) }
        .onDisappear { work?.cancel() }
        .confirmationDialog(
            L.healthDeleteConfirmTitle,
            isPresented: Binding(
                get: { noteToDelete != nil },
                set: { if !$0 { noteToDelete = nil } }
            ),
            titleVisibility: .visible,
            presenting: noteToDelete
        ) { note in
            Button(L.healthDeleteConfirmAction, role: .destructive) {
                model.deleteUnreadableNote(id: note.id, appState: appState)
            }
            Button(L.commonCancel, role: .cancel) {}
        } message: { _ in
            Text(L.healthDeleteConfirmMessage)
        }
    }

    // MARK: - Sections

    private var statusSection: some View {
        Section {
            if model.isScanning {
                progressRow(L.healthScanning)
            } else if let report = model.report {
                if report.isHealthy {
                    Label(L.healthAllGood, systemImage: "checkmark.seal.fill")
                        .foregroundStyle(.green)
                } else {
                    Label(L.healthProblemsFound, systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(.orange)
                }
                Text(L.healthNotesChecked(report.notesChecked))
                    .foregroundStyle(.secondary)
            }

            Button(L.healthScanAgain) {
                run { await model.scan(appState: appState) }
            }
            .disabled(model.isBusy)

            if let error = model.errorMessage {
                Text(error)
                    .foregroundStyle(.red)
            }
        }
    }

    private func unreadableSection(_ notes: [VaultHealthReport.UnreadableNote]) -> some View {
        Section {
            ForEach(notes) { note in
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(note.modifiedAt?.formatted(date: .abbreviated, time: .shortened) ?? L.healthUnknownDate)
                        Text(note.inBin ? L.healthNoteInBin(shortId(note.id)) : shortId(note.id))
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .accessibilityElement(children: .combine)

                    Spacer()

                    Button(role: .destructive) {
                        noteToDelete = note
                    } label: {
                        Image(systemName: "trash")
                    }
                    // Borderless, so inside a Form only the icon is tappable, not the row.
                    .buttonStyle(.borderless)
                    .accessibilityLabel(L.healthDeleteNote)
                }
            }
        } header: {
            Text(L.healthUnreadableHeader)
        } footer: {
            Text(L.healthUnreadableFooter)
        }
    }

    private func missingSection(_ attachments: [VaultHealthReport.MissingAttachment]) -> some View {
        Section {
            ForEach(attachments) { attachment in
                attachmentRow(
                    filename: attachment.filename,
                    noteTitle: attachment.noteTitle,
                    status: model.repairResults[attachment.attachmentId].map(describe)
                )
            }

            if appState.syncClient != nil {
                Button {
                    run { await model.repairMissing(appState: appState) }
                } label: {
                    busyLabel(L.healthRepairFromServer, isBusy: model.isRepairing)
                }
                .disabled(model.isBusy)
            }
        } header: {
            Text(L.healthMissingHeader)
        } footer: {
            Text(appState.syncClient != nil ? L.healthMissingFooter : L.healthMissingFooterNoSync)
        }
    }

    private var serverSection: some View {
        Section {
            Button {
                run { await model.checkServer(appState: appState) }
            } label: {
                busyLabel(L.healthCheckServer, isBusy: false)
            }
            .disabled(model.isBusy)

            if model.isCheckingServer {
                progressRow(L.healthCheckingServer)
            } else if let lost = model.missingFromServer {
                if lost.isEmpty {
                    Label(L.healthServerHasEverything, systemImage: "checkmark.seal.fill")
                        .foregroundStyle(.green)
                } else {
                    ForEach(lost) { attachment in
                        attachmentRow(
                            filename: attachment.filename,
                            noteTitle: attachment.noteTitle,
                            status: model.uploadResults[attachment.attachmentId].map(describe)
                        )
                    }

                    Button {
                        run { await model.reuploadMissing(appState: appState) }
                    } label: {
                        busyLabel(L.healthSendToServer, isBusy: model.isUploading)
                    }
                    .disabled(model.isBusy)
                }
            }
        } header: {
            Text(L.healthServerHeader)
        } footer: {
            Text(L.healthServerFooter)
        }
    }

    // MARK: - Rows

    private func attachmentRow(filename: String, noteTitle: String?, status: String?) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(filename)
            Text(noteTitle ?? L.healthUnreadableNoteTitle)
                .font(.caption)
                .foregroundStyle(.secondary)
            if let status {
                Text(status)
                    .font(.caption)
                    .foregroundStyle(.red)
            }
        }
        .accessibilityElement(children: .combine)
    }

    private func progressRow(_ title: String) -> some View {
        ProgressView(
            value: Double(model.progress?.completed ?? 0),
            total: Double(max(model.progress?.total ?? 1, 1))
        ) {
            Text(title)
        }
    }

    private func busyLabel(_ title: String, isBusy: Bool) -> some View {
        HStack {
            Text(title)
            Spacer()
            if isBusy {
                ProgressView()
                    .controlSize(.small)
            }
        }
    }

    /// The first part of an id. It's the only thing a note that can't be read
    /// has to tell it apart from another of the same age.
    private func shortId(_ id: String) -> String {
        String(id.prefix(8))
    }

    private func describe(_ result: VaultHealthService.RepairResult) -> String {
        switch result {
        case .repaired: return ""
        case .notOnServer: return L.healthRepairNotOnServer
        case .unreadable: return L.healthRepairUnreadable
        case .failed: return L.healthRepairFailed
        }
    }

    private func describe(_ result: VaultHealthService.UploadResult) -> String {
        switch result {
        case .uploaded: return ""
        case .notKept: return L.healthUploadNotKept
        case .unreadable: return L.healthUploadUnreadable
        case .failed: return L.healthRepairFailed
        }
    }

    private func run(_ action: @escaping @MainActor () async -> Void) {
        work = Task { @MainActor in await action() }
    }
}
