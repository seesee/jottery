import Foundation
import Testing

@testable import Jottery

/// Backups and exports can be far larger than memory, so they're read from
/// their file. The index has to be right whatever order the keys are in, and
/// however the elements fall across the scanner's read chunks.
struct JSONFileScannerTests {

    private func file(_ text: String) throws -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("jottery-scan-\(UUID().uuidString).json")
        try Data(text.utf8).write(to: url)
        return url
    }

    private func elements(_ index: JSONFileScanner.Index, of url: URL) throws -> [String] {
        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }
        return try index.elements.map { String(decoding: try JSONFileScanner.read($0, from: handle), as: UTF8.self) }
    }

    @Test func findsElementsAndOtherValues() throws {
        let url = try file(#"{"version":"3.0","batches":[{"a":1},{"b":[2,3]}],"summary":{"n":2},"count":7,"flag":true}"#)

        let index = try JSONFileScanner.scan(url, array: "batches")

        #expect(try elements(index, of: url) == [#"{"a":1}"#, #"{"b":[2,3]}"#])
        #expect(index.values["version"] == Data(#""3.0""#.utf8))
        #expect(index.values["summary"] == Data(#"{"n":2}"#.utf8))
        #expect(index.values["count"] == Data("7".utf8))
        #expect(index.values["flag"] == Data("true".utf8))
    }

    /// iOS used to write keys in sorted order, so the array can come first.
    @Test func theArrayCanComeBeforeEverythingElse() throws {
        let url = try file(#"{"batches":[{"a":1}],"createdAt":"x","version":"3.0"}"#)

        let index = try JSONFileScanner.scan(url, array: "batches")

        #expect(index.elements.count == 1)
        #expect(index.values["version"] == Data(#""3.0""#.utf8))
    }

    @Test func copesWithWhitespaceAndNewlines() throws {
        let url = try file("{\n\"version\" : \"1.0\",\n\"notes\" : [\n  {\"a\": 1} ,\n  {\"b\": 2}\n]\n}\n")

        let index = try JSONFileScanner.scan(url, array: "notes")

        #expect(try elements(index, of: url) == [#"{"a": 1}"#, #"{"b": 2}"#])
        #expect(index.values["version"] == Data(#""1.0""#.utf8))
    }

    @Test func bracesAndQuotesInStringsAreJustText() throws {
        let tricky = #"{"text":"a } ] \" { [ \\"}"#
        let url = try file(#"{"notes":["# + tricky + #","plain",3]}"#)

        let index = try JSONFileScanner.scan(url, array: "notes")

        #expect(try elements(index, of: url) == [tricky, #""plain""#, "3"])
    }

    @Test func anEmptyArrayHasNoElements() throws {
        let url = try file(#"{"batches":[],"version":"3.0"}"#)

        #expect(try JSONFileScanner.scan(url, array: "batches").elements.isEmpty)
    }

    @Test func elementsCanSpanReadChunks() throws {
        let long = String(repeating: "x", count: 3 << 20)
        let url = try file(#"{"notes":[{"a":"\#(long)"},{"b":1}],"version":"1.0"}"#)

        let index = try JSONFileScanner.scan(url, array: "notes")

        #expect(try elements(index, of: url) == [#"{"a":"\#(long)"}"#, #"{"b":1}"#])
        #expect(index.values["version"] == Data(#""1.0""#.utf8))
    }

    @Test(arguments: [
        "",
        "[1,2]",
        #"{"notes":[{"a":1}"#,
        #"{"notes":[{"a":"unterminated"#,
        #"{"version" "1.0"}"#,
        #"{"a":1} trailing"#,
    ])
    func refusesWhatIsntOneCompleteObject(text: String) throws {
        let url = try file(text)

        #expect(throws: JSONFileScanner.ScanError.malformed) {
            _ = try JSONFileScanner.scan(url, array: "notes")
        }
    }
}
