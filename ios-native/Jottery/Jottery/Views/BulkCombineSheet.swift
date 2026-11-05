import SwiftUI

/// Confirms a Combine: which notes go in, in what order, and what happens to
/// their tags. Nothing changes until Combine is tapped.
struct BulkCombineSheet: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss
    let noteIds: Set<String>
    let onComplete: () -> Void

    @State private var mergeTags = true

    /// The selection as it'll be combined.
    private var notes: [DecryptedNote] {
        NoteCombining.combinable((appState.notes + appState.archivedNotes).filter { noteIds.contains($0.id) })
    }

    var body: some View {
        let notes = self.notes
        let leftOut = noteIds.count - notes.count

        NavigationStack {
            Form {
                Section {
                    ForEach(notes) { note in
                        Text(note.title)
                            .lineLimit(1)
                    }
                } header: {
                    Text(L.bulkCombineOrder)
                } footer: {
                    VStack(alignment: .leading, spacing: 4) {
                        if leftOut > 0 {
                            Text(L.bulkCombineLeftOut(leftOut))
                        }
                        Text(L.bulkCombineExplanation)
                    }
                }

                Section {
                    Toggle(L.bulkCombineMergeTags, isOn: $mergeTags)
                } footer: {
                    Text(mergeTags ? L.bulkCombineMergeTagsOn : L.bulkCombineMergeTagsOff)
                }
            }
            .navigationTitle(L.bulkCombine)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.commonCancel) { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(L.bulkCombine) {
                        combine()
                        dismiss()
                        onComplete()
                    }
                    .disabled(notes.count < 2)
                }
            }
        }
    }

    private func combine() {
        let ids = noteIds
        let merging = mergeTags
        Task {
            do {
                try await appState.combineNotes(ids: ids, mergeTags: merging)
            } catch NoteCombining.Failure.attachmentNotOnDevice {
                appState.reportError(L.errorCombineAttachmentNotOnDevice)
            } catch {
                appState.reportError(L.errorCouldntCombineNotes)
            }
        }
    }
}
