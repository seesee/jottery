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
}
