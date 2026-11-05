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
        guard let firstLine = lines.first else { return L.noteUntitled }
        let trimmed = firstLine.trimmingCharacters(in: .whitespaces)
            .replacingOccurrences(of: "^#+\\s*", with: "", options: .regularExpression)
        return trimmed.isEmpty ? L.noteUntitled : String(trimmed.prefix(80))
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

struct ConflictDetailView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss
    let conflict: ConflictInfo

    @State private var version = ConflictVersion.local
    @State private var couldntResolve = false

    private var shownSideIsUnreadable: Bool {
        switch version {
        case .local: conflict.localUnreadable
        case .server: conflict.serverUnreadable
        case .ancestor: false  // An ancestor that won't open isn't offered at all.
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            // Toggle between local, server and where they split, if that's known
            Picker(L.conflictVersion, selection: $version) {
                Text(L.conflictLocal).tag(ConflictVersion.local)
                Text(L.conflictServer).tag(ConflictVersion.server)
                if conflict.versions.contains(.ancestor) {
                    Text(L.conflictAncestor).tag(ConflictVersion.ancestor)
                }
            }
            .pickerStyle(.segmented)
            .padding()

            // Content comparison
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    // Metadata
                    HStack {
                        if let modifiedAt = conflict.modifiedAt(of: version) {
                            Label(formattedDate(modifiedAt), systemImage: "clock")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }

                        Spacer()

                        let tags = conflict.tags(of: version)
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
                    if shownSideIsUnreadable {
                        Label(L.conflictUnreadable, systemImage: "lock.trianglebadge.exclamationmark")
                            .foregroundStyle(.secondary)
                            .padding(.horizontal)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    } else {
                        Text(conflict.content(of: version))
                            .font(.body.monospaced())
                            .padding(.horizontal)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
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
                .disabled(!conflict.allows(.keepLocal))

                Button {
                    resolveConflict(strategy: .keepServer)
                } label: {
                    Label(L.conflictKeepServer, systemImage: "cloud")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .disabled(!conflict.allows(.keepServer))

                Button {
                    resolveConflict(strategy: .keepBoth)
                } label: {
                    Label(L.conflictKeepBoth, systemImage: "doc.on.doc")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .tint(.secondary)
                .disabled(!conflict.allows(.keepBoth))
            }
            .padding()
        }
        .navigationTitle(L.conflictResolve)
        .navigationBarTitleDisplayMode(.inline)
        .alert(L.conflictCouldntResolve, isPresented: $couldntResolve) {
            Button(L.versionHistoryDone, role: .cancel) {}
        }
    }

    private func resolveConflict(strategy: ConflictResolutionStrategy) {
        Task {
            do {
                try await appState.resolveConflict(noteId: conflict.id, strategy: strategy)
                dismiss()
            } catch {
                // It stays waiting, so the user can try again.
                couldntResolve = true
            }
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
