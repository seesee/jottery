import CoreTransferable
import UniformTypeIdentifiers

/// A file dragged onto a note. The file a drop hands over lasts only as long as
/// this import, so it's copied to where it can be read from later.
struct DroppedAttachment: Transferable {
    let incoming: IncomingAttachment

    static var transferRepresentation: some TransferRepresentation {
        FileRepresentation(importedContentType: .data) { received in
            let filename = received.file.lastPathComponent
            return DroppedAttachment(incoming: .copying(
                received.file,
                filename: filename,
                mimeType: IncomingAttachment.mimeType(forFilename: filename)
            ))
        }
    }
}
