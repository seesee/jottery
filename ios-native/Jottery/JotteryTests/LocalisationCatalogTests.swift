import Foundation
import Testing

@testable import Jottery

/// Checks what ships: the compiled strings in each `.lproj` of the built app and
/// share extension, not the catalog files they came from. A language that never
/// reached the bundle, or a catalog the extension doesn't copy, fails here.
@Suite("Localisation catalog")
struct LocalisationCatalogTests {

    /// Every language the web app offers, as iOS names them. en-US is here too,
    /// but it holds only the words spelt differently and falls back to en-GB.
    static let translated = ["de", "el", "es", "fr", "it", "ja", "ko", "nl", "pl", "pt", "ru", "tr", "zh-Hans"]

    /// `%@`, `%lld` and their positional forms, ignoring the position, because a
    /// translation may reorder them.
    private static func placeholders(in value: String) -> [String] {
        let pattern = /%(?:\d+\$)?(lld|ld|d|@)/
        return value.matches(of: pattern).map { String($0.output.1) }.sorted()
    }

    /// The compiled table for one language, or empty if it isn't in the bundle.
    private static func table(_ name: String, language: String, in bundle: Bundle) -> [String: String] {
        guard let url = bundle.url(forResource: name, withExtension: "strings", subdirectory: nil, localization: language),
              let table = NSDictionary(contentsOf: url) as? [String: String] else { return [:] }
        return table
    }

    private static var shareExtension: Bundle? {
        guard let plugIns = Bundle.main.builtInPlugInsURL else { return nil }
        return Bundle(url: plugIns.appendingPathComponent("JotteryShare.appex"))
    }

    /// Keys that stay the same in every language: nothing in them to translate.
    private static func hasWords(_ key: String) -> Bool {
        key.contains { $0.isLetter }
    }

    @Test func theAppOffersEveryLanguageTheWebAppDoes() {
        let shipped = Set(Bundle.main.localizations)
        for language in Self.translated + ["en-GB", "en-US"] {
            #expect(shipped.contains(language), "\(language) isn't in the app")
        }
    }

    @Test(arguments: translated)
    func everyStringIsTranslated(language: String) {
        let english = Self.table("Localizable", language: "en-GB", in: .main)
        let table = Self.table("Localizable", language: language, in: .main)
        #expect(english.count > 300, "the English table didn't load")

        let missing = english.keys.filter { Self.hasWords($0) && table[$0]?.isEmpty != false }
        #expect(missing.isEmpty, "\(language) is missing \(missing.sorted().prefix(10))")
    }

    @Test(arguments: translated)
    func translationsKeepTheirPlaceholders(language: String) {
        let english = Self.table("Localizable", language: "en-GB", in: .main)
        let table = Self.table("Localizable", language: language, in: .main)

        let wrong = english.filter { key, value in
            guard let translation = table[key] else { return false }
            return Self.placeholders(in: translation) != Self.placeholders(in: value)
        }
        #expect(wrong.isEmpty, "\(language) changes the placeholders of \(wrong.keys.sorted().prefix(10))")
    }

    /// Short words like "OK" and "Face ID" are the same everywhere, but a language
    /// left mostly in English is a file that was merged wrongly.
    @Test(arguments: translated)
    func translationsAreNotJustEnglish(language: String) {
        let english = Self.table("Localizable", language: "en-GB", in: .main)
        let table = Self.table("Localizable", language: language, in: .main)

        let same = english.filter { key, value in table[key] == value && value.count > 12 }
        #expect(same.count < 12, "\(language) leaves these in English: \(same.keys.sorted().prefix(12))")
    }

    /// In Korean the word first chosen for "vault" was the word for "archive".
    /// They're different things, and one sits on the first screen a user sees.
    @Test(arguments: translated)
    func theVaultIsNeverCalledTheArchive(language: String) throws {
        let table = Self.table("Localizable", language: language, in: .main)
        let archive = try #require(table["noteList.archive"])
        let vault = try #require(table["setup.newVault"])

        #expect(!vault.localizedCaseInsensitiveContains(archive), "\(language): \(vault) reads as \(archive)")
    }

    /// The tag prefix is typed the same in every language, so what the chip shows
    /// must not be translated into something that doesn't work when typed.
    @Test func theTitleTagStaysOutOfTheCatalog() {
        for language in Self.translated {
            #expect(Self.table("Localizable", language: language, in: .main)["title: %@"] == nil)
        }
    }

    @Test(arguments: translated)
    func theShareExtensionIsTranslatedToo(language: String) throws {
        let bundle = try #require(Self.shareExtension, "JotteryShare.appex isn't in the app")
        let english = Self.table("Localizable", language: "en-GB", in: bundle)
        let table = Self.table("Localizable", language: language, in: bundle)
        #expect(english.count >= 5, "the extension's English table didn't load")

        for (key, value) in english {
            #expect(table[key]?.isEmpty == false, "\(language) is missing \(key)")
            #expect(table[key] != value, "\(language) leaves \(key) in English")
        }
    }

    /// The system shows these itself, before the app has a chance to explain.
    @Test(arguments: translated)
    func systemPromptsAreTranslated(language: String) {
        let table = Self.table("InfoPlist", language: language, in: .main)
        for key in ["NSFaceIDUsageDescription", "NSLocalNetworkUsageDescription"] {
            #expect(table[key]?.isEmpty == false, "\(language) is missing \(key)")
            #expect(table[key] != Bundle.main.object(forInfoDictionaryKey: key) as? String, "\(language) leaves \(key) in English")
        }
    }
}
