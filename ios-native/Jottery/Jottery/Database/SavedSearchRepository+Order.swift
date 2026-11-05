import Foundation
import GRDB

extension SavedSearchRepository {

    /// The order numbers for a list in its new order: the ones it already uses,
    /// so a search that hasn't moved keeps its number. Two devices can give
    /// searches the same number, which a move has to separate.
    static func orderNumbers(for searches: [SavedSearch]) -> [Int] {
        var numbers = searches.map(\.displayOrder).sorted()
        for index in numbers.indices.dropFirst() where numbers[index] <= numbers[index - 1] {
            numbers[index] = numbers[index - 1] + 1
        }
        return numbers
    }

    /// Store `searches` as the new order, and return them with their new numbers.
    /// Each one that moved is queued for a push, with `modified_at` moved on so it
    /// wins last-write-wins here and on the server. The rest are left alone: a
    /// search this device didn't move may have changed on another.
    func reorder(_ searches: [SavedSearch]) throws -> [SavedSearch] {
        let numbers = Self.orderNumbers(for: searches)
        var reordered = searches
        let now = Date().iso8601
        try db.dbPool.write { db in
            for index in reordered.indices where reordered[index].displayOrder != numbers[index] {
                reordered[index].displayOrder = numbers[index]
                try db.execute(sql: """
                    UPDATE saved_searches
                    SET display_order = ?, modified_at = ?, needs_sync = 1, version = version + 1
                    WHERE id = ?
                """, arguments: [numbers[index], now, reordered[index].id])
            }
        }
        return reordered
    }
}
