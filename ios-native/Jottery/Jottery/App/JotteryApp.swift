import SwiftUI

@main
struct JotteryApp: App {
    @State private var appState = AppState()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(appState)
        }
    }
}

struct ContentView: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        Group {
            if appState.isFirstLaunch {
                SetupScreen()
            } else if appState.isLocked {
                UnlockScreen()
            } else {
                MainView()
            }
        }
        .animation(.default, value: appState.isLocked)
        .animation(.default, value: appState.isFirstLaunch)
    }
}

struct MainView: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        NavigationSplitView {
            NoteListView()
        } detail: {
            if let noteId = appState.selectedNoteId,
               let note = appState.notes.first(where: { $0.id == noteId }) {
                NoteEditorView(note: note)
            } else {
                ContentUnavailableView(
                    "No Note Selected",
                    systemImage: "doc.text",
                    description: Text("Select a note from the list or create a new one.")
                )
            }
        }
    }
}
