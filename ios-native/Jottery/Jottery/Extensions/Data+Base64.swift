import Foundation

extension Data {
    /// Standard RFC 4648 base64 encoding (matching Foundation's default).
    var base64: String {
        base64EncodedString()
    }

    /// Decode standard base64.
    init?(base64 string: String) {
        self.init(base64Encoded: string)
    }
}
