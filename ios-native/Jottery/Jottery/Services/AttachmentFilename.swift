import CryptoKit
import Foundation

/// Attachment filenames as they're stored, shown and written to disk.
///
/// The server stores a note's attachment references outside the encrypted
/// content, and a name can also come from an imported file, so none of them
/// can be trusted. A name like `../../Library/Application Support/Jottery/jottery.db`
/// would otherwise overwrite the vault when the attachment is opened.
enum AttachmentFilename {

    /// Used when nothing of a name is left.
    static let fallback = "attachment"

    /// Room for the longest name a file system allows, less a margin.
    private static let maxBytes = 200

    /// `raw` reduced to a single path component that's safe to create.
    static func safe(_ raw: String) -> String {
        let lastComponent = raw.split(whereSeparator: { $0 == "/" || $0 == "\\" }).last.map(String.init) ?? ""
        let scalars = lastComponent.unicodeScalars.filter { !isUnwanted($0) }
        let cleaned = String(String.UnicodeScalarView(scalars)).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !cleaned.isEmpty, cleaned != ".", cleaned != ".." else { return fallback }
        return truncated(cleaned)
    }

    /// Whether `stored` is in the encrypted form a note should hold. Older
    /// builds could store a filename, and send it to the server, in plaintext.
    static func isEncrypted(_ stored: String) -> Bool {
        (try? CryptoService.parseEncryptedJSON(stored)) != nil
    }

    /// The name to show for a stored filename, always safe to use as a path
    /// component. A name that fails to decrypt is an error, not a name: the
    /// ciphertext is never shown instead.
    static func display(stored: String, key: SymmetricKey) -> String {
        guard let encrypted = try? CryptoService.parseEncryptedJSON(stored) else {
            return safe(stored)
        }
        guard let plaintext = try? CryptoService.decryptText(encrypted, key: key) else {
            return L.attachmentsUnreadableName
        }
        return safe(plaintext)
    }

    /// The form a filename is stored and synced in.
    static func encrypted(_ plaintext: String, key: SymmetricKey) throws -> String {
        try CryptoService.serializeEncryptedJSON(CryptoService.encryptText(safe(plaintext), key: key))
    }

    /// `refs` with any plaintext filename encrypted, and whether any was.
    static func encryptingPlaintext(
        in refs: [AttachmentRef],
        key: SymmetricKey
    ) throws -> (refs: [AttachmentRef], changed: Bool) {
        var changed = false
        var result = refs
        for index in result.indices where !isEncrypted(result[index].filename) {
            result[index].filename = try encrypted(result[index].filename, key: key)
            changed = true
        }
        return (result, changed)
    }

    /// Control characters, and the bidirectional overrides that make
    /// `invoice‮gpj.exe` read as a picture.
    private static func isUnwanted(_ scalar: Unicode.Scalar) -> Bool {
        if scalar.properties.generalCategory == .control { return true }
        return (0x202A...0x202E).contains(scalar.value) || (0x2066...0x2069).contains(scalar.value)
    }

    /// Shorten to `maxBytes` of UTF-8 on a character boundary, keeping the extension.
    private static func truncated(_ name: String) -> String {
        guard name.utf8.count > maxBytes else { return name }
        let ext = (name as NSString).pathExtension
        let suffix = ext.isEmpty || ext.utf8.count > 16 ? "" : ".\(ext)"
        var stem = suffix.isEmpty ? name : (name as NSString).deletingPathExtension
        while stem.utf8.count + suffix.utf8.count > maxBytes {
            stem.removeLast()
        }
        return stem + suffix
    }
}
