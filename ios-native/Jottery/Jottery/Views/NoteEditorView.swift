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
                    .onChange(of: content) { _, _ in
                        scheduleSave()
                    }
                    .onChange(of: tags) { _, _ in
                        scheduleSave()
                    }
            } else if syntaxLanguage == "outliner" {
                WebOutlinerEditorView(content: $content, fontSize: appState.editorFontSize)
                    .onChange(of: content) { _, _ in
                        scheduleSave()
                    }
                    .onChange(of: tags) { _, _ in
                        scheduleSave()
                    }
            } else {
                RunestoneEditorView(
                    text: $content,
                    syntaxLanguage: syntaxLanguage,
                    wordWrap: wordWrap,
                    isEditable: !isReadOnly,
                    fontSize: appState.editorFontSize
                )
                .onChange(of: content) { _, _ in
                    scheduleSave()
                }
                .onChange(of: tags) { _, _ in
                    scheduleSave()
                }
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
                        try? appState.removeAttachment(from: note.id, attachmentId: attachmentId)
                    }
                )
            }
        }
        .overlay {
            PinchGestureOverlay { scale in
                let newScale = pinchBaseScale * scale
                appState.editorFontScale = max(0.5, min(3.0, newScale))
            } onEnded: {
                pinchBaseScale = appState.editorFontScale
            }
        }
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
                        try? appState.togglePin(id: note.id)
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
                    }

                    Divider()

                    // Lock toggle
                    Button {
                        try? appState.toggleLock(id: note.id)
                    } label: {
                        Label(
                            note.locked ? L.editorUnlock : L.editorLock,
                            systemImage: note.locked ? "lock.open" : "lock"
                        )
                    }

                    // Archive toggle
                    if note.archived {
                        Button {
                            try? appState.unarchiveNote(id: note.id)
                        } label: {
                            Label(L.editorUnarchive, systemImage: "tray.and.arrow.up")
                        }
                    } else {
                        Button {
                            try? appState.archiveNote(id: note.id)
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
                        try? appState.duplicateNote(id: note.id)
                    } label: {
                        Label(L.editorDuplicate, systemImage: "doc.on.doc")
                    }

                    Divider()

                    // Delete
                    Button(role: .destructive) {
                        try? appState.deleteNote(id: note.id)
                    } label: {
                        Label(L.editorDelete, systemImage: "trash")
                    }
                } label: {
                    Image(systemName: "ellipsis.circle")
                }
            }
        }
        .onChange(of: note.id) { _, _ in
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
                try? appState.addAttachment(to: note.id, url: url, filename: filename, mimeType: mimeType)
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

    private func saveImmediately() {
        saveTask?.cancel()
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
        try? appState.addAttachment(to: note.id, url: url, filename: filename, mimeType: mimeType)
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

// MARK: - Pinch Gesture Overlay

/// UIKit-based pinch gesture that doesn't block scrolling or text selection.
///
/// The overlay view returns `nil` from `hitTest` so all touches pass through
/// to the editor underneath.  The `UIPinchGestureRecognizer` is installed on
/// the overlay's **superview** (a common ancestor of both the overlay and the
/// editor), so it still receives two-finger pinch events via the responder
/// chain while single-finger scrolls reach the text view unimpeded.
struct PinchGestureOverlay: UIViewRepresentable {
    var onChanged: (CGFloat) -> Void
    var onEnded: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(onChanged: onChanged, onEnded: onEnded)
    }

    func makeUIView(context: Context) -> GesturePassthroughView {
        let view = GesturePassthroughView()
        view.backgroundColor = .clear

        let pinch = UIPinchGestureRecognizer(
            target: context.coordinator,
            action: #selector(Coordinator.handlePinch(_:))
        )
        pinch.delegate = context.coordinator
        view.pinchGesture = pinch

        return view
    }

    func updateUIView(_ uiView: GesturePassthroughView, context: Context) {
        context.coordinator.onChanged = onChanged
        context.coordinator.onEnded = onEnded
    }

    class Coordinator: NSObject, UIGestureRecognizerDelegate {
        var onChanged: (CGFloat) -> Void
        var onEnded: () -> Void

        init(onChanged: @escaping (CGFloat) -> Void, onEnded: @escaping () -> Void) {
            self.onChanged = onChanged
            self.onEnded = onEnded
        }

        @objc func handlePinch(_ gesture: UIPinchGestureRecognizer) {
            switch gesture.state {
            case .changed:
                onChanged(gesture.scale)
            case .ended, .cancelled:
                onEnded()
            default:
                break
            }
        }

        func gestureRecognizer(
            _ gestureRecognizer: UIGestureRecognizer,
            shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer
        ) -> Bool {
            true
        }
    }
}

/// Passes all touches through to sibling views. The pinch gesture recognizer
/// is installed on the superview so it receives touches that hit-test to the
/// editor content rather than to this (invisible) overlay.
final class GesturePassthroughView: UIView {
    var pinchGesture: UIPinchGestureRecognizer?

    override func didMoveToSuperview() {
        super.didMoveToSuperview()
        if let pinch = pinchGesture, pinch.view == nil, let parent = superview {
            parent.addGestureRecognizer(pinch)
        }
    }

    override func willMove(toSuperview newSuperview: UIView?) {
        if let pinch = pinchGesture, let oldParent = pinch.view {
            oldParent.removeGestureRecognizer(pinch)
        }
        super.willMove(toSuperview: newSuperview)
    }

    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        nil
    }
}

