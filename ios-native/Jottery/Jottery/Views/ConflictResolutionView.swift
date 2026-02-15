import SwiftUI

struct ConflictResolutionView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                if appState.pendingConflicts.isEmpty {
                    ContentUnavailableView(
                        L.conflictNoConflicts,
                        systemImage: "checkmark.circle",
                        description: Text(L.conflictNoConflictsDescription)
                    )
                } else {
                    ForEach(appState.pendingConflicts) { conflict in
                        NavigationLink {
                            ConflictDetailView(conflict: conflict)
                        } label: {
                            ConflictRowView(conflict: conflict)
                        }
                    }
                }
            }
            .listStyle(.insetGrouped)
            .navigationTitle(L.conflictTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.versionHistoryDone) { dismiss() }
                }
            }
        }
    }
}

// MARK: - Conflict Row

private struct ConflictRowView: View {
    let conflict: ConflictInfo

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(noteTitle(from: conflict.localContent))
                .font(.headline)
                .lineLimit(1)

            HStack(spacing: 16) {
                Label {
                    Text(formattedDate(conflict.localModifiedAt))
                } icon: {
                    Image(systemName: "iphone")
                }
                .font(.caption)
                .foregroundStyle(.secondary)

                Label {
                    Text(formattedDate(conflict.serverModifiedAt))
                } icon: {
                    Image(systemName: "cloud")
                }
                .font(.caption)
                .foregroundStyle(.secondary)
            }

            if !conflict.localTags.isEmpty || !conflict.serverTags.isEmpty {
                HStack(spacing: 4) {
                    ForEach(Array(Set(conflict.localTags + conflict.serverTags)).sorted().prefix(3), id: \.self) { tag in
                        Text("#\(tag)")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
        .padding(.vertical, 2)
    }

    private func noteTitle(from content: String) -> String {
        let lines = content.split(separator: "\n", omittingEmptySubsequences: true)
        guard let firstLine = lines.first else { return "Untitled" }
        let trimmed = firstLine.trimmingCharacters(in: .whitespaces)
            .replacingOccurrences(of: "^#+\\s*", with: "", options: .regularExpression)
        return trimmed.isEmpty ? "Untitled" : String(trimmed.prefix(80))
    }

    private func formattedDate(_ iso: String) -> String {
        guard let date = Date(iso8601: iso) else { return iso }
        let formatter = DateFormatter()
        formatter.dateStyle = .short
        formatter.timeStyle = .short
        return formatter.string(from: date)
    }
}

// MARK: - Conflict Detail

private struct ConflictDetailView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss
    let conflict: ConflictInfo

    @State private var showingLocal = true

    var body: some View {
        VStack(spacing: 0) {
            // Toggle between local and server
            Picker(L.conflictVersion, selection: $showingLocal) {
                Text(L.conflictLocal).tag(true)
                Text(L.conflictServer).tag(false)
            }
            .pickerStyle(.segmented)
            .padding()

            // Content comparison
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    // Metadata
                    HStack {
                        Label(
                            formattedDate(showingLocal ? conflict.localModifiedAt : conflict.serverModifiedAt),
                            systemImage: "clock"
                        )
                        .font(.caption)
                        .foregroundStyle(.secondary)

                        Spacer()

                        let tags = showingLocal ? conflict.localTags : conflict.serverTags
                        if !tags.isEmpty {
                            HStack(spacing: 4) {
                                ForEach(tags, id: \.self) { tag in
                                    Text("#\(tag)")
                                        .font(.caption)
                                        .foregroundStyle(.blue)
                                }
                            }
                        }
                    }
                    .padding(.horizontal)

                    // Content
                    Text(showingLocal ? conflict.localContent : conflict.serverContent)
                        .font(.body.monospaced())
                        .padding(.horizontal)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }

            Divider()

            // Resolution buttons
            VStack(spacing: 8) {
                Button {
                    resolveConflict(strategy: .keepLocal)
                } label: {
                    Label(L.conflictKeepLocal, systemImage: "iphone")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)

                Button {
                    resolveConflict(strategy: .keepServer)
                } label: {
                    Label(L.conflictKeepServer, systemImage: "cloud")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)

                Button {
                    resolveConflict(strategy: .keepBoth)
                } label: {
                    Label(L.conflictKeepBoth, systemImage: "doc.on.doc")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .tint(.secondary)
            }
            .padding()
        }
        .navigationTitle(L.conflictResolve)
        .navigationBarTitleDisplayMode(.inline)
    }

    private func resolveConflict(strategy: ConflictResolutionStrategy) {
        Task {
            try? await appState.resolveConflict(noteId: conflict.id, strategy: strategy)
            dismiss()
        }
    }

    private func formattedDate(_ iso: String) -> String {
        guard let date = Date(iso8601: iso) else { return iso }
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .medium
        return formatter.string(from: date)
    }
}
