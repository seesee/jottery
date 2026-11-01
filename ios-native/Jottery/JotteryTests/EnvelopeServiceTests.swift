import CryptoKit
import Foundation
import Testing

@testable import Jottery

/// In-memory stand-in for the sync server's wrapped-key endpoint.
final class FakeKeyServerProtocol: URLProtocol, @unchecked Sendable {

    nonisolated(unsafe) static var storedBlob: String?
    nonisolated(unsafe) static var putCount = 0
    nonisolated(unsafe) static var failPuts = false
    nonisolated(unsafe) static var failGets = false
    /// DELETE requests seen: revoking the calling device (`sync/device`).
    nonisolated(unsafe) static var deleteCount = 0

    static func reset(blob: String? = nil) {
        storedBlob = blob
        putCount = 0
        failPuts = false
        failGets = false
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
                body = (try? JSONEncoder().encode(
                    WrappedKeyResponse(blob: blob, kdfVersion: 1, kdfIterations: 1_000_000)
                )) ?? Data()
            } else {
                status = 404
                body = Data("No wrapped key found".utf8)
            }
        case "PUT":
            Self.putCount += 1
            if Self.failPuts {
                status = 500
                body = Data("boom".utf8)
            } else if let put = try? JSONDecoder().decode(PutWrappedKeyRequest.self, from: Self.readBody(request)) {
                Self.storedBlob = put.blob
                status = 200
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
/// server sees in plaintext at login. Blobs written by older builds used the
/// server password, so downloads must still cope with those.
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
        let wrappingKey = CryptoService.deriveWrappingKey(password: password, userId: userId)
        return try CryptoService.serializeEncryptedJSON(
            CryptoService.wrapMasterKey(masterKey, with: wrappingKey)
        )
    }

    private func unwrap(_ blob: String, with password: String) -> Data? {
        let wrappingKey = CryptoService.deriveWrappingKey(password: password, userId: userId)
        guard let encrypted = try? CryptoService.parseEncryptedJSON(blob) else { return nil }
        return try? CryptoService.unwrapMasterKey(encrypted, with: wrappingKey)
    }

    // MARK: - First device publishes the key

    @Test func publishUploadsTheKeyWrappedWithTheNotesPassword() async throws {
        FakeKeyServerProtocol.reset()
        let masterKey = CryptoService.generateMasterKey()

        let uploaded = try await EnvelopeService.publishWrappedKeyIfMissing(
            syncClient: makeClient(),
            userId: userId,
            password: notesPassword,
            masterKeyData: masterKey
        )

        #expect(uploaded == true)
        let blob = try #require(FakeKeyServerProtocol.storedBlob)
        #expect(unwrap(blob, with: notesPassword) == masterKey)
        #expect(unwrap(blob, with: serverPassword) == nil)
    }

    @Test func publishNeverOverwritesAKeyTheServerAlreadyHolds() async throws {
        let existing = try wrap(CryptoService.generateMasterKey(), with: serverPassword)
        FakeKeyServerProtocol.reset(blob: existing)

        let uploaded = try await EnvelopeService.publishWrappedKeyIfMissing(
            syncClient: makeClient(),
            userId: userId,
            password: notesPassword,
            masterKeyData: CryptoService.generateMasterKey()
        )

        #expect(uploaded == false)
        #expect(FakeKeyServerProtocol.putCount == 0)
        #expect(FakeKeyServerProtocol.storedBlob == existing)
    }

    @Test func publishReportsAnUploadFailure() async throws {
        FakeKeyServerProtocol.reset()
        FakeKeyServerProtocol.failPuts = true

        await #expect(throws: (any Error).self) {
            _ = try await EnvelopeService.publishWrappedKeyIfMissing(
                syncClient: makeClient(),
                userId: userId,
                password: notesPassword,
                masterKeyData: CryptoService.generateMasterKey()
            )
        }
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
            localPassword: notesPassword,
            alternatePassword: serverPassword
        )

        #expect(key.withUnsafeBytes { Data($0) } == sharedKey)
    }

    @Test func onboardFallsBackToTheServerPasswordForOlderBlobs() async throws {
        let sharedKey = CryptoService.generateMasterKey()
        FakeKeyServerProtocol.reset(blob: try wrap(sharedKey, with: serverPassword))
        let state = try makeState()
        try await state.createVault(password: notesPassword)

        let key = try await EnvelopeService.onboardFromServer(
            syncClient: makeClient(),
            encryptionRepo: try #require(state.encryptionRepo),
            password: notesPassword,
            userId: userId,
            localPassword: notesPassword,
            alternatePassword: serverPassword
        )

        #expect(key.withUnsafeBytes { Data($0) } == sharedKey)
        // Local unlock password stays the notes password, not the server password
        #expect(await state.verifyVaultPassword(notesPassword) == true)
        #expect(await state.verifyVaultPassword(serverPassword) == false)
    }

    @Test func onboardFailsCleanlyWhenNeitherPasswordMatches() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: notesPassword))
        let state = try makeState()
        try await state.createVault(password: "some-local-vault-password")

        await #expect(throws: (any Error).self) {
            _ = try await EnvelopeService.onboardFromServer(
                syncClient: makeClient(),
                encryptionRepo: try #require(state.encryptionRepo),
                password: "wrong-notes-password",
                userId: userId,
                localPassword: "wrong-notes-password",
                alternatePassword: "wrong-server-password"
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
            syncClient: makeClient(), userId: userId, password: notesPassword, alternatePassword: serverPassword
        )

        #expect(key == nil)
    }

    @Test func fetchServerKeyOpensABlobWrappedWithEitherPassword() async throws {
        let sharedKey = CryptoService.generateMasterKey()

        FakeKeyServerProtocol.reset(blob: try wrap(sharedKey, with: notesPassword))
        let viaNotes = try await EnvelopeService.fetchServerKey(
            syncClient: makeClient(), userId: userId, password: notesPassword, alternatePassword: serverPassword
        )
        #expect(viaNotes == sharedKey)

        FakeKeyServerProtocol.reset(blob: try wrap(sharedKey, with: serverPassword))
        let viaServer = try await EnvelopeService.fetchServerKey(
            syncClient: makeClient(), userId: userId, password: notesPassword, alternatePassword: serverPassword
        )
        #expect(viaServer == sharedKey)
    }

    @Test func fetchServerKeyIgnoresAnEmptyAlternatePassword() async throws {
        let sharedKey = CryptoService.generateMasterKey()
        FakeKeyServerProtocol.reset(blob: try wrap(sharedKey, with: notesPassword))

        let key = try await EnvelopeService.fetchServerKey(
            syncClient: makeClient(), userId: userId, password: notesPassword, alternatePassword: ""
        )

        #expect(key == sharedKey)
    }

    @Test func fetchServerKeyReportsAPasswordMismatchAsItsOwnError() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: notesPassword))

        await #expect(throws: EnvelopeError.cannotUnwrapServerKey) {
            _ = try await EnvelopeService.fetchServerKey(
                syncClient: makeClient(),
                userId: userId,
                password: "wrong-notes-password",
                alternatePassword: "wrong-server-password"
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
                syncClient: makeClient(), userId: userId, password: notesPassword, alternatePassword: serverPassword
            )
            Issue.record("Expected a server failure to be thrown")
        } catch let error as EnvelopeError {
            Issue.record("Server failure was misreported as \(error)")
        } catch {
            // Expected: a transport / HTTP error
        }
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
            alternatePassword: serverPassword,
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
            alternatePassword: serverPassword,
            localMasterKeyData: sharedKey
        )

        #expect(status == .matchesLocalKey)
    }

    @Test func accountKeyCheckRefusesADifferentKey() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: notesPassword))

        await #expect(throws: EnvelopeError.accountKeyDiffers) {
            _ = try await EnvelopeService.checkAccountKey(
                syncClient: makeClient(),
                userId: userId,
                password: notesPassword,
                alternatePassword: serverPassword,
                localMasterKeyData: CryptoService.generateMasterKey()
            )
        }
    }

    @Test func accountKeyCheckSeesADifferentKeyWrappedWithTheServerPassword() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: serverPassword))

        await #expect(throws: EnvelopeError.accountKeyDiffers) {
            _ = try await EnvelopeService.checkAccountKey(
                syncClient: makeClient(),
                userId: userId,
                password: notesPassword,
                alternatePassword: serverPassword,
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
                alternatePassword: serverPassword,
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
                alternatePassword: serverPassword,
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

    @Test func joiningCheckRevokesTheDeviceWhenTheKeyDiffers() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: notesPassword))

        await #expect(throws: EnvelopeError.accountKeyDiffers) {
            _ = try await EnvelopeService.checkAccountKeyForNewDevice(
                syncClient: makeClient(),
                userId: userId,
                password: notesPassword,
                localMasterKeyData: CryptoService.generateMasterKey()
            )
        }
        #expect(FakeKeyServerProtocol.deleteCount == 1)
    }

    @Test func joiningCheckRevokesTheDeviceWhenTheKeyIsUnreadable() async throws {
        FakeKeyServerProtocol.reset(blob: try wrap(CryptoService.generateMasterKey(), with: "some-other-notes-password"))

        await #expect(throws: EnvelopeError.cannotUnwrapServerKey) {
            _ = try await EnvelopeService.checkAccountKeyForNewDevice(
                syncClient: makeClient(),
                userId: userId,
                password: notesPassword,
                alternatePassword: serverPassword,
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
}
