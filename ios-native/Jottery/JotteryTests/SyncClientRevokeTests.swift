import Testing
import Foundation
@testable import Jottery

/// Stub transport so the revoke path can be exercised without a server.
final class StubURLProtocol: URLProtocol, @unchecked Sendable {

    struct Recorded: Sendable {
        var method: String
        var url: String
        var authorization: String?
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
                authorization: request.value(forHTTPHeaderField: "Authorization")
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
}

/// Serialized: the stub protocol communicates through statics, which would race
/// if these ran in parallel (Swift Testing's default).
@Suite(.serialized)
struct SyncClientRevokeTests {

    private func makeClient(status: Int, body: String = "") -> SyncClient {
        StubURLProtocol.reset(status: status, body: body)
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [StubURLProtocol.self]
        return SyncClient(
            endpoint: "https://notes.example.org",
            apiKey: "test-api-key",
            configuration: config
        )
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
