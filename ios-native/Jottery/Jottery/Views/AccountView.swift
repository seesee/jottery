import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// The sync account: what it's using, its inbox token, and deleting its notes
/// from the server. These need a session, not the device's key, so it signs in
/// with the account password first. Pushed from Settings.
struct AccountView: View {
    @Environment(AppState.self) private var appState

    @State private var model = AccountModel()
    @State private var email = ""
    @State private var password = ""
    @State private var showDeleteConfirmation = false
    @State private var deletePassword = ""
    @State private var showReplaceTokenConfirmation = false
    @State private var showRevokeTokenConfirmation = false
    @State private var tokenCopied = false

    private var client: SyncClient? { appState.syncClient }

    var body: some View {
        Form {
            if model.isSignedIn {
                usageSection
                inboxSection
                deleteSection
            } else {
                signInSections
            }

            if let error = model.errorMessage {
                Section {
                    Text(error)
                        .foregroundStyle(.red)
                }
            }
        }
        .navigationTitle(L.accountTitle)
        .navigationBarTitleDisplayMode(.inline)
        .onDisappear {
            // The app locking takes this away too, and takes the session with it.
            model.signOut(client: client)
            password = ""
            deletePassword = ""
        }
        .alert(L.accountDeleteNotesTitle, isPresented: $showDeleteConfirmation) {
            SecureField(L.setupServerPassword, text: $deletePassword)
            Button(L.accountDeleteNotesAction, role: .destructive) {
                let entered = deletePassword
                deletePassword = ""
                perform { await model.deleteServerNotes(password: entered, client: $0) }
            }
            .disabled(deletePassword.isEmpty)
            Button(L.commonCancel, role: .cancel) { deletePassword = "" }
        } message: {
            // An unknown count isn't zero, on something that can't be undone.
            Text(model.info?.noteCount.map(L.accountDeleteNotesMessage) ?? L.accountDeleteNotesMessageNoCount)
        }
        .alert(L.accountReplaceTokenTitle, isPresented: $showReplaceTokenConfirmation) {
            Button(L.accountReplaceTokenAction, role: .destructive) {
                perform { await model.generateInboxToken(client: $0) }
            }
            Button(L.commonCancel, role: .cancel) {}
        } message: {
            Text(L.accountReplaceTokenMessage)
        }
        .alert(L.accountRevokeTokenTitle, isPresented: $showRevokeTokenConfirmation) {
            Button(L.accountRevokeTokenAction, role: .destructive) {
                perform { await model.revokeInboxToken(client: $0) }
            }
            Button(L.commonCancel, role: .cancel) {}
        } message: {
            Text(L.accountRevokeTokenMessage)
        }
    }

    // MARK: - Signing In

    @ViewBuilder
    private var signInSections: some View {
        Section {
            TextField(L.setupEmail, text: $email)
                .textContentType(.username)
                .keyboardType(.emailAddress)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()

            PasswordField(L.setupServerPassword, text: $password, contentType: .password)
        } footer: {
            Text(L.accountSignInHint)
        }

        Section {
            Button {
                let entered = password
                password = ""
                let address = email
                perform { await model.signIn(email: address, password: entered, client: $0) }
            } label: {
                HStack {
                    Text(L.accountSignIn)
                    Spacer()
                    if model.isWorking {
                        ProgressView()
                            .controlSize(.small)
                    }
                }
            }
            .disabled(email.isEmpty || password.isEmpty || model.isWorking)
        }
    }

    // MARK: - Usage

    @ViewBuilder
    private var usageSection: some View {
        if let info = model.info {
            Section(L.accountUsage) {
                if let email = info.email.flatMap(SyncValidation.displayName) {
                    LabeledContent(L.setupEmail, value: email)
                }
                if let notes = info.noteCount {
                    LabeledContent(L.settingsNotes, value: "\(max(notes, 0))")
                }
                if let attachments = info.attachmentCount {
                    LabeledContent(L.accountAttachments, value: "\(max(attachments, 0))")
                }
                if let used = info.storageUsedBytes {
                    storageRow(used: max(used, 0), info: info)
                }
            }
        }
    }

    @ViewBuilder
    private func storageRow(used: Int64, info: AccountInfo) -> some View {
        let usedText = ByteCountFormatter.string(fromByteCount: used, countStyle: .file)
        if let quota = info.quotaBytes, let fraction = info.usedFraction {
            let quotaText = ByteCountFormatter.string(fromByteCount: quota, countStyle: .file)
            ProgressView(value: fraction) {
                LabeledContent(L.accountStorage, value: L.accountStorageOfQuota(usedText, quotaText))
            }
            .tint(fraction >= 0.9 ? .red : .accentColor)
        } else {
            LabeledContent(L.accountStorage, value: usedText)
        }
    }

    // MARK: - Inbox

    private var hasToken: Bool { model.info?.inbox?.hasToken ?? false }

    private var inboxSection: some View {
        Section {
            if let items = model.info?.inbox?.itemCount {
                LabeledContent(L.accountInboxItems, value: "\(max(items, 0))")
            }
            LabeledContent(L.accountInboxToken, value: hasToken ? L.accountInboxTokenSet : L.accountInboxTokenNotSet)

            if let token = model.disclosedToken {
                VStack(alignment: .leading, spacing: 8) {
                    Text(verbatim: token)
                        .font(.caption.monospaced())
                        .textSelection(.enabled)
                    Button {
                        copy(token)
                    } label: {
                        Label(
                            tokenCopied ? L.accountTokenCopied : L.accountCopyToken,
                            systemImage: tokenCopied ? "checkmark" : "doc.on.doc"
                        )
                    }
                    // Borderless, so inside a Form only the label is tappable, not the row.
                    .buttonStyle(.borderless)
                }
            }

            Button(hasToken ? L.accountReplaceToken : L.accountGenerateToken) {
                if hasToken {
                    showReplaceTokenConfirmation = true
                } else {
                    perform { await model.generateInboxToken(client: $0) }
                }
            }
            .disabled(model.isWorking)

            if hasToken {
                Button(L.accountRevokeToken, role: .destructive) {
                    showRevokeTokenConfirmation = true
                }
                .disabled(model.isWorking)
            }
        } header: {
            Text(L.accountInbox)
        } footer: {
            Text(model.disclosedToken == nil ? L.accountInboxHint : L.accountTokenShownOnce)
        }
    }

    /// Kept to this device and taken off the pasteboard after a minute, as it's
    /// a password in all but name.
    private func copy(_ token: String) {
        UIPasteboard.general.setItems(
            [[UTType.plainText.identifier: token]],
            options: [.localOnly: true, .expirationDate: Date().addingTimeInterval(60)]
        )
        tokenCopied = true
    }

    // MARK: - Deleting Notes

    private var deleteSection: some View {
        Section {
            Button(L.accountDeleteNotesTitle, role: .destructive) {
                showDeleteConfirmation = true
            }
            .disabled(model.isWorking)
        } footer: {
            Text(L.accountDeleteNotesHint)
        }
    }

    private func perform(_ action: @escaping @MainActor (SyncClient) async -> Void) {
        guard let client else {
            model.errorMessage = L.deleteAccountNoServer
            return
        }
        tokenCopied = false
        Task { @MainActor in await action(client) }
    }
}
