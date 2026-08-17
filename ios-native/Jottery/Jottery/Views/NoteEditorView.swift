import SwiftUI
import UniformTypeIdentifiers

struct NoteEditorView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.colorScheme) private var colorScheme
    @State private var content: String
    @State private var tags: [String]
    @State private var syntaxLanguage: String
    @State private var wordWrap: Bool
    @State private var color: String?
    @State private var showPreview: Bool
    @State private var saveTask: Task<Void, Never>?
    @State private var didSaveDuringSession = false
    @State private var showVersionHistory = false
    @State private var showNoteInfo = false
    @State private var showAttachmentPicker = false
    @State private var showPhotoPicker = false
    @State private var pinchBaseScale: Double = 1.0

    let note: DecryptedNote

    init(note: DecryptedNote) {
        self.note = note
        _content = State(initialValue: note.content)
        _tags = State(initialValue: note.tags)
        _syntaxLanguage = State(initialValue: note.syntaxLanguage)
        _wordWrap = State(initialValue: note.wordWrap)
        _color = State(initialValue: note.color)
        _showPreview = State(initialValue: note.showPreview)
    }

    /// Whether the note is read-only (locked or archived).
    private var isReadOnly: Bool {
        note.locked || note.archived
    }

    var body: some View {
        VStack(spacing: 0) {
            // Tags
            if !tags.isEmpty || !isReadOnly {
                TagInputView(tags: isReadOnly ? .constant(tags) : $tags)
                    .disabled(isReadOnly)
                    .padding(.horizontal)
                    .padding(.vertical, 8)

                Divider()
            }

            // Editor or Markdown Preview
            Group {
                if showPreview && syntaxLanguage != "calc" && syntaxLanguage != "outliner" {
                    MarkdownPreviewView(
                        content: content,
                        fontSize: appState.editorFontSize,
                        attachments: note.attachments,
                        attachmentRepo: appState.attachmentRepo,
                        encryptionKey: appState.keyManager.masterKey
                    )
                } else if syntaxLanguage == "calc" {
                    WebCalcEditorView(content: $content, fontSize: appState.editorFontSize)
                } else if syntaxLanguage == "outliner" {
                    WebOutlinerEditorView(content: $content, fontSize: appState.editorFontSize)
                } else {
                    RunestoneEditorView(
                        text: $content,
                        syntaxLanguage: syntaxLanguage,
                        wordWrap: wordWrap,
                        isEditable: !isReadOnly,
                        fontSize: appState.editorFontSize
                    )
                }
            }
            // Attached once here (not per-branch) so tag/content edits made while in
            // preview mode — which has no editor of its own — still trigger a save.
            .onChange(of: content) { _, _ in
                scheduleSave()
            }
            .onChange(of: tags) { _, _ in
                scheduleSave()
            }

            // Attachments
            if !note.attachments.isEmpty, let attachmentRepo = appState.attachmentRepo,
               let key = appState.keyManager.masterKey {
                Divider()
                AttachmentListView(
                    attachments: note.attachments,
                    attachmentRepo: attachmentRepo,
                    encryptionKey: key,
                    onDelete: isReadOnly ? nil : { attachmentId in
                        do {
                            try appState.removeAttachment(from: note.id, attachmentId: attachmentId)
                        } catch {
                            appState.reportError(L.errorCouldntRemoveAttachment)
                        }
                    }
                )
            }
        }
        .simultaneousGesture(
            MagnifyGesture()
                .onChanged { value in
                    let newScale = pinchBaseScale * value.magnification
                    appState.editorFontScale = max(0.5, min(3.0, newScale))
                }
                .onEnded { _ in
                    pinchBaseScale = appState.editorFontScale
                }
        )
        .onAppear {
            pinchBaseScale = appState.editorFontScale
        }
        .navigationTitle(note.title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Menu {
                    // Pin toggle
                    Button {
                        let wasPinned = note.pinned
                        do {
                            try appState.togglePin(id: note.id)
                        } catch {
                            appState.reportError(wasPinned ? L.errorCouldntUnpinNote : L.errorCouldntPinNote)
                        }
                    } label: {
                        Label(note.pinned ? L.editorUnpin : L.editorPin, systemImage: note.pinned ? "pin.slash" : "pin")
                    }

                    // Word wrap toggle
                    Button {
                        wordWrap.toggle()
                        scheduleSave()
                    } label: {
                        Label(
                            wordWrap ? L.editorDisableWordWrap : L.editorEnableWordWrap,
                            systemImage: wordWrap ? "arrow.right.to.line" : "text.word.spacing"
                        )
                    }
                    .disabled(isReadOnly)

                    // Reset text size
                    if appState.editorFontScale != 1.0 {
                        Button {
                            appState.editorFontScale = 1.0
                            pinchBaseScale = 1.0
                        } label: {
                            Label(L.editorResetTextSize, systemImage: "textformat.size")
                        }
                    }

                    // Preview toggle
                    Button {
                        showPreview.toggle()
                        scheduleSave()
                    } label: {
                        Label(
                            showPreview ? L.editorHidePreview : L.editorShowPreview,
                            systemImage: showPreview ? "eye.slash" : "eye"
                        )
                    }
                    .disabled(isReadOnly)

                    // Language picker
                    Menu {
                        ForEach(syntaxLanguages, id: \.self) { lang in
                            Button {
                                syntaxLanguage = lang
                                scheduleSave()
                            } label: {
                                if lang == syntaxLanguage {
                                    Label(lang.capitalized, systemImage: "checkmark")
                                } else {
                                    Text(lang.capitalized)
                                }
                            }
                        }
                    } label: {
                        Label(L.editorLanguage(syntaxLanguage.capitalized), systemImage: "chevron.left.forwardslash.chevron.right")
                    }
                    .disabled(isReadOnly)

                    // Colour picker
                    Menu {
                        ForEach(Color.noteColorNames, id: \.self) { name in
                            Button {
                                color = name
                                scheduleSave()
                            } label: {
                                Label(name.capitalized, systemImage: color == name ? "checkmark.circle.fill" : "circle.fill")
                            }
                        }
                        Divider()
                        Button {
                            color = nil
                            scheduleSave()
                        } label: {
                            Label(L.editorCategoryNone, systemImage: color == nil ? "checkmark.circle" : "circle.slash")
                        }
                    } label: {
                        Label(L.editorCategory, systemImage: "paintbrush")
                    }
                    .disabled(isReadOnly)

                    // Add attachment
                    if !isReadOnly {
                        Button {
                            showAttachmentPicker = true
                        } label: {
                            Label(L.editorAddAttachment, systemImage: "paperclip")
                        }

                        Button {
                            showPhotoPicker = true
                        } label: {
                            Label(L.editorAddPhoto, systemImage: "photo")
                        }

                        // Runestone only pastes text, so images/PDFs on the
                        // clipboard are attached from here instead.
                        if PasteboardService.hasAttachableContent {
                            Button {
                                var failureCount = 0
                                for item in PasteboardService.readItems() {
                                    do {
                                        try appState.addAttachment(
                                            to: note.id,
                                            data: item.data,
                                            filename: item.filename,
                                            mimeType: item.mimeType
                                        )
                                    } catch {
                                        failureCount += 1
                                    }
                                }
                                if failureCount > 0 {
                                    appState.reportError(L.errorCouldntAddAttachment)
                                }
                            } label: {
                                Label(L.editorPasteFromClipboard, systemImage: "doc.on.clipboard")
                            }
                        }
                    }

                    Divider()

                    // Lock toggle
                    Button {
                        let wasLocked = note.locked
                        do {
                            try appState.toggleLock(id: note.id)
                        } catch {
                            appState.reportError(wasLocked ? L.errorCouldntUnlockNote : L.errorCouldntLockNote)
                        }
                    } label: {
                        Label(
                            note.locked ? L.editorUnlock : L.editorLock,
                            systemImage: note.locked ? "lock.open" : "lock"
                        )
                    }

                    // Archive toggle
                    if note.archived {
                        Button {
                            Task {
                                do {
                                    try await appState.unarchiveNote(id: note.id)
                                } catch {
                                    appState.reportError(L.errorCouldntUnarchiveNote)
                                }
                            }
                        } label: {
                            Label(L.editorUnarchive, systemImage: "tray.and.arrow.up")
                        }
                    } else {
                        Button {
                            do {
                                try appState.archiveNote(id: note.id)
                            } catch {
                                appState.reportError(L.errorCouldntArchiveNote)
                            }
                        } label: {
                            Label(L.editorArchive, systemImage: "archivebox")
                        }
                    }

                    // Version history
                    if note.version > 0 {
                        Button {
                            showVersionHistory = true
                        } label: {
                            Label(L.editorVersionHistory, systemImage: "clock.arrow.circlepath")
                        }
                    }

                    // Note info
                    Button {
                        showNoteInfo = true
                    } label: {
                        Label(L.noteInfoTitle, systemImage: "info.circle")
                    }

                    // Duplicate
                    Button {
                        do {
                            try appState.duplicateNote(id: note.id)
                        } catch {
                            appState.reportError(L.errorCouldntDuplicateNote)
                        }
                    } label: {
                        Label(L.editorDuplicate, systemImage: "doc.on.doc")
                    }

                    Divider()

                    // Delete
                    Button(role: .destructive) {
                        do {
                            try appState.deleteNote(id: note.id)
                        } catch {
                            appState.reportError(L.errorCouldntDeleteNote)
                        }
                    } label: {
                        Label(L.editorDelete, systemImage: "trash")
                    }
                } label: {
                    Image(systemName: "ellipsis.circle")
                }
            }
        }
        .onChange(of: note.id) { oldId, _ in
            // `note` already reflects the incoming note by the time this closure
            // runs, so the outgoing note's pending edits must be flushed using the
            // pre-reset @State values (still present below) against the outgoing
            // note's own stored record — not against `note`/`hasChanges`.
            flushOutgoingNote(id: oldId)
            // Reset state when switching notes
            content = note.content
            tags = note.tags
            syntaxLanguage = note.syntaxLanguage
            wordWrap = note.wordWrap
            color = note.color
            showPreview = note.showPreview
            didSaveDuringSession = false
        }
        .sheet(isPresented: $showVersionHistory) {
            VersionHistoryView(note: note)
        }
        .sheet(isPresented: $showNoteInfo) {
            NoteInfoView(note: note)
        }
        .fileImporter(
            isPresented: $showAttachmentPicker,
            allowedContentTypes: [.data],
            allowsMultipleSelection: false
        ) { result in
            handleFileImport(result)
        }
        .sheet(isPresented: $showPhotoPicker) {
            PhotoPickerView { url, filename, mimeType in
                do {
                    try appState.addAttachment(to: note.id, url: url, filename: filename, mimeType: mimeType)
                } catch {
                    appState.reportError(L.errorCouldntAddAttachment)
                }
            }
        }
        .onDisappear {
            saveImmediately()
            appState.pendingEditorNote = nil
            // Trigger a background sync if any save happened during this editing session
            if didSaveDuringSession && appState.syncEnabled {
                Task { await appState.triggerSync() }
            }
        }
    }

    // MARK: - Auto-Save

    /// Debounced save — waits 1 second of inactivity before saving.
    private func scheduleSave() {
        // Defence in depth: toolbar controls that mutate note state are
        // disabled while read-only, but nothing should ever schedule (or
        // stage a pending) save for a locked/archived note.
        guard !isReadOnly else { return }
        appState.keyManager.recordActivity()
        // Keep AppState updated with current editor state so lock()
        // can flush pending changes before wiping the encryption key.
        updatePendingNote()
        saveTask?.cancel()
        saveTask = Task {
            try? await Task.sleep(for: .seconds(1))
            guard !Task.isCancelled else { return }
            saveImmediately()
        }
    }

    private var hasChanges: Bool {
        content != note.content ||
        tags != note.tags ||
        syntaxLanguage != note.syntaxLanguage ||
        wordWrap != note.wordWrap ||
        color != note.color ||
        showPreview != note.showPreview
    }

    private func updatePendingNote() {
        guard hasChanges else { return }
        var updated = note
        updated.content = content
        updated.tags = tags
        updated.syntaxLanguage = syntaxLanguage
        updated.wordWrap = wordWrap
        updated.color = color
        updated.showPreview = showPreview
        appState.pendingEditorNote = updated
    }

    /// Flushes unsaved edits for the note being navigated *away from*, using the
    /// still-current (pre-reset) @State values, before `.onChange(of: note.id)`
    /// reseeds state for the incoming note. Cannot reuse `hasChanges`/`saveImmediately`
    /// because those compare against `note`, which already refers to the incoming
    /// note by the time this runs.
    ///
    /// Every early return below clears `appState.pendingEditorNote` when it
    /// still refers to the outgoing note (matched by id). That pending
    /// snapshot was staged by an earlier `scheduleSave()` while the note was
    /// still editable; if left behind it would be replayed verbatim by
    /// `AppState.flushPendingEditorNote()` on the next `lock()` —
    /// `noteRepo.update` there doesn't check `locked`/`archived`, so a stale
    /// pending note would silently overwrite a note that has since become
    /// read-only (or is gone, or was already unchanged). Only the successful
    /// save path at the bottom needs no extra clearing — it already nils
    /// `pendingEditorNote` unconditionally.
    private func flushOutgoingNote(id: String) {
        saveTask?.cancel()
        saveTask = nil
        guard let outgoing = appState.displayedNote(id: id) else {
            clearStalePendingNote(id: id)
            return
        }
        guard !outgoing.locked && !outgoing.archived else {
            clearStalePendingNote(id: id)
            return
        }

        let changed = content != outgoing.content ||
            tags != outgoing.tags ||
            syntaxLanguage != outgoing.syntaxLanguage ||
            wordWrap != outgoing.wordWrap ||
            color != outgoing.color ||
            showPreview != outgoing.showPreview
        guard changed else {
            clearStalePendingNote(id: id)
            return
        }

        var updated = outgoing
        updated.content = content
        updated.tags = tags
        updated.syntaxLanguage = syntaxLanguage
        updated.wordWrap = wordWrap
        updated.color = color
        updated.showPreview = showPreview
        try? appState.saveNote(updated)
        appState.pendingEditorNote = nil
        // Note: `didSaveDuringSession` is deliberately not set here — the
        // caller (`.onChange(of: note.id)`) resets it to false for the
        // incoming note immediately after this call returns, so any
        // assignment here would be dead (jottery-a3sb review residual).
    }

    /// Clears `appState.pendingEditorNote` only if it still refers to the
    /// given note id — never clobbers a pending snapshot staged for a
    /// different (e.g. newly-selected) note.
    private func clearStalePendingNote(id: String) {
        if appState.pendingEditorNote?.id == id {
            appState.pendingEditorNote = nil
        }
    }

    private func saveImmediately() {
        saveTask?.cancel()
        // Defence in depth: never persist edits to a note that is currently
        // locked/archived, and drop any stale pending snapshot for it so a
        // later lock() can't replay it either.
        guard !isReadOnly else {
            clearStalePendingNote(id: note.id)
            return
        }
        guard hasChanges else { return }
        var updated = note
        updated.content = content
        updated.tags = tags
        updated.syntaxLanguage = syntaxLanguage
        updated.wordWrap = wordWrap
        updated.color = color
        updated.showPreview = showPreview
        try? appState.saveNote(updated)
        appState.pendingEditorNote = nil
        didSaveDuringSession = true
    }

    private var syntaxLanguages: [String] {
        ["markdown", "calc", "outliner", "javascript", "typescript", "python", "perl", "json", "xml", "css", "bash", "sql", "plain"]
    }

    // MARK: - Attachment Import

    private func handleFileImport(_ result: Result<[URL], Error>) {
        guard case .success(let urls) = result, let url = urls.first else { return }
        guard url.startAccessingSecurityScopedResource() else { return }
        defer { url.stopAccessingSecurityScopedResource() }

        let filename = url.lastPathComponent
        let mimeType = UTType(filenameExtension: url.pathExtension)?.preferredMIMEType ?? "application/octet-stream"
        do {
            try appState.addAttachment(to: note.id, url: url, filename: filename, mimeType: mimeType)
        } catch {
            appState.reportError(L.errorCouldntAddAttachment)
        }
    }
}

// MARK: - Photo Picker

import PhotosUI

struct PhotoPickerView: UIViewControllerRepresentable {
    let onPick: (URL, String, String) -> Void

    func makeUIViewController(context: Context) -> PHPickerViewController {
        var config = PHPickerConfiguration()
        config.selectionLimit = 1
        config.filter = .any(of: [.images, .videos])
        let picker = PHPickerViewController(configuration: config)
        picker.delegate = context.coordinator
        return picker
    }

    func updateUIViewController(_ uiViewController: PHPickerViewController, context: Context) {}

    func makeCoordinator() -> Coordinator {
        Coordinator(onPick: onPick)
    }

    class Coordinator: NSObject, PHPickerViewControllerDelegate {
        let onPick: (URL, String, String) -> Void

        init(onPick: @escaping (URL, String, String) -> Void) {
            self.onPick = onPick
        }

        func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
            picker.dismiss(animated: true)
            guard let result = results.first else { return }

            let itemProvider = result.itemProvider

            // Try to load as file URL
            if itemProvider.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
                itemProvider.loadFileRepresentation(forTypeIdentifier: UTType.image.identifier) { [weak self] url, error in
                    guard let url, error == nil else { return }
                    let tempDir = FileManager.default.temporaryDirectory
                        .appendingPathComponent("jottery-import", isDirectory: true)
                    try? FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
                    let dest = tempDir.appendingPathComponent(url.lastPathComponent)
                    try? FileManager.default.removeItem(at: dest)
                    try? FileManager.default.copyItem(at: url, to: dest)
                    let filename = url.lastPathComponent
                    let mimeType = UTType(filenameExtension: url.pathExtension)?.preferredMIMEType ?? "image/jpeg"
                    DispatchQueue.main.async {
                        self?.onPick(dest, filename, mimeType)
                    }
                }
            } else if itemProvider.hasItemConformingToTypeIdentifier(UTType.movie.identifier) {
                itemProvider.loadFileRepresentation(forTypeIdentifier: UTType.movie.identifier) { [weak self] url, error in
                    guard let url, error == nil else { return }
                    let tempDir = FileManager.default.temporaryDirectory
                        .appendingPathComponent("jottery-import", isDirectory: true)
                    try? FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
                    let dest = tempDir.appendingPathComponent(url.lastPathComponent)
                    try? FileManager.default.removeItem(at: dest)
                    try? FileManager.default.copyItem(at: url, to: dest)
                    let filename = url.lastPathComponent
                    let mimeType = UTType(filenameExtension: url.pathExtension)?.preferredMIMEType ?? "video/mp4"
                    DispatchQueue.main.async {
                        self?.onPick(dest, filename, mimeType)
                    }
                }
            }
        }
    }
}


