import Foundation

/// The kinds of note there are, each stored as the note's syntax language.
enum NoteType {
    /// What a note is unless something says otherwise.
    static let fallback = "markdown"

    static let all = [
        "markdown", "calc", "outliner", "javascript", "typescript", "python",
        "perl", "json", "xml", "css", "bash", "sql", "plain",
    ]
}
