import SwiftUI

struct InboxView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    @State private var isLoading = false
    @State private var errorMessage: String?

    var body: some View {
        NavigationStack {
            Group {
                if isLoading && appState.inboxItems.isEmpty {
                    ProgressView()
                } else if appState.inboxItems.isEmpty {
                    ContentUnavailableView(
                        L.inboxEmpty,
                        systemImage: "tray",
                        description: Text(L.inboxEmptyDescription)
                    )
                } else {
                    itemList
                }
            }
            .navigationTitle(L.inboxTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.versionHistoryDone) { dismiss() }
                }
                if !appState.inboxItems.isEmpty {
                    ToolbarItemGroup(placement: .primaryAction) {
                        Button {
                            acceptAll()
                        } label: {
                            Label(L.inboxAcceptAll, systemImage: "checkmark.circle")
                        }

                        Button(role: .destructive) {
                            deleteAll()
                        } label: {
                            Label(L.inboxDeleteAll, systemImage: "trash")
                        }
                    }
                }
            }
            .overlay {
                if let error = errorMessage {
                    VStack {
                        Spacer()
                        Text(error)
                            .font(.caption)
                            .foregroundStyle(.red)
                            .padding()
                            .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 8))
                            .padding()
                    }
                }
            }
            .task {
                await loadItems()
            }
        }
    }

    private var itemList: some View {
        List {
            ForEach(appState.inboxItems) { item in
                InboxItemRow(item: item, onAccept: {
                    acceptItem(item)
                }, onDelete: {
                    deleteItem(item)
                })
            }
        }
        .listStyle(.insetGrouped)
        .refreshable {
            await loadItems()
        }
    }

    private func loadItems() async {
        isLoading = true
        errorMessage = nil
        do {
            try await appState.loadInboxItems()
        } catch {
            errorMessage = error.localizedDescription
        }
        isLoading = false
    }

    private func acceptItem(_ item: InboxItem) {
        Task {
            do {
                try await appState.acceptInboxItem(item)
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    private func deleteItem(_ item: InboxItem) {
        Task {
            do {
                try await appState.deleteInboxItem(item)
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    private func acceptAll() {
        Task {
            do {
                for item in appState.inboxItems {
                    try await appState.acceptInboxItem(item)
                }
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }

    private func deleteAll() {
        Task {
            do {
                try await appState.deleteAllInboxItems()
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }
}

// MARK: - Inbox Item Row

private struct InboxItemRow: View {
    let item: InboxItem
    let onAccept: () -> Void
    let onDelete: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(itemTitle)
                .font(.headline)
                .lineLimit(1)

            Text(item.content)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(3)

            HStack(spacing: 12) {
                if let date = Date(iso8601: item.createdAt) {
                    Label {
                        Text(date, style: .relative)
                    } icon: {
                        Image(systemName: "clock")
                    }
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
                }

                if let source = item.source, !source.isEmpty {
                    Label(source, systemImage: "arrow.right.circle")
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                }

                if !item.tags.isEmpty {
                    HStack(spacing: 2) {
                        ForEach(item.tags.prefix(3), id: \.self) { tag in
                            Text("#\(tag)")
                                .font(.caption2)
                                .foregroundStyle(.blue)
                        }
                    }
                }
            }
        }
        .padding(.vertical, 2)
        .swipeActions(edge: .leading, allowsFullSwipe: true) {
            Button {
                onAccept()
            } label: {
                Label(L.inboxAccept, systemImage: "checkmark")
            }
            .tint(.green)
        }
        .swipeActions(edge: .trailing, allowsFullSwipe: true) {
            Button(role: .destructive) {
                onDelete()
            } label: {
                Label(L.noteListDelete, systemImage: "trash")
            }
        }
    }

    private var itemTitle: String {
        let lines = item.content.split(separator: "\n", omittingEmptySubsequences: true)
        guard let firstLine = lines.first else { return "Untitled" }
        let trimmed = firstLine.trimmingCharacters(in: .whitespaces)
            .replacingOccurrences(of: "^#+\\s*", with: "", options: .regularExpression)
        return trimmed.isEmpty ? "Untitled" : String(trimmed.prefix(80))
    }
}
