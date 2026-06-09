//! CGRAM palette helper: BGR555 → XRGB8888 with brightness scaling.

/// Expand a BGR555 15-bit color to XRGB8888.
/// Component expansion: c8 = (c5 << 3) | (c5 >> 2).
#[inline]
pub fn bgr555_to_xrgb8888(bgr555: u16, brightness: u8) -> u32 {
    let r5 = ((bgr555) & 0x1F) as u32;
    let g5 = ((bgr555 >> 5) & 0x1F) as u32;
    let b5 = ((bgr555 >> 10) & 0x1F) as u32;

    let r8 = (r5 << 3) | (r5 >> 2);
    let g8 = (g5 << 3) | (g5 >> 2);
    let b8 = (b5 << 3) | (b5 >> 2);

    // Apply brightness: out = component * (brightness + 1) / 16
    let brt = (brightness as u32) + 1;
    let r = r8 * brt / 16;
    let g = g8 * brt / 16;
    let b = b8 * brt / 16;

    (r << 16) | (g << 8) | b
}

/// Read a BGR555 color from CGRAM at index `idx` (0-255).
#[inline]
pub fn read_cgram_color(cgram: &[u8; 512], idx: usize) -> u16 {
    let lo = cgram[idx * 2] as u16;
    let hi = cgram[idx * 2 + 1] as u16;
    lo | (hi << 8)
}
