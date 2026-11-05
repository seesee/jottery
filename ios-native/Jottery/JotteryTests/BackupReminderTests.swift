import Foundation
import GRDB
import Testing

@testable import Jottery

/// Stands in for the notification centre.
private final class FakeCentre: BackupReminderCentre, @unchecked Sendable {
    struct Scheduled: Equatable {
        let title: String
        let body: String
        let date: Date
    }

    private let lock = NSLock()
    private var _allowed: Bool
    private var _permissionRequests = 0
    private var _scheduled: [Scheduled] = []
    private var _cancellations = 0

    init(allowed: Bool = true) {
        _allowed = allowed
    }

    var permissionRequests: Int { lock.withLock { _permissionRequests } }
    var scheduled: [Scheduled] { lock.withLock { _scheduled } }
    var cancellations: Int { lock.withLock { _cancellations } }

    func requestPermission() async -> Bool {
        lock.withLock {
            _permissionRequests += 1
            return _allowed
        }
    }

    func isAllowed() async -> Bool {
        lock.withLock { _allowed }
    }

    func schedule(title: String, body: String, at date: Date) async throws {
        lock.withLock { _scheduled.append(Scheduled(title: title, body: body, date: date)) }
    }

    func cancel() {
        lock.withLock { _cancellations += 1 }
    }
}

/// A notification service that doesn't answer until the test says so.
private struct SilentCentre: BackupReminderCentre {
    let gate: AsyncGate

    func requestPermission() async -> Bool { false }
    func isAllowed() async -> Bool { false }
    func schedule(title: String, body: String, at date: Date) async throws {}
    func cancel() async { await gate.wait() }
}

private let day: TimeInterval = 24 * 60 * 60

/// Removing a reminder is a synchronous call to a system service. It once ran on the
/// main thread, where a busy service froze the app for five minutes.
@MainActor
struct BackupReminderBlockingTests {

    @Test func aWipeDoesntWaitForTheNotificationService() async throws {
        let gate = AsyncGate()
        let state = try TestVaults.makeState()
        state.backupReminders = SilentCentre(gate: gate)
        let inbox = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)

        state.wipeAllData(sharedInboxRoot: inbox)

        #expect(state.isFirstLaunch, "the wipe finished with the service still silent")
        await waitUntil { await gate.waitingCount == 1 }
        #expect(await gate.waitingCount == 1, "the reminder was still asked to go")
        await gate.open()
    }
}

/// The last backup date and the optional reminder (R2-143).
@MainActor
struct BackupReminderTests {

    private func makeVault(centre: FakeCentre) async throws -> AppState {
        let state = try TestVaults.makeState()
        state.backupReminders = centre
        try await state.createVault(password: "backup-reminder-\(UUID().uuidString)")
        return state
    }

    // MARK: - When it's due

    @Test func theFirstReminderIsOneIntervalAfterTheAnchor() {
        let anchor = Date(timeIntervalSince1970: 1_000_000)
        let next = BackupReminder.nextDate(anchor: anchor, everyDays: 7, after: anchor.addingTimeInterval(day))
        #expect(next == anchor.addingTimeInterval(7 * day))
    }

    /// Opening the app every day while overdue mustn't nag every day.
    @Test func anOverdueReminderKeepsToTheInterval() {
        let anchor = Date(timeIntervalSince1970: 1_000_000)
        let next = BackupReminder.nextDate(anchor: anchor, everyDays: 7, after: anchor.addingTimeInterval(9 * day))
        #expect(next == anchor.addingTimeInterval(14 * day))
    }

    @Test func aReminderExactlyDueMovesToTheNextInterval() {
        let anchor = Date(timeIntervalSince1970: 1_000_000)
        let now = anchor.addingTimeInterval(7 * day)
        #expect(BackupReminder.nextDate(anchor: anchor, everyDays: 7, after: now) > now)
    }

    @Test func anAnchorInTheFutureStillGivesALaterDate() {
        let now = Date(timeIntervalSince1970: 1_000_000)
        let next = BackupReminder.nextDate(anchor: now.addingTimeInterval(30 * day), everyDays: 7, after: now)
        #expect(next > now)
    }

    // MARK: - Permission

    @Test func nothingAsksForPermissionUntilTheReminderIsTurnedOn() async throws {
        let centre = FakeCentre()
        let state = try await makeVault(centre: centre)

        state.recordBackupMade()
        await state.refreshBackupReminder()
        state.handleScenePhaseChange(.active)

        #expect(centre.permissionRequests == 0)
        #expect(centre.scheduled.isEmpty)
    }

    @Test func turningItOnAsksOnceAndSchedules() async throws {
        let centre = FakeCentre()
        let state = try await makeVault(centre: centre)

        let allowed = await state.setBackupReminder(.weekly)

        #expect(allowed)
        #expect(centre.permissionRequests == 1)
        #expect(centre.scheduled.count == 1)
        #expect(state.backupReminderFrequency == .weekly)
        let date = try #require(centre.scheduled.first?.date)
        #expect(date > Date())
    }

    @Test func refusedPermissionLeavesItOff() async throws {
        let centre = FakeCentre(allowed: false)
        let state = try await makeVault(centre: centre)

        let allowed = await state.setBackupReminder(.weekly)

        #expect(!allowed)
        #expect(centre.scheduled.isEmpty)
        #expect(state.backupReminderFrequency == .off)
        #expect(try state.settingsRepo?.get().backupReminderDays == nil)
    }

    @Test func turningItOffCancelsAndAsksNothing() async throws {
        let centre = FakeCentre()
        let state = try await makeVault(centre: centre)
        await state.setBackupReminder(.monthly)

        await state.setBackupReminder(.off)

        #expect(centre.cancellations == 1)
        #expect(centre.permissionRequests == 1)
        #expect(state.backupReminderFrequency == .off)
    }

    // MARK: - What it says

    /// It shows on the lock screen, so it says the same thing whatever's in the vault.
    @Test func theReminderHoldsNoNoteContent() async throws {
        let centre = FakeCentre()
        let state = try await makeVault(centre: centre)
        let secret = "secret-\(UUID().uuidString)"
        _ = try state.createNote(content: secret, tags: [secret])

        await state.setBackupReminder(.daily)

        let scheduled = try #require(centre.scheduled.first)
        #expect(scheduled.title == L.backupReminderTitle)
        #expect(scheduled.body == L.backupReminderBody)
        #expect(!scheduled.title.contains(secret))
        #expect(!scheduled.body.contains(secret))
    }

    // MARK: - After a backup

    @Test func aBackupIsRememberedAcrossLaunches() async throws {
        let centre = FakeCentre()
        let state = try await makeVault(centre: centre)
        let made = Date(timeIntervalSince1970: 1_700_000_000)

        state.recordBackupMade(at: made)

        let stored = try #require(try state.settingsRepo?.get())
        #expect(stored.lastBackupDate == made)
        #expect(state.settings.lastBackupDate == made)
    }

    @Test func aBackupMovesTheReminderOn() async throws {
        let centre = FakeCentre()
        let state = try await makeVault(centre: centre)
        await state.setBackupReminder(.weekly)
        let first = try #require(centre.scheduled.last?.date)

        state.recordBackupMade(at: first.addingTimeInterval(-day))
        await waitUntil { centre.scheduled.count == 2 }

        let second = try #require(centre.scheduled.last?.date)
        #expect(second == first.addingTimeInterval(6 * day))
    }

    /// The reminder is one-shot, so coming back to the app arms the next one.
    @Test func comingBackToTheAppArmsTheNextReminder() async throws {
        let centre = FakeCentre()
        let state = try await makeVault(centre: centre)
        await state.setBackupReminder(.weekly)

        state.handleScenePhaseChange(.active)

        await waitUntil { centre.scheduled.count == 2 }
        #expect(centre.scheduled.count == 2)
    }

    /// With no backup to count from, it counts from the day it was turned on,
    /// and keeps doing so however often the app is opened.
    @Test func withNoBackupItCountsFromWhenItWasTurnedOn() async throws {
        let centre = FakeCentre()
        let state = try await makeVault(centre: centre)
        await state.setBackupReminder(.weekly)
        let first = try #require(centre.scheduled.last?.date)

        await state.refreshBackupReminder(now: Date().addingTimeInterval(3 * day))

        #expect(centre.scheduled.last?.date == first)
    }

    // MARK: - Stored data

    /// Rows saved by a build that had never heard of these still load.
    @Test func settingsFromBeforeRemindersStillDecode() throws {
        let older = """
        {"id":1,"language":"system","theme":"auto","sort_order":"recent",
         "auto_lock_timeout":15,"sync_enabled":false}
        """
        let settings = try JSONDecoder().decode(UserSettings.self, from: Data(older.utf8))
        #expect(settings.lastBackupAt == nil)
        #expect(settings.backupReminderDays == nil)
    }

    /// A date only this device wrote, but a bad one must not stop the screen.
    @Test func anUnreadableStoredDateIsNoDate() {
        var settings = UserSettings.defaults
        settings.lastBackupAt = "not a date"
        #expect(settings.lastBackupDate == nil)
    }

    @Test func aFrequencyFromSomewhereElseCountsAsOff() async throws {
        let centre = FakeCentre()
        let state = try await makeVault(centre: centre)
        var settings = state.settings
        settings.backupReminderDays = 13
        try state.updateSettings(settings)

        #expect(state.backupReminderFrequency == .off)
        await state.refreshBackupReminder()
        #expect(centre.scheduled.isEmpty)
    }
}
