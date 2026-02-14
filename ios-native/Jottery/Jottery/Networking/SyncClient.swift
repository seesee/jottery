import Foundation

/// HTTP client for the Jottery sync server.
/// All methods are async and throw `SyncClientError` on failure.
actor SyncClient {

    private let session: URLSession
    private let apiVersion = "v1"
    private let timeoutInterval: TimeInterval = 30

    var endpoint: String
    var apiKey: String?

    init(endpoint: String = "", apiKey: String? = nil) {
        self.endpoint = endpoint.hasSuffix("/") ? String(endpoint.dropLast()) : endpoint
        self.apiKey = apiKey
        let config = URLSessionConfiguration.default
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
        let url = apiURL("auth/register-device")
        let body = RegisterDeviceRequest(
            email: email,
            password: password,
            deviceName: deviceName,
            deviceType: "ios"
        )
        return try await post(url: url, body: body, authenticated: false)
    }

    func cloneDevice(apiKey: String, deviceName: String) async throws -> RegisterDeviceResponse {
        let url = apiURL("auth/clone-device")
        let body = CloneDeviceRequest(
            apiKey: apiKey,
            deviceName: deviceName,
            deviceType: "ios"
        )
        return try await post(url: url, body: body, authenticated: false)
    }

    // MARK: - Sync Operations

    func push(_ request: SyncPushRequest) async throws -> SyncPushResponse {
        let url = apiURL("sync/push")
        return try await post(url: url, body: request, authenticated: true)
    }

    func pull(_ request: SyncPullRequest) async throws -> SyncPullResponse {
        let url = apiURL("sync/pull")
        return try await post(url: url, body: request, authenticated: true)
    }

    func status() async throws -> SyncStatusResponse {
        let url = apiURL("sync/status")
        return try await get(url: url)
    }

    /// Get a short-lived SSE token.
    func getSSEToken() async throws -> SSETokenResponse {
        let url = apiURL("sync/events/token")
        return try await get(url: url)
    }

    /// Build the SSE URL with the given token.
    func sseURL(token: String) -> URL? {
        URL(string: "\(endpoint)/api/\(apiVersion)/sync/events?token=\(token)")
    }

    // MARK: - Private

    private func apiURL(_ path: String) -> URL {
        URL(string: "\(endpoint)/api/\(apiVersion)/\(path)")!
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

        print("[SyncClient] POST \(url.absoluteString)")
        do {
            let (data, response) = try await session.data(for: request)
            if let http = response as? HTTPURLResponse {
                print("[SyncClient] Response: \(http.statusCode)")
            }
            try validateResponse(response, data: data)
            return try JSONDecoder().decode(Response.self, from: data)
        } catch {
            print("[SyncClient] Error: \(error)")
            throw error
        }
    }

    private func get<Response: Decodable>(url: URL) async throws -> Response {
        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        if let apiKey {
            request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        }

        print("[SyncClient] GET \(url.absoluteString)")
        do {
            let (data, response) = try await session.data(for: request)
            if let http = response as? HTTPURLResponse {
                print("[SyncClient] Response: \(http.statusCode)")
            }
            try validateResponse(response, data: data)
            return try JSONDecoder().decode(Response.self, from: data)
        } catch {
            print("[SyncClient] Error: \(error)")
            throw error
        }
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

    var errorDescription: String? {
        switch self {
        case .invalidResponse: return "Invalid server response"
        case .httpError(let status, let body): return "HTTP \(status): \(body)"
        case .unauthorized: return "Unauthorised — check API key"
        }
    }
}
