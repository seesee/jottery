import SwiftUI

struct UnlockScreen: View {
    @Environment(AppState.self) private var appState
    @State private var password = ""
    @State private var error: String?
    @State private var isUnlocking = false
    @State private var showBiometricPrompt = false

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
            // Attempt biometric unlock automatically
            if appState.keyManager.isBiometricEnabled {
                biometricUnlock()
            }
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
                self.error = L.unlockIncorrectPassword
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
                print("[Sync] biometricUnlock: calling setupSync()")
                appState.setupSync()
                appState.scheduleSearchWarmUp()
                print("[Envelope] Biometric unlock — envelope migration skipped (password unavailable)")
            } else {
                print("[Sync] biometricUnlock: Face ID failed")
            }
        }
    }
}
