import SwiftUI

/// A row's context menu, which is a long press on a phone and a secondary click
/// with a pointer. While notes are being selected only the actions that leave
/// them as they are stay, so the selection can't go stale underneath the user.
struct NoteRowMenu: View {
    @Environment(AppState.self) private var appState
    let note: DecryptedNote
    @Binding var shareFile: ExportFile?
    let isSelecting: Bool

    private var actions: NoteActions { NoteActions(appState: appState) }
    private var isReadOnly: Bool { note.locked || note.archived }

    var body: some View {
        if !isSelecting {
            // As with the swipe actions, archived notes can't be pinned.
            if !note.archived {
                Button {
                    actions.togglePin(note)
                } label: {
                    Label(note.pinned ? L.noteListUnpin : L.noteListPin, systemImage: note.pinned ? "pin.slash" : "pin")
                }
            }

            Button {
                actions.duplicate(note)
            } label: {
                Label(L.editorDuplicate, systemImage: "doc.on.doc")
            }

            colourMenu

            if note.archived {
                Button {
                    Task { await actions.unarchive(note) }
                } label: {
                    Label(L.noteListUnarchive, systemImage: "tray.and.arrow.up")
                }
            } else {
                Button {
                    actions.archive(note)
                } label: {
                    Label(L.noteListArchive, systemImage: "archivebox")
                }
            }

            Divider()
        }

        NoteShareActions(note: note, shareFile: $shareFile)

        if !isSelecting {
            Divider()

            Button(role: .destructive) {
                Task { await actions.moveToBin(note) }
            } label: {
                Label(L.noteListDelete, systemImage: "trash")
            }
        }
    }

    private var colourMenu: some View {
        Menu {
            ForEach(Color.noteColorNames, id: \.self) { name in
                Button {
                    actions.setColour(name, on: note)
                } label: {
                    Label(appState.noteColours.displayName(for: name), systemImage: note.color == name ? "checkmark.circle.fill" : "circle.fill")
                }
            }
            Divider()
            Button {
                actions.setColour(nil, on: note)
            } label: {
                Label(L.editorCategoryNone, systemImage: note.color == nil ? "checkmark.circle" : "circle.slash")
            }
        } label: {
            Label(L.editorCategory, systemImage: "paintbrush")
        }
        .disabled(isReadOnly)
    }
}
