import Foundation

/// Validation and normalisation for user-supplied sync server addresses.
///
/// Users type these by hand during setup, so nothing here may trap: every
/// failure path returns a `SyncEndpointError` the UI can show inline.
enum SyncEndpoint {

    /// Trim, default to HTTPS when no scheme is given, drop a trailing slash,
    /// and reject anything that cannot form an absolute http(s) URL.
    ///
    /// - Throws: `SyncEndpointError` when the address is empty or malformed.
    static func normalise(_ raw: String) throws -> String {
        var candidate = raw.trimmingCharacters(in: .whitespacesAndNewlines)

        guard !candidate.isEmpty else {
            throw SyncEndpointError.empty
        }

        // Default to HTTPS only when no scheme was given. An explicit non-http(s)
        // scheme must be rejected outright — prefixing it would yield a string that
        // URLComponents parses as host "ftp" with a junk path.
        // Comparison is case-insensitive: "HTTPS://host" is a valid thing to type.
        if let separator = candidate.range(of: "://") {
            let scheme = candidate[..<separator.lowerBound].lowercased()
            guard scheme == "http" || scheme == "https" else {
                throw SyncEndpointError.malformed(raw)
            }
        } else {
            candidate = "https://\(candidate)"
        }

        if candidate.hasSuffix("/") {
            candidate = String(candidate.dropLast())
        }

        // URL(string:) is strict from iOS 17 but has been permissive in the past,
        // so reject whitespace and control characters explicitly rather than
        // relying on the parser to do it.
        guard candidate.rangeOfCharacter(from: .whitespacesAndNewlines) == nil,
              candidate.rangeOfCharacter(from: .controlCharacters) == nil else {
            throw SyncEndpointError.malformed(raw)
        }

        guard let components = URLComponents(string: candidate),
              let scheme = components.scheme?.lowercased(),
              scheme == "http" || scheme == "https",
              let host = components.host,
              !host.isEmpty else {
            throw SyncEndpointError.malformed(raw)
        }

        // ATS only guards connections to public host names. Plain HTTP to an IP
        // literal, an unqualified name or a .local host is exempt and keeps working;
        // plain HTTP to a public host is blocked, so reject it here with something
        // actionable rather than letting the request fail opaquely later.
        if scheme == "http" && !isATSExempt(host: host) {
            throw SyncEndpointError.insecurePublicHost(host)
        }

        // Confirm the result can actually build a request URL before we store it.
        // Rethrown against `raw` so the message quotes what the user typed.
        guard (try? apiURL(endpoint: candidate, apiVersion: "v1", path: "sync/status")) != nil else {
            throw SyncEndpointError.malformed(raw)
        }

        return candidate
    }

    /// Build an API URL from an already-normalised endpoint.
    ///
    /// - Throws: `SyncEndpointError.malformed` rather than trapping, so a bad
    ///   endpoint that reached the client via stored settings surfaces as an error.
    static func apiURL(endpoint: String, apiVersion: String, path: String) throws -> URL {
        guard let url = URL(string: "\(endpoint)/api/\(apiVersion)/\(path)"),
              url.scheme != nil,
              url.host != nil else {
            throw SyncEndpointError.malformed(endpoint)
        }
        return url
    }

    /// Whether App Transport Security leaves connections to this host alone.
    ///
    /// ATS applies only to public host names. Apple exempts IP literals,
    /// unqualified (single-label) host names, and the `.local` TLD — which
    /// between them cover the self-hosted-on-the-LAN cases.
    static func isATSExempt(host: String) -> Bool {
        let name = host.lowercased()

        if name.hasSuffix(".local") { return true }
        if isIPLiteral(name) { return true }
        // Unqualified: a single label with no dots, e.g. "nas".
        if !name.contains(".") { return true }

        return false
    }

    private static func isIPLiteral(_ host: String) -> Bool {
        // URLComponents.host keeps the brackets on an IPv6 literal ("[fe80::1]"),
        // so strip them before handing the address to inet_pton.
        let bare = host.hasPrefix("[") && host.hasSuffix("]")
            ? String(host.dropFirst().dropLast())
            : host

        var v4 = in_addr()
        if bare.withCString({ inet_pton(AF_INET, $0, &v4) }) == 1 { return true }

        var v6 = in6_addr()
        // Strip any zone identifier ("fe80::1%en0") before parsing.
        let withoutZone = bare.split(separator: "%", maxSplits: 1).first.map(String.init) ?? bare
        if withoutZone.withCString({ inet_pton(AF_INET6, $0, &v6) }) == 1 { return true }

        return false
    }

    /// Message to show for a failed sync.
    ///
    /// Endpoints saved before the ATS exception was narrowed may still be plain
    /// HTTP to a public host. Those now fail with an opaque ATS error, so name the
    /// real problem instead of surfacing it raw.
    static func describeSyncFailure(_ error: Error) -> String {
        if let urlError = error as? URLError,
           urlError.code == .appTransportSecurityRequiresSecureConnection {
            return L.syncEndpointInsecureStored
        }
        return error.localizedDescription
    }

    /// Percent-encode a value being interpolated into a URL path segment.
    static func encodePathComponent(_ raw: String) -> String {
        var allowed = CharacterSet.urlPathAllowed
        allowed.remove("/")
        return raw.addingPercentEncoding(withAllowedCharacters: allowed) ?? ""
    }
}

enum SyncEndpointError: LocalizedError, Equatable {
    case empty
    case malformed(String)
    case insecurePublicHost(String)

    var errorDescription: String? {
        switch self {
        case .empty:
            return L.syncEndpointEmpty
        case .malformed(let raw):
            return String(format: L.syncEndpointMalformed, raw)
        case .insecurePublicHost(let host):
            return String(format: L.syncEndpointInsecurePublicHost, host)
        }
    }
}
