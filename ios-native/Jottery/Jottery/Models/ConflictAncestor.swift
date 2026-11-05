import CryptoKit
import Foundation

/// The version a conflict's two sides grew from, when the server can find it.
struct ConflictAncestor: Equatable, Sendable {
    let content: String
    let tags: [String]

    /// From what a rejected push carried. Nil if there was none, which is often:
    /// an older server doesn't send one, and it isn't always found.
    ///
    /// All of it has to decrypt. There's no use for part of a version, and what
    /// won't decrypt is an error, so it's never shown instead.
    static func decrypted(content: String?, tags: [String]?, key: SymmetricKey) -> ConflictAncestor? {
        guard let content else { return nil }
        do {
            let text = try CryptoService.decryptText(CryptoService.parseEncryptedJSON(content), key: key)
            return ConflictAncestor(content: text, tags: try decryptedTags(tags ?? [], key: key))
        } catch {
            Log.debug("[Sync] A conflict's ancestor wouldn't decrypt, so it's left out: \(error)")
            return nil
        }
    }

    /// Each tag on its own, as notes hold them, or the whole list as one blob,
    /// as web and Android write version history, which is where this comes from.
    private static func decryptedTags(_ stored: [String], key: SymmetricKey) throws -> [String] {
        var tags: [String] = []
        for entry in stored {
            let text = try CryptoService.decryptText(CryptoService.parseEncryptedJSON(entry), key: key)
            let json = Data(text.utf8)
            if let tag = try? JSONDecoder().decode(String.self, from: json) {
                tags.append(tag)
            } else if let list = try? JSONDecoder().decode([String].self, from: json) {
                tags.append(contentsOf: list)
            } else if !text.isEmpty {
                tags.append(text)
            }
        }
        return tags
    }
}

/// Which side of a conflict is on screen.
enum ConflictVersion: Hashable {
    case local
    case server
    case ancestor
}

extension ConflictInfo {

    /// What can be looked at, in the order the picker offers it.
    var versions: [ConflictVersion] {
        ancestor == nil ? [.local, .server] : [.local, .server, .ancestor]
    }

    func content(of version: ConflictVersion) -> String {
        switch version {
        case .local: return localContent
        case .server: return serverContent
        case .ancestor: return ancestor?.content ?? ""
        }
    }

    func tags(of version: ConflictVersion) -> [String] {
        switch version {
        case .local: return localTags
        case .server: return serverTags
        case .ancestor: return ancestor?.tags ?? []
        }
    }

    /// When it was last changed. The server doesn't say for an ancestor.
    func modifiedAt(of version: ConflictVersion) -> String? {
        switch version {
        case .local: return localModifiedAt
        case .server: return serverModifiedAt
        case .ancestor: return nil
        }
    }
}
