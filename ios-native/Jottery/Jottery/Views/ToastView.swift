import SwiftUI

/// A transient, bottom-anchored error toast bound to `AppState.userErrorMessage`
/// (jottery-jqx8). Failures from `try?`-wrapped note actions (delete, pin,
/// archive, attachments, bulk operations, …) route through
/// `AppState.reportError(_:)`, which auto-clears the message after a few
/// seconds — this view just renders whatever is currently published.
///
/// Mounted once in `MainView` so it is visible regardless of which pane
/// (note list or editor) triggered the failure. `RecycleBinView` — presented
/// as a `.sheet`, which sits in its own presentation layer above `MainView`
/// — mounts its own copy so failures inside the recycle bin are visible too.
struct ToastView: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        VStack {
            Spacer()
            if let message = appState.userErrorMessage {
                HStack(spacing: 8) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(.red)
                        .font(.caption)
                    Text(message)
                        .font(.caption)
                        .foregroundStyle(.primary)
                        .lineLimit(2)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 12))
                .shadow(color: .black.opacity(0.15), radius: 8, y: 2)
                .padding(.horizontal, 20)
                .padding(.bottom, 20)
                .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .animation(.easeInOut(duration: 0.25), value: appState.userErrorMessage)
        .allowsHitTesting(false)
    }
}
