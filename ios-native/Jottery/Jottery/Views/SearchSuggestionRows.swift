import SwiftUI

/// The search field's suggestions: with nothing typed, recent searches and
/// examples of what can be searched for, and after a `#`, matching tags. The colours
/// are named in full, as `.primary` here would be a shade of the tint.
struct SearchSuggestionRows: View {
    @Environment(AppState.self) private var appState
    let tagCompletions: [SearchSuggestions.TagCompletion]

    var body: some View {
        if tagCompletions.isEmpty {
            recentSearches
            examples
        } else {
            ForEach(tagCompletions) { completion in
                Label(verbatim(completion.tag), systemImage: "tag")
                    .foregroundStyle(Color.primary)
                    .searchCompletion(completion.completion)
            }
        }
    }

    /// A tag is the user's own word, so it's never looked up in the catalog.
    private func verbatim(_ tag: String) -> String {
        "#" + tag
    }

    @ViewBuilder
    private var recentSearches: some View {
        let queries = appState.recentSearches.queries
        if !queries.isEmpty {
            Section(L.searchRecent) {
                ForEach(queries, id: \.self) { query in
                    Label(query, systemImage: "clock")
                        .foregroundStyle(Color.primary)
                        .searchCompletion(query)
                }
                Button(L.searchClearRecent) {
                    appState.recentSearches.clear()
                }
            }
        }
    }

    private var examples: some View {
        Section(L.searchExamples) {
            ForEach(SearchSuggestions.examples(firstTag: appState.allTags.first { !DecryptedNote.isTitleTag($0) })) { example in
                VStack(alignment: .leading, spacing: 2) {
                    Text(example.query)
                        .font(.body.monospaced())
                        .foregroundStyle(Color.primary)
                    Text(example.label)
                        .font(.caption)
                        .foregroundStyle(Color.secondary)
                }
                .accessibilityElement(children: .combine)
                .searchCompletion(example.query)
            }
        }
    }
}
