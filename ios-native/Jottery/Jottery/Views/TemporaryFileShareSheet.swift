import SwiftUI

/// A temporary file to hand to the share sheet.
struct ExportFile: Identifiable {
    let id = UUID()
    let url: URL
}

extension View {
    /// Present the system share sheet for `file`, and delete the file when the
    /// sheet goes away, however that happens.
    func shareTemporaryFile(_ file: Binding<ExportFile?>) -> some View {
        modifier(TemporaryFileShareSheet(file: file))
    }
}

private struct TemporaryFileShareSheet: ViewModifier {
    @Binding var file: ExportFile?
    /// `file` is nil again by the time the sheet reports it has gone.
    @State private var shown: URL?

    func body(content: Content) -> some View {
        content
            .sheet(item: $file, onDismiss: removeShownFile) { file in
                ActivityView(activityItems: [file.url])
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

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: activityItems, applicationActivities: nil)
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
