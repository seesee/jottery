import SwiftUI

/// This device's name on the server, and what's waiting to go up.
struct SyncDeviceRows: View {
    @Environment(AppState.self) private var appState
    @State private var deviceName: String?
    @State private var pending = 0

    var body: some View {
        Group {
            if let deviceName {
                LabeledContent(L.settingsSyncDevice, value: deviceName)
            }
            LabeledContent(L.settingsSyncPending, value: "\(pending)")
        }
        .task(id: appState.isSyncing) {
            pending = appState.pendingSyncCount()
            if let client = appState.syncClient {
                deviceName = await client.deviceName()
            }
        }
    }
}
