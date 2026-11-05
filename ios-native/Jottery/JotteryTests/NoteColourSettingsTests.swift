import CryptoKit
import Foundation
import GRDB
import Testing

@testable import Jottery

private func makeNote(
    id: String = UUID().uuidString,
    content: String = "",
    tags: [String] = [],
    color: String? = nil
) -> DecryptedNote {
    DecryptedNote(
        id: id, createdAt: Date(), modifiedAt: Date(), syncedAt: nil, content: content, tags: tags,
        attachments: [], pinned: false, archived: false, archivedAt: nil, locked: false,
        lockedAt: nil, deleted: false, deletedAt: nil, version: 1, wordWrap: true, syntaxLanguage: "plain",
        showPreview: false, color: color, contentHash: nil, parentHash: nil, hashChain: [], needsSync: false,
        decryptedAt: Date()
    )
}

/// Category names, tag colours and the default note type (R2-141).
@MainActor
struct NoteColourSettingsTests {

    private func makeVault() async throws -> (state: AppState, password: String) {
        let password = "colours-\(UUID().uuidString)"
        let state = try TestVaults.makeState()
        try await state.createVault(password: password)
        return (state, password)
    }

    private func rawSettings(_ state: AppState) throws -> Row {
        let database = try #require(state.db)
        let row = try database.dbPool.read { db in
            try Row.fetchOne(db, sql: "SELECT * FROM settings WHERE id = 1")
        }
        return try #require(row)
    }

    private func ids(_ query: String, in notes: [DecryptedNote], colours: NoteColourSettings) -> [String] {
        SearchService.filter(notes: notes, query: query, colours: colours).map(\.id).sorted()
    }

    // MARK: - Names

    @Test func aColourWithNoNameShowsItsOwn() {
        #expect(NoteColourSettings().displayName(for: "red") == "Red")
    }

    @Test func aColourShowsTheNameItWasGiven() {
        var colours = NoteColourSettings()
        colours.setCategoryName("Urgent", for: "red")
        #expect(colours.displayName(for: "red") == "Urgent")
    }

    @Test func aBlankNameGivesTheColourItsOwnBack() {
        var colours = NoteColourSettings()
        colours.setCategoryName("Urgent", for: "red")
        colours.setCategoryName("   ", for: "red")
        #expect(colours.displayName(for: "red") == "Red")
        #expect(colours.categoryNames.isEmpty)
    }

    /// Stored as typed, or the space in "Project X" would vanish on its way in.
    @Test func aNameKeepsItsSpaceWhileBeingTyped() {
        var colours = NoteColourSettings()
        colours.setCategoryName("Project ", for: "red")
        #expect(colours.categoryNames["red"] == "Project ")
        #expect(colours.displayName(for: "red") == "Project")
    }

    @Test func aNameForSomethingThatIsntAColourIsIgnored() {
        var colours = NoteColourSettings()
        colours.setCategoryName("Urgent", for: "mauve")
        #expect(colours.categoryNames.isEmpty)
    }

    // MARK: - Tag colours

    @Test func aTagColourIgnoresCase() {
        var colours = NoteColourSettings()
        colours.setColour("blue", forTag: "Work")
        #expect(colours.colourKey(forTag: "work") == "blue")
        #expect(colours.colourKey(forTag: "WORK ") == "blue")
        #expect(colours.colourKey(forTag: "home") == nil)
    }

    @Test func noColourRemovesATag() {
        var colours = NoteColourSettings()
        colours.setColour("blue", forTag: "work")
        colours.setColour(nil, forTag: "work")
        #expect(colours.tagColours.isEmpty)
    }

    @Test func aColourThatIsntOneIsIgnored() {
        var colours = NoteColourSettings()
        colours.setColour("mauve", forTag: "work")
        #expect(colours.tagColours.isEmpty)
    }

    // MARK: - Search (searchService.ts)

    private var named: NoteColourSettings {
        var colours = NoteColourSettings()
        colours.setCategoryName("Urgent", for: "red")
        colours.setCategoryName("Project X", for: "blue")
        colours.setColour("red", forTag: "fire")
        return colours
    }

    /// The scenario in the review: a colour named Urgent on the web.
    @Test func aCategoryIsFoundByItsName() {
        let red = makeNote(id: "red", color: "red")
        let blue = makeNote(id: "blue", color: "blue")

        #expect(ids("category:urgent", in: [red, blue], colours: named) == ["red"])
        #expect(ids("category:URGENT", in: [red, blue], colours: named) == ["red"])
        #expect(ids("category:Project X", in: [red, blue], colours: named) == ["blue"])
    }

    /// Once a colour has a name, that's the only name it answers to.
    @Test func aRenamedColourNoLongerAnswersToItsKeyAsACategory() {
        let red = makeNote(id: "red", color: "red")

        #expect(ids("category:red", in: [red, makeNote(id: "other")], colours: named) == ["other", "red"])
    }

    @Test func aCategoryWithNoNameStillGoesByItsKey() {
        let green = makeNote(id: "green", color: "green")

        #expect(ids("category:green", in: [green, makeNote(id: "other")], colours: named) == ["green"])
    }

    @Test func colorAlwaysGoesByKey() {
        let red = makeNote(id: "red", color: "red")

        #expect(ids("color:red", in: [red], colours: named) == ["red"])
        #expect(ids("color:urgent", in: [red], colours: named) == [])
    }

    @Test func aTaggedNoteMatchesThroughItsTagsColour() {
        let tagged = makeNote(id: "tagged", tags: ["Fire"])
        let plain = makeNote(id: "plain", tags: ["water"])

        #expect(ids("color:red", in: [tagged, plain], colours: named) == ["tagged"])
        #expect(ids("category:urgent", in: [tagged, plain], colours: named) == ["tagged"])
    }

    @Test func targetsChooseBetweenTheNoteAndItsTags() {
        let coloured = makeNote(id: "coloured", color: "red")
        let tagged = makeNote(id: "tagged", tags: ["fire"])
        let both = [coloured, tagged]

        #expect(ids("color:red", in: both, colours: named) == ["coloured", "tagged"])
        #expect(ids("color:red note", in: both, colours: named) == ["coloured"])
        #expect(ids("color:red tag", in: both, colours: named) == ["tagged"])
        #expect(ids("category:urgent note", in: both, colours: named) == ["coloured"])
        #expect(ids("category:urgent tag", in: both, colours: named) == ["tagged"])
    }

    /// As on the web: a name nothing has is no filter, not an empty result.
    @Test func anUnknownCategoryFiltersNothing() {
        let note = makeNote(id: "n")

        #expect(ids("category:nothing", in: [note], colours: named) == ["n"])
    }

    @Test func theListUsesTheVaultsNames() async throws {
        let (state, _) = try await makeVault()
        let note = try #require(try state.createNote(content: "Fire drill"))
        var updated = note
        updated.color = "red"
        try state.saveNote(updated)
        try state.setCategoryName("Urgent", for: "red")

        state.searchQuery = "category:urgent"
        state.scheduleSearch()

        await waitUntil { state.filteredNotes.map(\.id) == [note.id] }
        #expect(state.filteredNotes.map(\.id) == [note.id])
    }

    /// Changing a name changes what a search on screen finds, there and then.
    @Test func renamingRunsTheSearchAgain() async throws {
        let (state, _) = try await makeVault()
        let note = try #require(try state.createNote(content: "Fire drill"))
        var updated = note
        updated.color = "red"
        try state.saveNote(updated)
        _ = try state.createNote(content: "Something else")
        state.searchQuery = "category:urgent"
        state.scheduleSearch()
        await waitUntil { state.filteredNotes.count == 2 }

        try state.setCategoryName("Urgent", for: "red")

        await waitUntil { state.filteredNotes.map(\.id) == [note.id] }
        #expect(state.filteredNotes.map(\.id) == [note.id])
    }

    // MARK: - Storage

    /// Settings are plain columns in plain SQLite. A tag is as sensitive as a note.
    @Test func namesAndTagsAreEncryptedOnDisk() async throws {
        let (state, _) = try await makeVault()
        let secretName = "name-\(UUID().uuidString)"
        let secretTag = "tag-\(UUID().uuidString)"

        try state.setCategoryName(secretName, for: "red")
        try state.setTagColour("blue", forTag: secretTag)

        let row = try rawSettings(state)
        for column in row.columnNames {
            let text = (row[column] as DatabaseValue).description
            #expect(!text.contains(secretName))
            #expect(!text.lowercased().contains(secretTag))
        }
        let palette: String = try #require(row["color_palette"])
        let tags: String = try #require(row["tag_colors"])
        #expect(!(try CryptoService.parseEncryptedJSON(palette).ciphertext.isEmpty))
        #expect(!(try CryptoService.parseEncryptedJSON(tags).ciphertext.isEmpty))
    }

    @Test func theyCameBackAfterAnUnlock() async throws {
        let (state, password) = try await makeVault()
        try state.setCategoryName("Urgent", for: "red")
        try state.setTagColour("blue", forTag: "work")

        state.lock()
        try await state.unlock(password: password)

        #expect(state.noteColours.displayName(for: "red") == "Urgent")
        #expect(state.noteColours.colourKey(forTag: "work") == "blue")
    }

    @Test func lockingTakesThemOutOfMemory() async throws {
        let (state, _) = try await makeVault()
        try state.setCategoryName("Urgent", for: "red")
        try state.setTagColour("blue", forTag: "work")

        state.lock()

        #expect(state.noteColours == NoteColourSettings())
    }

    /// D3: what's in the column isn't trusted just because it parses as JSON.
    @Test func plaintextInTheColumnIsNeverUsed() async throws {
        let (state, password) = try await makeVault()
        try TestVaults.execute(state, "UPDATE settings SET color_palette = ?, tag_colors = ? WHERE id = 1", [
            #"{"red":"Planted"}"#, #"{"work":"red"}"#,
        ])

        state.lock()
        try await state.unlock(password: password)

        #expect(state.noteColours == NoteColourSettings())
    }

    @Test func aValueUnderAnotherKeyIsNeverUsed() async throws {
        let (state, password) = try await makeVault()
        let foreign = try TestVaults.foreignCiphertext()
        try TestVaults.execute(state, "UPDATE settings SET color_palette = ? WHERE id = 1", [foreign])

        state.lock()
        try await state.unlock(password: password)

        #expect(state.noteColours == NoteColourSettings())
    }

    /// One unreadable column costs only that column.
    @Test func oneUnreadableColumnLeavesTheOther() async throws {
        let (state, password) = try await makeVault()
        try state.setCategoryName("Urgent", for: "red")
        try state.setTagColour("blue", forTag: "work")
        let foreign = try TestVaults.foreignCiphertext()
        try TestVaults.execute(state, "UPDATE settings SET tag_colors = ? WHERE id = 1", [foreign])

        state.lock()
        try await state.unlock(password: password)

        #expect(state.noteColours.displayName(for: "red") == "Urgent")
        #expect(state.noteColours.tagColours.isEmpty)
    }

    @Test func savingSettingsLeavesThemAlone() async throws {
        let (state, password) = try await makeVault()
        try state.setCategoryName("Urgent", for: "red")

        var settings = state.settings
        settings.theme = "dark"
        try state.updateSettings(settings)
        state.lock()
        try await state.unlock(password: password)

        #expect(state.noteColours.displayName(for: "red") == "Urgent")
    }

    // MARK: - Changing the key

    /// A legacy password change re-encrypts the whole vault. These are under the
    /// same key, so without moving them too they'd be lost without a word.
    @Test func aLegacyPasswordChangeKeepsThem() async throws {
        let password = "legacy-\(UUID().uuidString)"
        let state = try await TestVaults.makeLegacyVault(password: password)
        try state.setCategoryName("Urgent", for: "red")
        try state.setTagColour("blue", forTag: "work")
        let newPassword = "new-\(UUID().uuidString)"

        try await state.changePassword(currentPassword: password, newPassword: newPassword)
        state.lock()
        try await state.unlock(password: newPassword)

        #expect(state.noteColours.displayName(for: "red") == "Urgent")
        #expect(state.noteColours.colourKey(forTag: "work") == "blue")
    }

    @Test func movingToTheAccountsKeyKeepsThem() async throws {
        let (state, password) = try await makeVault()
        try state.setCategoryName("Urgent", for: "red")

        try await state.switchVaultToMasterKey(CryptoService.generateMasterKey(), notesPassword: password)
        state.lock()
        try await state.unlock(password: password)

        #expect(state.noteColours.displayName(for: "red") == "Urgent")
    }

    /// They're preferences. One that was already unreadable can't be allowed to
    /// stop a password change, which is what a damaged note does to protect it.
    @Test func anUnreadableValueDoesntStopARekey() async throws {
        let (state, password) = try await makeVault()
        let foreign = try TestVaults.foreignCiphertext()
        try TestVaults.execute(state, "UPDATE settings SET color_palette = ? WHERE id = 1", [foreign])

        try await state.switchVaultToMasterKey(CryptoService.generateMasterKey(), notesPassword: password)

        #expect(try rawSettings(state)["color_palette"] == foreign)
    }

    // MARK: - Default note type

    private func setDefaultType(_ type: String?, on state: AppState) throws {
        var settings = state.settings
        settings.defaultSyntaxLanguage = type
        try state.updateSettings(settings)
    }

    @Test func aNewNoteTakesTheDefaultType() async throws {
        let (state, _) = try await makeVault()
        try setDefaultType("calc", on: state)

        let note = try #require(try state.createNote())

        #expect(note.syntaxLanguage == "calc")
    }

    @Test func withNoDefaultANewNoteIsMarkdown() async throws {
        let (state, _) = try await makeVault()

        #expect(try state.createNote()?.syntaxLanguage == "markdown")
    }

    /// The type is part of the hash, so a note can't have its type changed after
    /// the hash was taken without looking like an edit.
    @Test func theHashIsTakenWithTheType() async throws {
        let (state, _) = try await makeVault()
        try setDefaultType("calc", on: state)

        let note = try #require(try state.createNote())

        let record = try #require(try state.noteRepo?.getRaw(id: note.id))
        #expect(record.syntaxLanguage == "calc")
        let markdownHash = HashChainService.computeContentHash(
            encryptedContent: record.content, encryptedTags: [], attachmentIds: [],
            pinned: false, archived: false, locked: false, syntaxLanguage: "markdown",
            wordWrap: record.wordWrap, showPreview: record.showPreview, color: nil
        )
        let calcHash = HashChainService.computeContentHash(
            encryptedContent: record.content, encryptedTags: [], attachmentIds: [],
            pinned: false, archived: false, locked: false, syntaxLanguage: "calc",
            wordWrap: record.wordWrap, showPreview: record.showPreview, color: nil
        )
        #expect(record.contentHash == calcHash)
        #expect(record.contentHash != markdownHash)
    }

    /// Text arriving from elsewhere isn't a calculation because new notes are.
    @Test func aNoteWithContentKeepsTheUsualType() async throws {
        let (state, _) = try await makeVault()
        try setDefaultType("calc", on: state)

        #expect(try state.createNote(content: "Shared from Safari", tags: ["shared"])?.syntaxLanguage == "markdown")
    }

    @Test func aTypeThisBuildDoesntKnowIsIgnored() async throws {
        let (state, _) = try await makeVault()
        try setDefaultType("cobol", on: state)

        #expect(try state.createNote()?.syntaxLanguage == "markdown")
    }

    @Test func theDefaultTypeSurvivesRelaunching() async throws {
        let (state, _) = try await makeVault()
        try setDefaultType("outliner", on: state)

        #expect(try state.settingsRepo?.get().defaultSyntaxLanguage == "outliner")
    }

    @Test func settingsFromBeforeTheDefaultTypeStillDecode() throws {
        let older = """
        {"id":1,"language":"system","theme":"auto","sort_order":"recent",
         "auto_lock_timeout":15,"sync_enabled":false}
        """
        let settings = try JSONDecoder().decode(UserSettings.self, from: Data(older.utf8))
        #expect(settings.defaultSyntaxLanguage == nil)
    }
}
