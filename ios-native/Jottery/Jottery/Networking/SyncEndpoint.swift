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

    var errorDescription: String? {
        switch self {
        case .empty:
            return L.syncEndpointEmpty
        case .malformed(let raw):
            return String(format: L.syncEndpointMalformed, raw)
        }
    }
}
