import CryptoKit
import Foundation
import GRDB

/// Makes this device's vault out of a backup, for someone who has none yet.
///
/// The vault keeps the backup's master key instead of a new one of its own. That
/// key is what the notes on any account are encrypted with, so a key of its own
/// would make the device look like a different vault when it later connects.
/// The backup's password becomes the vault's, wrapping the key under a fresh
/// device salt, as for a new vault. It's an existing password, so the rules for
/// choosing one don't apply. A legacy backup's key comes from its password and
/// salt, and is wrapped the same way, which is what connecting does to a legacy
/// vault anyway.
///
/// Everything is restored into a database of its own first, and nothing reaches
/// the real one until all of it has worked. The notes and the vault's metadata
/// then go across in one transaction, so a vault either exists whole or not at
/// all: if the file turns out to be damaged, the restore is cancelled or the app
/// dies, the first-run screen is as it was. The cost is the disk space for a
/// second copy of the vault while restoring.
enum VaultFromBackup {

    /// What came out of a restore. `masterKey` is for the caller to load, and
    /// then wipe.
    struct Outcome: Sendable {
        let result: BackupService.RestoreResult
        var masterKey: Data
    }

    enum RestoreError: LocalizedError, Equatable {
        /// There's a vault here already, and a restore must never replace it.
        case vaultExists

        var errorDescription: String? {
            switch self {
            case .vaultExists: return L.restoreVaultExists
            }
        }
    }

    /// Where a restore in progress keeps its work. It's beside the database, not
    /// with the temporary files, as those can't be read once the phone locks and
    /// SQLite needs its files to stay open. It takes the database's protection.
    static func stagingFolder(besides database: DatabaseManager) -> URL {
        URL(fileURLWithPath: database.dbPool.path)
            .deletingLastPathComponent()
            .appendingPathComponent("restore-staging", isDirectory: true)
    }

    /// Clear what a restore left when the app died part-way through it.
    static func removeLeftovers(besides database: DatabaseManager) {
        try? FileManager.default.removeItem(at: stagingFolder(besides: database))
    }

    /// Make `database`'s vault from the backup at `url`. Run this off the main
    /// actor. Throws, leaving `database` as it was, unless the whole vault made
    /// it across. Stops with `CancellationError` if the task is cancelled.
    static func restore(
        from url: URL,
        password: String,
        into database: DatabaseManager,
        progress: (@Sendable (BackupService.Progress) -> Void)? = nil
    ) throws -> Outcome {
        guard try !EncryptionRepository(db: database).isVaultSetUp() else {
            throw RestoreError.vaultExists
        }

        var masterKey = try BackupService.masterKey(of: url, password: password)
        var succeeded = false
        defer { if !succeeded { masterKey.wipe() } }

        let metadata = try EnvelopeService.envelopeMetadata(
            for: masterKey,
            localPassword: password,
            current: EncryptionMetadata(
                salt: "", iterations: 0, createdAt: Date().iso8601, algorithm: "AES-256-GCM"
            )
        )

        let folder = stagingFolder(besides: database)
        removeLeftovers(besides: database)
        try FileManager.default.createDirectory(
            at: folder,
            withIntermediateDirectories: true,
            attributes: [.protectionKey: DatabaseManager.fileProtection]
        )
        defer { removeLeftovers(besides: database) }

        let stagedPath = folder.appendingPathComponent("staged.db").path
        let staged = try DatabaseManager(path: stagedPath)
        let versionRepo = VersionRepository(db: staged)
        let result = try BackupService.restoreBackup(
            from: url,
            password: password,
            noteRepo: NoteRepository(db: staged, versionRepo: versionRepo),
            versionRepo: versionRepo,
            attachmentRepo: AttachmentRepository(db: staged),
            savedSearchRepo: SavedSearchRepository(db: staged),
            currentKey: SymmetricKey(data: masterKey),
            progress: progress
        )
        guard result.notes > 0 else { throw BackupError.nothingToRestore }

        try Task.checkCancellation()
        try staged.dbPool.close()
        try commit(stagedAt: stagedPath, metadata: metadata, into: database)

        succeeded = true
        return Outcome(result: result, masterKey: masterKey)
    }

    /// Everything a restore writes, in an order that suits the foreign keys.
    private static let restoredTables = ["notes", "note_versions", "attachments", "saved_searches"]

    /// Copy the staged vault into `database`, all of it or none. Sync settings
    /// already saved here are left alone, which is why this isn't a file swap:
    /// the device may have been registered before coming to restore.
    private static func commit(
        stagedAt path: String,
        metadata: EncryptionMetadata,
        into database: DatabaseManager
    ) throws {
        try database.dbPool.writeWithoutTransaction { db in
            try db.execute(sql: "ATTACH DATABASE ? AS staged", arguments: [path])
            defer { try? db.execute(sql: "DETACH DATABASE staged") }
            try db.inTransaction {
                guard try EncryptionMetadata.fetchCount(db) == 0 else {
                    throw RestoreError.vaultExists
                }
                for table in restoredTables {
                    try db.execute(sql: "INSERT INTO main.\(table) SELECT * FROM staged.\(table)")
                }
                try metadata.insert(db)
                return .commit
            }
        }
    }
}
