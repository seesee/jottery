import CryptoKit
import Foundation
import GRDB

/// Reads and writes `NoteColourSettings`, encrypted with the master key, in two
/// columns of the settings row that `UserSettings` doesn't map. SQLite here is
/// unencrypted, so those are the only form the names ever take on disk.
///
/// A value that doesn't decrypt counts as not set, and is never used as it
/// stands. `VaultReKeyService` moves both columns when the key changes.
struct ColourSettingsRepository: Sendable {

    let db: DatabaseManager

    func load(key: SymmetricKey) throws -> NoteColourSettings {
        let row = try db.dbPool.read { db in
            try Row.fetchOne(db, sql: "SELECT color_palette, tag_colors FROM settings WHERE id = 1")
        }
        return NoteColourSettings(
            categoryNames: Self.decrypted(row?["color_palette"], key: key),
            tagColours: Self.decrypted(row?["tag_colors"], key: key)
        )
    }

    func save(_ colours: NoteColourSettings, key: SymmetricKey) throws {
        let names = try Self.encrypted(colours.categoryNames, key: key)
        let tags = try Self.encrypted(colours.tagColours, key: key)
        try db.dbPool.write { db in
            try db.execute(
                sql: "UPDATE settings SET color_palette = ?, tag_colors = ? WHERE id = 1",
                arguments: [names, tags]
            )
        }
    }

    private static func encrypted(_ values: [String: String], key: SymmetricKey) throws -> String? {
        guard !values.isEmpty else { return nil }
        let json = String(decoding: try JSONEncoder().encode(values), as: UTF8.self)
        return try CryptoService.serializeEncryptedJSON(CryptoService.encryptText(json, key: key))
    }

    private static func decrypted(_ stored: String?, key: SymmetricKey) -> [String: String] {
        guard let stored else { return [:] }
        do {
            let json = try CryptoService.decryptText(CryptoService.parseEncryptedJSON(stored), key: key)
            return try JSONDecoder().decode([String: String].self, from: Data(json.utf8))
        } catch {
            Log.debug("[Settings] Colour settings couldn't be read, so they're ignored: \(error)")
            return [:]
        }
    }
}
