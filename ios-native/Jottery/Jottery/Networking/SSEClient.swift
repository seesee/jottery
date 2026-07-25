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
    private let sseSession: URLSession = {
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 45   // Must receive data within 45 s (heartbeat is 30 s)
        config.timeoutIntervalForResource = 0   // No overall resource timeout
        config.requestCachePolicy = .reloadIgnoringLocalAndRemoteCacheData
        return URLSession(configuration: config)
    }()

    var onSyncEvent: (@Sendable () async -> Void)?

    init(syncClient: SyncClient) {
        self.syncClient = syncClient
    }

    /// Start listening for SSE events.
    func start() {
        stop()
        reconnectAttempts = 0
        task = Task { await connectLoop() }
    }

    /// Stop listening.
    func stop() {
        task?.cancel()
        task = nil
        reconnectAttempts = 0
    }

    // MARK: - Private

    private func connectLoop() async {
        while !Task.isCancelled {
            do {
                try await connect()
                reconnectAttempts = 0
            } catch {
                if Task.isCancelled { return }
                reconnectAttempts += 1
                let delay = min(pow(2.0, Double(min(reconnectAttempts, 5))), maxBackoffSeconds)
                Log.debug("[SSE] connection error (attempt \(reconnectAttempts)), retrying in \(delay)s")
                try? await Task.sleep(for: .seconds(delay))
            }
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
            throw SSEError.connectionFailed
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
}
