import SwiftUI

struct UnlockScreen: View {
    @Environment(AppState.self) private var appState
    @State private var password = ""
    @State private var error: String?
    @State private var isUnlocking = false
    @State private var showBiometricPrompt = false
    @State private var failedAttempts = 0
    @State private var showDeleteConfirm = false
    @State private var didAutoAttemptBiometric = false

    var body: some View {
        VStack(spacing: 32) {
            Spacer()

            Image(systemName: "lock.fill")
                .font(.system(size: 48))
                .foregroundStyle(.secondary)

            Text(L.unlockTitle)
                .font(.largeTitle.bold())

            Text(L.unlockPrompt)
                .foregroundStyle(.secondary)

            VStack(spacing: 16) {
                SecureField(L.unlockPassword, text: $password)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: 320)
                    .onSubmit { unlock() }
                    .disabled(isUnlocking)

                if let error {
                    Text(error)
                        .foregroundStyle(.red)
                        .font(.callout)
                }

                Button(action: unlock) {
                    if isUnlocking {
                        ProgressView()
                            .controlSize(.small)
                    } else {
                        Text(L.unlockAction)
                    }
                }
                .buttonStyle(.borderedProminent)
                .disabled(password.isEmpty || isUnlocking)

                if failedAttempts >= 5 {
                    Button(action: { showDeleteConfirm = true }) {
                        Text(L.unlockDeleteAndStartOver)
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(.red)
                }
            }

            // Biometric button
            if appState.keyManager.isBiometricEnabled {
                Button {
                    biometricUnlock()
                } label: {
                    Label(L.unlockFaceId, systemImage: "faceid")
                        .font(.title3)
                }
                .buttonStyle(.plain)
            }

            Spacer()
        }
        .padding()
        .onAppear {
            // Attempt biometric unlock automatically — once per lock cycle.
            // onAppear can fire more than once (Setup→Unlock branch swap,
            // scene-phase churn from the Face ID sheet itself), which
            // previously queued a second Face ID prompt.
            if appState.keyManager.isBiometricEnabled && !didAutoAttemptBiometric {
                didAutoAttemptBiometric = true
                biometricUnlock()
            }
        }
        .alert(L.unlockDeleteConfirmTitle, isPresented: $showDeleteConfirm) {
            Button(L.unlockDeleteConfirmAction, role: .destructive) {
                appState.wipeAllData()
            }
            Button(L.unlockDeleteConfirmCancel, role: .cancel) {}
        } message: {
            Text(L.unlockDeleteConfirmMessage)
        }
        .alert(L.unlockEnableFaceIdTitle, isPresented: $showBiometricPrompt) {
            Button(L.unlockEnableFaceIdAction) {
                try? appState.keyManager.enableBiometricUnlock()
            }
            Button(L.unlockEnableFaceIdLater, role: .cancel) {}
        } message: {
            Text(L.unlockEnableFaceIdMessage)
        }
    }

    private func unlock() {
        error = nil
        isUnlocking = true

        Task {
            do {
                try appState.unlock(password: password)
                password = ""
                isUnlocking = false

                // Offer to enable biometrics after first successful password unlock
                if appState.keyManager.isBiometricAvailable
                    && !appState.keyManager.isBiometricEnabled {
                    showBiometricPrompt = true
                }
            } catch {
                failedAttempts += 1
                if failedAttempts >= 5 {
                    self.error = L.unlockFailedAttempts
                } else {
                    self.error = L.unlockIncorrectPassword
                }
                self.isUnlocking = false
            }
        }
    }

    private func biometricUnlock() {
        Task {
            let success = await appState.keyManager.attemptBiometricUnlock()
            if success {
                try? appState.loadNotes()
                appState.isLocked = false
                Log.debug("[Sync] biometricUnlock: calling setupSync()")
                appState.setupSync()
                appState.scheduleSearchWarmUp()
                appState.importSharedInboxItems()
                Log.debug("[Envelope] Biometric unlock — envelope migration skipped (password unavailable)")
            } else {
                Log.debug("[Sync] biometricUnlock: Face ID failed")
            }
        }
    }
}
