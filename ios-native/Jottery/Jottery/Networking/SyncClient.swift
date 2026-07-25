import Foundation

/// HTTP client for the Jottery sync server.
/// All methods are async and throw `SyncClientError` on failure.
actor SyncClient {

    private let session: URLSession
    private let apiVersion = "v1"
    private let timeoutInterval: TimeInterval = 30

    var endpoint: String
    var apiKey: String?

    /// - Parameter configuration: overrides the URLSession configuration. Only
    ///   used by tests, to install a stub `URLProtocol`.
    init(
        endpoint: String = "",
        apiKey: String? = nil,
        configuration: URLSessionConfiguration? = nil
    ) {
        self.endpoint = endpoint.hasSuffix("/") ? String(endpoint.dropLast()) : endpoint
        self.apiKey = apiKey
        let config = configuration ?? URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 30
        config.timeoutIntervalForResource = 60
        self.session = URLSession(configuration: config)
    }

    func updateCredentials(endpoint: String, apiKey: String) {
        self.endpoint = endpoint.hasSuffix("/") ? String(endpoint.dropLast()) : endpoint
        self.apiKey = apiKey
    }

    // MARK: - Device Registration

    func registerDevice(email: String, password: String, deviceName: String) async throws -> RegisterDeviceResponse {
        let url = try apiURL("auth/register-device")
        let body = RegisterDeviceRequest(
            email: email,
            password: password,
            deviceName: deviceName,
            deviceType: "ios"
        )
        return try await post(url: url, body: body, authenticated: false)
    }

    func cloneDevice(apiKey: String, deviceName: String) async throws -> RegisterDeviceResponse {
        let url = try apiURL("auth/clone-device")
        let body = CloneDeviceRequest(
            apiKey: apiKey,
            deviceName: deviceName,
            deviceType: "ios"
        )
        return try await post(url: url, body: body, authenticated: false)
    }

    // MARK: - Sync Operations

    func push(_ request: SyncPushRequest) async throws -> SyncPushResponse {
        let url = try apiURL("sync/push")
        return try await post(url: url, body: request, authenticated: true)
    }

    func pull(_ request: SyncPullRequest) async throws -> SyncPullResponse {
        let url = try apiURL("sync/pull")
        return try await post(url: url, body: request, authenticated: true)
    }

    func status() async throws -> SyncStatusResponse {
        let url = try apiURL("sync/status")
        return try await get(url: url)
    }

    /// Get a short-lived SSE token.
    func getSSEToken() async throws -> SSETokenResponse {
        let url = try apiURL("sync/events/token")
        return try await get(url: url)
    }

    /// Build the SSE URL with the given token.
    func sseURL(token: String) -> URL? {
        URL(string: "\(endpoint)/api/\(apiVersion)/sync/events?token=\(token)")
    }

    // MARK: - Wrapped Key (Envelope Encryption)

    /// Fetch the server-stored wrapped master key. Returns nil if no key exists (404).
    func getWrappedKey() async throws -> WrappedKeyResponse? {
        let url = try apiURL("auth/wrapped-key")
        do {
            let response: WrappedKeyResponse = try await get(url: url)
            return response
        } catch SyncClientError.httpError(let status, _) where status == 404 {
            return nil
        }
    }

    /// Upload a wrapped master key to the server.
    func putWrappedKey(_ request: PutWrappedKeyRequest) async throws {
        let url = try apiURL("auth/wrapped-key")
        try await put(url: url, body: request)
    }

    // MARK: - Device

    /// Unlink this device from the server, using its own API key.
    ///
    /// Best-effort by design: users self-host, and a server older than this
    /// endpoint answers 404/405. `deviceRevokeUnsupported` lets the caller tell
    /// "your server is too old" apart from a genuine failure, so disconnecting
    /// locally can proceed either way.
    func revokeThisDevice() async throws {
        let url = try apiURL("sync/device")
        do {
            try await delete(url: url)
        } catch SyncClientError.httpError(let status, _) where status == 404 || status == 405 {
            throw SyncClientError.deviceRevokeUnsupported
        }
    }

    // MARK: - Inbox

    func listInboxItems() async throws -> [InboxItem] {
        let url = try apiURL("inbox")
        return try await get(url: url)
    }

    func deleteInboxItem(id: String) async throws {
        let url = try apiURL("inbox/\(SyncEndpoint.encodePathComponent(id))")
        try await delete(url: url)
    }

    func deleteAllInboxItems() async throws {
        let url = try apiURL("inbox")
        try await delete(url: url)
    }

    // MARK: - Private

    private func apiURL(_ path: String) throws -> URL {
        try SyncEndpoint.apiURL(endpoint: endpoint, apiVersion: apiVersion, path: path)
    }

    private func post<Body: Encodable, Response: Decodable>(
        url: URL,
        body: Body,
        authenticated: Bool
    ) async throws -> Response {
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        if authenticated, let apiKey {
            request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        }

        let encoder = JSONEncoder()
        request.httpBody = try encoder.encode(body)

        let (data, response) = try await session.data(for: request)
        try validateResponse(response, data: data)

        return try JSONDecoder().decode(Response.self, from: data)
    }

    private func put<Body: Encodable>(url: URL, body: Body) async throws {
        var request = URLRequest(url: url)
        request.httpMethod = "PUT"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        if let apiKey {
            request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        }

        let encoder = JSONEncoder()
        request.httpBody = try encoder.encode(body)

        let (data, response) = try await session.data(for: request)
        try validateResponse(response, data: data)
    }

    private func get<Response: Decodable>(url: URL) async throws -> Response {
        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        if let apiKey {
            request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        }

        let (data, response) = try await session.data(for: request)
        try validateResponse(response, data: data)

        return try JSONDecoder().decode(Response.self, from: data)
    }

    private func delete(url: URL) async throws {
        var request = URLRequest(url: url)
        request.httpMethod = "DELETE"
        if let apiKey {
            request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        }

        let (data, response) = try await session.data(for: request)
        try validateResponse(response, data: data)
    }

    private func validateResponse(_ response: URLResponse, data: Data) throws {
        guard let http = response as? HTTPURLResponse else {
            throw SyncClientError.invalidResponse
        }
        guard (200...299).contains(http.statusCode) else {
            let body = String(data: data, encoding: .utf8) ?? ""
            throw SyncClientError.httpError(status: http.statusCode, body: body)
        }
    }
}

enum SyncClientError: LocalizedError {
    case invalidResponse
    case httpError(status: Int, body: String)
    case unauthorized
    case deviceRevokeUnsupported

    var errorDescription: String? {
        switch self {
        case .invalidResponse: return "Invalid server response"
        case .httpError(let status, let body): return "HTTP \(status): \(body)"
        case .unauthorized: return "Unauthorised — check API key"
        case .deviceRevokeUnsupported: return L.syncDisconnectServerTooOld
        }
    }
}
