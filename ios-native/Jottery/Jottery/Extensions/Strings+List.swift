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

    // MARK: - Search Suggestions

    static var searchRecent: String { String(localized: "search.recent") }
    static var searchClearRecent: String { String(localized: "search.clearRecent") }
    static var searchExamples: String { String(localized: "search.examples") }
    static var searchExampleTag: String { String(localized: "search.exampleTag") }
    static var searchExamplePhrase: String { String(localized: "search.examplePhrase") }
    static var searchExampleEither: String { String(localized: "search.exampleEither") }
    static var searchExampleExclude: String { String(localized: "search.exampleExclude") }
    static var searchExampleStartsWith: String { String(localized: "search.exampleStartsWith") }
    static var searchExampleAttachment: String { String(localized: "search.exampleAttachment") }
    static var searchExampleCreated: String { String(localized: "search.exampleCreated") }
    static var searchExampleWords: String { String(localized: "search.exampleWords") }
    static var searchExampleColour: String { String(localized: "search.exampleColour") }
    static var searchExampleArchived: String { String(localized: "search.exampleArchived") }
    static func searchNoResults(_ query: String) -> String {
        String(format: String(localized: "search.noResults"), query)
    }
    static func searchMatchesInArchive(_ count: Int) -> String {
        String(localized: "search.matchesInArchive \(count)")
    }
    static func searchMatchesInNotes(_ count: Int) -> String {
        String(localized: "search.matchesInNotes \(count)")
    }

    // MARK: - Keyboard Commands

    static var commandsNoteMenu: String { String(localized: "commands.noteMenu") }
    static var commandsSearch: String { String(localized: "commands.search") }

    // MARK: - Tags and Counts

    static var tagsTitle: String { String(localized: "tags.title") }
    static var tagsEmpty: String { String(localized: "tags.empty") }
    static var tagsEmptyDescription: String { String(localized: "tags.emptyDescription") }
    static var aboutPinned: String { String(localized: "about.pinned") }
    static var aboutArchived: String { String(localized: "about.archived") }
    static var aboutInBin: String { String(localized: "about.inBin") }
}
