import Foundation

/// Finds its way around a JSON file too large to load, in one pass and a
/// fixed amount of memory.
///
/// Backups and exports are one JSON object with one huge array in it. A scan
/// keeps the object's other values and notes where each array element is, so
/// the elements can then be read one at a time.
enum JSONFileScanner {

    struct Index: Sendable {
        /// The raw JSON of each top-level value, except the array.
        var values: [String: Data] = [:]
        /// Where each element of the array is in the file.
        var elements: [Range<UInt64>] = []
    }

    enum ScanError: Error {
        case malformed
    }

    private static let chunkSize = 1 << 20

    /// `arrayKey` names the top-level array to index. Throws
    /// `ScanError.malformed` if the file isn't one complete JSON object.
    static func scan(_ url: URL, array arrayKey: String) throws -> Index {
        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }

        var scanner = Scanner(arrayKey: Array(arrayKey.utf8))
        var offset: UInt64 = 0
        while let chunk = try handle.read(upToCount: chunkSize), !chunk.isEmpty {
            try chunk.withUnsafeBytes { (bytes: UnsafeRawBufferPointer) in
                try scanner.consume(bytes.bindMemory(to: UInt8.self), startingAt: offset)
            }
            offset += UInt64(chunk.count)
        }
        return try scanner.finish()
    }

    /// The bytes of one element, from a handle the caller keeps open.
    static func read(_ range: Range<UInt64>, from handle: FileHandle) throws -> Data {
        try handle.seek(toOffset: range.lowerBound)
        let count = Int(range.upperBound - range.lowerBound)
        guard let data = try handle.read(upToCount: count), data.count == count else {
            throw ScanError.malformed
        }
        return data
    }
}

private let quote = UInt8(ascii: "\"")
private let backslash = UInt8(ascii: "\\")

private func isWhitespace(_ byte: UInt8) -> Bool {
    byte == 0x20 || byte == 0x0A || byte == 0x0D || byte == 0x09
}

/// Follows one JSON value, of any type, to its end.
private struct ValueTracker {
    enum Step {
        case more
        /// The value ended with this byte.
        case endedHere
        /// The value ended just before this byte.
        case endedBefore
    }

    private var depth = 0
    private(set) var inString = false
    private var escaped = false
    private var isScalar = false

    init(firstByte: UInt8) {
        switch firstByte {
        case UInt8(ascii: "{"), UInt8(ascii: "["):
            depth = 1
        case quote:
            inString = true
        default:
            isScalar = true
        }
    }

    mutating func feed(_ byte: UInt8) -> Step {
        if isScalar {
            let ends = byte == UInt8(ascii: ",") || byte == UInt8(ascii: "}") || byte == UInt8(ascii: "]")
            return ends || isWhitespace(byte) ? .endedBefore : .more
        }
        if inString {
            if escaped {
                escaped = false
            } else if byte == backslash {
                escaped = true
            } else if byte == quote {
                inString = false
                if depth == 0 { return .endedHere }
            }
            return .more
        }
        switch byte {
        case quote:
            inString = true
        case UInt8(ascii: "{"), UInt8(ascii: "["):
            depth += 1
        case UInt8(ascii: "}"), UInt8(ascii: "]"):
            depth -= 1
            if depth == 0 { return .endedHere }
        default:
            break
        }
        return .more
    }
}

private struct Scanner {

    private enum Phase {
        case beforeObject
        case expectKey
        case inKey
        case expectColon
        case expectValue
        case inValue(ValueTracker)
        case inArray
        case inElement(ValueTracker, start: UInt64)
        case done
    }

    let arrayKey: [UInt8]
    private var index = JSONFileScanner.Index()
    private var phase = Phase.beforeObject
    private var key: [UInt8] = []
    private var keyEscaped = false
    private var value: [UInt8] = []

    init(arrayKey: [UInt8]) {
        self.arrayKey = arrayKey
    }

    mutating func consume(_ bytes: UnsafeBufferPointer<UInt8>, startingAt offset: UInt64) throws {
        var position = 0
        while position < bytes.count {
            // Almost all of a large file is the inside of a long string.
            if case .inElement(let tracker, _) = phase, tracker.inString {
                while position < bytes.count, bytes[position] != quote, bytes[position] != backslash {
                    position += 1
                }
                guard position < bytes.count else { return }
            }
            if try step(bytes[position], at: offset + UInt64(position)) {
                position += 1
            }
        }
    }

    func finish() throws -> JSONFileScanner.Index {
        guard case .done = phase else { throw JSONFileScanner.ScanError.malformed }
        return index
    }

    /// Takes one byte. False means it wasn't used, and is for the next phase.
    private mutating func step(_ byte: UInt8, at offset: UInt64) throws -> Bool {
        switch phase {
        case .beforeObject:
            if isWhitespace(byte) { return true }
            guard byte == UInt8(ascii: "{") else { throw JSONFileScanner.ScanError.malformed }
            phase = .expectKey

        case .expectKey:
            if isWhitespace(byte) || byte == UInt8(ascii: ",") { return true }
            if byte == UInt8(ascii: "}") {
                phase = .done
            } else if byte == quote {
                key = []
                keyEscaped = false
                phase = .inKey
            } else {
                throw JSONFileScanner.ScanError.malformed
            }

        case .inKey:
            if keyEscaped {
                keyEscaped = false
                key.append(byte)
            } else if byte == backslash {
                keyEscaped = true
            } else if byte == quote {
                phase = .expectColon
            } else {
                key.append(byte)
            }

        case .expectColon:
            if isWhitespace(byte) { return true }
            guard byte == UInt8(ascii: ":") else { throw JSONFileScanner.ScanError.malformed }
            phase = .expectValue

        case .expectValue:
            if isWhitespace(byte) { return true }
            if key == arrayKey, byte == UInt8(ascii: "[") {
                phase = .inArray
            } else {
                value = [byte]
                phase = .inValue(ValueTracker(firstByte: byte))
            }

        case .inValue(var tracker):
            let result = tracker.feed(byte)
            if result != .endedBefore { value.append(byte) }
            if result == .more {
                phase = .inValue(tracker)
            } else {
                index.values[String(decoding: key, as: UTF8.self)] = Data(value)
                value = []
                phase = .expectKey
            }
            return result != .endedBefore

        case .inArray:
            if isWhitespace(byte) || byte == UInt8(ascii: ",") { return true }
            if byte == UInt8(ascii: "]") {
                phase = .expectKey
            } else {
                phase = .inElement(ValueTracker(firstByte: byte), start: offset)
            }

        case .inElement(var tracker, let start):
            switch tracker.feed(byte) {
            case .more:
                phase = .inElement(tracker, start: start)
            case .endedHere:
                index.elements.append(start..<(offset + 1))
                phase = .inArray
            case .endedBefore:
                index.elements.append(start..<offset)
                phase = .inArray
                return false
            }

        case .done:
            guard isWhitespace(byte) else { throw JSONFileScanner.ScanError.malformed }
        }
        return true
    }
}
