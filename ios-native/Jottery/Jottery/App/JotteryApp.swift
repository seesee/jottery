import SwiftUI

@main
struct JotteryApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    @State private var appState: AppState
    @State private var privacyCover: PrivacyCoverController

    init() {
        let state = AppState()
        _appState = State(initialValue: state)
        _privacyCover = State(initialValue: PrivacyCoverController(appState: state))
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(appState)
        }
        .commands {
            JotteryCommands(appState: appState)
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

// MARK: - Launch

/// Shown until the database is open. Usually gone before the progress appears, so
/// that a normal launch doesn't flash a spinner.
struct LaunchView: View {
    @State private var isTakingAWhile = false

    var body: some View {
        ZStack {
            Color(.systemBackground).ignoresSafeArea()
            if isTakingAWhile {
                ProgressView(L.launchOpening)
            }
        }
        .task {
            try? await Task.sleep(for: .milliseconds(600))
            isTakingAWhile = true
        }
    }
}

// MARK: - Content View

struct ContentView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        Group {
            if !appState.isDatabaseReady {
                LaunchView()
            } else if appState.isFirstLaunch {
                SetupScreen()
            } else if appState.isLocked {
                UnlockScreen()
            } else {
                MainView()
            }
        }
        .animation(.default, value: appState.isLocked)
        .animation(.default, value: appState.isFirstLaunch)
        .preferredColorScheme(appState.settings.themeMode.colorScheme)
        // Nothing can be saved until this is done, so nothing can be touched.
        .overlay {
            if appState.isReKeying {
                ZStack {
                    Color.clear.background(.ultraThinMaterial)
                    ProgressView(L.vaultReKeying)
                }
                .ignoresSafeArea()
            }
        }
        .task {
            await appState.initialise()
            pickUpPendingShortcut()
            #if DEBUG
            DemoSeedService.runIfRequested(appState: appState)
            runDemoUnlockIfRequested()
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

    #if DEBUG
    /// Perf-measurement harness: drives `AppState.unlock(password:)` directly
    /// with the demo password, since UI tests can't type into `SecureField`.
    /// Only fires when a vault already exists (i.e. after a prior `-demo-seed`
    /// launch locked the app) and logs stage timings via `Log.debug` inside
    /// `unlock()` itself. Temporary measurement tool for the unlock-performance
    /// work — see `.superpowers/sdd/task-2-report.md`.
    private func runDemoUnlockIfRequested() {
        guard ProcessInfo.processInfo.arguments.contains("-demo-unlock") else { return }
        guard !appState.isFirstLaunch else {
            Log.debug("[DemoUnlock] no vault present — skipping")
            return
        }
        Task {
            do {
                try await appState.unlock(password: DemoSeedService.demoPassword)
                Log.debug("[DemoUnlock] ✓ unlocked")
            } catch {
                Log.debug("[DemoUnlock] ✗ FAILED: \(error)")
            }
        }
    }
    #endif

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
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    @State private var accountKeyPassword = ""
    @State private var accountKeyPasswordWasWrong = false
    @State private var columnVisibility: NavigationSplitViewVisibility = .automatic
    /// Where a foldable display's fold is, if it runs top to bottom (see `FoldLayout`).
    @State private var foldGeometry = FoldGeometry()

    /// The list column width that puts the divider on the fold, when there is one.
    private var foldListWidth: CGFloat? {
        foldGeometry.fold.flatMap { FoldLayout.listWidth(fold: $0, containerWidth: foldGeometry.width) }
    }

    var body: some View {
        @Bindable var state = appState

        NavigationSplitView(columnVisibility: $columnVisibility) {
            NoteListView(showsTagsInList: horizontalSizeClass == .regular)
                .modifier(FoldAlignedColumn(width: foldListWidth))
        } detail: {
            if let noteId = appState.selectedNoteId {
                if let conflict = appState.pendingConflicts.first(where: { $0.id == noteId }) {
                    ConflictDetailView(conflict: conflict)
                } else if let note = appState.displayedNote(id: noteId) {
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
        .onChange(of: appState.listRevealRequests) { _, _ in
            if horizontalSizeClass == .compact {
                appState.selectedNoteId = nil
            } else {
                columnVisibility = .all
            }
        }
        .sheet(item: $state.presentedSheet) { AppSheetView(sheet: $0) }
        // iPhone Duo: with a fold running top to bottom, show the list and the
        // note side by side, one either side of the fold, like a book's pages.
        .onGeometryChange(for: FoldGeometry.self) { proxy in
            FoldGeometry(fold: FoldLayout.verticalFold(in: proxy), width: proxy.size.width)
        } action: { geometry in
            foldGeometry = geometry
        }
        .onChange(of: foldListWidth != nil) { _, splitOnFold in
            // Only when the fold appears or goes, so a sidebar the user hid
            // elsewhere (iPad) isn't forced back on every resize.
            columnVisibility = splitOnFold ? .all : .automatic
        }
        .overlay {
            ToastView()
        }
        // After a Face ID unlock, the account may be missing its server key,
        // which needs the notes password to publish (jottery-x40w).
        .alert(L.accountKeyPromptTitle, isPresented: Binding(
            get: { appState.needsNotesPasswordForAccountKey },
            set: { appState.needsNotesPasswordForAccountKey = $0 }
        )) {
            SecureField(L.syncSetupNotesPassword, text: $accountKeyPassword)
            Button(L.accountKeyPromptPublish) { publishAccountKey() }
            Button(L.accountKeyPromptNotNow, role: .cancel) {
                accountKeyPassword = ""
                accountKeyPasswordWasWrong = false
            }
        } message: {
            Text(accountKeyPasswordWasWrong ? L.accountKeyPromptWrongPassword : L.accountKeyPromptMessage)
        }
        // A legacy vault found a different key on the account. Moving onto it
        // uploads everything under that key, so it never happens unasked.
        .alert(L.syncMergeTitle, isPresented: Binding(
            get: { appState.pendingAccountKeyMerge != nil },
            set: { if !$0 { appState.pendingAccountKeyMerge = nil } }
        ), presenting: appState.pendingAccountKeyMerge) { pending in
            Button(L.syncMergeConfirm, role: .destructive) {
                Task { await appState.mergeOntoAccountKey(pending) }
            }
            Button(L.commonCancel, role: .cancel) {}
        } message: { _ in
            Text(L.syncMergeMessage(
                server: SyncEndpoint.displayHost(appState.settings.syncEndpoint ?? ""),
                noteCount: appState.notes.count + appState.archivedNotes.count
            ))
        }
        // The server can't prove a deletion came from one of the user's devices.
        // The notes are safe in the bin, and a real clear-out wouldn't get here.
        .alert(L.syncServerRemovedTitle, isPresented: Binding(
            get: { !appState.notesRemovedByServer.isEmpty },
            set: { if !$0 { appState.notesRemovedByServer = [] } }
        )) {
            Button(L.syncServerRemovedRestore) {
                Task { await appState.restoreNotesRemovedByServer() }
            }
            Button(L.syncServerRemovedKeep, role: .cancel) {}
        } message: {
            Text(L.syncServerRemovedMessage(appState.notesRemovedByServer.count))
        }
        .alert(L.unlockEnableFaceIdTitle, isPresented: Binding(
            get: { appState.offersFaceID },
            set: { appState.offersFaceID = $0 }
        )) {
            Button(L.unlockEnableFaceIdAction) {
                do {
                    try appState.keyManager.enableBiometricUnlock()
                } catch {
                    appState.reportError(error.localizedDescription)
                }
            }
            Button(L.unlockEnableFaceIdLater, role: .cancel) {
                appState.declineFaceID()
            }
        } message: {
            Text(L.unlockEnableFaceIdMessage)
        }
        .alert(accountKeyNoticeTitle, isPresented: Binding(
            get: { appState.accountKeyNotice != nil },
            set: { if !$0 { appState.accountKeyNotice = nil } }
        ), presenting: appState.accountKeyNotice) { _ in
            Button(L.commonOK, role: .cancel) {}
        } message: { notice in
            switch notice {
            case .passwordChangedElsewhere: Text(L.accountKeyPasswordChangedElsewhereMessage)
            case .passwordMatchesAccount: Text(L.accountKeyPasswordMatchesAccountMessage)
            }
        }
    }

    private var accountKeyNoticeTitle: String {
        switch appState.accountKeyNotice {
        case .passwordMatchesAccount: L.accountKeyPasswordMatchesAccountTitle
        default: L.accountKeyPasswordChangedElsewhereTitle
        }
    }

    private func publishAccountKey() {
        let password = accountKeyPassword
        accountKeyPassword = ""
        Task {
            switch await appState.publishMissingAccountKey(password: password) {
            case .published:
                accountKeyPasswordWasWrong = false
            case .wrongPassword:
                // Ask again, saying why — unless the app locked meanwhile.
                accountKeyPasswordWasWrong = true
                if !appState.isLocked {
                    appState.needsNotesPasswordForAccountKey = true
                }
            case .failed:
                accountKeyPasswordWasWrong = false
                appState.reportError(L.accountKeyPublishFailed)
            }
        }
    }
}
