import SwiftUI

/// A full screen that fills the space and centres like a plain stack when there's
/// room, and scrolls when there isn't: at large text sizes, or with the keyboard
/// up in landscape, where a fixed stack puts its buttons off screen.
struct ScrollableScreen<Content: View>: View {
    @ViewBuilder var content: Content

    var body: some View {
        GeometryReader { proxy in
            ScrollView {
                content
                    .frame(maxWidth: .infinity, minHeight: proxy.size.height)
            }
            .scrollBounceBehavior(.basedOnSize)
            .scrollDismissesKeyboard(.interactively)
        }
    }
}
