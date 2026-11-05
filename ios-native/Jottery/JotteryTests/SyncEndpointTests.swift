import Testing
import Foundation
@testable import Jottery

struct SyncEndpointTests {

    // MARK: - Normalisation

    @Test func addsHttpsWhenSchemeMissing() throws {
        #expect(try SyncEndpoint.normalise("notes.example.org") == "https://notes.example.org")
    }

    @Test func preservesExplicitSchemes() throws {
        #expect(try SyncEndpoint.normalise("https://notes.example.org") == "https://notes.example.org")
        #expect(try SyncEndpoint.normalise("http://192.168.1.10:3000") == "http://192.168.1.10:3000")
    }

    @Test func acceptsUppercaseScheme() throws {
        #expect(try SyncEndpoint.normalise("HTTPS://notes.example.org") == "HTTPS://notes.example.org")
        #expect(try SyncEndpoint.normalise("Http://192.168.1.10:3000") == "Http://192.168.1.10:3000")
    }

    @Test func stripsTrailingSlash() throws {
        #expect(try SyncEndpoint.normalise("https://notes.example.org/") == "https://notes.example.org")
    }

    @Test func trimsSurroundingWhitespace() throws {
        #expect(try SyncEndpoint.normalise("  notes.example.org \n") == "https://notes.example.org")
    }

    @Test func preservesPortAndPathPrefix() throws {
        #expect(try SyncEndpoint.normalise("notes.example.org:8443/jottery")
            == "https://notes.example.org:8443/jottery")
    }

    // MARK: - Rejection

    @Test func rejectsEmptyInput() {
        #expect(throws: SyncEndpointError.empty) { try SyncEndpoint.normalise("") }
        #expect(throws: SyncEndpointError.empty) { try SyncEndpoint.normalise("   ") }
    }

    /// The crash reported in jottery-hp3d.2: a space made URL(string:) return nil,
    /// which the old force-unwrapping apiURL turned into a trap.
    @Test func rejectsInternalWhitespace() {
        #expect(throws: (any Error).self) { try SyncEndpoint.normalise("my server.local") }
        #expect(throws: (any Error).self) { try SyncEndpoint.normalise("https://my server.local") }
    }

    @Test func rejectsMissingHost() {
        #expect(throws: (any Error).self) { try SyncEndpoint.normalise("https://") }
        #expect(throws: (any Error).self) { try SyncEndpoint.normalise("http:///path") }
    }

    @Test func rejectsNonHttpSchemes() {
        #expect(throws: (any Error).self) { try SyncEndpoint.normalise("ftp://notes.example.org") }
        #expect(throws: (any Error).self) { try SyncEndpoint.normalise("javascript:alert(1)") }
    }

    @Test func rejectsControlCharacters() {
        #expect(throws: (any Error).self) { try SyncEndpoint.normalise("notes.example.org\u{0}") }
        #expect(throws: (any Error).self) { try SyncEndpoint.normalise("notes\u{7F}.example.org") }
    }

    // MARK: - Insecure public hosts (ATS)

    /// ATS does not apply to IP literals, unqualified host names or .local,
    /// so plain HTTP to those keeps working once NSAllowsArbitraryLoads is gone.
    @Test func allowsPlainHttpToAtsExemptHosts() throws {
        #expect(try SyncEndpoint.normalise("http://192.168.1.10:3000") == "http://192.168.1.10:3000")
        #expect(try SyncEndpoint.normalise("http://10.0.0.5") == "http://10.0.0.5")
        #expect(try SyncEndpoint.normalise("http://nas:3000") == "http://nas:3000")
        #expect(try SyncEndpoint.normalise("http://jottery.local:3000") == "http://jottery.local:3000")
        #expect(try SyncEndpoint.normalise("http://[fe80::1]:3000") == "http://[fe80::1]:3000")
    }

    /// ATS blocks these, so reject them at setup with an actionable message
    /// rather than letting the request fail opaquely later.
    @Test func rejectsPlainHttpToPublicHosts() {
        #expect(throws: (any Error).self) { try SyncEndpoint.normalise("http://notes.example.org") }
        #expect(throws: (any Error).self) { try SyncEndpoint.normalise("http://notes.example.org:8080/jottery") }
    }

    @Test func allowsHttpsToPublicHosts() throws {
        #expect(try SyncEndpoint.normalise("https://notes.example.org") == "https://notes.example.org")
        #expect(try SyncEndpoint.normalise("notes.example.org") == "https://notes.example.org")
    }

    @Test func classifiesAtsExemptHosts() {
        #expect(SyncEndpoint.allowsPlainHTTP(host: "192.168.1.10"))
        #expect(SyncEndpoint.allowsPlainHTTP(host: "10.0.0.5"))
        #expect(SyncEndpoint.allowsPlainHTTP(host: "fe80::1"))
        #expect(SyncEndpoint.allowsPlainHTTP(host: "nas"))
        #expect(SyncEndpoint.allowsPlainHTTP(host: "jottery.local"))
        #expect(SyncEndpoint.allowsPlainHTTP(host: "JOTTERY.LOCAL"))
        #expect(!SyncEndpoint.allowsPlainHTTP(host: "notes.example.org"))
        #expect(!SyncEndpoint.allowsPlainHTTP(host: "example.com"))
    }

    // MARK: - API URL construction

    @Test func buildsApiURLFromNormalisedEndpoint() throws {
        let url = try SyncEndpoint.apiURL(
            endpoint: "https://notes.example.org",
            apiVersion: "v1",
            path: "sync/push"
        )
        #expect(url.absoluteString == "https://notes.example.org/api/v1/sync/push")
    }

    @Test func buildsApiURLWithPathPrefix() throws {
        let url = try SyncEndpoint.apiURL(
            endpoint: "https://notes.example.org/jottery",
            apiVersion: "v1",
            path: "sync/status"
        )
        #expect(url.absoluteString == "https://notes.example.org/jottery/api/v1/sync/status")
    }

    /// apiURL is defence-in-depth: a bad endpoint that somehow reached the client
    /// must throw rather than trap.
    @Test func apiURLThrowsOnMalformedEndpoint() {
        #expect(throws: (any Error).self) {
            try SyncEndpoint.apiURL(endpoint: "not a url", apiVersion: "v1", path: "sync/push")
        }
    }

    @Test func percentEncodesPathComponents() throws {
        let encoded = SyncEndpoint.encodePathComponent("id with space/../etc")
        #expect(!encoded.contains(" "))
        #expect(!encoded.contains("/"))

        let url = try SyncEndpoint.apiURL(
            endpoint: "https://notes.example.org",
            apiVersion: "v1",
            path: "inbox/\(encoded)"
        )
        #expect(url.absoluteString.hasPrefix("https://notes.example.org/api/v1/inbox/"))
    }

    // MARK: - Plain HTTP stays on the user's own network (R2-095)

    @Test(arguments: [
        "http://127.0.0.1:3000", "http://10.1.2.3", "http://172.16.0.1", "http://172.31.255.255",
        "http://192.168.0.1", "http://169.254.1.1", "http://100.100.1.1",
        "http://[::1]:3000", "http://[fe80::1]", "http://[fd12:3456::1]", "http://[::ffff:192.168.0.1]",
        "http://localhost:3000", "http://nas", "http://jottery.local",
    ])
    func plainHTTPIsAllowedOnAPrivateNetwork(_ address: String) throws {
        #expect(try SyncEndpoint.normalise(address) == address)
    }

    /// The password and API key would cross the internet in the clear.
    @Test(arguments: [
        "http://203.0.113.10:3000", "http://8.8.8.8", "http://172.32.0.1", "http://192.169.0.1",
        "http://100.128.0.1", "http://[2001:db8::1]", "http://[::ffff:8.8.8.8]",
        "http://134744072", "http://0x08080808",
    ])
    func plainHTTPToAPublicAddressIsRefused(_ address: String) {
        #expect(throws: (any Error).self) { try SyncEndpoint.normalise(address) }
    }

    @Test func aPublicAddressIsFineOverHTTPS() throws {
        #expect(try SyncEndpoint.normalise("https://203.0.113.10:3000") == "https://203.0.113.10:3000")
    }
}
