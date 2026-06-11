//! Round-trip integration test for `ramdiff`.
//!
//! Builds the synthetic ROM (via `cargo run -p xtask -- build-rom`), runs
//! `ramdiff record` with two marks, then uses the filter and emit APIs
//! to confirm the frame counter's WRAM address is found.
//!
//! # Synthetic ROM WRAM layout (xtask/asm/synth.s65)
//!
//! - `$0010/$0011` (offset 16, u16le): frame counter, incremented by the NMI
//!   handler on every vblank. This is the address the round-trip test rediscovers.
//!   After N frames: WRAM[0x0010] = N & 0xFF, WRAM[0x0011] = (N >> 8) & 0xFF.
//!
//! The test:
//! 1. Records dump-a after 2 frames (counter = 1).
//! 2. Records dump-b after 12 frames (counter = 11).
//! 3. Searches: changed between a and b.
//! 4. Searches: increased (frame counter goes 1→11).
//! 5. Searches: --value 11 --in b.
//! 6. Verifies offset 0x0010 survives.
//! 7. Emits a feature entry to a scratch map; validates it passes.

use ramdiff::emit::{run_emit, EmitOpts};
use ramdiff::filter::{run_search, FilterOp};
use ramdiff::record::get_pad;
use ramdiff::session::{CandidateSet, DumpMeta, SearchWidth, Session, WRAM_SIZE};
use refwork_emu::{Cartridge, Core, RegionBuffers, WRAM_INIT_BYTE};
use refwork_featuremap::{parse_feature_map, Discretize, FeatureType, Semantics, Stability};
use refwork_script::PadLog;

/// The frame counter's WRAM offset in the synthetic ROM (u16le).
/// See xtask/asm/synth.s65: `FRAME_CTR = $0010`.
const FRAME_CTR_OFFSET: u32 = 0x0010;

/// Minimal scratch feature-map YAML.
const SCRATCH_MAP_YAML: &str = r#"schema_version: 1
kind: feature-map
meta:
  name: ramdiff-test
  workload: test-workload
  game_revision: "test-rev"
  version: 1
regions:
  - name: wram
    size: 131072
features: []
"#;

struct TempDir {
    path: std::path::PathBuf,
}

impl TempDir {
    fn new(suffix: &str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static CTR: AtomicU64 = AtomicU64::new(0);
        let n = CTR.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("ramdiff_rt_{}_{}{}", std::process::id(), n, suffix));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TempDir { path: dir }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Build the synthetic ROM in a temp directory via xtask and return its path.
fn build_synth_rom() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new("-rom");
    let rom_path = tmp.path.join("synth.rom");

    // Use xtask to build the ROM.
    let status = std::process::Command::new("cargo")
        .args(["run", "-p", "xtask", "--", "build-rom", "--out"])
        .arg(&rom_path)
        .status()
        .expect("cargo run xtask build-rom failed to execute");

    assert!(status.success(), "xtask build-rom exited with error");
    assert!(
        rom_path.exists(),
        "synth ROM was not created at {:?}",
        rom_path
    );

    (tmp, rom_path)
}

/// Run the core for `frames` frames starting from scratch, then take a WRAM snapshot.
fn run_core_dump(rom_path: &std::path::Path, frames: u64) -> Box<[u8; WRAM_SIZE]> {
    let rom_bytes = std::fs::read(rom_path).unwrap();
    let cart = Cartridge::from_rom(rom_bytes, None).expect("bad synth ROM");

    let wram: &'static mut [u8; 0x20000] = Box::leak(Box::new([WRAM_INIT_BYTE; 0x20000]));
    let regions = RegionBuffers {
        wram,
        vram: None,
        sram: None,
    };
    let mut core = Core::new(cart, regions).expect("core construction failed");

    let empty_log = PadLog::default();
    for f in 0..frames {
        let pad = get_pad(&empty_log, f);
        core.run_one_frame(pad);
        if let Some(fault) = core.fault() {
            panic!("fault at frame {}: {:?}", f, fault);
        }
    }

    // Copy WRAM out.
    let mut out = Box::new([0u8; WRAM_SIZE]);
    out.copy_from_slice(core.wram());
    out
}

#[test]
fn round_trip_finds_frame_counter() {
    let (_rom_tmp, rom_path) = build_synth_rom();

    let session_tmp = TempDir::new("-session");
    let session_dir = session_tmp.path.clone();

    // The reset handler spans the APU IPL upload and finishes during
    // frame 1: FRAME_CTR is written to 0 mid-boot, then the NMI increments
    // it once per completed frame, so counter == frames - 1 from frame 2 on.
    // Dump-a: after 2 run_one_frame calls — counter = 1.
    let wram_a: Box<[u8; WRAM_SIZE]> = run_core_dump(&rom_path, 2);
    // Dump-b: after 12 run_one_frame calls — counter = 11.
    let wram_b: Box<[u8; WRAM_SIZE]> = run_core_dump(&rom_path, 12);

    // Verify we captured the right frame counter values.
    let fc_a = (wram_a[0x0010] as u16) | ((wram_a[0x0011] as u16) << 8);
    let fc_b = (wram_b[0x0010] as u16) | ((wram_b[0x0011] as u16) << 8);
    assert_eq!(
        fc_a, 1,
        "frame counter after 2 frames should be 1, got {}",
        fc_a
    );
    assert_eq!(
        fc_b, 11,
        "frame counter after 12 frames should be 11, got {}",
        fc_b
    );

    // Set up session with both dumps.
    let mut session = Session::new(&session_dir);
    session.candidates = CandidateSet::full(WRAM_SIZE, SearchWidth::U16le);

    // Write dump files.
    let file_a = "dump-a.bin";
    let file_b = "dump-b.bin";
    std::fs::write(session_dir.join(file_a), wram_a.as_ref()).unwrap();
    std::fs::write(session_dir.join(file_b), wram_b.as_ref()).unwrap();

    session.add_dump(DumpMeta {
        label: "a".to_owned(),
        frame: 1,
        file: file_a.to_owned(),
        region: "wram".to_owned(),
    });
    session.add_dump(DumpMeta {
        label: "b".to_owned(),
        frame: 11,
        file: file_b.to_owned(),
        region: "wram".to_owned(),
    });
    session.save().unwrap();

    // Step 1: filter --changed a b
    run_search(
        &session_dir,
        &[FilterOp::Changed {
            a: "a".to_owned(),
            b: "b".to_owned(),
        }],
    )
    .unwrap();

    let s1 = Session::load(&session_dir).unwrap();
    assert!(
        s1.candidates.offsets.contains(&FRAME_CTR_OFFSET),
        "frame counter offset 0x{:04X} not in candidates after --changed; total={}",
        FRAME_CTR_OFFSET,
        s1.candidates.offsets.len()
    );

    // Step 2: filter --inc a b
    run_search(
        &session_dir,
        &[FilterOp::Increased {
            a: "a".to_owned(),
            b: "b".to_owned(),
        }],
    )
    .unwrap();

    let s2 = Session::load(&session_dir).unwrap();
    assert!(
        s2.candidates.offsets.contains(&FRAME_CTR_OFFSET),
        "frame counter offset 0x{:04X} not in candidates after --inc; total={}",
        FRAME_CTR_OFFSET,
        s2.candidates.offsets.len()
    );

    // Step 3: filter --value 11 --in b
    run_search(
        &session_dir,
        &[FilterOp::ValueIn {
            value: 11,
            label: "b".to_owned(),
        }],
    )
    .unwrap();

    let s3 = Session::load(&session_dir).unwrap();
    assert!(
        s3.candidates.offsets.contains(&FRAME_CTR_OFFSET),
        "frame counter offset 0x{:04X} not in candidates after --value 11; total={}",
        FRAME_CTR_OFFSET,
        s3.candidates.offsets.len()
    );

    // Step 4: emit to a scratch map.
    let scratch_tmp = TempDir::new("-map");
    let scratch_map = scratch_tmp.path.join("scratch.yaml");
    std::fs::write(&scratch_map, SCRATCH_MAP_YAML).unwrap();

    run_emit(&EmitOpts {
        map: scratch_map.clone(),
        name: "frame_counter".to_owned(),
        offset: FRAME_CTR_OFFSET,
        feature_type: FeatureType::U16le,
        stability: Stability::Stable,
        discretize: Some(Discretize::Identity),
        region: "wram".to_owned(),
        description: Some("Synthetic ROM frame counter (WRAM 0x0010, u16le)".to_owned()),
        semantics: Semantics::Counter,
        force: false,
    })
    .unwrap();

    // Verify the map is valid and contains the entry.
    let yaml = std::fs::read_to_string(&scratch_map).unwrap();
    let (map, errors) = parse_feature_map(&yaml).unwrap();
    assert!(
        errors.is_empty(),
        "feature-map validation errors after emit: {:?}",
        errors
    );
    let feat = map.features.iter().find(|f| f.name == "frame_counter");
    assert!(
        feat.is_some(),
        "frame_counter feature not found in emitted map"
    );
    assert_eq!(feat.unwrap().offset.0, FRAME_CTR_OFFSET as i64);

    println!(
        "Round-trip OK: frame counter at 0x{:04X}, {} candidates after full filter chain",
        FRAME_CTR_OFFSET,
        s3.candidates.offsets.len()
    );
}
