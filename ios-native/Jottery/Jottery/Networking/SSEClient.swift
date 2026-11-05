import Foundation

/// Server-Sent Events client using URLSession bytes streaming.
/// Listens for `event: sync` / `data: pull` messages to trigger sync pulls.
/// Retries indefinitely with capped exponential backoff.
actor SSEClient {

    private var task: Task<Void, Never>?
    private let syncClient: SyncClient
    private var reconnectAttempts = 0
    private let maxBackoffSeconds: Double = 30

    /// Dedicated session for SSE — long timeouts, no caching.
    private let sseSession: URLSession

    /// Waits out a backoff.
    private let pause: @Sendable (Duration) async -> Void

    var onSyncEvent: (@Sendable () async -> Void)?

    /// - Parameters:
    ///   - configuration: replaces the session's configuration. Only used by
    ///     tests, to install a stub `URLProtocol`.
    ///   - pause: replaces the wait between attempts. Only used by tests, which
    ///     see what was asked for without a clock coming into it.
    init(
        syncClient: SyncClient,
        configuration: URLSessionConfiguration? = nil,
        pause: @escaping @Sendable (Duration) async -> Void = { try? await Task.sleep(for: $0) }
    ) {
        self.syncClient = syncClient
        self.pause = pause
        self.sseSession = URLSession(configuration: configuration ?? {
            let config = URLSessionConfiguration.default
            config.timeoutIntervalForRequest = 45   // Must receive data within 45 s (heartbeat is 30 s)
            config.timeoutIntervalForResource = 0   // No overall resource timeout
            config.requestCachePolicy = .reloadIgnoringLocalAndRemoteCacheData
            return config
        }())
    }

    /// Start listening for SSE events.
    func start() {
        cancelConnection()
        task = Task { await connectLoop() }
    }

    /// Stop listening, and let go of whoever was listening.
    func stop() {
        cancelConnection()
        onSyncEvent = nil
    }

    private func cancelConnection() {
        task?.cancel()
        task = nil
        reconnectAttempts = 0
    }

    /// Returns once listening has stopped by itself or been stopped.
    func waitUntilStopped() async {
        await task?.value
    }

    // MARK: - Private

    private func connectLoop() async {
        while !Task.isCancelled {
            do {
                try await connect()
                reconnectAttempts = 0
                // The stream ended cleanly. Connecting again straight away would
                // give a server or proxy that closes every stream a tight request
                // loop from every device.
                await pause(.seconds(1))
            } catch SSEError.tokenRefused where reconnectAttempts == 0 {
                // The token ran out, or the device is no longer welcome. Asking
                // for a new one finds out which, with the server's real answer,
                // so there's nothing to wait for. Any refusal after that backs off.
                reconnectAttempts = 1
            } catch {
                if Task.isCancelled { return }
                // The device or account is no longer welcome, and retrying can't
                // change that. The next sync reports it, and a reactivated
                // account picks up when the app next starts listening.
                if Self.isPermanent(error) {
                    Log.debug("[SSE] stopping: the server refused this device")
                    return
                }
                reconnectAttempts += 1
                let delay = min(pow(2.0, Double(min(reconnectAttempts, 5))), maxBackoffSeconds)
                Log.debug("[SSE] connection error (attempt \(reconnectAttempts)), retrying in \(delay)s")
                await pause(.seconds(delay))
            }
        }
    }

    private static func isPermanent(_ error: Error) -> Bool {
        switch error {
        case SyncClientError.accountDisabled, SyncClientError.accountPendingApproval:
            return true
        case SyncClientError.httpError(let status, _):
            return status == 401
        default:
            return false
        }
    }

    private func connect() async throws {
        let tokenResponse = try await syncClient.getSSEToken()
        guard let url = await syncClient.sseURL(token: tokenResponse.token) else {
            throw SSEError.invalidURL
        }

        var request = URLRequest(url: url)
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        request.setValue("no-cache", forHTTPHeaderField: "Cache-Control")

        let (bytes, response) = try await sseSession.bytes(for: request)
        guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
            throw (response as? HTTPURLResponse)?.statusCode == 401 ? SSEError.tokenRefused : SSEError.connectionFailed
        }

        reconnectAttempts = 0

        var currentEvent = ""
        var currentData = ""

        for try await line in bytes.lines {
            if Task.isCancelled { return }

            if line.hasPrefix(":") {
                // SSE comment / heartbeat — ignore
            } else if line.hasPrefix("event: ") {
                currentEvent = String(line.dropFirst(7))
            } else if line.hasPrefix("data: ") {
                currentData = String(line.dropFirst(6))
                // Process immediately once we have both event + data.
                // The SSE spec says to wait for a blank line, but HTTP/2
                // may buffer the trailing \n so it arrives with the next
                // heartbeat (~30 s later). Since our protocol only uses
                // single event+data pairs, this is safe.
                if currentEvent == "sync" && currentData == "pull" {
                    if let handler = onSyncEvent {
                        Task { await handler() }
                    }
                    currentEvent = ""
                    currentData = ""
                }
            } else if line.isEmpty {
                // Blank line — reset state (also handles late-arriving
                // terminators so we don't re-fire).
                currentEvent = ""
                currentData = ""
            }
        }
    }
}

enum SSEError: Error {
    case invalidURL
    case connectionFailed
    /// The stream wouldn't take the token it was given.
    case tokenRefused
}
