import Foundation
import GRDB

extension NoteRepository {

    /// Notes in the recycle bin, which aren't kept in memory like the others.
    func countInBin() throws -> Int {
        try db.dbPool.read { db in
            try NoteRecord.filter(Column("deleted") == true).fetchCount(db)
        }
    }
}
