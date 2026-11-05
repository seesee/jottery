import Foundation
import Observation

/// What the Account screen shows, and the session behind it.
///
/// The session token and any inbox token the server discloses live here and
/// nowhere else: this object goes with the screen, and neither is written down
/// or logged. The password isn't kept at all. It's asked for again to delete.
@MainActor @Observable
final class AccountModel {
    private var sessionId: String?

    private(set) var info: AccountInfo?
    /// An inbox token just generated. The server holds only its hash, so this is
    /// the one chance to see it.
    private(set) var disclosedToken: String?
    private(set) var isWorking = false
    var errorMessage: String?

    var isSignedIn: Bool { sessionId != nil }

    func signIn(email: String, password: String, client: SyncClient) async {
        await perform {
            do {
                self.sessionId = try await client.logIn(email: email, password: password)
            } catch SyncClientError.httpError(let status, _) where status == 401 {
                self.errorMessage = L.deleteAccountWrongCredentials
                return
            }
            try await self.load(client: client)
        }
    }

    func refresh(client: SyncClient) async {
        await perform { try await self.load(client: client) }
    }

    private func load(client: SyncClient) async throws {
        guard let sessionId else { return }
        info = try await client.accountInfo(sessionId: sessionId)
    }

    /// Delete every note on the server. `password` goes with the request, because
    /// a session alone shouldn't be able to do this. Returns whether it happened.
    @discardableResult
    func deleteServerNotes(password: String, client: SyncClient) async -> Bool {
        guard let sessionId, !password.isEmpty else { return false }
        var deleted = false
        await perform {
            do {
                try await client.deleteServerNotes(sessionId: sessionId, currentPassword: password)
                deleted = true
            } catch SyncClientError.httpError(let status, _) where status == 401 {
                self.errorMessage = L.accountDeleteWrongPassword
                return
            }
            try await self.load(client: client)
        }
        return deleted
    }

    /// Make a new inbox token, which replaces any there was.
    func generateInboxToken(client: SyncClient) async {
        guard let sessionId else { return }
        await perform {
            let token = try await client.generateInboxToken(sessionId: sessionId)
            // Shown as text and copied, so it has to look like what it claims to be.
            guard let usable = SyncValidation.opaqueToken(token), !usable.isEmpty else {
                self.errorMessage = L.accountRequestFailed
                return
            }
            self.disclosedToken = usable
            self.info?.inbox?.hasToken = true
        }
    }

    func revokeInboxToken(client: SyncClient) async {
        guard let sessionId else { return }
        await perform {
            try await client.revokeInboxToken(sessionId: sessionId)
            self.disclosedToken = nil
            self.info?.inbox?.hasToken = false
        }
    }

    /// Forget everything at once, and end the session on the server if it can be
    /// reached. It would expire there anyway.
    func signOut(client: SyncClient?) {
        let ended = sessionId
        sessionId = nil
        info = nil
        disclosedToken = nil
        errorMessage = nil
        if let ended, let client {
            Task { try? await client.logOut(sessionId: ended) }
        }
    }

    /// Run `work`, showing a failure in the app's own words. A server's error
    /// body isn't shown: what the user is told isn't for it to choose.
    private func perform(_ work: @escaping @MainActor () async throws -> Void) async {
        isWorking = true
        errorMessage = nil
        defer { isWorking = false }
        do {
            try await work()
        } catch SyncClientError.httpError(let status, _) where status == 401 {
            sessionId = nil
            info = nil
            disclosedToken = nil
            errorMessage = L.accountSessionEnded
        } catch SyncClientError.httpError {
            errorMessage = L.accountRequestFailed
        } catch is DecodingError {
            errorMessage = L.accountRequestFailed
        } catch {
            errorMessage = SyncEndpoint.describeSyncFailure(error)
        }
    }
}
