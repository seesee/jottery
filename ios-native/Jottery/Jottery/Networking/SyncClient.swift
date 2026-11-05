import Foundation

/// HTTP client for the Jottery sync server.
/// All methods are async and throw `SyncClientError` on failure.
actor SyncClient {

    private let session: URLSession
    private let apiVersion = "v1"
    private let timeoutInterval: TimeInterval = 30

    var endpoint: String
    var apiKey: String?

    /// - Parameter configuration: replaces the URLSession configuration, timeouts
    ///   included. Only used by tests, to install a stub `URLProtocol`.
    init(
        endpoint: String = "",
        apiKey: String? = nil,
        configuration: URLSessionConfiguration? = nil
    ) {
        self.endpoint = endpoint.hasSuffix("/") ? String(endpoint.dropLast()) : endpoint
        self.apiKey = apiKey
        self.session = URLSession(configuration: configuration ?? {
            let config = URLSessionConfiguration.default
            config.timeoutIntervalForRequest = 30
            config.timeoutIntervalForResource = 60
            return config
        }())
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

    // MARK: - Account Registration

    /// Create an account. Most servers then hold it for an administrator to
    /// approve, and a device can't connect until they have.
    func registerUser(email: String, password: String) async throws -> RegisterUserResponse {
        let url = try apiURL("auth/register-user")
        do {
            return try await post(url: url, body: UserLoginRequest(email: email, password: password), authenticated: false)
        } catch SyncClientError.httpError(let status, let body) where status == 409 || status == 400 {
            throw status == 409 ? SyncClientError.accountExists : SyncClientError.refused(Self.message(in: body))
        }
    }

    /// Where the account has got to. Asks with the password where the server
    /// can, which tells nobody else whether the address has an account.
    func userStatus(email: String, password: String) async throws -> UserStatusResponse {
        do {
            return try await post(
                url: try apiURL("user/status"),
                body: UserLoginRequest(email: email, password: password),
                authenticated: false
            )
        } catch SyncClientError.httpError(let status, _) where status == 404 || status == 405 {
            var components = URLComponents(url: try apiURL("user/status"), resolvingAgainstBaseURL: false)
            components?.queryItems = [URLQueryItem(name: "email", value: email)]
            guard let url = components?.url else { throw SyncClientError.invalidResponse }
            return try await get(url: url)
        }
    }

    /// The server's own words for a refusal, which only it can give: its
    /// password rules are configurable. Plain text, kept short.
    private static func message(in body: String) -> String {
        let error = (try? JSONDecoder().decode(ErrorBody.self, from: Data(body.utf8)))?.error
        return String((error ?? body).prefix(200))
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

    /// What the server calls this device, fit to show. Nil if it can't say.
    func deviceName() async -> String? {
        (try? await status()).flatMap { SyncValidation.displayName($0.deviceName) }
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

    // MARK: - Attachments

    /// One attachment's blob, or nil if the server has none for this user.
    func fetchAttachment(id: String) async throws -> SyncAttachment? {
        do {
            return try await get(url: attachmentURL(id: id))
        } catch SyncClientError.httpError(let status, _) where status == 404 {
            return nil
        }
    }

    /// Whether the server holds an attachment's blob, without downloading it.
    /// Anything other than a yes or a no throws. A proxy that won't pass a HEAD
    /// gets the full request instead.
    func hasAttachment(id: String) async throws -> Bool {
        var request = URLRequest(url: try attachmentURL(id: id))
        request.httpMethod = "HEAD"
        if let apiKey {
            request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        }
        do {
            let (data, response) = try await session.data(for: request)
            try validateResponse(response, data: data)
            return true
        } catch SyncClientError.httpError(let status, _) where status == 404 {
            return false
        } catch SyncClientError.httpError(let status, _) where status == 405 {
            return try await fetchAttachment(id: id) != nil
        }
    }

    private func attachmentURL(id: String) throws -> URL {
        try apiURL("sync/attachments/\(SyncEndpoint.encodePathComponent(id))")
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

    /// Upload a wrapped master key, saying what the upload is for. Throws
    /// `wrappedKeyConflict` if the server refuses it for that reason: read the
    /// key again and decide again, rather than retrying.
    func putWrappedKey(_ request: PutWrappedKeyRequest, as write: WrappedKeyWrite) async throws {
        let url = try apiURL("auth/wrapped-key")
        var body = request
        var ifMatch: String?
        switch write {
        case .firstUpload:
            body.replace = false
        case .repair(let updatedAt):
            ifMatch = SyncValidation.opaqueToken(updatedAt)
            // An older server has no updatedAt and ignores this. One that has
            // since been upgraded refuses it, which is the safe way to be wrong.
            if ifMatch == nil { body.replace = false }
        case .passwordChange:
            body.replace = true
        }
        do {
            try await put(url: url, body: body, ifMatch: ifMatch)
        } catch SyncClientError.httpError(let status, _) where status == 409 {
            throw SyncClientError.wrappedKeyConflict
        }
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

    // MARK: - Account

    /// Exchange account credentials for a session token.
    ///
    /// Account-level operations sit behind session auth on the server; a device
    /// API key is not sufficient. Requiring the account password for something
    /// irreversible is the right bar anyway — a stolen device key must not be
    /// able to delete the account.
    func logIn(email: String, password: String) async throws -> String {
        let url = try apiURL("user/login")
        let response: UserLoginResponse = try await post(
            url: url,
            body: UserLoginRequest(email: email, password: password),
            authenticated: false
        )
        return response.sessionId
    }

    /// Delete or deactivate the account this device syncs with.
    ///
    /// Uses a session token from `logIn`, not the device API key. Sending the
    /// password means this doesn't depend on having signed in a moment ago. An
    /// older server ignores it.
    func deleteAccount(sessionId: String, mode: AccountDeletionMode, currentPassword: String) async throws {
        let url = try apiURL("user/account?mode=\(mode.rawValue)")
        var request = URLRequest(url: url)
        request.httpMethod = "DELETE"
        request.setValue("Bearer \(sessionId)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(DeleteAccountRequest(currentPassword: currentPassword))

        let (data, response) = try await session.data(for: request)
        try validateResponse(response, data: data)
    }

    /// Log in and delete or deactivate the account, with one password for both,
    /// so the second can't drift from the first.
    func deleteAccount(email: String, password: String, mode: AccountDeletionMode) async throws {
        let sessionId = try await logIn(email: email, password: password)
        try await deleteAccount(sessionId: sessionId, mode: mode, currentPassword: password)
    }

    /// What the account holds, and how much of its quota that is.
    func accountInfo(sessionId: String) async throws -> AccountInfo {
        let (data, response) = try await session.data(for: sessionRequest("user/account", "GET", sessionId))
        try validateResponse(response, data: data)
        return try JSONDecoder().decode(AccountInfo.self, from: data)
    }

    /// Delete every note the account has on the server. Devices keep their
    /// copies. The password goes with it, as for deleting the account.
    func deleteServerNotes(sessionId: String, currentPassword: String) async throws {
        var request = try sessionRequest("user/notes", "DELETE", sessionId)
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(DeleteAccountRequest(currentPassword: currentPassword))

        let (data, response) = try await session.data(for: request)
        try validateResponse(response, data: data)
    }

    /// A new inbox token, replacing any there was. The server keeps only its
    /// hash, so this is the only time it's seen. Callers must not log or store it.
    func generateInboxToken(sessionId: String) async throws -> String {
        let (data, response) = try await session.data(for: sessionRequest("user/inbox-token", "POST", sessionId))
        try validateResponse(response, data: data)
        return try JSONDecoder().decode(InboxTokenResponse.self, from: data).token
    }

    func revokeInboxToken(sessionId: String) async throws {
        let (data, response) = try await session.data(for: sessionRequest("user/inbox-token", "DELETE", sessionId))
        try validateResponse(response, data: data)
    }

    /// End a session from `logIn`.
    func logOut(sessionId: String) async throws {
        let (data, response) = try await session.data(for: sessionRequest("user/logout", "POST", sessionId))
        try validateResponse(response, data: data)
    }

    private func sessionRequest(_ path: String, _ method: String, _ sessionId: String) throws -> URLRequest {
        var request = URLRequest(url: try apiURL(path))
        request.httpMethod = method
        request.setValue("Bearer \(sessionId)", forHTTPHeaderField: "Authorization")
        return request
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

    private func put<Body: Encodable>(url: URL, body: Body, ifMatch: String? = nil) async throws {
        var request = URLRequest(url: url)
        request.httpMethod = "PUT"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        if let ifMatch {
            request.setValue(ifMatch, forHTTPHeaderField: "If-Match")
        }
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
            let details = try? JSONDecoder().decode(ErrorBody.self, from: data)
            switch http.statusCode {
            case 403:
                if details?.code == "password_change_required" { throw SyncClientError.passwordChangeRequired }
                if details?.error?.localizedCaseInsensitiveContains("pending") == true {
                    throw SyncClientError.accountPendingApproval
                }
                throw SyncClientError.accountDisabled
            case 408:
                throw SyncClientError.requestTimedOut
            case 429:
                throw SyncClientError.rateLimited(retryAfter: Self.waitTime(http, details))
            case 503:
                throw SyncClientError.serverBusy(retryAfter: Self.waitTime(http, details))
            case 507:
                throw SyncClientError.storageQuotaExceeded
            default:
                throw SyncClientError.httpError(status: http.statusCode, body: String(data: data, encoding: .utf8) ?? "")
            }
        }
    }

    /// The wait the server asked for, kept to something sensible: it can't be
    /// allowed to stop this device syncing for a year.
    private static func waitTime(_ response: HTTPURLResponse, _ details: ErrorBody?) -> TimeInterval {
        let header = response.value(forHTTPHeaderField: "Retry-After").flatMap(TimeInterval.init)
        return min(max(header ?? details?.retryAfter.map(TimeInterval.init) ?? 60, 1), 3600)
    }

    /// The server's error shape. `error` is its own wording, used only to tell
    /// cases apart: what the user sees comes from the app.
    private struct ErrorBody: Decodable {
        var error: String?
        var code: String?
        var retryAfter: Int?
    }
}

enum SyncClientError: LocalizedError {
    case invalidResponse
    case httpError(status: Int, body: String)
    case unauthorized
    case deviceRevokeUnsupported
    /// 403: the key or password is right, but the account is deactivated.
    /// Keep the credentials: reactivating the account restores them.
    case accountDisabled
    /// 403: the account hasn't been approved yet.
    case accountPendingApproval
    /// 403: the account must change its password before anything else.
    case passwordChangeRequired
    /// 429: wait this many seconds before trying again.
    case rateLimited(retryAfter: TimeInterval)
    /// 503: the server is busy. Wait this many seconds before trying again.
    case serverBusy(retryAfter: TimeInterval)
    /// 408: the request took too long. Sending the same one again is safe.
    case requestTimedOut
    /// 507: the account is at its storage quota. Nothing was written, though a
    /// push that only deletes still works.
    case storageQuotaExceeded
    /// 409 to a wrapped key upload: the key isn't as this device last read it.
    case wrappedKeyConflict
    /// 409 to creating an account: the address has one.
    case accountExists
    /// 400 to creating an account, with the server's reason.
    case refused(String)

    /// How long the server asked for it to be left alone, if it did.
    var requestedWait: TimeInterval? {
        switch self {
        case .rateLimited(let wait), .serverBusy(let wait): return wait
        default: return nil
        }
    }

    var errorDescription: String? {
        switch self {
        case .invalidResponse: return L.syncErrorInvalidResponse
        case .httpError(let status, let body): return "HTTP \(status): \(body)"
        case .unauthorized: return L.syncErrorUnauthorised
        case .deviceRevokeUnsupported: return L.syncDisconnectServerTooOld
        case .accountDisabled: return L.syncErrorAccountDisabled
        case .accountPendingApproval: return L.syncErrorAccountPending
        case .passwordChangeRequired: return L.syncErrorPasswordChangeRequired
        case .rateLimited(let wait): return L.syncErrorRateLimited(Int(wait.rounded(.up)))
        case .serverBusy(let wait): return L.syncErrorServerBusy(Int(wait.rounded(.up)))
        case .requestTimedOut: return L.syncErrorTimedOut
        case .storageQuotaExceeded: return L.syncErrorStorageFull
        case .wrappedKeyConflict: return L.syncErrorKeyChanged
        case .accountExists: return L.syncErrorAccountExists
        case .refused(let reason): return reason
        }
    }
}
