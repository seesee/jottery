import Foundation

/// Strings for the note list: its bulk actions, saved searches, search
/// suggestions, keyboard commands and tags.
extension L {

    // MARK: - Combine

    static var bulkCombine: String { String(localized: "bulk.combine") }
    static var bulkCombineOrder: String { String(localized: "bulk.combineOrder") }
    static var bulkCombineMergeTags: String { String(localized: "bulk.combineMergeTags") }
    static var bulkCombineMergeTagsOn: String { String(localized: "bulk.combineMergeTagsOn") }
    static var bulkCombineMergeTagsOff: String { String(localized: "bulk.combineMergeTagsOff") }
    static var bulkCombineExplanation: String { String(localized: "bulk.combineExplanation") }
    static func bulkCombineLeftOut(_ count: Int) -> String {
        String(localized: "bulk.combineLeftOut \(count)")
    }
    static var errorCouldntCombineNotes: String { String(localized: "error.couldntCombineNotes") }
    static var errorCombineAttachmentNotOnDevice: String { String(localized: "error.combineAttachmentNotOnDevice") }

    // MARK: - Saved Searches

    static var savedSearchEdit: String { String(localized: "savedSearch.edit") }
    static var savedSearchName: String { String(localized: "savedSearch.name") }
    static var savedSearchQuery: String { String(localized: "savedSearch.query") }
    static var errorCouldntSaveSavedSearch: String { String(localized: "error.couldntSaveSavedSearch") }
    static var errorCouldntMoveSavedSearch: String { String(localized: "error.couldntMoveSavedSearch") }
}
