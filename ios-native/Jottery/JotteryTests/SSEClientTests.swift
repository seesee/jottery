import Foundation
import Testing

@testable import Jottery

/// What one test's server does and has seen. Each test has its own, on a host of
/// its own: a stopped client can still have a request in flight, and one test's
/// stragglers must not land in the next test's record.
private final class StreamScript: @unchecked Sendable {
    private let lock = NSLock()
    private var issued = 0
    private var statuses: [Int]
    private let refused: Set<String>
    private var requests: [String] = []

    init(tokenStatuses: [Int] = [], refusedTokens: Set<String> = []) {
        self.statuses = tokenStatuses
        self.refused = refusedTokens
    }

    /// A token, or the status a token request gets instead.
    func nextToken() -> (status: Int, token: String?) {
        lock.withLock {
            let status = statuses.isEmpty ? 200 : statuses.removeFirst()
            guard status == 200 else { return (status, nil) }
            issued += 1
            return (200, "token-\(issued)")
        }
    }

    func connect(with token: String) -> Bool {
        lock.withLock {
            requests.append(token)
            return !refused.contains(token)
        }
    }

    var tokensIssued: Int { lock.withLock { issued } }
    var streamRequests: [String] { lock.withLock { requests } }
    var statusesLeft: Int { lock.withLock { statuses.count } }
}

private final class ScriptedStreamProtocol: URLProtocol, @unchecked Sendable {

    private static let lock = NSLock()
    nonisolated(unsafe) private static var scripts: [String: StreamScript] = [:]

    static func add(_ script: StreamScript, host: String) {
        lock.withLock { scripts[host] = script }
    }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        let url = request.url!
        let script = Self.lock.withLock { Self.scripts[url.host ?? ""] }
        var status = 404
        var body = Data()

        if url.path.hasSuffix("sync/events/token"), let answer = script?.nextToken() {
            status = answer.status
            body = answer.token.map { Data(#"{"token":"\#($0)","expires_in":300}"#.utf8) } ?? Data(#"{"error":"refused"}"#.utf8)
        } else if url.path.hasSuffix("sync/events"), let script {
            let token = URLComponents(url: url, resolvingAgainstBaseURL: false)?
                .queryItems?.first { $0.name == "token" }?.value ?? ""
            if script.connect(with: token) {
                status = 200
                body = Data("event: sync\ndata: pull\n\n".utf8)
            } else {
                status = 401
            }
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
struct SSEClientTests {

    /// A refusal backs off for two seconds or more. A stream that ended cleanly
    /// waits one, which these tests aren't about.
    private func isBackoff(_ pause: Duration) -> Bool { pause > .seconds(1) }

    private func makeClient(_ script: StreamScript, pauses: PauseLog? = nil) -> SSEClient {
        let host = "\(UUID().uuidString).example.test"
        ScriptedStreamProtocol.add(script, host: host)
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [ScriptedStreamProtocol.self]
        let syncClient = SyncClient(endpoint: "https://\(host)", apiKey: "device-key", configuration: configuration)
        guard let pauses else { return SSEClient(syncClient: syncClient, configuration: configuration) }
        return SSEClient(syncClient: syncClient, configuration: configuration) { await pauses.record($0) }
    }

    @Test func aRefusedTokenIsReplacedAndTheStreamCarriesOn() async {
        let script = StreamScript(refusedTokens: ["token-1"])
        let client = makeClient(script)
        let events = EventCounter()
        await client.setOnSyncEvent { await events.record() }

        await client.start()
        await waitUntil { await events.count > 0 }
        await client.stop()

        #expect(await events.count > 0)
        #expect(script.streamRequests.prefix(2) == ["token-1", "token-2"])
    }

    /// A token that has run out is routine, and a wait here would leave a healthy
    /// device without live updates for no reason.
    @Test func aRefusedTokenIsReplacedWithoutWaiting() async {
        let script = StreamScript(refusedTokens: ["token-1"])
        let pauses = PauseLog()
        let client = makeClient(script, pauses: pauses)
        let events = EventCounter()
        await client.setOnSyncEvent { await events.record() }

        await client.start()
        await waitUntil { await events.count > 0 }
        await client.stop()

        #expect(await events.count > 0)
        #expect(await pauses.pauses.filter(isBackoff).isEmpty)
    }

    /// Only the first refusal is free. Otherwise a server refusing every token
    /// would be asked for another as fast as it could answer.
    @Test func aSecondRefusalInARowWaits() async {
        let script = StreamScript(refusedTokens: ["token-1", "token-2"])
        let pauses = PauseLog()
        let client = makeClient(script, pauses: pauses)
        let events = EventCounter()
        await client.setOnSyncEvent { await events.record() }

        await client.start()
        await waitUntil { await events.count > 0 }
        await client.stop()

        #expect(await events.count > 0)
        #expect(await pauses.pauses.filter(isBackoff).count == 1)
    }

    /// Whoever set the listener can be kept alive by it, along with the key they
    /// hold, so a stopped client has to let go (jottery-4u01).
    @Test func stoppingLetsGoOfTheListener() async {
        let client = makeClient(StreamScript())
        await client.setOnSyncEvent {}

        await client.stop()

        #expect(await client.onSyncEvent == nil)
    }

    @Test func aRevokedDeviceStopsWhenTheNewTokenIsRefused() async {
        let script = StreamScript(tokenStatuses: [200, 401], refusedTokens: ["token-1"])
        let client = makeClient(script)

        await client.start()
        await client.waitUntilStopped()

        #expect(script.streamRequests == ["token-1"])
        #expect(script.tokensIssued == 1)
        #expect(script.statusesLeft == 0, "it has to have asked")
    }

    @Test func aDisabledAccountStopsWhenTheNewTokenIsRefused() async {
        let script = StreamScript(tokenStatuses: [200, 403], refusedTokens: ["token-1"])
        let client = makeClient(script)

        await client.start()
        await client.waitUntilStopped()

        #expect(script.streamRequests == ["token-1"])
    }

    /// A stream that ends is how a revoked device finds out if it was connected
    /// already. The reconnection asks for a token, and that's refused.
    /// Without a wait, a server or proxy that closes every stream would get a
    /// tight request loop from every device.
    @Test func aStreamThatEndsWaitsBeforeConnectingAgain() async {
        let script = StreamScript(tokenStatuses: [200, 200, 401])
        let pauses = PauseLog()
        let client = makeClient(script, pauses: pauses)

        await client.start()
        await client.waitUntilStopped()

        #expect(script.streamRequests.count == 2)
        #expect(await !pauses.pauses.isEmpty)
    }

    @Test func aStreamThatEndsAsksForANewTokenBeforeConnectingAgain() async {
        let script = StreamScript(tokenStatuses: [200, 401])
        let client = makeClient(script)

        await client.start()
        await client.waitUntilStopped()

        #expect(script.streamRequests == ["token-1"])
    }
}
