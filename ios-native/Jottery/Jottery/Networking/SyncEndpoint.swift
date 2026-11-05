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

        // The account password and API key travel in the clear over plain HTTP,
        // so it's only for addresses on the user's own network.
        if scheme == "http" && !allowsPlainHTTP(host: host) {
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

    /// Whether plain HTTP to this host can stay on the user's own network:
    /// loopback, private and link-local addresses, `.local` names and
    /// unqualified (single-label) host names.
    ///
    /// ATS doesn't stop plain HTTP to any IP address, so a public one has to be
    /// refused here. An endpoint saved before this check keeps working.
    static func allowsPlainHTTP(host: String) -> Bool {
        let name = host.lowercased()

        if name.hasSuffix(".local") { return true }
        if let octets = ipv4Octets(name) { return isPrivate(octets) }
        if let bytes = ipv6Bytes(name) { return isPrivate(ipv6: bytes) }
        // Unqualified: a single label with no dots, e.g. "nas".
        return !name.contains(".")
    }

    /// `inet_aton` also takes the forms the system's resolver accepts, like
    /// `134744072` and `0x7f.1`, which would otherwise pass for host names.
    private static func ipv4Octets(_ host: String) -> [UInt8]? {
        var address = in_addr()
        guard host.withCString({ inet_aton($0, &address) }) == 1 else { return nil }
        return withUnsafeBytes(of: address.s_addr) { Array($0) }
    }

    private static func ipv6Bytes(_ host: String) -> [UInt8]? {
        // URLComponents.host keeps the brackets on an IPv6 literal ("[fe80::1]"),
        // so strip them before handing the address to inet_pton.
        let bare = host.hasPrefix("[") && host.hasSuffix("]")
            ? String(host.dropFirst().dropLast())
            : host
        // Strip any zone identifier ("fe80::1%en0") before parsing.
        let withoutZone = bare.split(separator: "%", maxSplits: 1).first.map(String.init) ?? bare
        var address = in6_addr()
        guard withoutZone.withCString({ inet_pton(AF_INET6, $0, &address) }) == 1 else { return nil }
        return withUnsafeBytes(of: address) { Array($0) }
    }

    /// Loopback, RFC 1918, link-local, and the carrier-grade range that
    /// Tailscale and other overlay networks use.
    private static func isPrivate(_ octets: [UInt8]) -> Bool {
        switch (octets[0], octets[1]) {
        case (127, _), (10, _), (192, 168), (169, 254): return true
        case (172, 16...31), (100, 64...127): return true
        default: return false
        }
    }

    /// Loopback, link-local, unique local, and an IPv4 address written as IPv6.
    private static func isPrivate(ipv6 bytes: [UInt8]) -> Bool {
        if bytes[..<15].allSatisfy({ $0 == 0 }) && bytes[15] == 1 { return true }
        if bytes[0] == 0xFE && bytes[1] & 0xC0 == 0x80 { return true }
        if bytes[0] & 0xFE == 0xFC { return true }
        if bytes[..<10].allSatisfy({ $0 == 0 }) && bytes[10] == 0xFF && bytes[11] == 0xFF {
            return isPrivate(Array(bytes[12...]))
        }
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

    /// The server's name, for telling the user where something will go.
    static func displayHost(_ endpoint: String) -> String {
        URLComponents(string: endpoint)?.host ?? endpoint
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
