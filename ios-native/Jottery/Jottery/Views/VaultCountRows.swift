import SwiftUI

/// What's in the vault besides the notes in use, for the About section.
struct VaultCountRows: View {
    @Environment(AppState.self) private var appState
    @State private var inBin = 0

    var body: some View {
        LabeledContent(L.aboutPinned, value: appState.notes.filter(\.pinned).count, format: .number)
        LabeledContent(L.aboutArchived, value: appState.archivedNotes.count, format: .number)
        LabeledContent(L.aboutInBin, value: inBin, format: .number)
            .task { inBin = (try? appState.noteRepo?.countInBin()) ?? 0 }
    }
}
