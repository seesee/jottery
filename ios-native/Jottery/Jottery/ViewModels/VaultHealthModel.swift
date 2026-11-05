import Foundation
import Observation

/// What the Vault Health screen shows, and the work behind its buttons. One thing
/// runs at a time, and all of it stops when the task running it is cancelled.
@MainActor @Observable
final class VaultHealthModel {
    private(set) var report: VaultHealthReport?
    private(set) var isScanning = false
    private(set) var isRepairing = false
    private(set) var isCheckingServer = false
    private(set) var isUploading = false
    private(set) var progress: VaultHealthService.Progress?
    /// By attachment id. A repaired attachment leaves the report, so what's here is what's left.
    private(set) var repairResults: [String: VaultHealthService.RepairResult] = [:]
    /// Nil until the server has been asked.
    private(set) var missingFromServer: [VaultHealthReport.HeldAttachment]?
    private(set) var uploadResults: [String: VaultHealthService.UploadResult] = [:]
    var errorMessage: String?

    var isBusy: Bool { isScanning || isRepairing || isCheckingServer || isUploading }

    func scan(appState: AppState) async {
        guard !isBusy, let noteRepo = appState.noteRepo, let attachmentRepo = appState.attachmentRepo,
              let key = appState.keyManager.masterKey else { return }
        isScanning = true
        errorMessage = nil
        defer {
            isScanning = false
            progress = nil
        }
        do {
            report = try await offMainActor {
                try VaultHealthService.scan(
                    noteRepo: noteRepo,
                    attachmentRepo: attachmentRepo,
                    key: key,
                    progress: { [weak self] update in
                        Task { @MainActor in self?.progress = update }
                    }
                )
            }
            repairResults = [:]
            uploadResults = [:]
            missingFromServer = nil
        } catch is CancellationError {
            // The screen went away, so there's nobody to tell.
        } catch {
            errorMessage = L.healthScanFailed
        }
    }

    /// Fetch every missing blob the server can supply.
    func repairMissing(appState: AppState) async {
        guard !isBusy, let client = appState.syncClient, let attachmentRepo = appState.attachmentRepo,
              let missing = report?.missingAttachments else { return }
        isRepairing = true
        defer { isRepairing = false }

        for attachment in missing {
            // Checked each time: the app can lock, and blobs must never be
            // stored under a key the vault is moving away from.
            guard !Task.isCancelled, !appState.isReKeying, let key = appState.keyManager.masterKey else { return }
            let result = await VaultHealthService.repair(
                attachment, client: client, attachmentRepo: attachmentRepo, key: key
            )
            repairResults[attachment.attachmentId] = result
            if result == .repaired {
                report?.missingAttachments.removeAll { $0.attachmentId == attachment.attachmentId }
            }
        }
    }

    /// Ask the server about every blob it ought to have.
    func checkServer(appState: AppState) async {
        guard !isBusy, let client = appState.syncClient, let held = report?.heldAttachments else { return }
        isCheckingServer = true
        errorMessage = nil
        defer {
            isCheckingServer = false
            progress = nil
        }
        do {
            missingFromServer = try await VaultHealthService.missingFromServer(
                held,
                client: client,
                progress: { [weak self] update in
                    Task { @MainActor in self?.progress = update }
                }
            )
            uploadResults = [:]
        } catch is CancellationError {
            // As for a scan.
        } catch {
            // Not the error's own text, which can be the server's words.
            errorMessage = L.healthServerCheckFailed
        }
    }

    /// Send the server what it has lost.
    func reuploadMissing(appState: AppState) async {
        guard !isBusy, let client = appState.syncClient, let attachmentRepo = appState.attachmentRepo,
              let lost = missingFromServer else { return }
        isUploading = true
        defer { isUploading = false }

        for attachment in lost {
            guard !Task.isCancelled, !appState.isReKeying, let key = appState.keyManager.masterKey else { return }
            let result = await VaultHealthService.reupload(
                attachment, client: client, attachmentRepo: attachmentRepo, key: key
            )
            uploadResults[attachment.attachmentId] = result
            if result == .uploaded {
                missingFromServer?.removeAll { $0.attachmentId == attachment.attachmentId }
            }
        }
    }

    /// Delete a note that can't be read, here and on every other device.
    func deleteUnreadableNote(id: String, appState: AppState) {
        do {
            try appState.permanentlyDeleteNote(id: id)
            report?.unreadableNotes.removeAll { $0.id == id }
            report?.missingAttachments.removeAll { $0.noteId == id }
            report?.heldAttachments.removeAll { $0.noteId == id }
            missingFromServer?.removeAll { $0.noteId == id }
        } catch {
            errorMessage = L.healthDeleteFailed
        }
    }
}
