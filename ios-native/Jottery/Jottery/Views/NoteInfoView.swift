import SwiftUI

/// Displays detailed metadata about a note — ID, timestamps, sync status, etc.
struct NoteInfoView: View {
    @Environment(\.dismiss) private var dismiss

    let note: DecryptedNote

    private var dateFormatter: DateFormatter {
        let f = DateFormatter()
        f.dateStyle = .medium
        f.timeStyle = .medium
        return f
    }

    var body: some View {
        NavigationStack {
            List {
                // Identification
                Section {
                    row(L.noteInfoNoteId, value: note.id)
                }

                // Timestamps
                Section {
                    row(L.noteInfoCreated, value: dateFormatter.string(from: note.createdAt))
                    row(L.noteInfoModified, value: dateFormatter.string(from: note.modifiedAt))
                    if let syncedAt = note.syncedAt {
                        row(L.noteInfoSyncedAt, value: dateFormatter.string(from: syncedAt))
                    }
                }

                // Content stats
                Section {
                    row(L.noteInfoWordCount, value: "\(note.wordCount)")
                    row(L.noteInfoCharacterCount, value: "\(note.content.count)")
                    row(L.noteInfoAttachmentCount, value: "\(note.attachments.count)")
                }

                // Sync info
                Section {
                    row(L.noteInfoVersion, value: "\(note.version)")
                    row(L.noteInfoSyncStatus, value: syncStatusText)
                    if let hash = note.contentHash {
                        row(L.noteInfoContentHash, value: String(hash.prefix(12)) + "…")
                    }
                }
            }
            .navigationTitle(L.noteInfoTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button(L.settingsDone) { dismiss() }
                }
            }
        }
    }

    private var syncStatusText: String {
        if note.needsSync {
            return L.noteInfoPendingSync
        } else if note.syncedAt != nil {
            return L.noteInfoSynced
        } else {
            return L.noteInfoNeverSynced
        }
    }

    private func row(_ label: String, value: String) -> some View {
        HStack {
            Text(label)
            Spacer()
            Text(value)
                .foregroundStyle(.secondary)
                .textSelection(.enabled)
                .lineLimit(1)
                .truncationMode(.middle)
        }
    }
}
