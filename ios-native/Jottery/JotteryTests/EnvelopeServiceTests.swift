import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// In-memory stand-in for the sync server's wrapped-key endpoint.
final class FakeKeyServerProtocol: URLProtocol, @unchecked Sendable {

    nonisolated(unsafe) static var storedBlob: String? {
        didSet { revision += 1 }
    }
    nonisolated(unsafe) static var revision = 0
    nonisolated(unsafe) static var putCount = 0
    /// False for an older server: no `updatedAt`, and any PUT overwrites.
    nonisolated(unsafe) static var supportsConditionalWrites = true
    /// What the last PUT asked for.
    nonisolated(unsafe) static var lastReplace: Bool?
    nonisolated(unsafe) static var lastIfMatch: String?
    /// Runs as the next PUT arrives: another device getting in first.
    nonisolated(unsafe) static var beforeNextPut: (() -> Void)?
    nonisolated(unsafe) static var failPuts = false
    nonisolated(unsafe) static var failGets = false
    /// First uploads to refuse although nothing's stored: a key that appeared
    /// as this device wrote, and has gone again by the next read.
    nonisolated(unsafe) static var firstUploadsToLose = 0
    /// Sent as `updatedAt` in place of the real revision.
    nonisolated(unsafe) static var plantedUpdatedAt: String?
    /// DELETE requests seen: revoking the calling device (`sync/device`).
    nonisolated(unsafe) static var deleteCount = 0

    static func reset(blob: String? = nil) {
        storedBlob = blob
        putCount = 0
        supportsConditionalWrites = true
        lastReplace = nil
        lastIfMatch = nil
        beforeNextPut = nil
        failPuts = false
        failGets = false
        firstUploadsToLose = 0
        plantedUpdatedAt = nil
        deleteCount = 0
    }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        let status: Int
        var body = Data()

        switch request.httpMethod {
        case "GET":
            if Self.failGets {
                status = 503
                body = Data("unavailable".utf8)
            } else if let blob = Self.storedBlob {
                status = 200
                body = (try? JSONEncoder().encode(WrappedKeyResponse(
                    blob: blob, kdfVersion: 1, kdfIterations: 1_000_000,
                    updatedAt: Self.plantedUpdatedAt ?? (Self.supportsConditionalWrites ? "revision-\(Self.revision)" : nil)
                ))) ?? Data()
            } else {
                status = 404
                body = Data("No wrapped key found".utf8)
            }
        case "PUT":
            Self.putCount += 1
            Self.beforeNextPut?()
            Self.beforeNextPut = nil
            let ifMatch = request.value(forHTTPHeaderField: "If-Match")
            Self.lastIfMatch = ifMatch
            if Self.failPuts {
                status = 500
                body = Data("boom".utf8)
            } else if let put = try? JSONDecoder().decode(PutWrappedKeyRequest.self, from: Self.readBody(request)) {
                Self.lastReplace = put.replace
                let refused = Self.supportsConditionalWrites && (ifMatch.map {
                    $0 != "revision-\(Self.revision)" || Self.storedBlob == nil
                } ?? (put.replace == false && Self.storedBlob != nil))
                let lost = put.replace == false && Self.firstUploadsToLose > 0
                if lost { Self.firstUploadsToLose -= 1 }
                if refused || lost {
                    status = 409
                } else {
                    Self.storedBlob = put.blob
                    status = 200
                }
            } else {
                status = 400
            }
        case "DELETE":
            Self.deleteCount += 1
            status = 200
        default:
            status = 405
        }

        let response = HTTPURLResponse(
            url: request.url!, statusCode: status, httpVersion: "HTTP/1.1", headerFields: nil
        )!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: body)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}

    private static func readBody(_ request: URLRequest) -> Data {
        if let body = request.httpBody { return body }
        guard let stream = request.httpBodyStream else { return Data() }
        stream.open()
        defer { stream.close() }
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while stream.hasBytesAvailable {
            let count = stream.read(&buffer, maxLength: buffer.count)
            if count <= 0 { break }
            data.append(buffer, count: count)
        }
        return data
    }
}

/// The server holds a copy of the master key wrapped with a key derived from a
/// password and the user ID. It must be wrapped with the notes password, which
/// never leaves the device, and never with the server password, which the
/// server sees in plaintext at login. A key the server password opens proves
/// nothing, because the server could have wrapped it, so it is never adopted.
/// Blobs older builds wrote that way are repaired by a device that has synced.
@Suite(.serialized)
@MainActor
struct EnvelopeServiceTests {

    private let userId = "b3c1f5a2-8f0e-4c55-9d52-2f6a1c0e7a11"
    private let notesPassword = "notes-password-correct-horse"
    private let serverPassword = "server-password-battery-staple"

    private func makeState() throws -> AppState {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-envelope-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let state = AppState()
        try state.initialise(database: DatabaseManager(path: dir.appendingPathComponent("test.db").path))
        return state
    }

    private func makeClient() -> SyncClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [FakeKeyServerProtocol.self]
        return SyncClient(
            endpoint: "https://sync.example.test", apiKey: "device-key", configuration: configuration
        )
    }

    private func wrap(_ masterKey: Data, with password: String) throws -> String {
        let wrappingKey = try CryptoService.deriveWrappingKey(password: password, userId: userId)
        return try CryptoService.serializeEncryptedJSON(
            CryptoService.wrapMasterKey(masterKey, with: wrappingKey)
        )
    }

    private func unwrap(_ blob: String, with password: String) -> Data? {
        guard let wrappingKey = try? CryptoService.deriveWrappingKey(password: password, userId: userId),
              let encrypted = try? CryptoService.parseEncryptedJSON(blob) else { return nil }
        return try? CryptoService.unwrapMasterKey(encrypted, with: wrappingKey)
    }

    // MARK: - First device publishes the key

    private func reconcile(
        masterKey: Data,
        hasSynced: Bool,
        hasOpened: Bool = false
    ) async throws -> EnvelopeService.WrappedKeyOutcome {
        try await EnvelopeService.reconcileWrappedKey(
            syncClient: makeClient(),
            userId: userId,
            password: notesPassword,
            masterKeyData: masterKey,
            hasSyncedWithAccount: hasSynced,
            hasOpenedServerKey: hasOpened
        )
    }

    @Test func publishUploadsTheKeyWrappedWithTheNotesPassword() async throws {
        FakeKeyServerProtocol.reset()
        let masterKey = CryptoService.generateMasterKey()

        let outcome = try await reconcile(masterKey: masterKey, hasSynced: false)

        #expect(outcome == .published)
        let blob = try #require(FakeKeyServerProtocol.storedBlob)
        #expect(unwrap(blob, with: notesPassword) == masterKey)
        #expect(unwrap(blob, with: serverPassword) == nil)
    }

    @Test func aKeyTheNotesPasswordOpensIsNeverOverwrittenEvenIfItDiffers() async throws {
        let existing = try wrap(CryptoService.generateMasterKey(), with: notesPassword)
        FakeKeyServerProtocol.reset(blob: existing)

        // A different key under the same password means this device has
        // diverged, and replacing the account's key would damage the account.
        let outcome = try await reconcile(masterKey: CryptoService.generateMasterKey(), hasSynced: true)

        #expect(outcome == .leftAlone)
        #expect(FakeKeyServerProtocol.putCount == 0)
        #expect(FakeKeyServerProtocol.storedBlob == existing)
    }

    @Test func aDeviceThatHasSyncedRepairsAKeyTheNotesPasswordCantOpen() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: serverPassword))
        let masterKey = CryptoService.generateMasterKey()

        let outcome = try await reconcile(masterKey: masterKey, hasSynced: true)

        #expect(outcome == .repaired)
        let blob = try #require(FakeKeyServerProtocol.storedBlob)
        #expect(unwrap(blob, with: notesPassword) == masterKey)
        #expect(unwrap(blob, with: serverPassword) == nil)
    }

    @Test func aDeviceThatHasNeverSyncedLeavesAKeyItCantOpen() async throws {
        let existing = try wrap(CryptoService.generateMasterKey(), with: serverPassword)
        FakeKeyServerProtocol.reset(blob: existing)

        // It can't know that it holds the account's key.
        let outcome = try await reconcile(masterKey: CryptoService.generateMasterKey(), hasSynced: false)

        #expect(outcome == .unreadable)
        #expect(FakeKeyServerProtocol.putCount == 0)
        #expect(FakeKeyServerProtocol.storedBlob == existing)
    }

    /// Without this, a device on the old password overwrites the blob after a
    /// password change elsewhere, and the two take turns to undo each other.
    @Test func aDeviceThatHasOpenedTheKeyNeverOverwritesOneItCantOpen() async throws {
        let existing = try wrap(CryptoService.generateMasterKey(), with: "changed-on-another-device")
        FakeKeyServerProtocol.reset(blob: existing)

        let outcome = try await reconcile(
            masterKey: CryptoService.generateMasterKey(), hasSynced: true, hasOpened: true
        )

        #expect(outcome == .passwordChangedElsewhere)
        #expect(FakeKeyServerProtocol.putCount == 0)
        #expect(FakeKeyServerProtocol.storedBlob == existing)
    }

    @Test func aMalformedServerBlobCountsAsUnreadable() async throws {
        FakeKeyServerProtocol.reset(blob: "not a wrapped key")
        let masterKey = CryptoService.generateMasterKey()

        #expect(try await reconcile(masterKey: masterKey, hasSynced: false) == .unreadable)
        #expect(try await reconcile(masterKey: masterKey, hasSynced: true) == .repaired)
    }

    @Test func publishReportsAnUploadFailure() async throws {
        FakeKeyServerProtocol.reset()
        FakeKeyServerProtocol.failPuts = true

        await #expect(throws: (any Error).self) {
            _ = try await reconcile(masterKey: CryptoService.generateMasterKey(), hasSynced: false)
        }
    }

    @Test func aFailedCheckNeverOverwritesTheServerKey() async throws {
        let existing = try wrap(CryptoService.generateMasterKey(), with: serverPassword)
        FakeKeyServerProtocol.reset(blob: existing)
        FakeKeyServerProtocol.failGets = true

        await #expect(throws: (any Error).self) {
            _ = try await reconcile(masterKey: CryptoService.generateMasterKey(), hasSynced: true)
        }
        #expect(FakeKeyServerProtocol.putCount == 0)
    }

    private func runEnvelopeSetup(on state: AppState, password: String) async throws -> Bool {
        await EnvelopeService.tryEnvelopeSetup(
            appState: state, password: password, masterKey: try #require(state.keyManager.masterKey)
        )
    }

    /// Two devices, one account. Device A changes the notes password, then
    /// device B unlocks with the old one. B has opened the blob before, so it
    /// knows this isn't an old-format blob, and must leave A's change alone.
    @Test func aPasswordChangeOnOneDeviceSurvivesTheOtherUnlockingWithTheOldPassword() async throws {
        let deviceA = try await makeSyncedState()
        let deviceB = try await makeSyncedState()
        let masterKey = try #require(TestVaults.keyData(deviceA))
        deviceB.keyManager.unlockWithKeyData(masterKey)
        FakeKeyServerProtocol.reset(blob: try wrap(masterKey, with: notesPassword))
        for device in [deviceA, deviceB] {
            try #require(device.syncRepo).updateLastSync(at: Date().iso8601)
            #expect(try await runEnvelopeSetup(on: device, password: notesPassword))
            #expect(try device.syncRepo?.getMetadata()?.serverKeyOpened == true)
        }

        let newPassword = "a-brand-new-notes-password"
        await EnvelopeService.changePasswordServer(
            syncRepo: try #require(deviceA.syncRepo),
            syncClient: makeClient(),
            newPassword: newPassword,
            masterKeyData: masterKey
        )
        let changed = try #require(FakeKeyServerProtocol.storedBlob)
        let putsAfterChange = FakeKeyServerProtocol.putCount

        #expect(try await runEnvelopeSetup(on: deviceB, password: notesPassword))

        #expect(FakeKeyServerProtocol.putCount == putsAfterChange)
        #expect(FakeKeyServerProtocol.storedBlob == changed)
        #expect(unwrap(changed, with: newPassword) == masterKey)
        #expect(deviceB.accountKeyNotice == .passwordChangedElsewhere)

        // Device A is unaffected, and neither device has been told anything wrong.
        #expect(try await runEnvelopeSetup(on: deviceA, password: newPassword))
        #expect(deviceA.accountKeyNotice == nil)
        #expect(FakeKeyServerProtocol.storedBlob == changed)
    }

    @Test func aDeviceHealsAnOldBlobOnlyOnce() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: serverPassword))
        let state = try await makeSyncedState()
        try #require(state.syncRepo).updateLastSync(at: Date().iso8601)

        #expect(try await runEnvelopeSetup(on: state, password: notesPassword))
        #expect(FakeKeyServerProtocol.putCount == 1)
        #expect(try state.syncRepo?.getMetadata()?.serverKeyOpened == true)

        // Another device now changes the password, and this one must leave it.
        let later = try wrap(try #require(TestVaults.keyData(state)), with: "changed-on-another-device")
        FakeKeyServerProtocol.storedBlob = later
        #expect(try await runEnvelopeSetup(on: state, password: notesPassword))

        #expect(FakeKeyServerProtocol.putCount == 1)
        #expect(FakeKeyServerProtocol.storedBlob == later)
        #expect(state.accountKeyNotice == .passwordChangedElsewhere)
    }

    /// The flag belongs to one account: another account's blob is a new blob.
    @Test func connectingAnotherAccountStartsWithNothingKnownAboutItsKey() async throws {
        let state = try await makeSyncedState()
        let syncRepo = try #require(state.syncRepo)
        try syncRepo.updateLastSync(at: Date().iso8601)
        try syncRepo.markServerKeyOpened()
        try syncRepo.savePendingWrappedKey("blob")

        try syncRepo.beginAccount(userId: UUID().uuidString)

        let meta = try #require(try syncRepo.getMetadata())
        #expect(meta.serverKeyOpened == false)
        #expect(meta.lastSyncAt == nil)
        #expect(meta.pendingWrappedKey == nil)
    }

    // MARK: - Saying what an upload is for

    /// Two devices could both read "no key", and the second would overwrite the
    /// first. An upload says it expects to be the first, and the server refuses
    /// it if it isn't.
    @Test func aFirstUploadAsksNotToReplaceAnything() async throws {
        FakeKeyServerProtocol.reset()

        _ = try await reconcile(masterKey: CryptoService.generateMasterKey(), hasSynced: false)

        #expect(FakeKeyServerProtocol.lastReplace == false)
        #expect(FakeKeyServerProtocol.lastIfMatch == nil)
    }

    @Test func aRepairNamesTheBlobItRead() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: serverPassword))
        let readRevision = "revision-\(FakeKeyServerProtocol.revision)"

        let outcome = try await reconcile(masterKey: CryptoService.generateMasterKey(), hasSynced: true)

        #expect(outcome == .repaired)
        #expect(FakeKeyServerProtocol.lastIfMatch == readRevision)
        #expect(FakeKeyServerProtocol.lastReplace == nil)
    }

    @Test func aPasswordChangeSaysToReplace() async throws {
        let state = try await makeSyncedState()
        let masterKey = try #require(TestVaults.keyData(state))
        FakeKeyServerProtocol.reset(blob: try wrap(masterKey, with: notesPassword))

        await EnvelopeService.changePasswordServer(
            syncRepo: try #require(state.syncRepo),
            syncClient: makeClient(),
            newPassword: "a-brand-new-notes-password",
            masterKeyData: masterKey
        )

        #expect(FakeKeyServerProtocol.lastReplace == true)
    }

    @Test func aFirstUploadThatLosesTheRaceLeavesTheWinnersKey() async throws {
        FakeKeyServerProtocol.reset()
        let theirs = try wrap(CryptoService.generateMasterKey(), with: notesPassword)
        FakeKeyServerProtocol.beforeNextPut = { FakeKeyServerProtocol.storedBlob = theirs }

        let outcome = try await reconcile(masterKey: CryptoService.generateMasterKey(), hasSynced: false)

        #expect(outcome == .leftAlone)
        #expect(FakeKeyServerProtocol.storedBlob == theirs)
        #expect(FakeKeyServerProtocol.putCount == 1)
    }

    /// The race D1 is about: this device reads an old blob, another changes the
    /// password, and this device's repair must not undo that. The blob that
    /// arrives still won't open here, which is exactly what makes it tempting.
    @Test func aRepairThatLosesTheRaceLeavesWhatTheOtherDeviceWrote() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: serverPassword))
        let theirs = try wrap(CryptoService.generateMasterKey(), with: "changed-on-another-device")
        FakeKeyServerProtocol.beforeNextPut = { FakeKeyServerProtocol.storedBlob = theirs }

        let outcome = try await reconcile(masterKey: CryptoService.generateMasterKey(), hasSynced: true)

        #expect(outcome == .passwordChangedElsewhere)
        #expect(FakeKeyServerProtocol.storedBlob == theirs)
        #expect(FakeKeyServerProtocol.putCount == 1)  // read again, and didn't try again
    }

    @Test func aDeviceThatLostARepairNeverTriesAgain() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: serverPassword))
        let state = try await makeSyncedState()
        try #require(state.syncRepo).updateLastSync(at: Date().iso8601)
        let theirs = try wrap(CryptoService.generateMasterKey(), with: "changed-on-another-device")
        FakeKeyServerProtocol.beforeNextPut = { FakeKeyServerProtocol.storedBlob = theirs }

        #expect(try await runEnvelopeSetup(on: state, password: notesPassword))
        #expect(state.accountKeyNotice == .passwordChangedElsewhere)
        #expect(FakeKeyServerProtocol.putCount == 1)

        #expect(try await runEnvelopeSetup(on: state, password: notesPassword))

        #expect(FakeKeyServerProtocol.putCount == 1)
        #expect(FakeKeyServerProtocol.storedBlob == theirs)
    }

    /// The refusal is the proof, so it must survive the read after it failing.
    /// Otherwise the next unlock sees an old-looking blob, with nothing saying
    /// otherwise, and repairs over the password change that caused the refusal.
    @Test func aRefusedRepairIsProofEvenIfReadingItAgainFails() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: serverPassword))
        let state = try await makeSyncedState()
        try #require(state.syncRepo).updateLastSync(at: Date().iso8601)
        let theirs = try wrap(CryptoService.generateMasterKey(), with: "changed-on-another-device")
        FakeKeyServerProtocol.beforeNextPut = {
            FakeKeyServerProtocol.storedBlob = theirs
            FakeKeyServerProtocol.failGets = true
        }

        _ = try await runEnvelopeSetup(on: state, password: notesPassword)
        #expect(FakeKeyServerProtocol.putCount == 1)

        FakeKeyServerProtocol.failGets = false
        _ = try await runEnvelopeSetup(on: state, password: notesPassword)

        #expect(FakeKeyServerProtocol.putCount == 1, "repaired over another device's password change")
        #expect(FakeKeyServerProtocol.storedBlob == theirs)
    }

    @Test func aKeyThatKeepsChangingIsLeftAloneByAFirstUpload() async throws {
        FakeKeyServerProtocol.reset()
        let theirs = try wrap(CryptoService.generateMasterKey(), with: "changed-on-another-device")
        FakeKeyServerProtocol.beforeNextPut = {
            // Appears as this device writes, and has gone again by the next read.
            FakeKeyServerProtocol.storedBlob = theirs
            FakeKeyServerProtocol.failGets = true
        }

        let outcome = try await reconcile(masterKey: CryptoService.generateMasterKey(), hasSynced: true)

        #expect(outcome == .changedUnderThisDevice)
        #expect(FakeKeyServerProtocol.putCount == 1)
    }

    /// Two tries is as far as it goes. This unlock leaves it, and the next one looks again.
    @Test func aKeyThatBeatsBothFirstUploadsIsLeftForTheNextUnlock() async throws {
        FakeKeyServerProtocol.reset()
        FakeKeyServerProtocol.firstUploadsToLose = 2

        let outcome = try await reconcile(masterKey: CryptoService.generateMasterKey(), hasSynced: true)

        #expect(outcome == .changedUnderThisDevice)
        #expect(FakeKeyServerProtocol.putCount == 2)
        #expect(FakeKeyServerProtocol.storedBlob == nil)
    }

    @Test func aLegacyVaultThatLosesARepairNeverTriesAgain() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: serverPassword))
        let state = try await makeSyncedLegacyVault()
        try #require(state.syncRepo).updateLastSync(at: Date().iso8601)
        let theirs = try wrap(CryptoService.generateMasterKey(), with: "changed-on-another-device")
        FakeKeyServerProtocol.beforeNextPut = { FakeKeyServerProtocol.storedBlob = theirs }

        #expect(try await !runEnvelopeSetup(on: state, password: notesPassword))
        #expect(state.accountKeyNotice == .passwordChangedElsewhere)
        #expect(try state.encryptionRepo?.get()?.envelopeVersion == nil)  // nothing changed here

        state.accountKeyNotice = nil
        #expect(try await runEnvelopeSetup(on: state, password: notesPassword))

        #expect(FakeKeyServerProtocol.putCount == 1)
        #expect(FakeKeyServerProtocol.storedBlob == theirs)
        #expect(state.accountKeyNotice == .passwordChangedElsewhere)
    }

    @Test func anOlderServerStillGetsRepaired() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: serverPassword))
        FakeKeyServerProtocol.supportsConditionalWrites = false
        let masterKey = CryptoService.generateMasterKey()

        let outcome = try await reconcile(masterKey: masterKey, hasSynced: true)

        #expect(outcome == .repaired)
        #expect(FakeKeyServerProtocol.lastIfMatch == nil)
        // With no revision to name it can't be conditional, so it asks not to
        // replace. One upgraded since refuses it: the safe way to be wrong.
        #expect(FakeKeyServerProtocol.lastReplace == false)
        #expect(unwrap(try #require(FakeKeyServerProtocol.storedBlob), with: notesPassword) == masterKey)
    }

    /// The server chooses this value, and it goes straight into a header. One
    /// that's unfit counts as a server with no revisions.
    @Test(arguments: ["two\r\nX-Planted: lines", "caf\u{e9}", String(repeating: "x", count: 257)])
    func aRevisionUnfitForAHeaderIsNeverSent(updatedAt: String) async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: serverPassword))
        FakeKeyServerProtocol.plantedUpdatedAt = updatedAt

        _ = try await reconcile(masterKey: CryptoService.generateMasterKey(), hasSynced: true)

        #expect(FakeKeyServerProtocol.putCount == 1)
        #expect(FakeKeyServerProtocol.lastIfMatch == nil)
        #expect(FakeKeyServerProtocol.lastReplace == false)
    }

    @Test func anOlderServerStillTakesAFirstUpload() async throws {
        FakeKeyServerProtocol.reset()
        FakeKeyServerProtocol.supportsConditionalWrites = false

        #expect(try await reconcile(masterKey: CryptoService.generateMasterKey(), hasSynced: false) == .published)
    }

    // MARK: - A password change the server hasn't heard about (jottery-rkg9)

    @Test func aFailedKeyUploadAfterAPasswordChangeIsKeptAndReported() async throws {
        let state = try await makeSyncedState()
        let masterKey = try #require(TestVaults.keyData(state))
        FakeKeyServerProtocol.reset(blob: try wrap(masterKey, with: notesPassword))
        FakeKeyServerProtocol.failPuts = true

        let uploaded = await EnvelopeService.changePasswordServer(
            syncRepo: try #require(state.syncRepo),
            syncClient: makeClient(),
            newPassword: "a-brand-new-notes-password",
            masterKeyData: masterKey
        )

        #expect(!uploaded)
        let pending = try #require(try state.syncRepo?.getMetadata()?.pendingWrappedKey)
        #expect(unwrap(pending, with: "a-brand-new-notes-password") == masterKey)
    }

    /// The server still has the old blob, which the new password can't open.
    /// That must not read as "changed on another device": this device did it.
    @Test func theNextUnlockUploadsTheKeyAPasswordChangeLeftWaiting() async throws {
        let state = try await makeSyncedState()
        let masterKey = try #require(TestVaults.keyData(state))
        let syncRepo = try #require(state.syncRepo)
        FakeKeyServerProtocol.reset(blob: try wrap(masterKey, with: notesPassword))
        try syncRepo.updateLastSync(at: Date().iso8601)
        try syncRepo.markServerKeyOpened()
        let newPassword = "a-brand-new-notes-password"
        try syncRepo.savePendingWrappedKey(try wrap(masterKey, with: newPassword))

        #expect(try await runEnvelopeSetup(on: state, password: newPassword))

        #expect(state.accountKeyNotice == nil)
        #expect(try syncRepo.getMetadata()?.pendingWrappedKey == nil)
        #expect(unwrap(try #require(FakeKeyServerProtocol.storedBlob), with: newPassword) == masterKey)
    }

    @Test func aPendingKeyThatStillCantBeUploadedStaysQueued() async throws {
        let state = try await makeSyncedState()
        let masterKey = try #require(TestVaults.keyData(state))
        let syncRepo = try #require(state.syncRepo)
        FakeKeyServerProtocol.reset(blob: try wrap(masterKey, with: notesPassword))
        FakeKeyServerProtocol.failPuts = true
        try syncRepo.markServerKeyOpened()
        try syncRepo.savePendingWrappedKey("waiting")

        #expect(try await !runEnvelopeSetup(on: state, password: notesPassword))

        #expect(try syncRepo.getMetadata()?.pendingWrappedKey == "waiting")
    }

    /// Repairing runs as part of a password unlock's envelope setup.
    @Test func envelopeSetupRepairsTheKeyOnceTheDeviceHasSynced() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: serverPassword))
        let state = try await makeSyncedState()
        let masterKey = try #require(state.keyManager.masterKey)

        #expect(await EnvelopeService.tryEnvelopeSetup(
            appState: state, password: notesPassword, masterKey: masterKey
        ) == false)
        #expect(FakeKeyServerProtocol.putCount == 0)

        try #require(state.syncRepo).updateLastSync(at: Date().iso8601)
        #expect(await EnvelopeService.tryEnvelopeSetup(
            appState: state, password: notesPassword, masterKey: masterKey
        ) == true)

        let blob = try #require(FakeKeyServerProtocol.storedBlob)
        #expect(unwrap(blob, with: notesPassword) == masterKey.withUnsafeBytes { Data($0) })
    }

    /// A vault created on this build is already in envelope format, so the old
    /// "already migrated, nothing to do" shortcut meant a first device never
    /// uploaded anything and every other device got "no wrapped key found".
    @Test func envelopeSetupOnAnEnvelopeVaultUploadsTheMissingKey() async throws {
        FakeKeyServerProtocol.reset()
        let state = try makeState()
        try await state.createVault(password: notesPassword)
        let masterKey = try #require(state.keyManager.masterKey)

        try state.settingsRepo?.updateSync(enabled: true, endpoint: "https://sync.example.test")
        state.settings.syncEnabled = true
        if var meta = try state.syncRepo?.getMetadata() {
            meta.userId = userId
            try state.syncRepo?.saveMetadata(meta)
        }
        state.syncClient = makeClient()

        let ok = await EnvelopeService.tryEnvelopeSetup(
            appState: state, password: notesPassword, masterKey: masterKey
        )

        #expect(ok == true)
        let blob = try #require(FakeKeyServerProtocol.storedBlob)
        #expect(unwrap(blob, with: notesPassword) == masterKey.withUnsafeBytes { Data($0) })
    }

    /// `sync_metadata.sync_enabled` is created as false and never written. The
    /// envelope code used to gate on it, so no iOS device ever uploaded a key.
    @Test func envelopeSetupDoesNotDependOnTheUnusedMetadataFlag() async throws {
        FakeKeyServerProtocol.reset()
        let state = try makeState()
        try await state.createVault(password: notesPassword)
        let masterKey = try #require(state.keyManager.masterKey)

        state.settings.syncEnabled = true
        var meta = try #require(try state.syncRepo?.getMetadata())
        #expect(meta.syncEnabled == false)  // the column really is left false
        meta.userId = userId
        try state.syncRepo?.saveMetadata(meta)
        state.syncClient = makeClient()

        _ = await EnvelopeService.tryEnvelopeSetup(
            appState: state, password: notesPassword, masterKey: masterKey
        )

        #expect(FakeKeyServerProtocol.storedBlob != nil)
    }

    @Test func envelopeSetupDoesNothingWhenSyncIsOff() async throws {
        FakeKeyServerProtocol.reset()
        let state = try makeState()
        try await state.createVault(password: notesPassword)
        let masterKey = try #require(state.keyManager.masterKey)

        state.settings.syncEnabled = false
        var meta = try #require(try state.syncRepo?.getMetadata())
        meta.userId = userId
        try state.syncRepo?.saveMetadata(meta)
        state.syncClient = makeClient()

        let ok = await EnvelopeService.tryEnvelopeSetup(
            appState: state, password: notesPassword, masterKey: masterKey
        )

        #expect(ok == true)
        #expect(FakeKeyServerProtocol.storedBlob == nil)
        #expect(FakeKeyServerProtocol.putCount == 0)
    }

    /// Settings sync setup runs envelope setup before the app's sync client is
    /// started, so `appState.syncClient` is still nil at that point. Setup used
    /// to skip as "sync not configured", leaving the account without a key
    /// until the next password unlock (never, for Face ID users).
    @Test func envelopeSetupUsesTheClientItIsGivenBeforeSyncHasStarted() async throws {
        FakeKeyServerProtocol.reset()
        let state = try makeState()
        try await state.createVault(password: notesPassword)
        let masterKey = try #require(state.keyManager.masterKey)

        state.settings.syncEnabled = true
        var meta = try #require(try state.syncRepo?.getMetadata())
        meta.userId = userId
        try state.syncRepo?.saveMetadata(meta)
        #expect(state.syncClient == nil)

        let ok = await EnvelopeService.tryEnvelopeSetup(
            appState: state, password: notesPassword, masterKey: masterKey, syncClient: makeClient()
        )

        #expect(ok == true)
        let blob = try #require(FakeKeyServerProtocol.storedBlob)
        #expect(unwrap(blob, with: notesPassword) == masterKey.withUnsafeBytes { Data($0) })
    }

    // MARK: - Second device downloads the key

    @Test func onboardOpensABlobWrappedWithTheNotesPassword() async throws {
        let sharedKey = CryptoService.generateMasterKey()
        FakeKeyServerProtocol.reset(blob: try wrap(sharedKey, with: notesPassword))
        let state = try makeState()
        try await state.createVault(password: notesPassword)

        let key = try await EnvelopeService.onboardFromServer(
            syncClient: makeClient(),
            encryptionRepo: try #require(state.encryptionRepo),
            password: notesPassword,
            userId: userId,
            localPassword: notesPassword
        )

        #expect(key.withUnsafeBytes { Data($0) } == sharedKey)
    }

    @Test func onboardRefusesABlobWrappedWithTheServerPassword() async throws {
        let plantedKey = CryptoService.generateMasterKey()
        FakeKeyServerProtocol.reset(blob: try wrap(plantedKey, with: serverPassword))
        let state = try makeState()
        try await state.createVault(password: notesPassword)
        let keyBefore = TestVaults.keyData(state)

        await #expect(throws: EnvelopeError.cannotUnwrapServerKey) {
            _ = try await EnvelopeService.onboardFromServer(
                syncClient: makeClient(),
                encryptionRepo: try #require(state.encryptionRepo),
                password: notesPassword,
                userId: userId,
                localPassword: notesPassword
            )
        }

        #expect(TestVaults.keyData(state) == keyBefore)
        #expect(await state.verifyVaultPassword(notesPassword) == true)
    }

    @Test func onboardFailsCleanlyWhenThePasswordDoesNotMatch() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: notesPassword))
        let state = try makeState()
        try await state.createVault(password: "some-local-vault-password")

        await #expect(throws: (any Error).self) {
            _ = try await EnvelopeService.onboardFromServer(
                syncClient: makeClient(),
                encryptionRepo: try #require(state.encryptionRepo),
                password: "wrong-notes-password",
                userId: userId,
                localPassword: "wrong-notes-password"
            )
        }

        // The local vault is untouched
        #expect(await state.verifyVaultPassword("some-local-vault-password") == true)
    }

    @Test func onboardReportsAMissingServerKeyDistinctly() async throws {
        FakeKeyServerProtocol.reset()
        let state = try makeState()
        try await state.createVault(password: notesPassword)

        await #expect(throws: EnvelopeError.self) {
            _ = try await EnvelopeService.onboardFromServer(
                syncClient: makeClient(),
                encryptionRepo: try #require(state.encryptionRepo),
                password: notesPassword,
                userId: userId
            )
        }
    }

    // MARK: - Checking the key before anything is created

    /// The connect flow must know, before it creates a vault, whether the
    /// entered password opens the account's key. Fetching and unwrapping is
    /// therefore separate from saving, and a mismatch is its own error.

    @Test func fetchServerKeyReturnsNilWhenTheServerHasNone() async throws {
        FakeKeyServerProtocol.reset()

        let key = try await EnvelopeService.fetchServerKey(
            syncClient: makeClient(), userId: userId, password: notesPassword
        )

        #expect(key == nil)
    }

    @Test func fetchServerKeyOpensABlobWrappedWithTheNotesPassword() async throws {
        let sharedKey = CryptoService.generateMasterKey()
        FakeKeyServerProtocol.reset(blob: try wrap(sharedKey, with: notesPassword))

        let key = try await EnvelopeService.fetchServerKey(
            syncClient: makeClient(), userId: userId, password: notesPassword
        )

        #expect(key == sharedKey)
    }

    @Test func fetchServerKeyRefusesABlobWrappedWithTheServerPassword() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: serverPassword))

        await #expect(throws: EnvelopeError.cannotUnwrapServerKey) {
            _ = try await EnvelopeService.fetchServerKey(
                syncClient: makeClient(), userId: userId, password: notesPassword
            )
        }
    }

    @Test func fetchServerKeyReportsAPasswordMismatchAsItsOwnError() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: notesPassword))

        await #expect(throws: EnvelopeError.cannotUnwrapServerKey) {
            _ = try await EnvelopeService.fetchServerKey(
                syncClient: makeClient(),
                userId: userId,
                password: "wrong-notes-password"
            )
        }
    }

    /// An unreachable server is not the same as "no key" or "wrong password":
    /// treating it as either would let setup carry on with a different key.
    @Test func fetchServerKeyPropagatesServerFailures() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: notesPassword))
        FakeKeyServerProtocol.failGets = true

        do {
            _ = try await EnvelopeService.fetchServerKey(
                syncClient: makeClient(), userId: userId, password: notesPassword
            )
            Issue.record("Expected a server failure to be thrown")
        } catch let error as EnvelopeError {
            Issue.record("Server failure was misreported as \(error)")
        } catch {
            // Expected: a transport / HTTP error
        }
    }

    // MARK: - A device with no vault yet connects to an account

    private func accountKeyForNewVault(notesPassword: String, accountPassword: String) async throws -> Data? {
        try await EnvelopeService.accountKeyForNewVault(
            syncClient: makeClient(),
            userId: userId,
            notesPassword: notesPassword,
            accountPassword: accountPassword
        )
    }

    @Test func aNewDeviceGetsTheKeyTheNotesPasswordOpens() async throws {
        let sharedKey = CryptoService.generateMasterKey()
        FakeKeyServerProtocol.reset(blob: try wrap(sharedKey, with: notesPassword))

        let key = try await accountKeyForNewVault(notesPassword: notesPassword, accountPassword: serverPassword)

        #expect(key == sharedKey)
    }

    /// The attack: the server has the account password from registration, and
    /// it picks the user ID, so it can wrap a key of its own with them and
    /// read everything written under it. This is the only blob it can forge.
    @Test func aNewDeviceRefusesAKeyTheServerCouldHavePlanted() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: serverPassword))

        await #expect(throws: SyncSetupError.encryptionPasswordMismatch) {
            _ = try await accountKeyForNewVault(notesPassword: notesPassword, accountPassword: serverPassword)
        }
    }

    /// Being refused would stop these users adding a device at all. The match
    /// is an existing vault's, so the app warns about it instead.
    @Test func aNewDeviceCanJoinAnAccountWhoseNotesPasswordMatchesTheAccountPassword() async throws {
        let sharedKey = CryptoService.generateMasterKey()
        FakeKeyServerProtocol.reset(blob: try wrap(sharedKey, with: serverPassword))

        let key = try await accountKeyForNewVault(notesPassword: serverPassword, accountPassword: serverPassword)

        #expect(key == sharedKey)
    }

    @Test func aNewVaultCantChooseANotesPasswordMatchingTheAccountPassword() async throws {
        FakeKeyServerProtocol.reset()

        await #expect(throws: SyncSetupError.notesPasswordMatchesAccountPassword) {
            _ = try await accountKeyForNewVault(notesPassword: serverPassword, accountPassword: serverPassword)
        }
    }

    @Test func aShortPasswordCanOpenAnAccountsExistingKey() async throws {
        let sharedKey = CryptoService.generateMasterKey()
        FakeKeyServerProtocol.reset(blob: try wrap(sharedKey, with: "short"))

        // It's the account's password already: existing passwords keep working.
        let key = try await accountKeyForNewVault(notesPassword: "short", accountPassword: serverPassword)

        #expect(key == sharedKey)
    }

    @Test func aShortPasswordCantStartANewAccountKey() async throws {
        FakeKeyServerProtocol.reset()

        await #expect(throws: SyncSetupError.notesPasswordTooShort) {
            _ = try await accountKeyForNewVault(notesPassword: "short", accountPassword: serverPassword)
        }
    }

    @Test func aLongEnoughPasswordCanStartANewAccountKey() async throws {
        FakeKeyServerProtocol.reset()

        let key = try await accountKeyForNewVault(notesPassword: notesPassword, accountPassword: serverPassword)

        #expect(key == nil)
    }

    @Test func adoptServerKeySavesTheKeyUnderTheLocalPassword() async throws {
        let sharedKey = CryptoService.generateMasterKey()
        let state = try makeState()
        try await state.createVault(password: "throwaway-vault-password")

        let key = try EnvelopeService.adoptServerKey(
            sharedKey,
            encryptionRepo: try #require(state.encryptionRepo),
            localPassword: notesPassword
        )

        #expect(key.withUnsafeBytes { Data($0) } == sharedKey)
        #expect(await state.verifyVaultPassword(notesPassword) == true)
        #expect(await state.verifyVaultPassword("throwaway-vault-password") == false)
    }

    // MARK: - Settings sync setup: don't join an account with a different key

    /// A device that already has a vault (envelope format) and connects to an
    /// existing account must have the same master key as the account. If it
    /// does not, syncing would leave notes unreadable in both directions, so
    /// the check refuses before anything is saved.

    @Test func accountKeyCheckReportsWhenTheServerHasNoKey() async throws {
        FakeKeyServerProtocol.reset()

        let status = try await EnvelopeService.checkAccountKey(
            syncClient: makeClient(),
            userId: userId,
            password: notesPassword,
            localMasterKeyData: CryptoService.generateMasterKey()
        )

        #expect(status == .noServerKey)
    }

    @Test func accountKeyCheckAcceptsTheSameKey() async throws {
        let sharedKey = CryptoService.generateMasterKey()
        FakeKeyServerProtocol.reset(blob: try wrap(sharedKey, with: notesPassword))

        let status = try await EnvelopeService.checkAccountKey(
            syncClient: makeClient(),
            userId: userId,
            password: notesPassword,
            localMasterKeyData: sharedKey
        )

        #expect(status == .matchesLocalKey)
    }

    @Test func accountKeyCheckReportsADifferentKeyWithTheAccountKey() async throws {
        let accountKey = CryptoService.generateMasterKey()
        FakeKeyServerProtocol.reset(blob: try wrap(accountKey, with: notesPassword))

        let status = try await EnvelopeService.checkAccountKey(
            syncClient: makeClient(),
            userId: userId,
            password: notesPassword,
            localMasterKeyData: CryptoService.generateMasterKey()
        )

        // The unwrapped key comes back, so the caller can merge onto it.
        #expect(status == .differs(accountKey: accountKey))
    }

    /// This status is what offers to re-encrypt a whole vault onto the account's
    /// key, so a key the server could have planted must never reach it.
    @Test func accountKeyCheckRefusesADifferentKeyWrappedWithTheServerPassword() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: serverPassword))

        await #expect(throws: EnvelopeError.cannotUnwrapServerKey) {
            _ = try await EnvelopeService.checkAccountKey(
                syncClient: makeClient(),
                userId: userId,
                password: notesPassword,
                localMasterKeyData: CryptoService.generateMasterKey()
            )
        }
    }

    @Test func accountKeyCheckReportsAnUnreadableKeyDistinctly() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: "some-other-notes-password"))

        await #expect(throws: EnvelopeError.cannotUnwrapServerKey) {
            _ = try await EnvelopeService.checkAccountKey(
                syncClient: makeClient(),
                userId: userId,
                password: notesPassword,
                localMasterKeyData: CryptoService.generateMasterKey()
            )
        }
    }

    @Test func accountKeyCheckPropagatesServerFailures() async throws {
        FakeKeyServerProtocol.reset()
        FakeKeyServerProtocol.failGets = true

        do {
            _ = try await EnvelopeService.checkAccountKey(
                syncClient: makeClient(),
                userId: userId,
                password: notesPassword,
                localMasterKeyData: CryptoService.generateMasterKey()
            )
            Issue.record("Expected a server failure to be thrown")
        } catch let error as EnvelopeError {
            Issue.record("Server failure was misreported as \(error)")
        } catch {
            // Expected: a transport / HTTP error
        }
    }

    // MARK: - A refused join leaves no device behind

    /// The check runs after the device has been registered (or cloned) on the
    /// account, because the key can only be fetched with the device's API key.
    /// A refusal must revoke that device again, or every failed attempt leaves
    /// an orphan on the account.

    @Test func joiningCheckKeepsTheDeviceWhenTheKeyDiffers() async throws {
        let accountKey = CryptoService.generateMasterKey()
        FakeKeyServerProtocol.reset(blob: try wrap(accountKey, with: notesPassword))

        let status = try await EnvelopeService.checkAccountKeyForNewDevice(
            syncClient: makeClient(),
            userId: userId,
            password: notesPassword,
            localMasterKeyData: CryptoService.generateMasterKey()
        )

        // The user may still choose to merge, so the device is not revoked
        // here; SyncSetupView revokes it if they decline.
        #expect(status == .differs(accountKey: accountKey))
        #expect(FakeKeyServerProtocol.deleteCount == 0)
    }

    @Test func joiningCheckRevokesTheDeviceWhenTheKeyIsUnreadable() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: "some-other-notes-password"))

        await #expect(throws: EnvelopeError.cannotUnwrapServerKey) {
            _ = try await EnvelopeService.checkAccountKeyForNewDevice(
                syncClient: makeClient(),
                userId: userId,
                password: notesPassword,
                localMasterKeyData: CryptoService.generateMasterKey()
            )
        }
        #expect(FakeKeyServerProtocol.deleteCount == 1)
    }

    @Test func joiningCheckRevokesTheDeviceWhenTheServerFails() async throws {
        FakeKeyServerProtocol.reset()
        FakeKeyServerProtocol.failGets = true

        await #expect(throws: (any Error).self) {
            _ = try await EnvelopeService.checkAccountKeyForNewDevice(
                syncClient: makeClient(),
                userId: userId,
                password: notesPassword,
                localMasterKeyData: CryptoService.generateMasterKey()
            )
        }
        #expect(FakeKeyServerProtocol.deleteCount == 1)
    }

    @Test func joiningCheckKeepsTheDeviceWhenTheKeyMatches() async throws {
        let sharedKey = CryptoService.generateMasterKey()
        FakeKeyServerProtocol.reset(blob: try wrap(sharedKey, with: notesPassword))

        let status = try await EnvelopeService.checkAccountKeyForNewDevice(
            syncClient: makeClient(),
            userId: userId,
            password: notesPassword,
            localMasterKeyData: sharedKey
        )

        #expect(status == .matchesLocalKey)
        #expect(FakeKeyServerProtocol.deleteCount == 0)
    }

    @Test func joiningCheckKeepsTheDeviceWhenTheServerHasNoKey() async throws {
        FakeKeyServerProtocol.reset()

        let status = try await EnvelopeService.checkAccountKeyForNewDevice(
            syncClient: makeClient(),
            userId: userId,
            password: notesPassword,
            localMasterKeyData: CryptoService.generateMasterKey()
        )

        #expect(status == .noServerKey)
        #expect(FakeKeyServerProtocol.deleteCount == 0)
    }

    // MARK: - Face ID unlock: publish a missing key (jottery-x40w)

    /// Envelope setup publishes a missing server key on password unlock, but a
    /// Face ID unlock has no password to wrap it with. A device that only ever
    /// uses Face ID would never publish, so other devices could never connect.
    /// After Face ID, the app checks the server (no password needed) and asks
    /// for the notes password once if the key is missing.

    /// An unlocked vault with sync on, as a Face ID unlock leaves it.
    private func makeSyncedState() async throws -> AppState {
        let state = try makeState()
        try await state.createVault(password: notesPassword)
        state.settings.syncEnabled = true
        var meta = try #require(try state.syncRepo?.getMetadata())
        meta.userId = userId
        try state.syncRepo?.saveMetadata(meta)
        state.syncClient = makeClient()
        return state
    }

    @Test func faceIDUnlockAsksForThePasswordWhenTheServerHasNoKey() async throws {
        FakeKeyServerProtocol.reset()
        let state = try await makeSyncedState()

        await state.checkAccountKeyAfterBiometricUnlock()

        #expect(state.needsNotesPasswordForAccountKey)
        #expect(FakeKeyServerProtocol.putCount == 0)  // nothing published without the password
    }

    @Test func faceIDUnlockDoesNotAskWhenTheServerHasAKey() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: notesPassword))
        let state = try await makeSyncedState()

        await state.checkAccountKeyAfterBiometricUnlock()

        #expect(!state.needsNotesPasswordForAccountKey)
    }

    @Test func faceIDUnlockDoesNotAskWhenTheServerFails() async throws {
        FakeKeyServerProtocol.reset()
        FakeKeyServerProtocol.failGets = true
        let state = try await makeSyncedState()

        await state.checkAccountKeyAfterBiometricUnlock()

        #expect(!state.needsNotesPasswordForAccountKey)  // offline: try again next unlock
    }

    @Test func faceIDUnlockDoesNotAskWhenSyncIsOff() async throws {
        FakeKeyServerProtocol.reset()
        let state = try await makeSyncedState()
        state.settings.syncEnabled = false

        await state.checkAccountKeyAfterBiometricUnlock()

        #expect(!state.needsNotesPasswordForAccountKey)
    }

    @Test func faceIDUnlockDoesNotAskWithoutAUserId() async throws {
        FakeKeyServerProtocol.reset()
        let state = try await makeSyncedState()
        var meta = try #require(try state.syncRepo?.getMetadata())
        meta.userId = nil
        try state.syncRepo?.saveMetadata(meta)

        await state.checkAccountKeyAfterBiometricUnlock()

        #expect(!state.needsNotesPasswordForAccountKey)  // the key can't be wrapped without it
    }

    @Test func publishingWithTheNotesPasswordUploadsTheKey() async throws {
        FakeKeyServerProtocol.reset()
        let state = try await makeSyncedState()
        let masterKey = try #require(state.keyManager.masterKey)
        state.needsNotesPasswordForAccountKey = true

        let result = await state.publishMissingAccountKey(password: notesPassword)

        #expect(result == .published)
        #expect(!state.needsNotesPasswordForAccountKey)
        let blob = try #require(FakeKeyServerProtocol.storedBlob)
        #expect(unwrap(blob, with: notesPassword) == masterKey.withUnsafeBytes { Data($0) })
    }

    @Test func publishingWithAWrongPasswordUploadsNothing() async throws {
        FakeKeyServerProtocol.reset()
        let state = try await makeSyncedState()
        state.needsNotesPasswordForAccountKey = true

        let result = await state.publishMissingAccountKey(password: "not-the-notes-password")

        #expect(result == .wrongPassword)
        #expect(FakeKeyServerProtocol.putCount == 0)
        #expect(FakeKeyServerProtocol.storedBlob == nil)
    }

    @Test func publishingReportsAnUploadFailure() async throws {
        FakeKeyServerProtocol.reset()
        FakeKeyServerProtocol.failPuts = true
        let state = try await makeSyncedState()

        let result = await state.publishMissingAccountKey(password: notesPassword)

        #expect(result == .failed)
        #expect(FakeKeyServerProtocol.storedBlob == nil)
    }

    @Test func lockingClearsThePendingPasswordPrompt() async throws {
        FakeKeyServerProtocol.reset()
        let state = try await makeSyncedState()
        state.needsNotesPasswordForAccountKey = true

        state.lock()

        #expect(!state.needsNotesPasswordForAccountKey)
    }

    // MARK: - Legacy vault onto an existing account key (jottery-crh5)

    /// A legacy vault joining an account that already has a key must move onto
    /// that key. This used to save the server key as the vault's key first and
    /// then re-encrypt with a loop that skipped failures, so any skipped note
    /// was left under the old key and became unreadable. Moving uploads the
    /// whole vault under that key, so it waits for the user to agree.

    /// A legacy vault with one note, sync on, and a user ID, as unlock leaves it.
    private func makeSyncedLegacyVault() async throws -> AppState {
        let state = try await TestVaults.makeLegacyVault(password: notesPassword)
        _ = try state.createNote(content: "legacy note", tags: ["old"])
        state.settings.syncEnabled = true
        var meta = try #require(try state.syncRepo?.getMetadata())
        meta.userId = userId
        try state.syncRepo?.saveMetadata(meta)
        state.syncClient = makeClient()
        return state
    }

    @Test func aLegacyVaultIsNeverMovedOntoTheAccountKeyUnasked() async throws {
        let accountKey = CryptoService.generateMasterKey()
        FakeKeyServerProtocol.reset(blob: try wrap(accountKey, with: notesPassword))
        let state = try await makeSyncedLegacyVault()
        let keyBefore = TestVaults.keyData(state)
        let before = try TestVaults.snapshot(state)

        let ok = await EnvelopeService.tryEnvelopeSetup(
            appState: state, password: notesPassword, masterKey: try #require(state.keyManager.masterKey)
        )

        #expect(ok)
        #expect(state.pendingAccountKeyMerge?.accountKey == accountKey)
        #expect(TestVaults.keyData(state) == keyBefore)
        #expect(try TestVaults.snapshot(state) == before)
    }

    @Test func aLegacyVaultMovesOntoTheAccountKeyOnceTheUserAgrees() async throws {
        let accountKey = CryptoService.generateMasterKey()
        FakeKeyServerProtocol.reset(blob: try wrap(accountKey, with: notesPassword))
        let state = try await makeSyncedLegacyVault()

        await EnvelopeService.tryEnvelopeSetup(
            appState: state, password: notesPassword, masterKey: try #require(state.keyManager.masterKey)
        )
        await state.mergeOntoAccountKey(try #require(state.pendingAccountKeyMerge))

        #expect(state.pendingAccountKeyMerge == nil)
        #expect(TestVaults.keyData(state) == accountKey)
        #expect(try state.encryptionRepo?.get()?.envelopeVersion == 1)
        #expect(state.notes.map(\.content) == ["legacy note"])
        #expect(FakeKeyServerProtocol.putCount == 0)  // the account's key is never replaced

        state.settings.syncEnabled = false
        state.lock()
        try await state.unlock(password: notesPassword)
        #expect(TestVaults.keyData(state) == accountKey)
        #expect(state.notes.map(\.content) == ["legacy note"])
    }

    @Test func aLegacyVaultIsNotMovedOntoAKeyTheServerCouldHavePlanted() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: serverPassword))
        let state = try await makeSyncedLegacyVault()
        let before = try TestVaults.snapshot(state)

        let ok = await EnvelopeService.tryEnvelopeSetup(
            appState: state, password: notesPassword, masterKey: try #require(state.keyManager.masterKey)
        )

        #expect(!ok)
        #expect(state.pendingAccountKeyMerge == nil)
        #expect(FakeKeyServerProtocol.putCount == 0)
        #expect(try TestVaults.snapshot(state) == before)
    }

    @Test func aLegacyVaultThatHasSyncedRepairsAKeyItCantOpen() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: serverPassword))
        let state = try await makeSyncedLegacyVault()
        let masterKey = try #require(TestVaults.keyData(state))
        try #require(state.syncRepo).updateLastSync(at: Date().iso8601)

        let ok = await EnvelopeService.tryEnvelopeSetup(
            appState: state, password: notesPassword, masterKey: try #require(state.keyManager.masterKey)
        )

        #expect(ok)
        #expect(state.pendingAccountKeyMerge == nil)
        #expect(TestVaults.keyData(state) == masterKey)  // the vault stays on its own key
        #expect(unwrap(try #require(FakeKeyServerProtocol.storedBlob), with: notesPassword) == masterKey)
    }

    @Test func lockingForgetsAPendingMerge() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: notesPassword))
        let state = try await makeSyncedLegacyVault()
        await EnvelopeService.tryEnvelopeSetup(
            appState: state, password: notesPassword, masterKey: try #require(state.keyManager.masterKey)
        )
        #expect(state.pendingAccountKeyMerge != nil)

        state.lock()

        #expect(state.pendingAccountKeyMerge == nil)
    }

    @Test func aLegacyVaultThatCannotBeReEncryptedIsLeftAsItWas() async throws {
        let accountKey = CryptoService.generateMasterKey()
        FakeKeyServerProtocol.reset(blob: try wrap(accountKey, with: notesPassword))
        let state = try await makeSyncedLegacyVault()
        let oldKeyData = try #require(TestVaults.keyData(state))
        _ = try state.createNote(content: "doomed", tags: [])
        let doomed = try #require(state.notes.first { $0.content == "doomed" })
        try TestVaults.execute(
            state, "UPDATE notes SET tags = ? WHERE id = ?", [try TestVaults.foreignCiphertext(), doomed.id]
        )
        let before = try TestVaults.snapshot(state)

        await state.mergeOntoAccountKey(PendingAccountKeyMerge(accountKey: accountKey, notesPassword: notesPassword))

        #expect(state.userErrorMessage == L.syncMergeFailedOnUnlock)
        #expect(TestVaults.keyData(state) == oldKeyData)
        #expect(try TestVaults.snapshot(state) == before)
        #expect(try state.encryptionRepo?.get()?.envelopeVersion == nil)
    }

    @Test func aLegacyVaultIsNotMigratedOntoItsOwnKeyWhenTheServerFails() async throws {
        FakeKeyServerProtocol.reset()
        FakeKeyServerProtocol.failGets = true
        let state = try await makeSyncedLegacyVault()
        let before = try TestVaults.snapshot(state)

        let ok = await EnvelopeService.tryEnvelopeSetup(
            appState: state, password: notesPassword, masterKey: try #require(state.keyManager.masterKey)
        )

        // A failed check used to count as "no key on the server", so the vault
        // was migrated onto its own key and diverged from the account's.
        #expect(!ok)
        #expect(try TestVaults.snapshot(state) == before)
        #expect(try state.encryptionRepo?.get()?.envelopeVersion == nil)
    }
}
