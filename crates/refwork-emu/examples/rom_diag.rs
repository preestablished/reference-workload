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

use refwork_emu::{Cartridge, Core, FrameFlags, RegionBuffers, FB_BYTES};

fn hash64(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn nonzero_count(bytes: &[u8]) -> usize {
    bytes.iter().filter(|&&b| b != 0).count()
}

fn fmt_opt_u64(v: Option<u64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_else(|| "-".to_owned())
}

fn fmt_opt_pc(v: Option<u16>) -> String {
    v.map(|pc| format!("{pc:#06x}"))
        .unwrap_or_else(|| "-".to_owned())
}

fn fmt_recent_pcs(pcs: [u16; 16], pos: usize, steps: u64) -> String {
    let len = (steps as usize).min(pcs.len());
    if len == 0 {
        return "-".to_owned();
    }
    let start = pos.wrapping_sub(len);
    let mut out = String::new();
    for i in 0..len {
        if i != 0 {
            out.push(',');
        }
        let pc = pcs[(start + i) & (pcs.len() - 1)];
        out.push_str(&format!("{pc:#06x}"));
    }
    out
}

fn fmt_pc_prefix(pcs: [u16; 16], len: usize) -> String {
    let len = len.min(pcs.len());
    if len == 0 {
        return "-".to_owned();
    }
    let mut out = String::new();
    for (i, pc) in pcs.iter().take(len).enumerate() {
        if i != 0 {
            out.push(',');
        }
        out.push_str(&format!("{pc:#06x}"));
    }
    out
}

fn fmt_blocks(addrs: [Option<u16>; 8], bytes: [u64; 8]) -> String {
    let mut out = String::new();
    for (addr, count) in addrs.into_iter().zip(bytes) {
        let Some(addr) = addr else { continue };
        if !out.is_empty() {
            out.push(',');
        }
        out.push_str(&format!("{addr:#06x}:{count}"));
    }
    if out.is_empty() {
        "-".to_owned()
    } else {
        out
    }
}

fn fmt_io(counts: [u64; 16], pcs: [Option<u16>; 16]) -> String {
    let mut out = String::new();
    for (reg, count) in counts.into_iter().enumerate() {
        if count == 0 {
            continue;
        }
        if !out.is_empty() {
            out.push(',');
        }
        out.push_str(&format!("F{:x}@{}:{}", reg, fmt_opt_pc(pcs[reg]), count));
    }
    if out.is_empty() {
        "-".to_owned()
    } else {
        out
    }
}

fn main() {
    let path = std::env::var("REFWORK_DIAG_ROM")
        .expect("set REFWORK_DIAG_ROM to the ROM path (no default)");
    let frames: u64 = std::env::var("REFWORK_DIAG_FRAMES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let interval: u64 = std::env::var("REFWORK_DIAG_INTERVAL")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60)
        .max(1);

    let rom = std::fs::read(&path).expect("read ROM");
    let cart = Cartridge::from_rom(rom, None).expect("cartridge load");
    let wram: &'static mut [u8; 0x20000] = Box::leak(Box::new([0u8; 0x20000]));
    let regions = RegionBuffers {
        wram,
        vram: None,
        sram: None,
    };
    let mut core = Core::new(cart, regions).expect("core construction");
    let mut fb = [0u8; FB_BYTES];

    println!(
        "frame fblank bright bgmode tm ts tmw cgwsel cgadsub fixedColor cgram_nz vram_nz \
         oam_nz cgram_colors renderFinal renderMain renderMainColor clip math bg1opq bg2opq \
         bg3opq selBg1 selBg2 selBg3 selObj selBd \
         main_pc spc_pc inIPL nmi autojoy rd_apu wr_apu wr_CC postCCw postCCdelta \
         svc0 ccSvcPC load0 loadN jump blocks bytes blockMap spcWr0 spcWr1 spcWr2 spcWr3 \
         spcWrPC0 spcWrPC1 spcWrPC2 spcWrPC3 spcRd0 spcRd1 spcRd2 spcRd3 \
         spcRdPC0 spcRdPC1 spcRdPC2 spcRdPC3 spcIoW spcIoR spcSteps spcFirst spcRecent \
         spc_port0_is_CC fb_nz fb_hash"
    );

    let mut last_fb = true;
    for _ in 1..=frames {
        let flags = core.run_one_frame(0);
        let d = core.diag_snapshot();
        core.blit_completed_frame(&mut fb);
        let fb_hash = hash64(&fb);
        let changed = d.force_blank != last_fb;
        if d.frame <= 3 || d.frame.is_multiple_of(interval) || changed || d.frame == frames {
            println!(
                "f={} fblank={} bright={} bgmode={} tm={:#04x} ts={:#04x} tmw={:#04x} \
                 cgwsel={:#04x} cgadsub={:#04x} fixedColor={:#06x} cgram_nz={} vram_nz={} \
                 oam_nz={} cgram_colors={} renderFinal={} renderMain={} renderMainColor={} \
                 clip={} math={} bg1opq={} bg2opq={} bg3opq={} selBg1={} selBg2={} \
                 selBg3={} selObj={} selBd={} main_pc={:#06x} spc_pc={:#06x} inIPL={} \
                 nmi={} autojoy={} rd_apu={} wr_apu={} wr_CC={} postCCw={} \
                 postCCdelta={} svc0={} ccSvcPC={} load0={} loadN={} jump={} blocks={} bytes={} blockMap={} \
                 spcWr0={} spcWr1={} spcWr2={} spcWr3={} spcWrPC0={} spcWrPC1={} \
                 spcWrPC2={} spcWrPC3={} spcRd0={} spcRd1={} spcRd2={} spcRd3={} \
                 spcRdPC0={} spcRdPC1={} spcRdPC2={} spcRdPC3={} spcIoW={} spcIoR={} spcSteps={} \
                 spcFirst={} spcRecent={} spc_port0_is_CC={} fb_nz={} fb_hash={:016x}",
                d.frame,
                d.force_blank,
                d.brightness,
                d.bg_mode,
                d.main_screen,
                d.sub_screen,
                d.main_window,
                d.cgwsel,
                d.cgadsub,
                d.fixed_color,
                d.cgram_nz,
                d.vram_nz,
                d.oam_nz,
                d.cgram_distinct_colors,
                d.frame_final_nonzero,
                d.frame_main_nonbackdrop,
                d.frame_main_color_nonzero,
                d.frame_main_clipped,
                d.frame_math_applied,
                d.frame_mode1_bg_opaque[0],
                d.frame_mode1_bg_opaque[1],
                d.frame_mode1_bg_opaque[2],
                d.frame_mode1_selected[0],
                d.frame_mode1_selected[1],
                d.frame_mode1_selected[2],
                d.frame_mode1_selected[3],
                d.frame_mode1_selected[4],
                d.main_pc,
                d.spc_pc,
                d.spc_in_ipl,
                d.nmi_enabled,
                d.autojoy_enabled,
                d.rd_apu,
                d.wr_apu,
                d.wr_cc_port0,
                d.post_cc_port0_writes,
                fmt_opt_u64(d.first_post_cc_port0_delta_mclk),
                d.apu_port0_service_count,
                fmt_opt_pc(d.first_cc_service_spc_pc),
                fmt_opt_pc(d.ipl_first_load_addr),
                fmt_opt_pc(d.ipl_last_load_addr),
                fmt_opt_pc(d.ipl_jump_addr),
                d.ipl_block_count,
                d.ipl_bytes_stored,
                fmt_blocks(d.ipl_block_addrs, d.ipl_block_bytes),
                d.spc_port_writes[0],
                d.spc_port_writes[1],
                d.spc_port_writes[2],
                d.spc_port_writes[3],
                fmt_opt_pc(d.spc_port_last_write_pc[0]),
                fmt_opt_pc(d.spc_port_last_write_pc[1]),
                fmt_opt_pc(d.spc_port_last_write_pc[2]),
                fmt_opt_pc(d.spc_port_last_write_pc[3]),
                d.spc_port_reads[0],
                d.spc_port_reads[1],
                d.spc_port_reads[2],
                d.spc_port_reads[3],
                fmt_opt_pc(d.spc_port_last_read_pc[0]),
                fmt_opt_pc(d.spc_port_last_read_pc[1]),
                fmt_opt_pc(d.spc_port_last_read_pc[2]),
                fmt_opt_pc(d.spc_port_last_read_pc[3]),
                fmt_io(d.spc_io_writes, d.spc_io_last_write_pc),
                fmt_io(d.spc_io_reads, d.spc_io_last_read_pc),
                d.spc_step_count,
                fmt_pc_prefix(d.spc_first_pcs, d.spc_first_pc_count),
                fmt_recent_pcs(d.spc_recent_pcs, d.spc_recent_pc_pos, d.spc_step_count),
                d.spc_port0_is_cc,
                nonzero_count(&fb),
                fb_hash
            );
        }
        last_fb = d.force_blank;
        if flags.contains(FrameFlags::FAULTED) {
            println!("FAULTED at frame {}", d.frame);
            break;
        }
    }
}
