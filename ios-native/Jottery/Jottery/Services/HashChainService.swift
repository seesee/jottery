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
    ///
    /// The server accepts a push when its hash appears in the chain. The chain
    /// is trimmed to a fixed length, so a note edited that many times between
    /// syncs would lose the server's hash and come back as a conflict with its
    /// own older text. `serverHash` is kept in place of the oldest entry. Only
    /// membership matters, so the chain needn't be contiguous.
    static func updateHashChain(currentHash: String, parentChain: [String], keeping serverHash: String? = nil) -> [String] {
        var chain = CryptoService.updateHashChain(currentHash: currentHash, parentChain: parentChain)
        if let serverHash, !chain.contains(serverHash), !chain.isEmpty {
            chain[chain.count - 1] = serverHash
        }
        return chain
    }

    /// Find common ancestor between two chains.
    static func findCommonAncestor(chainA: [String], chainB: [String]) -> String? {
        CryptoService.findCommonAncestor(chainA: chainA, chainB: chainB)
    }
}
