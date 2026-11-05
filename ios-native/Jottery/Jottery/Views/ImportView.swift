import SwiftUI
import UniformTypeIdentifiers

struct ImportView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    @State private var showFilePicker = false
    @State private var strategy: ImportService.Strategy = .skip
    @State private var parsedExport: ImportService.ImportFile?
    /// The picked file is read in place, so its security scope stays open
    /// until the import ends or this view goes away.
    @State private var scopedURL: URL?
    @State private var isOpeningFile = false
    @State private var work: Task<Void, Never>?
    @State private var importResult: ImportService.ImportResult?
    @State private var didStartImport = false
    @State private var isImporting = false
    @State private var progress: (current: Int, total: Int)?
    @State private var errorMessage: String?

    var body: some View {
        NavigationStack {
            Form {
                if let export = parsedExport {
                    // File loaded — show summary and options
                    Section(L.importFileLoaded) {
                        HStack {
                            Text(L.importNoteCount)
                            Spacer()
                            Text("\(export.noteCount)")
                                .foregroundStyle(.secondary)
                        }
                        HStack {
                            Text(L.importExportDate)
                            Spacer()
                            Text(formatDate(export.exportDate))
                                .foregroundStyle(.secondary)
                        }
                    }

                    Section(L.importStrategy) {
                        Picker(L.importStrategy, selection: $strategy) {
                            ForEach(ImportService.Strategy.allCases) { strat in
                                VStack(alignment: .leading) {
                                    Text(strat.displayName)
                                }
                                .tag(strat)
                            }
                        }
                        .pickerStyle(.inline)
                        .labelsHidden()

                        Text(strategy.description)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }

                    if let progress {
                        Section {
                            ProgressView(value: Double(progress.current), total: Double(progress.total)) {
                                Text(L.importProgress(progress.current, progress.total))
                            }
                        }
                    }

                    Section {
                        Button {
                            performImport(export)
                        } label: {
                            HStack {
                                Spacer()
                                if isImporting {
                                    ProgressView()
                                        .controlSize(.small)
                                        .padding(.trailing, 8)
                                }
                                Text(L.importAction)
                                    .bold()
                                Spacer()
                            }
                        }
                        .disabled(isImporting)
                    }
                } else if let result = importResult {
                    // Import complete — show results
                    Section(L.importComplete) {
                        HStack {
                            Text(L.importImported)
                            Spacer()
                            Text("\(result.imported)")
                                .foregroundStyle(.secondary)
                        }
                        if result.skipped > 0 {
                            HStack {
                                Text(L.importSkipped)
                                Spacer()
                                Text("\(result.skipped)")
                                    .foregroundStyle(.secondary)
                            }
                        }
                        if !result.errors.isEmpty {
                            HStack {
                                Text(L.importErrors)
                                Spacer()
                                Text("\(result.errors.count)")
                                    .foregroundStyle(.red)
                            }
                        }
                    }

                    if !result.errors.isEmpty {
                        Section(L.importErrors) {
                            ForEach(result.errors, id: \.self) { error in
                                Text(error)
                                    .font(.caption)
                                    .foregroundStyle(.red)
                            }
                        }
                    }
                } else {
                    // Initial state — pick file
                    Section {
                        Button {
                            showFilePicker = true
                        } label: {
                            HStack {
                                Label(L.importSelectFile, systemImage: "doc.badge.plus")
                                Spacer()
                                if isOpeningFile {
                                    ProgressView()
                                        .controlSize(.small)
                                }
                            }
                        }
                        .disabled(isOpeningFile)
                    }

                    if let error = errorMessage {
                        Section {
                            Text(error)
                                .foregroundStyle(.red)
                                .font(.caption)
                        }
                    }
                }
            }
            .navigationTitle(L.importTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(importResult != nil ? L.versionHistoryDone : L.commonCancel) {
                        dismiss()
                    }
                }
            }
            .onDisappear {
                work?.cancel()
                releaseScopedFile()
                // However the sheet went: a swipe skips the toolbar button, and an
                // import stopped part way has still written notes.
                if didStartImport {
                    Task { await appState.reloadAfterImport() }
                }
            }
            .fileImporter(
                isPresented: $showFilePicker,
                allowedContentTypes: [UTType.json],
                allowsMultipleSelection: false
            ) { result in
                handleFileSelection(result)
            }
        }
    }

    // MARK: - Actions

    private func handleFileSelection(_ result: Result<[URL], Error>) {
        switch result {
        case .success(let urls):
            guard let url = urls.first else { return }
            if url.startAccessingSecurityScopedResource() {
                scopedURL = url
            }
            isOpeningFile = true
            errorMessage = nil
            work = Task { @MainActor in
                defer { isOpeningFile = false }
                do {
                    parsedExport = try await offMainActor { try ImportService.open(url) }
                } catch {
                    releaseScopedFile()
                    errorMessage = error.localizedDescription
                }
            }
        case .failure(let error):
            errorMessage = error.localizedDescription
        }
    }

    private func releaseScopedFile() {
        scopedURL?.stopAccessingSecurityScopedResource()
        scopedURL = nil
    }

    private func performImport(_ file: ImportService.ImportFile) {
        guard let noteRepo = appState.noteRepo,
              let attachmentRepo = appState.attachmentRepo,
              let key = appState.keyManager.masterKey else { return }

        isImporting = true
        didStartImport = true
        let currentStrategy = strategy
        work = Task { @MainActor in
            defer {
                parsedExport = nil
                isImporting = false
                progress = nil
                releaseScopedFile()
            }
            do {
                importResult = try await offMainActor {
                    try ImportService.importNotes(
                        from: file,
                        strategy: currentStrategy,
                        noteRepo: noteRepo,
                        attachmentRepo: attachmentRepo,
                        key: key,
                        progress: { current, total in
                            Task { @MainActor in
                                progress = (current, total)
                            }
                        }
                    )
                }
            } catch {
                importResult = ImportService.ImportResult(errors: [error.localizedDescription])
            }
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
