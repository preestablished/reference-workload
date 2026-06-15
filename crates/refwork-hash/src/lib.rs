//! `refwork-hash` — shared frame-hash and hash-chain primitives.
//!
//! Centralises the two hashing operations that must be bit-identical across
//! every tool in the determinism suite (`xtask hash-chain`, `refwork-verify
//! play/double-run`).  Keeping the definitions here means xtask and
//! refwork-verify can never silently drift apart on the algorithm.
//!
//! **Algorithm** (from ARCHITECTURE.md §1 / xtask/src/hash_chain.rs):
//!
//! ```text
//! frame_hash  = blake3(wram ‖ fb)
//! chain_next  = blake3(chain ‖ frame_hash)
//! chain_start = [0u8; 32]
//! ```
//!
//! This crate is a host-side helper; it is deliberately **outside** the deny
//! scope that covers `refwork-emu`, `refwork-harness`, and
//! `refwork-protocol`.

#![forbid(unsafe_code)]

/// Compute the per-frame hash: `blake3(wram ‖ fb)`.
///
/// `wram` is the 128 KiB working RAM snapshot; `fb` is the completed
/// framebuffer (256×224 XRGB8888, `FB_BYTES` long).
pub fn frame_hash(wram: &[u8], fb: &[u8]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(wram);
    h.update(fb);
    h.finalize().into()
}

/// Advance the running hash chain: `blake3(chain ‖ frame_hash)`.
///
/// Initialise `chain` as `[0u8; 32]` before the first frame.
pub fn chain_update(chain: &[u8; 32], frame_hash: &[u8; 32]) -> [u8; 32] {
    let mut c = blake3::Hasher::new();
    c.update(chain);
    c.update(frame_hash);
    c.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke: deterministic — same inputs produce the same output across calls.
    #[test]
    fn frame_hash_is_stable() {
        let wram = vec![0x55u8; 0x20000];
        let fb = vec![0u8; 256 * 224 * 4];
        let h1 = frame_hash(&wram, &fb);
        let h2 = frame_hash(&wram, &fb);
        assert_eq!(h1, h2);
    }

    /// Chain over 8 frames of known bytes is bit-stable.
    #[test]
    fn chain_over_known_bytes_is_stable() {
        let wram = [0x5Au8; 0x20000];
        let fb = [0xA5u8; 256 * 224 * 4];
        let mut chain = [0u8; 32];
        for _ in 0..8 {
            let fh = frame_hash(&wram, &fb);
            chain = chain_update(&chain, &fh);
        }
        // Re-run from scratch — must match.
        let mut chain2 = [0u8; 32];
        for _ in 0..8 {
            let fh = frame_hash(&wram, &fb);
            chain2 = chain_update(&chain2, &fh);
        }
        assert_eq!(chain, chain2, "chain must be bit-stable across runs");
    }

    /// Different wram content produces a different frame hash.
    #[test]
    fn frame_hash_distinguishes_content() {
        let wram_a = vec![0x00u8; 0x20000];
        let wram_b = vec![0xFFu8; 0x20000];
        let fb = vec![0u8; 256 * 224 * 4];
        assert_ne!(frame_hash(&wram_a, &fb), frame_hash(&wram_b, &fb));
    }

    /// chain_update distinguishes different frame hashes.
    #[test]
    fn chain_update_distinguishes_frame_hashes() {
        let chain = [0u8; 32];
        let fh_a = [0x01u8; 32];
        let fh_b = [0x02u8; 32];
        assert_ne!(chain_update(&chain, &fh_a), chain_update(&chain, &fh_b));
    }
}
