import SwiftUI

/// One tag and its count. Choosing it searches for it.
private struct TagRow: View {
    let tag: TagCount
    let onChoose: (String) -> Void

    var body: some View {
        Button {
            onChoose(tag.tag)
        } label: {
            Label(title, systemImage: "tag")
                .foregroundStyle(Color.primary)
        }
        .badge(tag.count)
    }

    /// The user's own word, so it's never looked up in the catalog.
    private var title: String {
        "#" + tag.tag
    }
}

/// The sheet of tags on a phone.
struct TagBrowserSheet: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let tags = TagCount.counts(in: appState.displayedNotes)

        NavigationStack {
            List(tags) { tag in
                TagRow(tag: tag) { chosen in
                    appState.browse(tag: chosen)
                    dismiss()
                }
            }
            .overlay {
                if tags.isEmpty {
                    ContentUnavailableView(
                        L.tagsEmpty,
                        systemImage: "tag",
                        description: Text(L.tagsEmptyDescription)
                    )
                }
            }
            .navigationTitle(L.tagsTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.versionHistoryDone) { dismiss() }
                }
            }
        }
        .presentationDetents([.medium, .large])
    }
}

/// The tags at the top of the sidebar on iPad, folded away until they're wanted.
/// Its own view, so that typing in the search field doesn't count them again.
struct TagSidebarSection: View {
    @Environment(AppState.self) private var appState
    @SceneStorage("noteList.tagsExpanded") private var isExpanded = false

    var body: some View {
        let tags = TagCount.counts(in: appState.displayedNotes)

        if !tags.isEmpty {
            Section {
                DisclosureGroup(isExpanded: $isExpanded) {
                    ForEach(tags) { tag in
                        TagRow(tag: tag) { appState.browse(tag: $0) }
                    }
                } label: {
                    Label(L.tagsTitle, systemImage: "tag")
                }
            }
        }
    }
}
