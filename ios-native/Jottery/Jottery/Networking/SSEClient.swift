import Foundation

/// Server-Sent Events client using URLSession bytes streaming.
/// Listens for `event: sync` / `data: pull` messages to trigger sync pulls.
/// Retries indefinitely with capped exponential backoff.
actor SSEClient {

    private var task: Task<Void, Never>?
    private let syncClient: SyncClient
    private var reconnectAttempts = 0
    private let maxBackoffSeconds: Double = 30

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
                // Connection ended normally — reset backoff
                reconnectAttempts = 0
            } catch {
                if Task.isCancelled { return }
                reconnectAttempts += 1
                // Cap exponent at 5 (2^5 = 32) to stabilise backoff quickly
                let delay = min(pow(2.0, Double(min(reconnectAttempts, 5))), maxBackoffSeconds)
                try? await Task.sleep(for: .seconds(delay))
            }
        }
    }

    private func connect() async throws {
        // Get a short-lived token
        let tokenResponse = try await syncClient.getSSEToken()
        guard let url = await syncClient.sseURL(token: tokenResponse.token) else {
            throw SSEError.invalidURL
        }

        var request = URLRequest(url: url)
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        request.timeoutInterval = 300 // 5 minute timeout for long-lived connection

        let (bytes, response) = try await URLSession.shared.bytes(for: request)
        guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
            throw SSEError.connectionFailed
        }

        var currentEvent = ""
        var currentData = ""

        for try await line in bytes.lines {
            if Task.isCancelled { return }

            if line.hasPrefix("event: ") {
                currentEvent = String(line.dropFirst(7))
            } else if line.hasPrefix("data: ") {
                currentData = String(line.dropFirst(6))
            } else if line.isEmpty {
                // End of event — process it
                if currentEvent == "sync" && currentData == "pull" {
                    await onSyncEvent?()
                }
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
