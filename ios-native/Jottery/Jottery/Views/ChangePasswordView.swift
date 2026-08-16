import SwiftUI

struct ChangePasswordView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    @State private var currentPassword = ""
    @State private var newPassword = ""
    @State private var confirmPassword = ""
    @State private var errorMessage: String?
    @State private var isProcessing = false
    @State private var showSuccess = false

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    SecureField(L.changePasswordCurrent, text: $currentPassword)
                        .textContentType(.password)
                }

                Section {
                    SecureField(L.changePasswordNew, text: $newPassword)
                        .textContentType(.newPassword)
                    SecureField(L.changePasswordConfirm, text: $confirmPassword)
                        .textContentType(.newPassword)
                } footer: {
                    Text(L.changePasswordWarning)
                        .foregroundStyle(.secondary)
                }

                if let errorMessage {
                    Section {
                        Text(errorMessage)
                            .foregroundStyle(.red)
                    }
                }

                Section {
                    Button {
                        changePassword()
                    } label: {
                        HStack {
                            Text(L.changePasswordAction)
                            Spacer()
                            if isProcessing {
                                ProgressView()
                                    .controlSize(.small)
                            }
                        }
                    }
                    .disabled(!isValid || isProcessing)
                }
            }
            .navigationTitle(L.changePasswordTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.commonCancel) { dismiss() }
                }
            }
            .alert(L.changePasswordSuccessTitle, isPresented: $showSuccess) {
                Button(L.settingsDone) { dismiss() }
            } message: {
                Text(L.changePasswordSuccessMessage)
            }
        }
    }

    private var isValid: Bool {
        !currentPassword.isEmpty &&
        !newPassword.isEmpty &&
        newPassword == confirmPassword &&
        newPassword != currentPassword
    }

    private func changePassword() {
        errorMessage = nil
        isProcessing = true

        Task { @MainActor in
            defer { isProcessing = false }

            do {
                try await appState.changePassword(
                    currentPassword: currentPassword,
                    newPassword: newPassword
                )
                showSuccess = true
            } catch AppStateError.wrongPassword {
                errorMessage = L.unlockIncorrectPassword
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }
}
