import SwiftUI

@main
struct JotteryApp: App {
    @State private var appState = AppState()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(appState)
                .environment(\.locale, resolvedLocale)
        }
    }

    private var resolvedLocale: Locale {
        switch appState.settings.language {
        case "en-US": return Locale(identifier: "en_US")
        case "en-GB": return Locale(identifier: "en_GB")
        default: return .current  // "system"
        }
    }
}

struct ContentView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.scenePhase) private var scenePhase

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
        .onAppear {
            appState.initialise()
        }
        .onChange(of: scenePhase) { _, newPhase in
            appState.handleScenePhaseChange(newPhase)
        }
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
                    L.editorNoNoteSelected,
                    systemImage: "doc.text",
                    description: Text(L.editorNoNoteSelectedDescription)
                )
            }
        }
        .onChange(of: appState.selectedNoteId) { _, _ in
            appState.keyManager.recordActivity()
        }
    }
}
