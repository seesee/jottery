import Foundation
import Testing

@testable import Jottery

/// The offer used to be set on a screen that had already gone (jottery-wrw2).
@MainActor
struct FaceIDOfferTests {

    @Test(arguments: [
        (true, false, false, true),
        (false, false, false, false),
        (true, true, false, false),
        (true, false, true, false),
    ])
    func whenToOffer(available: Bool, enabled: Bool, declined: Bool, expected: Bool) {
        #expect(AppState.shouldOfferFaceID(isAvailable: available, isEnabled: enabled, wasDeclined: declined) == expected)
    }

    /// The flag lives on AppState, so the screen that shows it outlives the unlock.
    @Test func theOfferSurvivesTheUnlockScreenGoing() async throws {
        let state = try TestVaults.makeState()
        try await state.createVault(password: "notes-password-correct-horse")
        state.lock()
        try await state.unlock(password: "notes-password-correct-horse")

        state.offersFaceID = true

        #expect(!state.isLocked)
        #expect(state.offersFaceID)
    }
}
