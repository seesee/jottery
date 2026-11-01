import Foundation
import Testing

@testable import Jottery

/// Importing credentials in Settings turns sync on for a vault that already
/// exists, so an envelope vault must check that the account's key is its own.
/// That check needs the notes password, except for `jottery:v1:` credentials,
/// which are encrypted with the exporting device's master key and so can only
/// be opened by a vault holding the same key.
@MainActor
struct SyncSetupImportTests {

    @Test func envelopeVaultNeedsTheNotesPasswordForARawAPIKey() {
        #expect(SyncSetupView.importNeedsNotesPassword("a1b2c3d4e5f6", vaultIsEnvelope: true))
    }

    @Test func envelopeVaultNeedsTheNotesPasswordForLegacyCredentials() throws {
        let json = #"{"endpoint":"https://sync.example.test","apiKey":"a1b2c3","clientId":"c1"}"#
        let legacy = Data(json.utf8).base64EncodedString()
        #expect(SyncSetupView.importNeedsNotesPassword(legacy, vaultIsEnvelope: true))
    }

    @Test func encryptedCredentialsNeedNoPassword() {
        #expect(!SyncSetupView.importNeedsNotesPassword("jottery:v1:c2FsdA==.e30=", vaultIsEnvelope: true))
        #expect(!SyncSetupView.importNeedsNotesPassword("  jottery:v1:c2FsdA==.e30=\n", vaultIsEnvelope: true))
    }

    @Test func legacyVaultNeverNeedsThePassword() {
        #expect(!SyncSetupView.importNeedsNotesPassword("a1b2c3d4e5f6", vaultIsEnvelope: false))
    }

    @Test func emptyInputNeedsNoPassword() {
        #expect(!SyncSetupView.importNeedsNotesPassword("   ", vaultIsEnvelope: true))
    }
}
