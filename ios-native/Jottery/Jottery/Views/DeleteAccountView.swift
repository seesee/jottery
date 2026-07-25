import SwiftUI

/// Delete or deactivate the sync account on the user's own server.
///
/// Account-level operations need session auth, so this asks for the account
/// password rather than reusing the device API key. That is also the right bar
/// for an irreversible action: a stolen device should not be able to delete the
/// account behind it.
///
/// Notes on this device are untouched — deleting the sync account is not the
/// same as deleting the notes, and Settings → Reset already covers that.
struct DeleteAccountView: View {
    @Environment(AppState.self) private var appState
    @Environment(\.dismiss) private var dismiss

    /// Called after the server confirms deletion, so Settings can clear local sync state.
    let onDeleted: () -> Void

    @State private var email = ""
    @State private var password = ""
    @State private var mode: AccountDeletionMode = .deactivate
    @State private var isWorking = false
    @State private var error: String?
    @State private var showConfirmation = false

    private var canSubmit: Bool {
        !email.isEmpty && !password.isEmpty && !isWorking
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Text(L.deleteAccountExplanation)
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }

                Section(L.deleteAccountCredentials) {
                    TextField(L.setupEmail, text: $email)
                        .textContentType(.username)
                        .keyboardType(.emailAddress)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()

                    SecureField(L.setupServerPassword, text: $password)
                        .textContentType(.password)
                }

                Section {
                    Picker(L.deleteAccountMode, selection: $mode) {
                        Text(L.deleteAccountModeDeactivate).tag(AccountDeletionMode.deactivate)
                        Text(L.deleteAccountModeDelete).tag(AccountDeletionMode.delete)
                    }
                    .pickerStyle(.inline)
                } footer: {
                    Text(mode == .deactivate
                         ? L.deleteAccountDeactivateHint
                         : L.deleteAccountDeleteHint)
                }

                if let error {
                    Section {
                        Text(error)
                            .foregroundStyle(.red)
                            .font(.callout)
                    }
                }

                Section {
                    Button(role: .destructive) {
                        showConfirmation = true
                    } label: {
                        if isWorking {
                            ProgressView().controlSize(.small)
                        } else {
                            Text(L.deleteAccountAction)
                        }
                    }
                    .disabled(!canSubmit)
                }
            }
            .navigationTitle(L.deleteAccountTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.commonCancel) { dismiss() }
                        .disabled(isWorking)
                }
            }
            .alert(L.deleteAccountConfirmTitle, isPresented: $showConfirmation) {
                Button(L.deleteAccountAction, role: .destructive) {
                    Task { await submit() }
                }
                Button(L.commonCancel, role: .cancel) {}
            } message: {
                Text(mode == .deactivate
                     ? L.deleteAccountDeactivateHint
                     : L.deleteAccountDeleteHint)
            }
        }
    }

    private func submit() async {
        guard let client = appState.syncClient else {
            error = L.deleteAccountNoServer
            return
        }

        error = nil
        isWorking = true
        defer { isWorking = false }

        do {
            let sessionId = try await client.logIn(email: email, password: password)
            try await client.deleteAccount(sessionId: sessionId, mode: mode)
            onDeleted()
            dismiss()
        } catch SyncClientError.httpError(let status, _) where status == 401 {
            error = L.deleteAccountWrongCredentials
        } catch {
            self.error = SyncEndpoint.describeSyncFailure(error)
        }
    }
}
