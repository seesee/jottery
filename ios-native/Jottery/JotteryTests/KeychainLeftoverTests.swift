import Foundation
import Testing

@testable import Jottery

/// iOS keeps Keychain items when an app is deleted, so someone who deleted Jottery
/// to remove their notes left the master key and API key on the phone (R2-105).
@MainActor
struct KeychainLeftoverTests {

    /// A database of its own, with a recording stand-in for the Keychain: other
    /// tests run alongside this one, and the real items are shared between them.
    private func openDatabase(at path: String, into state: AppState) throws -> Bool {
        var forgot = false
        try state.initialise(database: DatabaseManager(path: path), forgetEarlierInstall: { forgot = true })
        return forgot
    }

    private func temporaryPath() throws -> String {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("jottery-keychain-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.appendingPathComponent("test.db").path
    }

    @Test func aFreshInstallLetsGoOfWhatAnEarlierOneLeft() throws {
        let forgot = try openDatabase(at: temporaryPath(), into: AppState())

        #expect(forgot)
    }

    @Test func aVaultKeepsItsKeychainItems() async throws {
        let path = try temporaryPath()
        let first = AppState()
        _ = try openDatabase(at: path, into: first)
        try await first.createVault(password: "notes-password-correct-horse")

        let forgot = try openDatabase(at: path, into: AppState())

        #expect(!forgot)
    }

    /// A locked device can't open the database, which is no sign of a fresh install.
    @Test func aDatabaseThatWontOpenForgetsNothing() throws {
        let state = AppState()
        var forgot = false
        let unusable = try temporaryPath() + "/not/a/folder/test.db"

        #expect(throws: (any Error).self) {
            try state.initialise(database: DatabaseManager(path: unusable), forgetEarlierInstall: { forgot = true })
        }

        #expect(!forgot)
    }
}

/// The one test that touches the real Keychain, so it runs by itself.
@Suite(.serialized)
struct KeychainDeleteAllTests {

    @Test func deleteAllTakesEverythingTheAppStored() throws {
        let earlierKey = KeychainService.retrieveAPIKey()
        let earlierClient = KeychainService.retrieveClientId()
        defer {
            if let earlierKey { try? KeychainService.storeAPIKey(earlierKey) }
            if let earlierClient { try? KeychainService.storeClientId(earlierClient) }
        }
        try KeychainService.storeAPIKey("left-by-an-earlier-install")
        try KeychainService.storeClientId("left-by-an-earlier-install")

        KeychainService.deleteAll()

        #expect(KeychainService.retrieveAPIKey() == nil)
        #expect(KeychainService.retrieveClientId() == nil)
    }
}
