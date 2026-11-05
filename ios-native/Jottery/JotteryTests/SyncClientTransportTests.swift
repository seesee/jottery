import Testing
import Foundation
@testable import Jottery

/// Stub transport so request-shaping can be exercised without a server.
final class StubURLProtocol: URLProtocol, @unchecked Sendable {

    struct Recorded: Sendable {
        var method: String
        var url: String
        var authorization: String?
        var contentType: String?
        var body: Data
    }

    nonisolated(unsafe) static var status: Int = 204
    nonisolated(unsafe) static var body: Data = Data()
    nonisolated(unsafe) static var recorded: [Recorded] = []

    static func reset(status: Int, body: String = "") {
        self.status = status
        self.body = Data(body.utf8)
        self.recorded = []
    }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        StubURLProtocol.recorded.append(
            Recorded(
                method: request.httpMethod ?? "?",
                url: request.url?.absoluteString ?? "?",
                authorization: request.value(forHTTPHeaderField: "Authorization"),
                contentType: request.value(forHTTPHeaderField: "Content-Type"),
                body: request.httpBody ?? Self.read(request.httpBodyStream)
            )
        )

        let response = HTTPURLResponse(
            url: request.url!,
            statusCode: StubURLProtocol.status,
            httpVersion: "HTTP/1.1",
            headerFields: nil
        )!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: StubURLProtocol.body)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}

    private static func read(_ stream: InputStream?) -> Data {
        guard let stream else { return Data() }
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

private func makeStubbedClient(status: Int, body: String = "", apiKey: String) -> SyncClient {
    StubURLProtocol.reset(status: status, body: body)
    let config = URLSessionConfiguration.ephemeral
    config.protocolClasses = [StubURLProtocol.self]
    return SyncClient(
        endpoint: "https://notes.example.org",
        apiKey: apiKey,
        configuration: config
    )
}

/// `.serialized` on the parent applies to the nested suites too, so they cannot
/// interleave. Both drive the same stub, which communicates through statics —
/// marking each child suite `.serialized` individually is *not* enough, because
/// that only orders tests within a suite and lets the suites race each other.
@Suite(.serialized)
struct SyncClientTransportTests {

    // MARK: - Device self-revoke

    @Suite struct Revoke {

        private func makeClient(status: Int, body: String = "") -> SyncClient {
            makeStubbedClient(status: status, body: body, apiKey: "test-api-key")
        }

        @Test func revokeSendsAuthenticatedDeleteToDeviceEndpoint() async throws {
            let client = makeClient(status: 204)

            try await client.revokeThisDevice()

            let calls = StubURLProtocol.recorded
            #expect(calls.count == 1)
            #expect(calls.first?.method == "DELETE")
            #expect(calls.first?.url == "https://notes.example.org/api/v1/sync/device")
            #expect(calls.first?.authorization == "Bearer test-api-key")
        }

        /// A self-hosted server older than this endpoint answers 404 or 405. That
        /// must be distinguishable from a real failure so Disconnect can still
        /// proceed locally and tell the user the device is still registered.
        @Test func revokeMapsNotFoundToUnsupported() async {
            let client = makeClient(status: 404)

            await #expect(throws: SyncClientError.self) {
                try await client.revokeThisDevice()
            }
        }

        @Test func revokeMapsMethodNotAllowedToUnsupported() async throws {
            let client = makeClient(status: 405)

            do {
                try await client.revokeThisDevice()
                Issue.record("expected revokeThisDevice to throw")
            } catch SyncClientError.deviceRevokeUnsupported {
                // expected
            } catch {
                Issue.record("expected deviceRevokeUnsupported, got \(error)")
            }
        }

        /// Anything else is a genuine failure and must surface as an HTTP error,
        /// not be quietly treated as "server too old".
        @Test func revokePropagatesOtherHTTPErrors() async throws {
            let client = makeClient(status: 500, body: "boom")

            do {
                try await client.revokeThisDevice()
                Issue.record("expected revokeThisDevice to throw")
            } catch SyncClientError.deviceRevokeUnsupported {
                Issue.record("500 must not be treated as unsupported")
            } catch SyncClientError.httpError(let status, _) {
                #expect(status == 500)
            }
        }

        @Test func revokeTreatsUnauthorizedAsRealFailure() async throws {
            let client = makeClient(status: 401)

            do {
                try await client.revokeThisDevice()
                Issue.record("expected revokeThisDevice to throw")
            } catch SyncClientError.deviceRevokeUnsupported {
                Issue.record("401 must not be treated as unsupported")
            } catch {
                // any other error is fine
            }
        }
    }

    // MARK: - Account deletion

    @Suite struct Account {

        private static let loginBody = #"{"sessionId":"sess-123","expiresAt":"2026-08-01T00:00:00Z","user":{"id":"u1","email":"a@b.c","isAdmin":false}}"#

        private func makeClient(status: Int, body: String = "") -> SyncClient {
            makeStubbedClient(status: status, body: body, apiKey: "device-api-key")
        }

        @Test func logInPostsCredentialsAndReturnsSessionId() async throws {
            let client = makeClient(status: 200, body: Self.loginBody)

            let sessionId = try await client.logIn(email: "a@b.c", password: "pw")

            #expect(sessionId == "sess-123")
            let calls = StubURLProtocol.recorded
            #expect(calls.count == 1)
            #expect(calls.first?.method == "POST")
            #expect(calls.first?.url == "https://notes.example.org/api/v1/user/login")
        }

        /// Login must not send the device API key — it is establishing a different
        /// identity, and the server rejects a device key on this route anyway.
        @Test func logInDoesNotSendDeviceApiKey() async throws {
            let client = makeClient(status: 200, body: Self.loginBody)

            _ = try await client.logIn(email: "a@b.c", password: "pw")

            #expect(StubURLProtocol.recorded.first?.authorization == nil)
        }

        @Test func logInPropagatesUnauthorized() async throws {
            let client = makeClient(status: 401, body: "bad credentials")

            do {
                _ = try await client.logIn(email: "a@b.c", password: "wrong")
                Issue.record("expected logIn to throw")
            } catch SyncClientError.httpError(let status, _) {
                #expect(status == 401)
            }
        }

        /// Deletion authenticates with the session token, never the device key.
        @Test func deleteAccountUsesSessionTokenNotApiKey() async throws {
            let client = makeClient(status: 204)

            try await client.deleteAccount(sessionId: "sess-123", mode: .delete, currentPassword: "the-password")

            let call = StubURLProtocol.recorded.first
            #expect(call?.method == "DELETE")
            #expect(call?.authorization == "Bearer sess-123")
            #expect(call?.authorization != "Bearer device-api-key")
        }

        @Test func deleteAccountSendsRequestedMode() async throws {
            let client = makeClient(status: 204)
            try await client.deleteAccount(sessionId: "s", mode: .delete, currentPassword: "the-password")
            #expect(StubURLProtocol.recorded.first?.url.hasSuffix("/api/v1/user/account?mode=delete") == true)

            let client2 = makeClient(status: 204)
            try await client2.deleteAccount(sessionId: "s", mode: .deactivate, currentPassword: "the-password")
            #expect(StubURLProtocol.recorded.first?.url.hasSuffix("/api/v1/user/account?mode=deactivate") == true)
        }

        /// Without it, this only works within five minutes of signing in.
        @Test func deleteAccountSendsThePassword() async throws {
            let client = makeStubbedClient(status: 204, apiKey: "device-key")

            try await client.deleteAccount(sessionId: "s", mode: .delete, currentPassword: "the-password")

            let request = try #require(StubURLProtocol.recorded.first)
            #expect(request.contentType == "application/json")
            let sent = try JSONDecoder().decode(DeleteAccountRequest.self, from: request.body)
            #expect(sent.currentPassword == "the-password")
        }

        @Test func deleteAccountPropagatesFailure() async throws {
            let client = makeClient(status: 500, body: "boom")

            do {
                try await client.deleteAccount(sessionId: "s", mode: .delete, currentPassword: "the-password")
                Issue.record("expected deleteAccount to throw")
            } catch SyncClientError.httpError(let status, _) {
                #expect(status == 500)
            }
        }
    }

    // MARK: - Creating an account (R2-138)

    @Suite struct Registration {

        private func makeClient(status: Int, body: String = "") -> SyncClient {
            makeStubbedClient(status: status, body: body, apiKey: "test-api-key")
        }

        @Test func anAccountIsCreatedWithoutADeviceKey() async throws {
            let client = makeClient(status: 201, body: #"{"userId":"u","email":"a@b.c","status":"pending_approval","message":"m"}"#)

            let account = try await client.registerUser(email: "a@b.c", password: "a-long-account-password")

            #expect(!account.isApproved)
            let call = StubURLProtocol.recorded.first
            #expect(call?.method == "POST")
            #expect(call?.url.hasSuffix("/api/v1/auth/register-user") == true)
            #expect(call?.authorization == nil)
        }

        @Test func anApprovedAccountCanConnectAtOnce() async throws {
            let client = makeClient(status: 201, body: #"{"userId":"u","email":"a@b.c","status":"approved","message":"m"}"#)

            #expect(try await client.registerUser(email: "a@b.c", password: "a-long-account-password").isApproved)
        }

        @Test func anAddressWithAnAccountSaysSo() async throws {
            let client = makeClient(status: 409, body: #"{"error":"Email already registered"}"#)

            await #expect {
                _ = try await client.registerUser(email: "a@b.c", password: "a-long-account-password")
            } throws: { error in
                if case SyncClientError.accountExists = error { return true }
                return false
            }
        }

        /// The server's password rules are configurable, so only it can say.
        @Test func aRefusalGivesTheServersReason() async throws {
            let client = makeClient(status: 400, body: #"{"error":"Password is too short"}"#)

            await #expect {
                _ = try await client.registerUser(email: "a@b.c", password: "short")
            } throws: { error in
                error.localizedDescription == "Password is too short"
            }
        }

        @Test func statusIsAskedForWithThePassword() async throws {
            let client = makeClient(status: 200, body: #"{"exists":true,"isApproved":true,"isActive":true}"#)

            let status = try await client.userStatus(email: "a@b.c", password: "a-long-account-password")

            #expect(status.isApproved)
            #expect(StubURLProtocol.recorded.first?.method == "POST")
        }

        @Test func anOlderServerIsAskedTheOldWay() async throws {
            let client = makeClient(status: 405, body: "")

            // The stub answers 405 to both, so the second call shows the fallback.
            _ = try? await client.userStatus(email: "a@b.c", password: "a-long-account-password")

            let calls = StubURLProtocol.recorded
            #expect(calls.map(\.method) == ["POST", "GET"])
            #expect(calls.last?.url.contains("user/status?email=a@b.c") == true)
        }

        @Test func aPendingAccountIsToldApartFromADisabledOne() async throws {
            let client = makeClient(status: 403, body: #"{"error":"Account pending admin approval"}"#)

            await #expect {
                _ = try await client.logIn(email: "a@b.c", password: "a-long-account-password")
            } throws: { error in
                if case SyncClientError.accountPendingApproval = error { return true }
                return false
            }
        }
    }
}
