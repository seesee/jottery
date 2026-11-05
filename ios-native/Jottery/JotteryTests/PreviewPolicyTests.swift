import Foundation
import Testing

@testable import Jottery

/// A note can come from the server or an imported file, so what its preview
/// asks of the app has to be limited here, whatever the page does.
struct PreviewPolicyTests {

    @Test(arguments: ["http://example.com", "https://example.com/a?b=c", "HTTPS://EXAMPLE.COM", "mailto:a@example.com"])
    func ordinaryLinksOpen(link: String) throws {
        #expect(PreviewPolicy.canOpenExternally(try #require(URL(string: link))))
    }

    @Test(arguments: ["tel:999", "sms:999", "file:///etc/passwd", "javascript:alert(1)", "facetime:1", "itms-services://?action=download", "shortcuts://run", "data:text/html,x", "jottery://x"])
    func nothingElseDoes(link: String) throws {
        #expect(!PreviewPolicy.canOpenExternally(try #require(URL(string: link))))
    }

    @Test func aLinkWithNoSchemeDoesntOpen() throws {
        #expect(!PreviewPolicy.canOpenExternally(try #require(URL(string: "example.com/page"))))
    }

    private let bundle = URL(fileURLWithPath: "/App.app/preview.html")

    @Test func theBundleLoads() {
        #expect(PreviewPolicy.allowsNavigation(to: bundle, bundle: bundle, isMainFrame: true))
    }

    @Test func aFragmentOfTheBundleIsStillTheBundle() throws {
        let url = try #require(URL(string: "file:///App.app/preview.html#heading"))
        #expect(PreviewPolicy.allowsNavigation(to: url, bundle: bundle, isMainFrame: true))
    }

    @Test(arguments: ["https://example.com", "file:///App.app/jottery.db", "file:///etc/passwd", "about:blank", "data:text/html,x"])
    func nothingElseLoads(link: String) throws {
        #expect(!PreviewPolicy.allowsNavigation(to: URL(string: link), bundle: bundle, isMainFrame: true))
    }

    @Test func aFrameNeverLoads() {
        #expect(!PreviewPolicy.allowsNavigation(to: bundle, bundle: bundle, isMainFrame: false))
    }

    @Test func nothingLoadsWithoutABundle() {
        #expect(!PreviewPolicy.allowsNavigation(to: bundle, bundle: nil, isMainFrame: true))
    }
}
