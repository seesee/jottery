import Foundation

/// Content hash computation for sync conflict detection.
enum HashChainService {

    /// Compute the content hash for a decrypted note's encrypted form.
    /// The note must already have been re-encrypted before calling this.
    static func computeContentHash(
        encryptedContent: String,
        encryptedTags: [String],
        attachmentIds: [String],
        pinned: Bool,
        archived: Bool,
        locked: Bool,
        syntaxLanguage: String?,
        wordWrap: Bool,
        showPreview: Bool,
        color: String?
    ) -> String {
        let fields = SyncableNoteFields(
            content: encryptedContent,
            tags: encryptedTags,
            attachmentIds: attachmentIds,
            pinned: pinned,
            archived: archived,
            locked: locked,
            syntaxLanguage: syntaxLanguage,
            wordWrap: wordWrap,
            showPreview: showPreview,
            color: color
        )
        return CryptoService.computeContentHash(for: fields)
    }

    /// Update hash chain by prepending new hash.
    static func updateHashChain(currentHash: String, parentChain: [String]) -> [String] {
        CryptoService.updateHashChain(currentHash: currentHash, parentChain: parentChain)
    }

    /// Find common ancestor between two chains.
    static func findCommonAncestor(chainA: [String], chainB: [String]) -> String? {
        CryptoService.findCommonAncestor(chainA: chainA, chainB: chainB)
    }
}
