import Foundation
import UserNotifications

/// How often to be reminded to make a backup.
enum BackupReminderFrequency: Int, CaseIterable, Identifiable {
    case off = 0
    case daily = 1
    case weekly = 7
    case monthly = 30

    var id: Int { rawValue }

    var displayName: String {
        switch self {
        case .off: return L.backupReminderOff
        case .daily: return L.backupReminderDaily
        case .weekly: return L.backupReminderWeekly
        case .monthly: return L.backupReminderMonthly
        }
    }
}

/// What a reminder needs from the system. Replaced in tests.
protocol BackupReminderCentre: Sendable {
    /// Ask to show notifications, which prompts the first time. True if allowed.
    func requestPermission() async -> Bool
    /// Whether notifications are allowed now. Never prompts.
    func isAllowed() async -> Bool
    /// Show this at `date`, in place of any reminder already waiting.
    func schedule(title: String, body: String, at date: Date) async throws
    /// Remove any reminder waiting. Async, because it can take as long as a system
    /// service does to answer, and that mustn't be on the main thread.
    func cancel() async
}

enum BackupReminder {

    /// The one reminder that can be waiting. A new one replaces it.
    static let identifier = "org.jottery.backup-reminder"

    /// The next reminder after `now`, counting whole intervals from `anchor`.
    /// While a backup is overdue it stays on those dates, so opening the app
    /// doesn't bring it round any sooner.
    static func nextDate(anchor: Date, everyDays days: Int, after now: Date) -> Date {
        let interval = TimeInterval(days) * 24 * 60 * 60
        let elapsed = now.timeIntervalSince(anchor)
        let periods = max(1, (elapsed / interval).rounded(.down) + 1)
        return anchor.addingTimeInterval(periods * interval)
    }
}

/// The real thing. A reminder is a fixed sentence: it shows on the lock screen,
/// so nothing from the vault goes in it, and it's built by `AppState`.
struct SystemBackupReminderCentre: BackupReminderCentre {

    func requestPermission() async -> Bool {
        (try? await UNUserNotificationCenter.current().requestAuthorization(options: [.alert])) ?? false
    }

    func isAllowed() async -> Bool {
        let status = await UNUserNotificationCenter.current().notificationSettings().authorizationStatus
        return status == .authorized || status == .provisional
    }

    func schedule(title: String, body: String, at date: Date) async throws {
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        let trigger = UNTimeIntervalNotificationTrigger(
            timeInterval: max(date.timeIntervalSinceNow, 1),
            repeats: false
        )
        try await UNUserNotificationCenter.current().add(
            UNNotificationRequest(identifier: BackupReminder.identifier, content: content, trigger: trigger)
        )
    }

    /// This call waits on a system service, which can take minutes if that's
    /// busy, so it runs on a thread of its own, not the caller's.
    func cancel() async {
        await withCheckedContinuation { (done: CheckedContinuation<Void, Never>) in
            DispatchQueue.global(qos: .utility).async {
                UNUserNotificationCenter.current().removePendingNotificationRequests(
                    withIdentifiers: [BackupReminder.identifier]
                )
                done.resume()
            }
        }
    }
}
