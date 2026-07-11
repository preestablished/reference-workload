//! Host-only, dependency-light benchmark lane for `refwork-emu`.

#![forbid(unsafe_code)]

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use refwork_emu::{Cartridge, Core, RegionBuffers, FB_BYTES};
use refwork_hash::{chain_update, frame_hash};
use refwork_script::{parse as parse_padlog, PadLog};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Input {
    Synthetic,
    Script(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    rom: PathBuf,
    case_name: String,
    warmup_frames: u64,
    measure_frames: u64,
    input: Input,
    perf_control: Option<PathBuf>,
    perf_ack: Option<PathBuf>,
}

fn usage() -> &'static str {
    "Usage: refwork-emu-bench --rom PATH --case NAME --warmup-frames N \
--measure-frames N (--synthetic-input | --script PATH) \
[--perf-control FIFO --perf-ack FIFO]"
}

fn parse_positive(value: &str, flag: &str, allow_zero: bool) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("{flag} requires a non-negative integer"))?;
    if !allow_zero && parsed == 0 {
        return Err(format!("{flag} must be greater than zero"));
    }
    Ok(parsed)
}

fn next_value(args: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut rom = None;
    let mut case_name = None;
    let mut warmup_frames = None;
    let mut measure_frames = None;
    let mut input = None;
    let mut perf_control = None;
    let mut perf_ack = None;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--rom" => rom = Some(PathBuf::from(next_value(args, &mut i, "--rom")?)),
            "--case" => case_name = Some(next_value(args, &mut i, "--case")?),
            "--warmup-frames" => {
                let value = next_value(args, &mut i, "--warmup-frames")?;
                warmup_frames = Some(parse_positive(&value, "--warmup-frames", true)?);
            }
            "--measure-frames" => {
                let value = next_value(args, &mut i, "--measure-frames")?;
                measure_frames = Some(parse_positive(&value, "--measure-frames", false)?);
            }
            "--synthetic-input" => {
                if input.replace(Input::Synthetic).is_some() {
                    return Err("input source may only be supplied once".to_owned());
                }
            }
            "--script" => {
                let value = next_value(args, &mut i, "--script")?;
                if input.replace(Input::Script(PathBuf::from(value))).is_some() {
                    return Err("input source may only be supplied once".to_owned());
                }
            }
            "--perf-control" => {
                perf_control = Some(PathBuf::from(next_value(args, &mut i, "--perf-control")?));
            }
            "--perf-ack" => {
                perf_ack = Some(PathBuf::from(next_value(args, &mut i, "--perf-ack")?));
            }
            "--help" | "-h" => return Err(usage().to_owned()),
            other => return Err(format!("unknown option {other:?}")),
        }
        i += 1;
    }

    let case_name = case_name.ok_or_else(|| "--case is required".to_owned())?;
    if case_name.is_empty()
        || !case_name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err("--case must contain only ASCII letters, digits, '.', '-' or '_'".to_owned());
    }

    if perf_control.is_some() != perf_ack.is_some() {
        return Err("--perf-control and --perf-ack must be supplied together".to_owned());
    }

    Ok(Options {
        rom: rom.ok_or_else(|| "--rom is required".to_owned())?,
        case_name,
        warmup_frames: warmup_frames.ok_or_else(|| "--warmup-frames is required".to_owned())?,
        measure_frames: measure_frames.ok_or_else(|| "--measure-frames is required".to_owned())?,
        input: input
            .ok_or_else(|| "one of --synthetic-input or --script is required".to_owned())?,
        perf_control,
        perf_ack,
    })
}

fn file_blake3(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("cannot open input: {e}"))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|e| format!("cannot read input: {e}"))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn synthetic_pad(frame: u64) -> u16 {
    (frame as u16).wrapping_mul(0x9E37) & 0x0FFF
}

fn pad_at(input: &InputData, frame: u64) -> u16 {
    match input {
        InputData::Synthetic => synthetic_pad(frame),
        InputData::Script(log) => log
            .frames
            .get(frame as usize)
            .or_else(|| log.frames.last())
            .copied()
            .unwrap_or(0),
    }
}

enum InputData {
    Synthetic,
    Script(PadLog),
}

struct PerfControl {
    writer: File,
    ack: BufReader<File>,
}

impl PerfControl {
    fn open(path: &Path, ack_path: &Path) -> Result<Self, String> {
        let writer = OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|e| format!("cannot open perf control FIFO: {e}"))?;
        let ack = OpenOptions::new()
            .read(true)
            .open(ack_path)
            .map_err(|e| format!("cannot open perf ack FIFO: {e}"))?;
        Ok(Self {
            writer,
            ack: BufReader::new(ack),
        })
    }

    fn command(&mut self, command: &str) -> Result<(), String> {
        self.writer
            .write_all(command.as_bytes())
            .and_then(|_| self.writer.flush())
            .map_err(|e| format!("cannot send perf control command: {e}"))?;
        let mut acknowledgement = String::new();
        self.ack
            .read_line(&mut acknowledgement)
            .map_err(|e| format!("cannot read perf acknowledgement: {e}"))?;
        let acknowledgement = acknowledgement.trim().trim_matches('\0');
        if acknowledgement != "ack" {
            return Err(format!(
                "unexpected perf acknowledgement: {:?}",
                acknowledgement
            ));
        }
        Ok(())
    }
}

fn make_core(rom: Vec<u8>) -> Result<Core, String> {
    let cart = Cartridge::from_rom(rom, None).map_err(|e| format!("invalid ROM: {e:?}"))?;
    let wram = Box::leak(Box::new([0u8; 0x20000]));
    Core::new(
        cart,
        RegionBuffers {
            wram,
            vram: None,
            sram: None,
        },
    )
    .map_err(|e| format!("cannot construct emulator: {e:?}"))
}

fn run(opts: &Options) -> Result<String, String> {
    let rom_hash = file_blake3(&opts.rom)?;
    let rom = fs::read(&opts.rom).map_err(|e| format!("cannot read ROM: {e}"))?;
    let (input, script_hash) = match &opts.input {
        Input::Synthetic => (InputData::Synthetic, None),
        Input::Script(path) => {
            let hash = file_blake3(path)?;
            let contents =
                fs::read_to_string(path).map_err(|e| format!("cannot read input script: {e}"))?;
            let log = parse_padlog(&contents).map_err(|e| format!("invalid input script: {e}"))?;
            (InputData::Script(log), Some(hash))
        }
    };
    let mut core = make_core(rom)?;
    let mut framebuffer = Box::new([0u8; FB_BYTES]);
    let mut chain = [0u8; 32];

    for frame in 0..opts.warmup_frames {
        core.run_one_frame(pad_at(&input, frame));
        if let Some(fault) = core.fault() {
            return Err(format!(
                "emulator fault during warmup at frame {frame}: {fault:?}"
            ));
        }
        core.blit_completed_frame(&mut framebuffer);
    }

    let mut control = opts
        .perf_control
        .as_deref()
        .zip(opts.perf_ack.as_deref())
        .map(|(control, ack)| PerfControl::open(control, ack))
        .transpose()?;
    let started = control.is_none().then(Instant::now);
    if let Some(control) = &mut control {
        control.command("enable\n")?;
    }
    for offset in 0..opts.measure_frames {
        let frame = opts
            .warmup_frames
            .checked_add(offset)
            .ok_or_else(|| "frame index overflow".to_owned())?;
        core.run_one_frame(pad_at(&input, frame));
        if let Some(fault) = core.fault() {
            return Err(format!(
                "emulator fault during measurement at frame {frame}: {fault:?}"
            ));
        }
        core.blit_completed_frame(&mut framebuffer);
    }
    if let Some(control) = &mut control {
        control.command("disable\n")?;
    }
    let elapsed_json = started
        .map(|started| started.elapsed().as_nanos().to_string())
        .unwrap_or_else(|| "null".to_owned());

    // Hash once, after the timed/counted window, so proof work is not assigned
    // to emulator frames. The established chain primitive still identifies the
    // final state without exposing memory or pixels.
    let final_hash = frame_hash(core.wram(), &framebuffer[..]);
    chain = chain_update(&chain, &final_hash);
    let script_json = script_hash
        .map(|hash| format!("\"{hash}\""))
        .unwrap_or_else(|| "null".to_owned());

    Ok(format!(
        "{{\"schema\":1,\"case\":\"{}\",\"rom_blake3\":\"{}\",\"script_blake3\":{},\"warmup_frames\":{},\"measure_frames\":{},\"elapsed_ns\":{},\"final_frame\":{},\"fault\":null,\"final_state_chain\":\"{}\"}}",
        opts.case_name,
        rom_hash,
        script_json,
        opts.warmup_frames,
        opts.measure_frames,
        elapsed_json,
        core.frame_counter(),
        blake3::Hash::from_bytes(chain).to_hex(),
    ))
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let opts = match parse_args(&args) {
        Ok(opts) => opts,
        Err(err) => {
            eprintln!("refwork-emu-bench: {err}");
            if err != usage() {
                eprintln!("{}", usage());
            }
            std::process::exit(2);
        }
    };
    match run(&opts) {
        Ok(record) => println!("{record}"),
        Err(err) => {
            eprintln!("refwork-emu-bench: {err}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn parses_synthetic_case() {
        let parsed = parse_args(&strings(&[
            "--rom",
            "game.rom",
            "--case",
            "synth-steady",
            "--warmup-frames",
            "600",
            "--measure-frames",
            "2000",
            "--synthetic-input",
        ]))
        .unwrap();
        assert_eq!(parsed.warmup_frames, 600);
        assert_eq!(parsed.measure_frames, 2000);
        assert_eq!(parsed.input, Input::Synthetic);
    }

    #[test]
    fn rejects_zero_measurement() {
        let error = parse_args(&strings(&[
            "--rom",
            "game.rom",
            "--case",
            "bad",
            "--warmup-frames",
            "0",
            "--measure-frames",
            "0",
            "--synthetic-input",
        ]))
        .unwrap_err();
        assert!(error.contains("greater than zero"));
    }

    #[test]
    fn rejects_private_case_name_characters() {
        let error = parse_args(&strings(&[
            "--rom",
            "game.rom",
            "--case",
            "private/path",
            "--warmup-frames",
            "0",
            "--measure-frames",
            "1",
            "--synthetic-input",
        ]))
        .unwrap_err();
        assert!(error.contains("--case"));
    }

    #[test]
    fn requires_perf_control_and_ack_together() {
        let error = parse_args(&strings(&[
            "--rom",
            "game.rom",
            "--case",
            "perf",
            "--warmup-frames",
            "0",
            "--measure-frames",
            "1",
            "--synthetic-input",
            "--perf-control",
            "control.fifo",
        ]))
        .unwrap_err();
        assert!(error.contains("must be supplied together"));
    }

    #[test]
    fn synthetic_schedule_matches_xtask_contract() {
        for frame in 0..10_000u64 {
            let expected = (frame as u16).wrapping_mul(0x9E37) & 0x0FFF;
            assert_eq!(synthetic_pad(frame), expected);
        }
    }
}
