import Foundation
import GRDB

extension NoteRepository {

    /// Every note there is: in use, archived and in the bin.
    func allNoteIds() throws -> [String] {
        try db.dbPool.read { try String.fetchAll($0, sql: "SELECT id FROM notes") }
    }

    /// Notes with a change still to go up, in the bin or not.
    func countNeedingSync() throws -> Int {
        try db.dbPool.read { try NoteRecord.filter(Column("needs_sync") == true).fetchCount($0) }
    }

    /// These notes, undecrypted, in no particular order.
    func rawRecords(ids: [String]) throws -> [NoteRecord] {
        try db.dbPool.read { try NoteRecord.filter(keys: ids).fetchAll($0) }
    }
}
