import SwiftUI

/// A temporary file to hand to the share sheet.
struct ExportFile: Identifiable {
    let id = UUID()
    let url: URL
    /// Text to offer alongside the file, for apps that take text and not files.
    var text: String?
}

extension View {
    /// Present the system share sheet for `file`, and delete the file when the
    /// sheet goes away, however that happens. `onShared` runs if it went
    /// somewhere, and not if the sheet was cancelled.
    func shareTemporaryFile(_ file: Binding<ExportFile?>, onShared: (() -> Void)? = nil) -> some View {
        modifier(TemporaryFileShareSheet(file: file, onShared: onShared))
    }
}

private struct TemporaryFileShareSheet: ViewModifier {
    @Binding var file: ExportFile?
    let onShared: (() -> Void)?
    /// `file` is nil again by the time the sheet reports it has gone.
    @State private var shown: URL?

    func body(content: Content) -> some View {
        content
            .sheet(item: $file, onDismiss: removeShownFile) { file in
                ActivityView(
                    activityItems: [file.text as Any?, file.url].compactMap { $0 },
                    onCompleted: onShared
                )
            }
            .onChange(of: file?.url) { _, url in
                if let url { shown = url }
            }
    }

    private func removeShownFile() {
        if let shown { TemporaryFiles.shared.remove(shown) }
        shown = nil
    }
}

// MARK: - Activity View (Share Sheet)

struct ActivityView: UIViewControllerRepresentable {
    let activityItems: [Any]
    var onCompleted: (() -> Void)?

    func makeUIViewController(context: Context) -> UIActivityViewController {
        let controller = UIActivityViewController(activityItems: activityItems, applicationActivities: nil)
        let onCompleted = onCompleted
        controller.completionWithItemsHandler = { _, completed, _, _ in
            if completed { onCompleted?() }
        }
        return controller
    }

    func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}

/// Run `work` off the main actor, cancelling it if the calling task is cancelled.
func offMainActor<T: Sendable>(_ work: @escaping @Sendable () throws -> T) async throws -> T {
    let task = Task.detached(priority: .userInitiated, operation: work)
    return try await withTaskCancellationHandler {
        try await task.value
    } onCancel: {
        task.cancel()
    }
}
