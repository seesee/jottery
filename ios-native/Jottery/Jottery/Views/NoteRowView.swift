import SwiftUI

struct NoteRowView: View {
    let note: DecryptedNote
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        HStack(spacing: 12) {
            // Colour indicator
            if let colorName = note.color,
               let color = Color.noteColor(named: colorName, scheme: colorScheme) {
                RoundedRectangle(cornerRadius: 2)
                    .fill(color)
                    .frame(width: 4)
            }

            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    if note.pinned {
                        Image(systemName: "pin.fill")
                            .font(.caption)
                            .foregroundStyle(.orange)
                    }

                    if note.locked {
                        Image(systemName: "lock.fill")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }

                    Text(note.title)
                        .font(.headline)
                        .lineLimit(1)

                    Spacer()
                }

                if !note.preview.isEmpty {
                    Text(note.preview)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                }

                HStack(spacing: 8) {
                    Text(note.modifiedAt.relativeDescription)
                        .font(.caption)
                        .foregroundStyle(.tertiary)

                    if !note.regularTags.isEmpty {
                        HStack(spacing: 4) {
                            ForEach(note.regularTags.prefix(3), id: \.self) { tag in
                                Text("#\(tag)")
                                    .font(.caption)
                                    .foregroundStyle(.accent)
                            }
                            if note.regularTags.count > 3 {
                                Text("+\(note.regularTags.count - 3)")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }

                    if !note.attachments.isEmpty {
                        Image(systemName: "paperclip")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
        .padding(.vertical, 4)
    }
}
