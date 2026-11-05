import Foundation
import Testing

@testable import Jottery

/// Checks what ships: the compiled strings in each `.lproj` of the built app and
/// share extension, not the catalog files they came from. A language that never
/// reached the bundle, or a catalog the extension doesn't copy, fails here.
@Suite("Localisation catalog")
struct LocalisationCatalogTests {

    /// Every language the web app offers, as iOS names them. en-US is checked
    /// separately, as it has to match en-GB word for word.
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

    /// A language doesn't fall back to another for a string it lacks: the app
    /// shows the raw key. So American English needs everything British English
    /// has, even though most of it is spelt the same.
    @Test func americanEnglishHasEveryString() throws {
        for bundle in [Bundle.main, try #require(Self.shareExtension)] {
            let british = Self.table("Localizable", language: "en-GB", in: bundle)
            let american = Self.table("Localizable", language: "en-US", in: bundle)

            #expect(!british.isEmpty)
            #expect(Set(british.keys).subtracting(american.keys).isEmpty)
        }
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

    // MARK: - Plurals

    /// Which forms each language needs. English has two, Russian and Polish four,
    /// and Japanese, Korean and Chinese one (`other`).
    private static func pluralCategories(for language: String) -> Set<String> {
        switch language {
        case "ru", "pl": ["one", "few", "many", "other"]
        case "ja", "ko", "zh-Hans": ["other"]
        default: ["one", "other"]
        }
    }

    /// The forms for each plural key: `[key: [category: text]]`.
    private static func plurals(language: String) -> [String: [String: String]] {
        guard let url = Bundle.main.url(
            forResource: "Localizable", withExtension: "stringsdict", subdirectory: nil, localization: language
        ), let table = NSDictionary(contentsOf: url) as? [String: [String: Any]] else { return [:] }

        return table.mapValues { entry in
            // One argument per string, under a name the compiler chooses.
            let argument = entry.values.compactMap { $0 as? [String: Any] }.first ?? [:]
            return argument.compactMapValues { $0 as? String }.filter { !$0.key.hasPrefix("NSString") }
        }
    }

    @Test(arguments: translated)
    func everyPluralHasTheFormsItsLanguageNeeds(language: String) {
        let english = Self.plurals(language: "en-GB")
        let table = Self.plurals(language: language)
        #expect(english.count >= 20, "the English plurals didn't load")

        for key in english.keys {
            #expect(table[key] != nil, "\(language) is missing \(key)")
            guard let forms = table[key] else { continue }
            #expect(Set(forms.keys) == Self.pluralCategories(for: language), "\(language): \(key)")
            for (category, text) in forms {
                #expect(text.contains("%"), "\(language): \(key) \(category) loses its number")
            }
        }
    }

    /// Takes the actual lookup, not just the table: the count picks the form.
    @Test func aCountChoosesTheRightForm() throws {
        let path = try #require(Bundle.main.path(forResource: "ru", ofType: "lproj"))
        let russian = try #require(Bundle(path: path))
        func banner(_ count: Int) -> String {
            String(localized: "conflict.banner \(count)", bundle: russian, locale: Locale(identifier: "ru"))
        }

        #expect(banner(1).hasPrefix("1 "))
        #expect(banner(2) != banner(5).replacingOccurrences(of: "5", with: "2"), "2 and 5 take different forms")
        #expect(banner(5) == banner(11).replacingOccurrences(of: "11", with: "5"), "5 and 11 take the same form")
        #expect(banner(1) != banner(5).replacingOccurrences(of: "5", with: "1"), "1 and 5 take different forms")
    }

    /// Short words like "OK" and "Face ID" are the same everywhere, but a language
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
