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
    /// What the editor last loaded or saved. Only a difference from this is an edit.
    @State private var baseline: NoteEditorFields
    /// Changes when the text is replaced from outside, for a fresh editor: Runestone
    /// ignores new text while it has focus, and would type over it.
    @State private var editorGeneration = 0
    /// The calc or outliner editor keeps dying on this note, so it's shown as text.
    @State private var webEditorIsUnusable = false
    @State private var saveTask: Task<Void, Never>?
    @State private var didSaveDuringSession = false
    @State private var showVersionHistory = false
    @State private var showNoteInfo = false
    @State private var showAttachmentPicker = false
    @State private var showPhotoPicker = false
    @State private var pinchBaseScale: Double = 1.0
    @State private var shareFile: ExportFile?
    @State private var editorCursor = EditorCursor()
    @State private var isDropTargeted = false

    let note: DecryptedNote

    init(note: DecryptedNote) {
        self.note = note
        _content = State(initialValue: note.content)
        _tags = State(initialValue: note.tags)
        _syntaxLanguage = State(initialValue: note.syntaxLanguage)
        _wordWrap = State(initialValue: note.wordWrap)
        _color = State(initialValue: note.color)
        _showPreview = State(initialValue: note.showPreview)
        _baseline = State(initialValue: NoteEditorFields(note: note))
    }

    /// Whether the note is read-only (locked or archived).
    private var isReadOnly: Bool {
        note.locked || note.archived
    }

    private func fallBackToPlainText() {
        webEditorIsUnusable = true
        appState.reportError(L.editorShownAsPlainText)
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
                        encryptionKey: appState.keyManager.masterKey,
                        noteTitles: appState.noteTitles(linkedFrom: content),
                        onOpenNote: { appState.openLinkedNote(id: $0) }
                    )
                } else if syntaxLanguage == "calc" && !webEditorIsUnusable {
                    WebCalcEditorView(
                        content: $content, fontSize: appState.editorFontSize, isReadOnly: isReadOnly,
                        onUnusable: fallBackToPlainText
                    )
                } else if syntaxLanguage == "outliner" && !webEditorIsUnusable {
                    WebOutlinerEditorView(
                        content: $content, fontSize: appState.editorFontSize, isReadOnly: isReadOnly,
                        onUnusable: fallBackToPlainText
                    )
                } else {
                    RunestoneEditorView(
                        text: $content,
                        syntaxLanguage: syntaxLanguage,
                        wordWrap: wordWrap,
                        isEditable: !isReadOnly,
                        fontSize: appState.editorFontSize,
                        cursor: editorCursor,
                        onPasteAttachment: pasteFromClipboard
                    )
                }
            }
            .id(editorGeneration)
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
        .overlay {
            if isDropTargeted && !isReadOnly {
                RoundedRectangle(cornerRadius: 8)
                    .strokeBorder(.tint, lineWidth: 2)
                    .padding(2)
                    .allowsHitTesting(false)
            }
        }
        .dropDestination(for: DroppedAttachment.self) { dropped, _ in
            guard !isReadOnly, !dropped.isEmpty else {
                dropped.forEach { $0.incoming.discard() }
                return false
            }
            // Whatever is selected stays: a drop isn't aimed at it, as a paste is.
            attach(dropped.map(\.incoming), insertingReferences: true, replacingSelection: false)
            return true
        } isTargeted: { isDropTargeted = $0 }
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
                                Label(appState.noteColours.displayName(for: name), systemImage: color == name ? "checkmark.circle.fill" : "circle.fill")
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
                        // clipboard are attached from here too.
                        if PasteboardService.hasAttachableContent {
                            Button(action: pasteFromClipboard) {
                                Label(L.editorPasteFromClipboard, systemImage: "doc.on.clipboard")
                            }
                        }
                    }

                    Divider()

                    // What's on screen, which may be newer than what's saved.
                    NoteShareActions(note: currentNote, shareFile: $shareFile)

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
        .onChange(of: NoteEditorFields(note: note)) { previous, incoming in
            if previous.noteId != incoming.noteId {
                // Switching notes: the @State values still hold the outgoing note's
                // edits, so flush those before loading the incoming note.
                flushOutgoingNote(id: previous.noteId)
                load(incoming)
                didSaveDuringSession = false
            } else {
                noteChanged(to: incoming)
            }
        }
        .shareTemporaryFile($shareFile)
        .focusedSceneValue(\.noteEditorCommands, editorCommands)
        .sheet(isPresented: $showVersionHistory) {
            VersionHistoryView(note: note)
        }
        .sheet(isPresented: $showNoteInfo) {
            NoteInfoView(note: note)
        }
        .fileImporter(
            isPresented: $showAttachmentPicker,
            allowedContentTypes: [.data],
            allowsMultipleSelection: true
        ) { result in
            handleFileImport(result)
        }
        .sheet(isPresented: $showPhotoPicker) {
            PhotoPickerView { attach($0) }
        }
        .onDisappear {
            saveImmediately()
            // A failed save stays staged, to be retried.
            if !hasChanges { clearStalePendingNote(id: baseline.noteId) }
            // Trigger a background sync if any save happened during this editing session
            if didSaveDuringSession && appState.syncEnabled {
                Task { await appState.triggerSync() }
            }
        }
    }

    /// For the menus and their shortcuts, which do what this view's own menu does.
    private var editorCommands: NoteEditorCommands? {
        guard !isReadOnly else { return nil }
        return NoteEditorCommands(
            isPreviewing: showPreview,
            wrapsWords: wordWrap,
            togglePreview: {
                showPreview.toggle()
                scheduleSave()
            },
            toggleWordWrap: {
                wordWrap.toggle()
                scheduleSave()
            }
        )
    }

    // MARK: - Auto-Save

    /// Debounced save — waits 1 second of inactivity before saving.
    private func scheduleSave() {
        // Defence in depth: toolbar controls that mutate note state are
        // disabled while read-only, but nothing should ever schedule (or
        // stage a pending) save for a locked/archived note.
        guard !isReadOnly else { return }
        // Loading a note from outside lands here too, but that isn't an edit.
        guard hasChanges else {
            saveTask?.cancel()
            clearStalePendingNote(id: baseline.noteId)
            return
        }
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

    /// The note with what's in the editor now.
    private var currentNote: DecryptedNote {
        fields.applied(to: note)
    }

    /// What's in the editor now.
    private var fields: NoteEditorFields {
        var fields = baseline
        fields.content = content
        fields.tags = tags
        fields.syntaxLanguage = syntaxLanguage
        fields.wordWrap = wordWrap
        fields.color = color
        fields.showPreview = showPreview
        return fields
    }

    private var hasChanges: Bool {
        fields != baseline
    }

    private func load(_ incoming: NoteEditorFields) {
        content = incoming.content
        tags = incoming.tags
        syntaxLanguage = incoming.syntaxLanguage
        wordWrap = incoming.wordWrap
        color = incoming.color
        showPreview = incoming.showPreview
        baseline = incoming
        editorGeneration += 1
    }

    /// The stored note changed while it's open: a sync, a version restore, or the
    /// editor's own save coming back.
    private func noteChanged(to incoming: NoteEditorFields) {
        switch NoteEditorFields.reconcile(editor: fields, baseline: baseline, incoming: incoming) {
        case .nothing:
            baseline = incoming
        case .adopt:
            load(incoming)
        case .keepEdits:
            try? appState.keepCurrentVersion(ofNote: incoming.noteId)
            baseline = incoming
        }
    }

    private func updatePendingNote() {
        guard hasChanges, let latest = appState.displayedNote(id: baseline.noteId) else { return }
        appState.pendingEditorNote = fields.applied(to: latest)
    }

    /// Flushes unsaved edits for the note being navigated *away from*, using the
    /// still-current (pre-reset) @State values, before the incoming note is loaded.
    ///
    /// Every early return below clears `appState.pendingEditorNote` when it
    /// still refers to the outgoing note (matched by id). That pending
    /// snapshot was staged by an earlier `scheduleSave()` while the note was
    /// still editable; if left behind it would be replayed verbatim by
    /// `AppState.flushPendingEditorNote()` on the next `lock()` —
    /// `noteRepo.update` there doesn't check `locked`/`archived`, so a stale
    /// pending note would silently overwrite a note that has since become
    /// read-only (or is gone, or was already unchanged). The save path at the
    /// bottom needs no extra clearing: `saveEditorEdit` keeps the pending note
    /// exactly when the save failed.
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

        guard hasChanges else {
            clearStalePendingNote(id: id)
            return
        }

        appState.saveEditorEdit(fields)
        // Note: `didSaveDuringSession` is deliberately not set here — the
        // caller resets it to false for the
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
        // later lock() can't replay it either. This looks the note up afresh:
        // a debounced save holds a copy of this view from when it was scheduled.
        guard let latest = appState.displayedNote(id: baseline.noteId), !latest.locked, !latest.archived else {
            clearStalePendingNote(id: baseline.noteId)
            return
        }
        guard hasChanges else { return }
        let saving = fields
        guard appState.saveEditorEdit(saving) else { return }
        baseline = saving
        didSaveDuringSession = true
    }

    private var syntaxLanguages: [String] {
        NoteType.all
    }

    // MARK: - Attachment Import

    private func handleFileImport(_ result: Result<[URL], Error>) {
        guard case .success(let urls) = result else { return }
        attach(urls.map(IncomingAttachment.init(pickedFile:)))
    }

    private func pasteFromClipboard() {
        let pasted = PasteboardService.readItems().map {
            IncomingAttachment(filename: $0.filename, mimeType: $0.mimeType, contents: .data($0.data))
        }
        attach(pasted, insertingReferences: true)
    }

    /// Add attachments to the open note and say what didn't go in. A paste or a
    /// drop also puts them in the text, as they were aimed at it. The pickers add
    /// to the list below, as on the web.
    private func attach(
        _ incoming: [IncomingAttachment],
        insertingReferences: Bool = false,
        replacingSelection: Bool = true
    ) {
        let noteId = baseline.noteId
        Task {
            let result = await appState.addAttachments(incoming, to: noteId)
            if let problem = result.problem { appState.reportError(problem) }
            // A slow file may arrive after the user has moved on to another note.
            guard insertingReferences, !result.added.isEmpty, baseline.noteId == noteId else { return }
            insertReferences(to: result.added, replacingSelection: replacingSelection)
        }
    }

    private func insertReferences(to attachments: [AttachmentRef], replacingSelection: Bool) {
        let markdown = AttachmentMarkdown.references(to: attachments)
        if editorCursor.insert(markdown, replacingSelection: replacingSelection) { return }
        // Calc and outliner notes hold data of their own, which a link would break.
        guard syntaxLanguage != "calc", syntaxLanguage != "outliner" else { return }
        // The preview has no cursor.
        content = AttachmentMarkdown.appending(markdown, to: content)
    }
}

// MARK: - Sharing a Note

/// Copy, Copy Link, Share and Print, for the editor's menu and a row's context
/// menu. Share covers saving to a file, as the system's sheet offers that.
struct NoteShareActions: View {
    @Environment(AppState.self) private var appState
    let note: DecryptedNote
    @Binding var shareFile: ExportFile?

    var body: some View {
        Button {
            UIPasteboard.general.string = note.content
        } label: {
            Label(L.editorCopy, systemImage: "doc.on.clipboard")
        }

        Button {
            UIPasteboard.general.string = NoteSharing.link(to: note.id)
        } label: {
            Label(L.editorCopyLink, systemImage: "link")
        }

        Button {
            do {
                shareFile = try NoteSharing.shareFile(for: note)
            } catch {
                appState.reportError(L.errorCouldntExport)
            }
        } label: {
            Label(L.editorShare, systemImage: "square.and.arrow.up")
        }

        Button {
            NotePrinter.print(
                note,
                noteTitles: appState.noteTitles(linkedFrom: note.content),
                attachmentRepo: appState.attachmentRepo,
                key: appState.keyManager.masterKey
            )
        } label: {
            Label(L.editorPrint, systemImage: "printer")
        }
    }
}

// MARK: - Photo Picker

import PhotosUI

struct PhotoPickerView: UIViewControllerRepresentable {
    let onPick: ([IncomingAttachment]) -> Void

    /// As many as the user likes, of what can be attached.
    static var configuration: PHPickerConfiguration {
        var config = PHPickerConfiguration()
        config.selectionLimit = 0
        config.filter = .any(of: [.images, .videos])
        return config
    }

    func makeUIViewController(context: Context) -> PHPickerViewController {
        let picker = PHPickerViewController(configuration: Self.configuration)
        picker.delegate = context.coordinator
        return picker
    }

    func updateUIViewController(_ uiViewController: PHPickerViewController, context: Context) {}

    func makeCoordinator() -> Coordinator {
        Coordinator(onPick: onPick)
    }

    class Coordinator: NSObject, PHPickerViewControllerDelegate {
        let onPick: ([IncomingAttachment]) -> Void

        init(onPick: @escaping ([IncomingAttachment]) -> Void) {
            self.onPick = onPick
        }

        func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
            picker.dismiss(animated: true)
            guard !results.isEmpty else { return }

            let providers = results.map(\.itemProvider)
            Task {
                // One at a time, to keep them in the order they were picked.
                var incoming: [IncomingAttachment] = []
                for provider in providers {
                    incoming.append(await Self.load(provider))
                }
                onPick(incoming)
            }
        }

        static func load(_ provider: NSItemProvider, files: TemporaryFiles = .shared) async -> IncomingAttachment {
            if provider.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
                return await load(provider, type: .image, fallbackMimeType: "image/jpeg", files: files)
            }
            if provider.hasItemConformingToTypeIdentifier(UTType.movie.identifier) {
                return await load(provider, type: .movie, fallbackMimeType: "video/mp4", files: files)
            }
            return unreadable(named: provider.suggestedName)
        }

        /// The file only exists during the callback, so it's copied somewhere the
        /// attachment can be read from.
        private static func load(
            _ provider: NSItemProvider,
            type: UTType,
            fallbackMimeType: String,
            files: TemporaryFiles
        ) async -> IncomingAttachment {
            let suggestedName = provider.suggestedName
            return await withCheckedContinuation { continuation in
                provider.loadFileRepresentation(forTypeIdentifier: type.identifier) { url, error in
                    guard let url, error == nil else {
                        continuation.resume(returning: unreadable(named: suggestedName))
                        return
                    }
                    let filename = url.lastPathComponent
                    continuation.resume(returning: .copying(
                        url,
                        filename: filename,
                        mimeType: IncomingAttachment.mimeType(forFilename: filename, fallback: fallbackMimeType),
                        files: files
                    ))
                }
            }
        }

        private static func unreadable(named name: String?) -> IncomingAttachment {
            IncomingAttachment(
                filename: name ?? AttachmentFilename.fallback,
                mimeType: "application/octet-stream",
                contents: .refused(.unreadable)
            )
        }
    }
}
