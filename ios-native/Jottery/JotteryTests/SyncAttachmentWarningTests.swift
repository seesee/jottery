import Foundation
import Testing

@testable import Jottery

/// Coverage for the server's attachmentWarnings push-response field and the
/// client reaction that re-uploads blobs this device still holds.
struct SyncAttachmentWarningTests {

    @Test func pushResponseDecodesWithWarnings() throws {
        let json = """
        {"accepted":[],"rejected":[],"attachmentWarnings":[{"noteId":"n1","attachmentIds":["a1","a2"]}]}
        """
        let response = try JSONDecoder().decode(SyncPushResponse.self, from: Data(json.utf8))
        let warnings = try #require(response.attachmentWarnings)
        #expect(warnings.count == 1)
        #expect(warnings[0].noteId == "n1")
        #expect(warnings[0].attachmentIds == ["a1", "a2"])
    }

    @Test func pushResponseDecodesWithoutWarnings() throws {
        let json = """
        {"accepted":[],"rejected":[]}
        """
        let response = try JSONDecoder().decode(SyncPushResponse.self, from: Data(json.utf8))
        #expect(response.attachmentWarnings == nil)
    }

    @Test func collectRepairAttachmentsReturnsHeldBlobsOnly() {
        let warnings = [
            AttachmentWarning(noteId: "n1", attachmentIds: ["held", "ghost"]),
            AttachmentWarning(noteId: "n2", attachmentIds: ["held"]), // duplicate id
        ]
        let blob = Data("hello".utf8)

        let repairs = SyncService.collectRepairAttachments(for: warnings) { id in
            id == "held" ? blob : nil
        }

        #expect(repairs.count == 1)
        #expect(repairs[0].id == "held")
        #expect(repairs[0].data == blob.base64EncodedString())
    }

    @Test func collectRepairAttachmentsEmptyWhenNothingHeld() {
        let warnings = [AttachmentWarning(noteId: "n1", attachmentIds: ["ghost"])]
        let repairs = SyncService.collectRepairAttachments(for: warnings) { _ in nil }
        #expect(repairs.isEmpty)
    }
}
