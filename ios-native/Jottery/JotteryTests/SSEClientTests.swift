import Foundation
import Testing

@testable import Jottery

/// A server that hands out tokens and refuses a stream until told otherwise.
private final class ScriptedStreamProtocol: URLProtocol, @unchecked Sendable {

    nonisolated(unsafe) static var tokensIssued = 0
    /// What a token request gets, in turn. When they run out, each gets a token.
    nonisolated(unsafe) static var tokenStatuses: [Int] = []
    /// The tokens the stream refuses.
    nonisolated(unsafe) static var refusedTokens: Set<String> = []
    nonisolated(unsafe) static var streamRequests: [String] = []

    static func reset() {
        tokensIssued = 0
        tokenStatuses = []
        refusedTokens = []
        streamRequests = []
    }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        let url = request.url!
        var status = 200
        var body = Data()

        if url.path.hasSuffix("sync/events/token") {
            status = Self.tokenStatuses.isEmpty ? 200 : Self.tokenStatuses.removeFirst()
            if status == 200 {
                Self.tokensIssued += 1
                body = Data(#"{"token":"token-\#(Self.tokensIssued)","expires_in":300}"#.utf8)
            } else {
                body = Data(#"{"error":"refused"}"#.utf8)
            }
        } else if url.path.hasSuffix("sync/events") {
            let token = URLComponents(url: url, resolvingAgainstBaseURL: false)?
                .queryItems?.first { $0.name == "token" }?.value ?? ""
            Self.streamRequests.append(token)
            if Self.refusedTokens.contains(token) {
                status = 401
            } else {
                body = Data("event: sync\ndata: pull\n\n".utf8)
            }
        } else {
            status = 404
        }

        let response = HTTPURLResponse(url: url, statusCode: status, httpVersion: "HTTP/1.1", headerFields: nil)!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: body)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

private actor EventCounter {
    private(set) var count = 0
    func record() { count += 1 }
}

/// A stream can end, and connecting can get 401, for a revoked device. The token
/// request is what says why (docs/sync-protocol.md).
@Suite(.serialized)
struct SSEClientTests {

    private func makeClient() -> SSEClient {
        ScriptedStreamProtocol.reset()
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [ScriptedStreamProtocol.self]
        let syncClient = SyncClient(endpoint: "https://sync.example.test", apiKey: "device-key", configuration: configuration)
        return SSEClient(syncClient: syncClient, configuration: configuration)
    }

    @Test func aRefusedTokenIsReplacedAndTheStreamCarriesOn() async {
        let client = makeClient()
        ScriptedStreamProtocol.refusedTokens = ["token-1"]
        let events = EventCounter()
        await client.setOnSyncEvent { await events.record() }

        await client.start()
        await waitUntil { await events.count > 0 }
        await client.stop()

        #expect(await events.count > 0)
        #expect(ScriptedStreamProtocol.streamRequests.prefix(2) == ["token-1", "token-2"])
    }

    @Test func aRevokedDeviceStopsWhenTheNewTokenIsRefused() async {
        let client = makeClient()
        ScriptedStreamProtocol.refusedTokens = ["token-1"]
        ScriptedStreamProtocol.tokenStatuses = [200, 401]

        await client.start()
        await client.waitUntilStopped()

        #expect(ScriptedStreamProtocol.streamRequests == ["token-1"])
        #expect(ScriptedStreamProtocol.tokensIssued == 1)
        #expect(ScriptedStreamProtocol.tokenStatuses.isEmpty, "it has to have asked")
    }

    @Test func aDisabledAccountStopsWhenTheNewTokenIsRefused() async {
        let client = makeClient()
        ScriptedStreamProtocol.refusedTokens = ["token-1"]
        ScriptedStreamProtocol.tokenStatuses = [200, 403]

        await client.start()
        await client.waitUntilStopped()

        #expect(ScriptedStreamProtocol.streamRequests == ["token-1"])
    }

    /// A stream that ends is how a revoked device finds out if it was connected
    /// already. The reconnection asks for a token, and that's refused.
    @Test func aStreamThatEndsAsksForANewTokenBeforeConnectingAgain() async {
        let client = makeClient()
        ScriptedStreamProtocol.tokenStatuses = [200, 401]

        await client.start()
        await client.waitUntilStopped()

        #expect(ScriptedStreamProtocol.streamRequests == ["token-1"])
    }
}
