import SwiftUI

@main
struct JotteryApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    @State private var appState = AppState()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(appState)
                .environment(\.locale, resolvedLocale)
        }
        .commands {
            CommandGroup(after: .newItem) {
                Button(L.noteListNewNote) {
                    guard !appState.isLocked else { return }
                    let _ = try? appState.createNote()
                }
                .keyboardShortcut("n", modifiers: .command)

                Button(L.noteListLock) {
                    appState.lock()
                }
                .keyboardShortcut("l", modifiers: .command)
            }
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

// MARK: - Quick Action Delegates

/// Captures the shortcut item on cold launch via `configurationForConnecting`.
class AppDelegate: NSObject, UIApplicationDelegate {
    static var pendingShortcutType: String?

    func application(
        _ application: UIApplication,
        configurationForConnecting connectingSceneSession: UISceneSession,
        options: UIScene.ConnectionOptions
    ) -> UISceneConfiguration {
        if let item = options.shortcutItem {
            Self.pendingShortcutType = item.type
        }
        let config = UISceneConfiguration(
            name: nil, sessionRole: connectingSceneSession.role
        )
        config.delegateClass = QuickActionSceneDelegate.self
        return config
    }
}

/// Handles quick actions when the app is already running (warm launch).
final class QuickActionSceneDelegate: UIResponder, UIWindowSceneDelegate {
    func windowScene(
        _ windowScene: UIWindowScene,
        performActionFor shortcutItem: UIApplicationShortcutItem,
        completionHandler: @escaping (Bool) -> Void
    ) {
        AppDelegate.pendingShortcutType = shortcutItem.type
        completionHandler(true)
    }
}

// MARK: - Content View

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
            pickUpPendingShortcut()
            #if DEBUG
            DemoSeedService.runIfRequested(appState: appState)
            #endif
        }
        .onChange(of: scenePhase) { _, newPhase in
            appState.handleScenePhaseChange(newPhase)
            if newPhase == .active {
                pickUpPendingShortcut()
            }
        }
        .onChange(of: appState.isLocked) { wasLocked, isLocked in
            if wasLocked && !isLocked {
                executePendingQuickAction()
            }
        }
        #if DEBUG
        .sheet(isPresented: Binding(
            get: { appState.demoShowSettings },
            set: { appState.demoShowSettings = $0 }
        )) {
            SettingsView()
        }
        #endif
    }

    /// Move a shortcut captured by the delegates into appState, or execute immediately if unlocked.
    private func pickUpPendingShortcut() {
        guard let action = AppDelegate.pendingShortcutType else { return }
        AppDelegate.pendingShortcutType = nil

        if !appState.isFirstLaunch && !appState.isLocked {
            handleQuickAction(action)
        } else {
            appState.pendingQuickAction = action
        }
    }

    /// Called when the app transitions from locked to unlocked.
    private func executePendingQuickAction() {
        guard let action = appState.pendingQuickAction else { return }
        appState.pendingQuickAction = nil
        handleQuickAction(action)
    }

    private func handleQuickAction(_ action: String) {
        switch action {
        case "com.jottery.newNote":
            let _ = try? appState.createNote()
        case "com.jottery.newCalc":
            if var note = try? appState.createNote() {
                note.syntaxLanguage = "calc"
                try? appState.saveNote(note)
            }
        case "com.jottery.search":
            appState.searchFocused = true
        default:
            break
        }
    }
}

// MARK: - Main View

struct MainView: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        NavigationSplitView {
            NoteListView()
        } detail: {
            if let noteId = appState.selectedNoteId {
                if let conflict = appState.pendingConflicts.first(where: { $0.id == noteId }) {
                    ConflictDetailView(conflict: conflict)
                } else if let note = appState.notes.first(where: { $0.id == noteId }) {
                    NoteEditorView(note: note)
                } else {
                    ContentUnavailableView(
                        L.editorNoNoteSelected,
                        systemImage: "doc.text",
                        description: Text(L.editorNoNoteSelectedDescription)
                    )
                }
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
