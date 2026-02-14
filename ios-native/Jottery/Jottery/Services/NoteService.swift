import CryptoKit
import Foundation

/// Business logic over NoteRepository.
struct NoteService {

    let noteRepo: NoteRepository
    let key: SymmetricKey

    /// Create a new empty note.
    func createNote(content: String = "", tags: [String] = []) throws -> DecryptedNote {
        try noteRepo.create(content: content, tags: tags, key: key)
    }

    /// Save changes to an existing note.
    func saveNote(_ note: DecryptedNote) throws {
        try noteRepo.update(note, key: key)
    }

    /// Soft-delete a note.
    func deleteNote(id: String) throws {
        try noteRepo.softDelete(id: id)
    }

    /// Restore a deleted note.
    func restoreNote(id: String) throws {
        try noteRepo.restore(id: id)
    }

    /// Permanently delete a note.
    func permanentlyDelete(id: String) throws {
        try noteRepo.hardDelete(id: id)
    }

    /// Toggle pin status.
    func togglePin(id: String) throws {
        try noteRepo.togglePin(id: id)
    }

    /// Load all active notes.
    func loadActiveNotes() throws -> [DecryptedNote] {
        try noteRepo.listActive(key: key)
    }

    /// Load deleted notes.
    func loadDeletedNotes() throws -> [DecryptedNote] {
        try noteRepo.listDeleted(key: key)
    }

    /// Get a single note.
    func getNote(id: String) throws -> DecryptedNote? {
        try noteRepo.get(id: id, key: key)
    }
}
