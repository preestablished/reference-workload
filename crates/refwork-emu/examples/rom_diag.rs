//! Clean-room-safe ROM diagnostic (introspect feature only; compiled out of the
//! guest binary). Reads a ROM path from `REFWORK_DIAG_ROM`, runs N frames, and
//! prints a per-frame table of **booleans, counts, and hardware addresses**.
//!
//! It NEVER emits ROM bytes, framebuffer pixels, memory contents, APU/DMA
//! payload, or the header title — only functional hardware state, so it is safe
//! to run against an operator-private copyrighted ROM.
//!
//! Run:
//!   REFWORK_DIAG_ROM=/path/to/game.img REFWORK_DIAG_FRAMES=300 \
//!     cargo run -p refwork-emu --features introspect --example rom_diag

use refwork_emu::{Cartridge, Core, FrameFlags, RegionBuffers};

fn main() {
    let path = std::env::var("REFWORK_DIAG_ROM")
        .expect("set REFWORK_DIAG_ROM to the ROM path (no default)");
    let frames: u64 = std::env::var("REFWORK_DIAG_FRAMES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);

    let rom = std::fs::read(&path).expect("read ROM");
    let cart = Cartridge::from_rom(rom, None).expect("cartridge load");
    let wram: &'static mut [u8; 0x20000] = Box::leak(Box::new([0u8; 0x20000]));
    let regions = RegionBuffers {
        wram,
        vram: None,
        sram: None,
    };
    let mut core = Core::new(cart, regions).expect("core construction");

    println!(
        "{:>5} {:>6} {:>6} {:>6} {:>4} {:>9} {:>8} {:>7} {:>7} {:>6} {:>4} {:>4}",
        "frame", "fblank", "bright", "bgmode", "TM", "cgram_nz", "vram_nz", "oam_nz",
        "spc_pc", "inIPL", "nmi", "ajoy"
    );

    let mut last_fb = true;
    for _ in 1..=frames {
        let flags = core.run_one_frame(0);
        let d = core.diag_snapshot();
        let changed = d.force_blank != last_fb;
        if d.frame <= 3 || d.frame % 60 == 0 || changed || d.frame == frames {
            println!(
                "f={:<4} fblank={} main_pc={:#06x} spc_pc={:#06x} inIPL={} nmi={} \
                 rd[apu]={} wr[apu]={} wr_CC={} spc_port0_is_CC={} cgram_nz={}",
                d.frame, d.force_blank, d.main_pc, d.spc_pc, d.spc_in_ipl,
                d.nmi_enabled, d.rd_apu, d.wr_apu, d.wr_cc_port0, d.spc_port0_is_cc, d.cgram_nz
            );
        }
        last_fb = d.force_blank;
        if flags.contains(FrameFlags::FAULTED) {
            println!("FAULTED at frame {}", d.frame);
            break;
        }
    }
}
