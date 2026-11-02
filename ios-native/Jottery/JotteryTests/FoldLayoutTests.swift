import CoreGraphics
import Testing

@testable import Jottery

/// On iPhone Duo's inner display the note list and the open note sit either
/// side of the fold. These cover the geometry behind that, which the simulator
/// can only show by opening the device in Device Hub.
struct FoldLayoutTests {

    /// Roughly the inner display in portrait: 669 pt wide, fold down the middle.
    private let innerWidth: CGFloat = 669
    private let fold = CGRect(x: 324, y: 0, width: 21, height: 951)

    @Test func aFoldRunningTopToBottomIsUsed() {
        #expect(FoldLayout.verticalFold(among: [fold], containerWidth: innerWidth) == fold)
    }

    @Test func aFoldRunningSideToSideIsIgnored() {
        // The inner display held sideways: a side-by-side split can't honour it.
        let horizontal = CGRect(x: 0, y: 324, width: 951, height: 21)
        #expect(FoldLayout.verticalFold(among: [horizontal], containerWidth: 951) == nil)
    }

    @Test func noFoldMeansNoChange() {
        #expect(FoldLayout.verticalFold(among: [], containerWidth: innerWidth) == nil)
    }

    @Test func aFoldOutsideTheViewIsIgnored() {
        // e.g. measured in a view that doesn't span the fold.
        let elsewhere = CGRect(x: 700, y: 0, width: 21, height: 951)
        #expect(FoldLayout.verticalFold(among: [elsewhere], containerWidth: innerWidth) == nil)
    }

    @Test func theDividerGoesOnTheFoldsCentreLine() {
        #expect(FoldLayout.listWidth(fold: fold, containerWidth: innerWidth) == 335)
    }

    @Test func aFoldTooCloseToAnEdgeLeavesTheDefaultWidth() {
        let nearLeading = CGRect(x: 150, y: 0, width: 21, height: 951)
        let nearTrailing = CGRect(x: 520, y: 0, width: 21, height: 951)
        #expect(FoldLayout.listWidth(fold: nearLeading, containerWidth: innerWidth) == nil)
        #expect(FoldLayout.listWidth(fold: nearTrailing, containerWidth: innerWidth) == nil)
    }
}
