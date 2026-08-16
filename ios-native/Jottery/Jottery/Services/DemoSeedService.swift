import Foundation

#if DEBUG
/// Configuration parsed from launch arguments for demo screenshot seeding.
/// See docs/superpowers/specs/2026-07-14-appstore-screenshots-design.md.
struct DemoSeedConfig: Equatable {
    enum Screen: Equatable {
        case list
        case note(titleContains: String)
        case search(query: String)
        case sync
        case lock
    }

    var notesPath: String
    var theme: String
    var screen: Screen

    static func parse(arguments: [String], environment: [String: String]) -> DemoSeedConfig? {
        guard arguments.contains("-demo-seed"),
              let path = environment["DEMO_NOTES_PATH"], !path.isEmpty else {
            return nil
        }

        var theme = "dark"
        if let i = arguments.firstIndex(of: "-demo-theme"), i + 1 < arguments.count {
            theme = arguments[i + 1]
        }

        var screen: Screen = .list
        if let i = arguments.firstIndex(of: "-demo-screen"), i + 1 < arguments.count {
            let raw = arguments[i + 1]
            switch raw {
            case "list": screen = .list
            case "sync": screen = .sync
            case "lock": screen = .lock
            default:
                if raw.hasPrefix("note:") {
                    screen = .note(titleContains: String(raw.dropFirst("note:".count)))
                } else if raw.hasPrefix("search:") {
                    screen = .search(query: String(raw.dropFirst("search:".count)))
                }
            }
        }

        return DemoSeedConfig(notesPath: path, theme: theme, screen: screen)
    }
}

enum DemoSeedError: Error {
    case appNotReady
}

/// Seeds the app with demo data for App Store screenshot capture.
/// Runs only in DEBUG builds when launched with `-demo-seed`.
@MainActor
enum DemoSeedService {
    static let demoPassword = "demo-pass-2026"

    static func runIfRequested(appState: AppState) {
        guard let config = DemoSeedConfig.parse(
            arguments: ProcessInfo.processInfo.arguments,
            environment: ProcessInfo.processInfo.environment
        ) else { return }

        // run() is async now that loadNotes() decrypts off-main (jottery-bzar);
        // the capture pipeline's ~6s settle before each screenshot absorbs the
        // extra hop back onto the MainActor here.
        Task {
            do {
                try await run(config: config, appState: appState)
                Log.debug("[DemoSeed] ✓ Seeded — screen: \(config.screen), theme: \(config.theme)")
            } catch {
                Log.debug("[DemoSeed] ✗ FAILED: \(error)")
            }
        }
    }

    static func run(config: DemoSeedConfig, appState: AppState) async throws {
        // 1. Fresh vault with a known password
        appState.wipeAllData()
        try await appState.createVault(password: demoPassword)

        // 2. Import the demo pack from the host filesystem
        guard let noteRepo = appState.noteRepo,
              let attachmentRepo = appState.attachmentRepo,
              let key = appState.keyManager.masterKey else {
            throw DemoSeedError.appNotReady
        }
        let rawData = try Data(contentsOf: URL(fileURLWithPath: config.notesPath))
        // The demo pack follows the documented cross-platform export format
        // (see CLAUDE.md), which omits fields like `archived`/`locked` for
        // notes that don't need them. ExportNote.init(from:) defaults every
        // genuinely-optional field, so this decodes directly.
        let export = try ImportService.parse(rawData)
        _ = try ImportService.importNotes(
            export, strategy: .replace,
            noteRepo: noteRepo, attachmentRepo: attachmentRepo, key: key
        )

        // 3. One calc note — the demo pack has none, and frame 7 needs one
        guard var calc = try appState.createNote() else {
            throw DemoSeedError.appNotReady
        }
        calc.content = """
        # Holiday budget
        flights = 420
        hotel = 380 * 4
        car_hire = 190
        total = flights + hotel + car_hire
        """
        calc.syntaxLanguage = "calc"
        calc.tags = ["budget", "travel"]
        try appState.saveNote(calc)

        // Backdate so the list shows a relative date consistent with the
        // imported demo notes ("5 mo ago") instead of "in 0 sec". `saveNote`
        // (via `NoteRepository.update`) always stamps `modified_at` with
        // `Date()`, so patch the raw record directly afterwards — same
        // approach `ImportService` uses to control timestamps precisely.
        if var calcRecord = try noteRepo.getRaw(id: calc.id) {
            let backdated = Date().addingTimeInterval(-60 * 60 * 24 * 60).iso8601
            calcRecord.createdAt = backdated
            calcRecord.modifiedAt = backdated
            try noteRepo.updateRaw(calcRecord)
        }

        try await appState.loadNotes()

        // `createNote()` above auto-selected the calc note; clear that so
        // screens that don't pick their own selection (search, sync, lock)
        // don't inherit it.
        appState.selectedNoteId = nil

        // 4. Theme
        var settings = appState.settings
        settings.theme = config.theme
        try appState.updateSettings(settings)

        // 5. Target screen
        switch config.screen {
        case .list:
            appState.selectedNoteId = nil
        case .note(let fragment):
            appState.selectedNoteId = appState.notes.first {
                $0.title.localizedCaseInsensitiveContains(fragment)
            }?.id
        case .search(let query):
            appState.searchQuery = query
            // Setting `searchQuery` alone doesn't recompute `filteredNotes` —
            // the UI normally triggers that via `.onChange(of: searchQuery)`,
            // which never fires here since we set it before the view renders.
            appState.scheduleSearch()
            // Setting `searchQuery` alone also doesn't present the search UI —
            // `NoteListView` only reveals the search field when `searchFocused`
            // flips true (same mechanism the `com.jottery.search` quick action
            // uses), so without this the field never becomes visible for the
            // screenshot even though the query text and filtering are correct.
            appState.searchFocused = true
        case .sync:
            settings.syncEnabled = true
            settings.syncEndpoint = "https://notes.example.org"
            try appState.updateSettings(settings)
            appState.syncEnabled = true
            // Backdated so the frame shows "5 min ago" rather than "in 0 sec"
            appState.lastSyncAt = Date().addingTimeInterval(-300)
            appState.demoShowSettings = true
        case .lock:
            appState.lock()
        }
    }
}
#endif
