import SwiftUI

/// Layout around the fold of a foldable display (iPhone Duo's inner display).
///
/// When the display has a fold running top to bottom, the note list and the
/// open note sit either side of it, like the two pages of a book: the column
/// divider goes on the fold's centre line, so neither the list nor the note's
/// text spans it. When the device is partly folded, content across the fold is
/// hard to read and controls on it are hard to reach.
///
/// Everywhere else (the outer display, other iPhones, iPad, or the inner
/// display held so that the fold runs side to side) the split view keeps its
/// default behaviour.
enum FoldLayout {

    /// The narrowest list column worth showing beside the fold.
    static let minimumListWidth: CGFloat = 240

    /// The fold running top to bottom through the view `proxy` measures, in
    /// that view's coordinates, or nil when there is none. Includes an
    /// inactive fold (the device opened flat), so the layout doesn't jump as
    /// the hinge moves.
    static func verticalFold(in proxy: GeometryProxy) -> CGRect? {
        guard #available(iOS 27.1, *) else { return nil }
        let frames = proxy.reservedRegions(kind: .division, options: .includeInactive).map(\.frame)
        return verticalFold(among: frames, containerWidth: proxy.size.width)
    }

    /// The fold among `frames` that runs top to bottom and lies inside the
    /// container. Taller than wide means vertical; a fold running side to side
    /// cannot be honoured by a side-by-side split, so it is ignored.
    static func verticalFold(among frames: [CGRect], containerWidth: CGFloat) -> CGRect? {
        frames.first { frame in
            frame.height > frame.width && frame.midX > 0 && frame.midX < containerWidth
        }
    }

    /// The list column width that puts the split view's divider on the fold's
    /// centre line, or nil when that would leave either side too narrow.
    static func listWidth(fold: CGRect, containerWidth: CGFloat) -> CGFloat? {
        let width = fold.midX.rounded()
        guard width >= minimumListWidth, containerWidth - width >= minimumListWidth else { return nil }
        return width
    }
}

/// The fold (if any) and the width of the view it was measured in.
struct FoldGeometry: Equatable {
    var fold: CGRect?
    var width: CGFloat = 0
}

/// Fixes the list column's width so the split view's divider sits on the fold.
/// Without a fold the column keeps the split view's default width.
struct FoldAlignedColumn: ViewModifier {
    let width: CGFloat?

    func body(content: Content) -> some View {
        if let width {
            content.navigationSplitViewColumnWidth(width)
        } else {
            content
        }
    }
}
