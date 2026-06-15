//! Decode a feature value from WRAM using the `refwork-featuremap` types.
//!
//! All reads are pure — no side effects on the emulator state.

use refwork_featuremap::{Feature, FeatureMap, FeatureType};

/// Decoded feature value, represented as a signed 64-bit integer.
///
/// For `bytes` type features the value is the XOR-fold of all bytes (used
/// only for change-detection; the raw bytes are not surfaced here).
pub type FeatureValue = i64;

/// Resolve a feature by name; returns `None` if the name is unknown.
pub fn find_feature<'a>(map: &'a FeatureMap, name: &str) -> Option<&'a Feature> {
    map.features.iter().find(|f| f.name == name)
}

/// Check whether a feature's `valid_when` guard passes at the current WRAM
/// state.  Features without a guard are always valid.
pub fn is_valid(map: &FeatureMap, feat: &Feature, wram: &[u8; 0x20000]) -> bool {
    let vw = match &feat.valid_when {
        Some(v) => v,
        None => return true,
    };
    let guard_feat = match find_feature(map, &vw.feature) {
        Some(f) => f,
        None => return false, // broken map; treat as invalid
    };
    let gval = read_feature_value(guard_feat, wram);
    let threshold = vw.value.0;
    use refwork_featuremap::CompareOp;
    match vw.op {
        CompareOp::Eq => gval == threshold,
        CompareOp::Ne => gval != threshold,
        CompareOp::Lt => gval < threshold,
        CompareOp::Le => gval <= threshold,
        CompareOp::Gt => gval > threshold,
        CompareOp::Ge => gval >= threshold,
    }
}

/// Read the current value of `feat` from `wram`.
///
/// The `wram` slice is 128 KiB (indices 0x00000–0x1FFFF). Feature `offset`
/// values in a feature-map are relative to the declared region; this function
/// assumes the region is `wram` with base 0 (the only region used by the
/// synthetic ROM and demo-game maps).
pub fn read_feature_value(feat: &Feature, wram: &[u8; 0x20000]) -> FeatureValue {
    let off = feat.offset.0 as usize;
    match feat.feature_type {
        FeatureType::U8 | FeatureType::Bitflags8 | FeatureType::Bcd8 => {
            wram.get(off).copied().unwrap_or(0) as i64
        }
        FeatureType::I8 => wram.get(off).copied().unwrap_or(0) as i8 as i64,
        FeatureType::U16le | FeatureType::Bitflags16le | FeatureType::Bcd16le => {
            let lo = wram.get(off).copied().unwrap_or(0) as u16;
            let hi = wram.get(off + 1).copied().unwrap_or(0) as u16;
            (lo | (hi << 8)) as i64
        }
        FeatureType::U16be => {
            let hi = wram.get(off).copied().unwrap_or(0) as u16;
            let lo = wram.get(off + 1).copied().unwrap_or(0) as u16;
            (lo | (hi << 8)) as i64
        }
        FeatureType::I16le => {
            let lo = wram.get(off).copied().unwrap_or(0) as u16;
            let hi = wram.get(off + 1).copied().unwrap_or(0) as u16;
            (lo | (hi << 8)) as i16 as i64
        }
        FeatureType::I16be => {
            let hi = wram.get(off).copied().unwrap_or(0) as u16;
            let lo = wram.get(off + 1).copied().unwrap_or(0) as u16;
            (lo | (hi << 8)) as i16 as i64
        }
        FeatureType::U32le | FeatureType::Bitflags32le => {
            let b0 = wram.get(off).copied().unwrap_or(0) as u32;
            let b1 = wram.get(off + 1).copied().unwrap_or(0) as u32;
            let b2 = wram.get(off + 2).copied().unwrap_or(0) as u32;
            let b3 = wram.get(off + 3).copied().unwrap_or(0) as u32;
            (b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)) as i64
        }
        FeatureType::U32be => {
            let b0 = wram.get(off).copied().unwrap_or(0) as u32;
            let b1 = wram.get(off + 1).copied().unwrap_or(0) as u32;
            let b2 = wram.get(off + 2).copied().unwrap_or(0) as u32;
            let b3 = wram.get(off + 3).copied().unwrap_or(0) as u32;
            (b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)).swap_bytes() as i32 as i64
        }
        FeatureType::I32le => {
            let b0 = wram.get(off).copied().unwrap_or(0) as u32;
            let b1 = wram.get(off + 1).copied().unwrap_or(0) as u32;
            let b2 = wram.get(off + 2).copied().unwrap_or(0) as u32;
            let b3 = wram.get(off + 3).copied().unwrap_or(0) as u32;
            (b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)) as i32 as i64
        }
        FeatureType::I32be => {
            let b0 = wram.get(off).copied().unwrap_or(0) as u32;
            let b1 = wram.get(off + 1).copied().unwrap_or(0) as u32;
            let b2 = wram.get(off + 2).copied().unwrap_or(0) as u32;
            let b3 = wram.get(off + 3).copied().unwrap_or(0) as u32;
            (b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)).swap_bytes() as i32 as i64
        }
        FeatureType::Bytes => {
            // XOR-fold all bytes in the width range (change detection only).
            let width = feat.width.unwrap_or(1) as usize;
            let mut acc: u64 = 0;
            for i in 0..width {
                acc ^= wram.get(off + i).copied().unwrap_or(0) as u64;
            }
            acc as i64
        }
    }
}
