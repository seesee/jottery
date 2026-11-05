import SwiftUI

/// A screen opened from the toolbar or a keyboard command. It's presented from the
/// root, because on a phone the list isn't on screen while a note is open.
enum AppSheet: Identifiable {
    case settings
    case recycleBin
    case noteInfo(DecryptedNote)
    case versionHistory(DecryptedNote)

    var id: String {
        switch self {
        case .settings: "settings"
        case .recycleBin: "recycleBin"
        case .noteInfo(let note): "noteInfo-\(note.id)"
        case .versionHistory(let note): "versionHistory-\(note.id)"
        }
    }
}

struct AppSheetView: View {
    let sheet: AppSheet

    var body: some View {
        switch sheet {
        case .settings: SettingsView()
        case .recycleBin: RecycleBinView()
        case .noteInfo(let note): NoteInfoView(note: note)
        case .versionHistory(let note): VersionHistoryView(note: note)
        }
    }
}
