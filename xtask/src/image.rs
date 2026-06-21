//! `xtask image`: build and validate package-04 workload-image handoff artifacts.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_yaml::{Mapping, Value};

const WORKLOAD_NAME: &str = "refwork-demo";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const FPS_NUM: u64 = 21_477_272;
const FPS_DEN: u64 = 357_366;

const REQUIRED_REGIONS: &[RegionSpec] = &[
    RegionSpec {
        name: "wram",
        size: 131_072,
        format: None,
        layout_version: 1,
    },
    RegionSpec {
        name: "framebuffer",
        size: 229_376,
        format: Some("xrgb8888-256x224-stride1024"),
        layout_version: 1,
    },
    RegionSpec {
        name: "meta",
        size: 4_096,
        format: None,
        layout_version: 1,
    },
];

const PAD_BUTTONS: &[(&str, u64)] = &[
    ("A", 0),
    ("B", 1),
    ("X", 2),
    ("Y", 3),
    ("L", 4),
    ("R", 5),
    ("Up", 6),
    ("Down", 7),
    ("Left", 8),
    ("Right", 9),
    ("Start", 10),
    ("Select", 11),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RegionSpec {
    name: &'static str,
    size: u64,
    format: Option<&'static str>,
    layout_version: u64,
}

#[derive(Debug)]
pub enum ImageError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    CommandFailed {
        program: String,
        status: String,
    },
    MissingInput(String),
    InvalidInput(String),
    Yaml(serde_yaml::Error),
    Validation(Vec<String>),
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageError::Io { path, source } => write!(f, "{}: {}", path.display(), source),
            ImageError::CommandFailed { program, status } => {
                write!(f, "{program} failed with {status}")
            }
            ImageError::MissingInput(msg) | ImageError::InvalidInput(msg) => f.write_str(msg),
            ImageError::Yaml(err) => write!(f, "yaml parse failed: {err}"),
            ImageError::Validation(errors) => {
                writeln!(f, "validation failed with {} issue(s):", errors.len())?;
                for err in errors {
                    writeln!(f, "  - {err}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ImageError {}

pub fn build_image(workspace_root: &Path, agent_bin: &Path) -> Result<PathBuf, ImageError> {
    if !agent_bin.is_file() {
        return Err(ImageError::MissingInput(format!(
            "agent binary does not exist: {}",
            agent_bin.display()
        )));
    }

    let image_dir = workspace_root.join("image");
    for rel in [
        "kernel.lock",
        "builder.lock",
        "guest-sdk.lock",
        "boot.toml",
        "harness.toml",
        "expected-regions.toml",
    ] {
        let path = image_dir.join(rel);
        if !path.is_file() {
            return Err(ImageError::MissingInput(format!(
                "missing image input {}",
                path.display()
            )));
        }
    }

    build_static_harness(workspace_root)?;

    let out_dir = workspace_root
        .join("dist")
        .join(format!("workload-image-{VERSION}"));
    if out_dir.exists() {
        remove_dir_all(&out_dir)?;
    }
    create_dir_all(&out_dir)?;

    let boot = read_to_string(&image_dir.join("boot.toml"))?;
    let harness = read_to_string(&image_dir.join("harness.toml"))?;
    let expected_regions = read_to_string(&image_dir.join("expected-regions.toml"))?;
    let kernel_lock = read_to_string(&image_dir.join("kernel.lock"))?;
    let guest_sdk_lock = read_to_string(&image_dir.join("guest-sdk.lock"))?;
    let kernel_payload = quoted_value(&kernel_lock, "placeholder_payload")?;
    let guest_sdk_rev = quoted_value(&guest_sdk_lock, "rev")?;

    write(out_dir.join("boot.toml"), boot.as_bytes())?;
    write(out_dir.join("harness.toml"), harness.as_bytes())?;
    write(
        out_dir.join("expected-regions.toml"),
        expected_regions.as_bytes(),
    )?;
    write(out_dir.join("bzImage"), kernel_payload.as_bytes())?;

    let harness_bin = workspace_root
        .join("target")
        .join("x86_64-unknown-linux-musl")
        .join("release")
        .join("refwork-harness");
    if !harness_bin.is_file() {
        return Err(ImageError::MissingInput(format!(
            "static harness build did not produce {}",
            harness_bin.display()
        )));
    }

    let work_dir = workspace_root.join("target").join("image-work");
    if work_dir.exists() {
        remove_dir_all(&work_dir)?;
    }
    create_dir_all(&work_dir)?;

    let raw_cpio = work_dir.join("initramfs.cpio");
    write_newc_initramfs(
        &raw_cpio,
        agent_bin,
        &harness_bin,
        boot.as_bytes(),
        harness.as_bytes(),
        expected_regions.as_bytes(),
    )?;
    compress_zstd(&raw_cpio, &out_dir.join("initramfs.cpio.zst"))?;

    let git_rev = git_rev(workspace_root)?;
    let kernel_hash = blake3_file(&out_dir.join("bzImage"))?;
    let initramfs_hash = blake3_file(&out_dir.join("initramfs.cpio.zst"))?;
    write_workload_manifest(
        &out_dir.join("workload-image.yaml"),
        &git_rev,
        &guest_sdk_rev,
        &kernel_hash,
        &initramfs_hash,
    )?;
    write_unstamped_sidecar(&out_dir.join("determinism.unstamped.yaml"), &git_rev)?;
    write_dist_readme(&out_dir.join("README.md"))?;

    validate_manifest(&out_dir.join("workload-image.yaml"))?;
    Ok(out_dir)
}

pub fn validate_manifest(manifest: &Path) -> Result<(), ImageError> {
    let content = read_to_string(manifest)?;
    let yaml: Value = serde_yaml::from_str(&content).map_err(ImageError::Yaml)?;
    let base = manifest.parent().ok_or_else(|| {
        ImageError::InvalidInput(format!("manifest has no parent: {}", manifest.display()))
    })?;

    let mut errors = Vec::new();
    let root = mapping(&yaml, "root", &mut errors);

    if let Some(root) = root {
        expect_u64(root, "schema_version", 1, "schema_version", &mut errors);
        expect_string(root, "kind", "workload-image", "kind", &mut errors);
    }
    if let Some(meta) = child_map(root, "meta", "meta", &mut errors) {
        expect_string(meta, "name", WORKLOAD_NAME, "meta.name", &mut errors);
    }

    validate_artifacts(base, root, &mut errors);
    validate_boot(root, &mut errors);
    validate_machine(root, &mut errors);
    validate_regions(root, &mut errors);
    validate_fps(root, &mut errors);
    validate_pad_layout(root, &mut errors);
    validate_defaults(root, &mut errors);
    validate_handoff_files(base, &mut errors);
    validate_no_game_content(base, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ImageError::Validation(errors))
    }
}

fn build_static_harness(workspace_root: &Path) -> Result<(), ImageError> {
    let status = Command::new("cargo")
        .arg("build")
        .arg("--locked")
        .arg("--release")
        .arg("--target")
        .arg("x86_64-unknown-linux-musl")
        .arg("-p")
        .arg("refwork-harness")
        .current_dir(workspace_root)
        .status()
        .map_err(|source| ImageError::Io {
            path: PathBuf::from("cargo"),
            source,
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(ImageError::CommandFailed {
            program: "cargo build --target x86_64-unknown-linux-musl -p refwork-harness".into(),
            status: status.to_string(),
        })
    }
}

fn write_newc_initramfs(
    out: &Path,
    agent_bin: &Path,
    harness_bin: &Path,
    boot_toml: &[u8],
    harness_toml: &[u8],
    expected_regions: &[u8],
) -> Result<(), ImageError> {
    let agent = read(agent_bin)?;
    let harness = read(harness_bin)?;
    let entries = vec![
        CpioEntry::dir("."),
        CpioEntry::dir("dev"),
        CpioEntry::dir("etc"),
        CpioEntry::dir("etc/detguest"),
        CpioEntry::dir("etc/refwork"),
        CpioEntry::dir("sbin"),
        CpioEntry::dir("usr"),
        CpioEntry::dir("usr/bin"),
        CpioEntry::file("etc/detguest/boot.toml", 0o100644, boot_toml.to_vec()),
        CpioEntry::file(
            "etc/detguest/expected-regions.toml",
            0o100644,
            expected_regions.to_vec(),
        ),
        CpioEntry::file("etc/refwork/harness.toml", 0o100644, harness_toml.to_vec()),
        CpioEntry::file("init", 0o100755, init_script()),
        CpioEntry::file("sbin/detguest-agent", 0o100755, agent),
        CpioEntry::file("usr/bin/refwork-harness", 0o100755, harness),
    ];
    write(out, &newc_archive(&entries))
}

fn init_script() -> Vec<u8> {
    b"#!/bin/sh\nmount -t devtmpfs devtmpfs /dev 2>/dev/null || true\nmount -t proc proc /proc 2>/dev/null || true\nexec /sbin/detguest-agent /etc/detguest/boot.toml\n".to_vec()
}

fn compress_zstd(input: &Path, output: &Path) -> Result<(), ImageError> {
    let status = Command::new("zstd")
        .arg("-q")
        .arg("--no-progress")
        .arg("-19")
        .arg("-f")
        .arg("-o")
        .arg(output)
        .arg(input)
        .status()
        .map_err(|source| ImageError::Io {
            path: PathBuf::from("zstd"),
            source,
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(ImageError::CommandFailed {
            program: "zstd".into(),
            status: status.to_string(),
        })
    }
}

struct CpioEntry {
    path: &'static str,
    mode: u32,
    data: Vec<u8>,
}

impl CpioEntry {
    fn dir(path: &'static str) -> Self {
        Self {
            path,
            mode: 0o040755,
            data: Vec::new(),
        }
    }

    fn file(path: &'static str, mode: u32, data: Vec<u8>) -> Self {
        Self { path, mode, data }
    }
}

fn newc_archive(entries: &[CpioEntry]) -> Vec<u8> {
    let mut out = Vec::new();
    for (idx, entry) in entries.iter().enumerate() {
        write_newc_entry(
            &mut out,
            idx as u32 + 1,
            entry.path,
            entry.mode,
            &entry.data,
        );
    }
    write_newc_entry(&mut out, entries.len() as u32 + 1, "TRAILER!!!", 0, &[]);
    out
}

fn write_newc_entry(out: &mut Vec<u8>, ino: u32, path: &str, mode: u32, data: &[u8]) {
    let namesize = path.len() as u32 + 1;
    let nlink = if mode & 0o170000 == 0o040000 { 2 } else { 1 };
    let fields = [
        ino,
        mode,
        0,
        0,
        nlink,
        0,
        data.len() as u32,
        0,
        0,
        0,
        0,
        namesize,
        0,
    ];
    out.extend_from_slice(b"070701");
    for field in fields {
        out.extend_from_slice(format!("{field:08x}").as_bytes());
    }
    out.extend_from_slice(path.as_bytes());
    out.push(0);
    pad4(out);
    out.extend_from_slice(data);
    pad4(out);
}

fn pad4(bytes: &mut Vec<u8>) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
}

fn write_workload_manifest(
    path: &Path,
    git_rev: &str,
    guest_sdk_rev: &str,
    kernel_hash: &str,
    initramfs_hash: &str,
) -> Result<(), ImageError> {
    let manifest = format!(
        r#"schema_version: 1
kind: workload-image
meta:
  name: {WORKLOAD_NAME}
  version: "{VERSION}"
  built_from:
    repo: reference-workload
    git_rev: "{git_rev}"
    guest_sdk_rev: "{guest_sdk_rev}"
artifacts:
  kernel:
    file: bzImage
    blake3: "{kernel_hash}"
  initramfs:
    file: initramfs.cpio.zst
    blake3: "{initramfs_hash}"
boot:
  protocol: linux-direct
  cmdline: "quiet"
machine:
  vcpus: 1
  mem_mib: 128
  devices:
    - {{ kind: virtio-blk, role: game-image, readonly: true, required: true }}
    - {{ kind: detguest-channel, required: true }}
    - {{ kind: pv-pad, required: true }}
regions:
  - {{ name: wram, size: 131072 }}
  - {{ name: framebuffer, size: 229376, format: xrgb8888-256x224-stride1024 }}
  - {{ name: meta, size: 4096 }}
  - {{ name: vram, size: 65536, optional: true }}
  - {{ name: sram, size: 0, optional: true, note: "cart-dependent" }}
fps:
  num: {FPS_NUM}
  den: {FPS_DEN}
pad_layout:
  layout_version: 1
  buttons:
    - {{ name: A, bit: 0 }}
    - {{ name: B, bit: 1 }}
    - {{ name: X, bit: 2 }}
    - {{ name: Y, bit: 3 }}
    - {{ name: L, bit: 4 }}
    - {{ name: R, bit: 5 }}
    - {{ name: Up, bit: 6 }}
    - {{ name: Down, bit: 7 }}
    - {{ name: Left, bit: 8 }}
    - {{ name: Right, bit: 9 }}
    - {{ name: Start, bit: 10 }}
    - {{ name: Select, bit: 11 }}
  reserved_bits: [12, 13, 14, 15]
defaults:
  feature_map: demo-game@latest
  scoring_program: demo-game@latest
determinism:
  suite: refwork-verify
"#
    );
    write(path, manifest.as_bytes())
}

fn write_unstamped_sidecar(path: &Path, git_rev: &str) -> Result<(), ImageError> {
    let content = format!(
        r#"schema_version: 1
kind: determinism-unstamped
workload_image: {WORKLOAD_NAME}@{VERSION}
git_rev: "{git_rev}"
reason: "package 06 owns the full determinism green stamp"
"#
    );
    write(path, content.as_bytes())
}

fn write_dist_readme(path: &Path) -> Result<(), ImageError> {
    let content = format!(
        r#"# Workload Image {VERSION}

Generated by `cargo run --locked -p xtask -- image build --agent-bin <path>`.

This directory is an image handoff bundle for `refwork-demo`. It contains a
documented `bzImage` placeholder from `image/kernel.lock`, a deterministic
`newc` initramfs compressed as `initramfs.cpio.zst`, `workload-image.yaml`,
`boot.toml`, `harness.toml`, and guest-sdk expected-region handoff data.

No game ROM, SRAM, framebuffer golden, or game-derived bytes are included. The
operator ROM is attached separately by the hypervisor as the read-only
`game-image` block device (`/dev/vdb` inside the guest).

Validate with:

```sh
cargo run --locked -p xtask -- image validate {}/workload-image.yaml
```
"#,
        path.parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or("dist/workload-image")
    );
    write(path, content.as_bytes())
}

fn validate_artifacts(base: &Path, root: Option<&Mapping>, errors: &mut Vec<String>) {
    let Some(artifacts) = child_map(root, "artifacts", "artifacts", errors) else {
        return;
    };
    for key in ["kernel", "initramfs"] {
        let Some(artifact) = child_map(Some(artifacts), key, &format!("artifacts.{key}"), errors)
        else {
            continue;
        };
        let Some(file) = string_field(artifact, "file", &format!("artifacts.{key}.file"), errors)
        else {
            continue;
        };
        let Some(expected) = string_field(
            artifact,
            "blake3",
            &format!("artifacts.{key}.blake3"),
            errors,
        ) else {
            continue;
        };
        let path = base.join(file);
        match blake3_file(&path) {
            Ok(actual) if actual == expected => {}
            Ok(actual) => errors.push(format!(
                "artifacts.{key}.blake3 mismatch: expected {expected}, got {actual}"
            )),
            Err(err) => errors.push(format!("cannot hash {}: {err}", path.display())),
        }
    }
}

fn validate_boot(root: Option<&Mapping>, errors: &mut Vec<String>) {
    let Some(boot) = child_map(root, "boot", "boot", errors) else {
        return;
    };
    expect_string(boot, "protocol", "linux-direct", "boot.protocol", errors);
    let Some(cmdline) = string_field(boot, "cmdline", "boot.cmdline", errors) else {
        return;
    };
    for banned in ["console=", "init=", "panic=", "random.trust_cpu="] {
        if cmdline.contains(banned) {
            errors.push(format!(
                "boot.cmdline must be append-only and not restate {banned}"
            ));
        }
    }
}

fn validate_machine(root: Option<&Mapping>, errors: &mut Vec<String>) {
    let Some(machine) = child_map(root, "machine", "machine", errors) else {
        return;
    };
    expect_u64(machine, "vcpus", 1, "machine.vcpus", errors);
    expect_u64(machine, "mem_mib", 128, "machine.mem_mib", errors);
    let Some(devices) = seq_field(machine, "devices", "machine.devices", errors) else {
        return;
    };
    for required in ["virtio-blk", "detguest-channel", "pv-pad"] {
        let found = devices.iter().any(|device| {
            mapping(device, "machine.devices[]", errors)
                .and_then(|map| string_field(map, "kind", "machine.devices[].kind", errors))
                == Some(required)
        });
        if !found {
            errors.push(format!("machine.devices missing {required}"));
        }
    }
}

fn validate_regions(root: Option<&Mapping>, errors: &mut Vec<String>) {
    let Some(root) = root else {
        return;
    };
    let Some(regions) = seq_field(root, "regions", "regions", errors) else {
        return;
    };

    for spec in REQUIRED_REGIONS {
        let found = regions.iter().find_map(|region| {
            let map = mapping(region, "regions[]", errors)?;
            let name = string_field(map, "name", "regions[].name", errors)?;
            (name == spec.name).then_some(map)
        });

        let Some(region) = found else {
            errors.push(format!("regions missing {}", spec.name));
            continue;
        };
        match u64_field(
            region,
            "size",
            &format!("regions.{}.size", spec.name),
            errors,
        ) {
            Some(size) if size >= spec.size => {}
            Some(size) => errors.push(format!(
                "regions.{} size {size} is smaller than {}",
                spec.name, spec.size
            )),
            None => {}
        }
        if let Some(format) = spec.format {
            expect_string(
                region,
                "format",
                format,
                &format!("regions.{}.format", spec.name),
                errors,
            );
        }
        if has_key(region, "layout_version") {
            errors.push(format!(
                "regions.{} must not carry guest-sdk layout_version in workload-image.yaml",
                spec.name
            ));
        }
    }
}

fn validate_fps(root: Option<&Mapping>, errors: &mut Vec<String>) {
    let Some(fps) = child_map(root, "fps", "fps", errors) else {
        return;
    };
    expect_u64(fps, "num", FPS_NUM, "fps.num", errors);
    expect_u64(fps, "den", FPS_DEN, "fps.den", errors);
}

fn validate_pad_layout(root: Option<&Mapping>, errors: &mut Vec<String>) {
    let Some(pad) = child_map(root, "pad_layout", "pad_layout", errors) else {
        return;
    };
    expect_u64(
        pad,
        "layout_version",
        1,
        "pad_layout.layout_version",
        errors,
    );
    let Some(buttons) = seq_field(pad, "buttons", "pad_layout.buttons", errors) else {
        return;
    };
    if buttons.len() != PAD_BUTTONS.len() {
        errors.push(format!(
            "pad_layout.buttons length {} != {}",
            buttons.len(),
            PAD_BUTTONS.len()
        ));
        return;
    }
    for (idx, (name, bit)) in PAD_BUTTONS.iter().enumerate() {
        let Some(button) = mapping(&buttons[idx], "pad_layout.buttons[]", errors) else {
            continue;
        };
        expect_string(
            button,
            "name",
            name,
            &format!("pad_layout.buttons[{idx}].name"),
            errors,
        );
        expect_u64(
            button,
            "bit",
            *bit,
            &format!("pad_layout.buttons[{idx}].bit"),
            errors,
        );
    }

    let expected_reserved = [12_u64, 13, 14, 15];
    let Some(reserved) = seq_field(pad, "reserved_bits", "pad_layout.reserved_bits", errors) else {
        return;
    };
    let actual: Vec<_> = reserved.iter().filter_map(|value| value.as_u64()).collect();
    if actual != expected_reserved {
        errors.push(format!(
            "pad_layout.reserved_bits mismatch: expected {:?}, got {:?}",
            expected_reserved, actual
        ));
    }
}

fn validate_defaults(root: Option<&Mapping>, errors: &mut Vec<String>) {
    let Some(defaults) = child_map(root, "defaults", "defaults", errors) else {
        return;
    };
    expect_string(
        defaults,
        "feature_map",
        "demo-game@latest",
        "defaults.feature_map",
        errors,
    );
    expect_string(
        defaults,
        "scoring_program",
        "demo-game@latest",
        "defaults.scoring_program",
        errors,
    );
}

fn validate_handoff_files(base: &Path, errors: &mut Vec<String>) {
    for rel in [
        "boot.toml",
        "harness.toml",
        "expected-regions.toml",
        "README.md",
        "determinism.unstamped.yaml",
    ] {
        if !base.join(rel).is_file() {
            errors.push(format!("missing dist handoff file {rel}"));
        }
    }

    match read_to_string(&base.join("expected-regions.toml")) {
        Ok(content) => validate_expected_regions_toml(&content, errors),
        Err(err) => errors.push(format!("cannot read expected-regions.toml: {err}")),
    }
}

fn validate_expected_regions_toml(content: &str, errors: &mut Vec<String>) {
    for spec in REQUIRED_REGIONS {
        let Some(block) = region_block(content, spec.name) else {
            errors.push(format!("expected-regions.toml missing {}", spec.name));
            continue;
        };
        if field_u64(block, "size") != Some(spec.size) {
            errors.push(format!(
                "expected-regions.toml {} size must be {}",
                spec.name, spec.size
            ));
        }
        if field_u64(block, "layout_version") != Some(spec.layout_version) {
            errors.push(format!(
                "expected-regions.toml {} layout_version must be {}",
                spec.name, spec.layout_version
            ));
        }
    }
}

fn validate_no_game_content(base: &Path, errors: &mut Vec<String>) {
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) => {
                errors.push(format!("cannot read {}: {err}", dir.display()));
                continue;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let ext = path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if matches!(ext.as_str(), "rom" | "sfc" | "smc" | "srm" | "sav")
                || name.contains("game.")
            {
                errors.push(format!(
                    "game content-like file present: {}",
                    path.display()
                ));
            }
        }
    }
}

fn mapping<'a>(value: &'a Value, path: &str, errors: &mut Vec<String>) -> Option<&'a Mapping> {
    match value {
        Value::Mapping(map) => Some(map),
        _ => {
            errors.push(format!("{path} must be a mapping"));
            None
        }
    }
}

fn child_map<'a>(
    parent: Option<&'a Mapping>,
    key: &str,
    path: &str,
    errors: &mut Vec<String>,
) -> Option<&'a Mapping> {
    let parent = parent?;
    let Some(value) = parent.get(Value::String(key.into())) else {
        errors.push(format!("missing {path}"));
        return None;
    };
    mapping(value, path, errors)
}

fn seq_field<'a>(
    parent: &'a Mapping,
    key: &str,
    path: &str,
    errors: &mut Vec<String>,
) -> Option<&'a Vec<Value>> {
    let Some(value) = parent.get(Value::String(key.into())) else {
        errors.push(format!("missing {path}"));
        return None;
    };
    match value {
        Value::Sequence(seq) => Some(seq),
        _ => {
            errors.push(format!("{path} must be a sequence"));
            None
        }
    }
}

fn string_field<'a>(
    parent: &'a Mapping,
    key: &str,
    path: &str,
    errors: &mut Vec<String>,
) -> Option<&'a str> {
    let Some(value) = parent.get(Value::String(key.into())) else {
        errors.push(format!("missing {path}"));
        return None;
    };
    match value {
        Value::String(value) => Some(value),
        _ => {
            errors.push(format!("{path} must be a string"));
            None
        }
    }
}

fn u64_field(parent: &Mapping, key: &str, path: &str, errors: &mut Vec<String>) -> Option<u64> {
    let Some(value) = parent.get(Value::String(key.into())) else {
        errors.push(format!("missing {path}"));
        return None;
    };
    match value {
        Value::Number(number) => number.as_u64().or_else(|| {
            errors.push(format!("{path} must be an unsigned integer"));
            None
        }),
        _ => {
            errors.push(format!("{path} must be an unsigned integer"));
            None
        }
    }
}

fn expect_string(
    parent: &Mapping,
    key: &str,
    expected: &str,
    path: &str,
    errors: &mut Vec<String>,
) {
    if let Some(actual) = string_field(parent, key, path, errors) {
        if actual != expected {
            errors.push(format!("{path} expected {expected:?}, got {actual:?}"));
        }
    }
}

fn expect_u64(parent: &Mapping, key: &str, expected: u64, path: &str, errors: &mut Vec<String>) {
    if let Some(actual) = u64_field(parent, key, path, errors) {
        if actual != expected {
            errors.push(format!("{path} expected {expected}, got {actual}"));
        }
    }
}

fn has_key(parent: &Mapping, key: &str) -> bool {
    parent.contains_key(Value::String(key.into()))
}

fn region_block<'a>(content: &'a str, name: &str) -> Option<&'a str> {
    content
        .split("[[regions]]")
        .skip(1)
        .find(|block| field_string(block, "name").as_deref() == Some(name))
}

fn field_string(block: &str, key: &str) -> Option<String> {
    let value = field_value(block, key)?;
    Some(value.strip_prefix('"')?.strip_suffix('"')?.to_owned())
}

fn field_u64(block: &str, key: &str) -> Option<u64> {
    field_value(block, key)?.parse().ok()
}

fn field_value<'a>(block: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key} = ");
    block
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(&prefix))
}

fn quoted_value(content: &str, key: &str) -> Result<String, ImageError> {
    let prefix = format!("{key} = \"");
    let line = content
        .lines()
        .find(|line| line.starts_with(&prefix))
        .ok_or_else(|| ImageError::InvalidInput(format!("missing {key}")))?;
    let value = line
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix('"'))
        .ok_or_else(|| ImageError::InvalidInput(format!("malformed {key}")))?;
    Ok(value.replace("\\n", "\n"))
}

fn git_rev(workspace_root: &Path) -> Result<String, ImageError> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(workspace_root)
        .output()
        .map_err(|source| ImageError::Io {
            path: PathBuf::from("git"),
            source,
        })?;
    if !output.status.success() {
        return Err(ImageError::CommandFailed {
            program: "git rev-parse HEAD".into(),
            status: output.status.to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn blake3_file(path: &Path) -> Result<String, ImageError> {
    let bytes = read(path)?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

fn read(path: &Path) -> Result<Vec<u8>, ImageError> {
    std::fs::read(path).map_err(|source| ImageError::Io {
        path: path.to_owned(),
        source,
    })
}

fn read_to_string(path: &Path) -> Result<String, ImageError> {
    std::fs::read_to_string(path).map_err(|source| ImageError::Io {
        path: path.to_owned(),
        source,
    })
}

fn write(path: impl AsRef<Path>, bytes: &[u8]) -> Result<(), ImageError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    std::fs::write(path, bytes).map_err(|source| ImageError::Io {
        path: path.to_owned(),
        source,
    })
}

fn create_dir_all(path: &Path) -> Result<(), ImageError> {
    std::fs::create_dir_all(path).map_err(|source| ImageError::Io {
        path: path.to_owned(),
        source,
    })
}

fn remove_dir_all(path: &Path) -> Result<(), ImageError> {
    std::fs::remove_dir_all(path).map_err(|source| ImageError::Io {
        path: path.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static TEMP_ID: AtomicUsize = AtomicUsize::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let id = TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("refwork-image-test-{}-{id}", std::process::id()));
            if path.exists() {
                std::fs::remove_dir_all(&path).unwrap();
            }
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn valid_dist() -> TempDir {
        let tmp = TempDir::new();
        std::fs::write(tmp.path.join("bzImage"), b"kernel").unwrap();
        std::fs::write(tmp.path.join("initramfs.cpio.zst"), b"initramfs").unwrap();
        std::fs::write(tmp.path.join("boot.toml"), b"boot").unwrap();
        std::fs::write(tmp.path.join("harness.toml"), b"harness").unwrap();
        std::fs::write(
            tmp.path.join("expected-regions.toml"),
            b"schema_version = 1\n\n[[regions]]\nname = \"wram\"\nsize = 131072\nlayout_version = 1\n\n[[regions]]\nname = \"framebuffer\"\nsize = 229376\nlayout_version = 1\n\n[[regions]]\nname = \"meta\"\nsize = 4096\nlayout_version = 1\n",
        )
        .unwrap();
        std::fs::write(tmp.path.join("README.md"), b"readme").unwrap();
        std::fs::write(tmp.path.join("determinism.unstamped.yaml"), b"unstamped").unwrap();
        let kernel_hash = blake3_file(&tmp.path.join("bzImage")).unwrap();
        let initramfs_hash = blake3_file(&tmp.path.join("initramfs.cpio.zst")).unwrap();
        write_workload_manifest(
            &tmp.path.join("workload-image.yaml"),
            "0123456789012345678901234567890123456789",
            "placeholder:test",
            &kernel_hash,
            &initramfs_hash,
        )
        .unwrap();
        tmp
    }

    fn manifest_text(tmp: &TempDir) -> String {
        std::fs::read_to_string(tmp.path.join("workload-image.yaml")).unwrap()
    }

    fn write_manifest_text(tmp: &TempDir, text: &str) {
        std::fs::write(tmp.path.join("workload-image.yaml"), text).unwrap();
    }

    fn validation_errors(tmp: &TempDir) -> Vec<String> {
        match validate_manifest(&tmp.path.join("workload-image.yaml")) {
            Ok(()) => panic!("validation unexpectedly passed"),
            Err(ImageError::Validation(errors)) => errors,
            Err(err) => panic!("unexpected error: {err}"),
        }
    }

    #[test]
    fn validator_accepts_generated_manifest_shape() {
        let tmp = valid_dist();
        validate_manifest(&tmp.path.join("workload-image.yaml")).unwrap();
    }

    #[test]
    fn validator_rejects_wrong_vcpu_count() {
        let tmp = valid_dist();
        let manifest = manifest_text(&tmp).replace("  vcpus: 1", "  vcpus: 2");
        write_manifest_text(&tmp, &manifest);

        let errors = validation_errors(&tmp);

        assert!(errors
            .iter()
            .any(|err| err.contains("machine.vcpus expected 1")));
    }

    #[test]
    fn validator_rejects_float_fps() {
        let tmp = valid_dist();
        let manifest = manifest_text(&tmp).replace("  num: 21477272", "  num: 60.0");
        write_manifest_text(&tmp, &manifest);

        let errors = validation_errors(&tmp);

        assert!(errors.iter().any(|err| err.contains("fps.num must be")));
    }

    #[test]
    fn validator_rejects_pad_layout_drift() {
        let tmp = valid_dist();
        let manifest =
            manifest_text(&tmp).replace("{ name: Start, bit: 10 }", "{ name: Start, bit: 9 }");
        write_manifest_text(&tmp, &manifest);

        let errors = validation_errors(&tmp);

        assert!(errors
            .iter()
            .any(|err| err.contains("pad_layout.buttons[10].bit expected 10")));
    }

    #[test]
    fn validator_rejects_region_layout_version_in_workload_manifest() {
        let tmp = valid_dist();
        let manifest = manifest_text(&tmp).replace(
            "- { name: wram, size: 131072 }",
            "- { name: wram, size: 131072, layout_version: 1 }",
        );
        write_manifest_text(&tmp, &manifest);

        let errors = validation_errors(&tmp);

        assert!(errors
            .iter()
            .any(|err| err.contains("must not carry guest-sdk layout_version")));
    }

    #[test]
    fn validator_rejects_game_like_payload_files() {
        let tmp = valid_dist();
        std::fs::write(tmp.path.join("game.sfc"), b"not allowed").unwrap();

        let errors = validation_errors(&tmp);

        assert!(errors
            .iter()
            .any(|err| err.contains("game content-like file")));
    }

    #[test]
    fn newc_archive_is_deterministic_and_contains_trailer() {
        let entries = [
            CpioEntry::dir("."),
            CpioEntry::file("init", 0o100755, b"hello".to_vec()),
        ];

        let first = newc_archive(&entries);
        let second = newc_archive(&entries);

        assert_eq!(first, second);
        assert!(String::from_utf8_lossy(&first).contains("TRAILER!!!"));
    }
}
