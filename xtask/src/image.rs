//! `xtask image`: build and validate package-04 workload-image handoff artifacts.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use serde_yaml::{Mapping, Value};

const WORKLOAD_NAME: &str = "refwork-demo";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const FPS_NUM: u64 = 21_477_272;
const FPS_DEN: u64 = 357_366;
const ZSTD_VERSION: &str = "1.5.5";
const DOUBLE_BUILD_ROOT: &str = "image-double-build";
const UNSTAMPED_FILE: &str = "determinism.unstamped.yaml";
const GREEN_STAMP_FILE: &str = "determinism.last_green";
const GREEN_STAMP_SENTINEL: &str = "image/register-requires-green-stamp";
const PAD_LAYOUT_ID: &str = "console16-12btn-v1";

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactComparison {
    pub file: &'static str,
    pub bytes: u64,
    pub blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoubleBuildReport {
    pub first_manifest: PathBuf,
    pub second_manifest: PathBuf,
    pub artifacts: Vec<ArtifactComparison>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterMode {
    DirectDistUnstamped,
    DirectDistStamped,
}

impl fmt::Display for RegisterMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegisterMode::DirectDistUnstamped => {
                f.write_str("unstamped sidecar accepted until package 06 green stamp lands")
            }
            RegisterMode::DirectDistStamped => f.write_str("green stamp present"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterReport {
    pub manifest: PathBuf,
    pub manifest_blake3: String,
    pub mode: RegisterMode,
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

/// `agent_bin: None` builds detguest-agent from the sibling guest-sdk
/// checkout at the rev pinned in `image/guest-sdk.lock` (the normal path);
/// `Some(path)` is the test/escape hatch and skips the rev check.
pub fn build_image(workspace_root: &Path, agent_bin: Option<&Path>) -> Result<PathBuf, ImageError> {
    build_image_with_git_rev(workspace_root, agent_bin, None)
}

fn build_image_with_git_rev(
    workspace_root: &Path,
    agent_bin: Option<&Path>,
    source_rev: Option<&str>,
) -> Result<PathBuf, ImageError> {
    let agent_bin = match agent_bin {
        Some(path) => {
            if !path.is_file() {
                return Err(ImageError::MissingInput(format!(
                    "agent binary does not exist: {}",
                    path.display()
                )));
            }
            path.to_path_buf()
        }
        None => build_agent_from_pinned_sibling(workspace_root)?,
    };
    let agent_bin = agent_bin.as_path();

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
    let guest_sdk_lock = read_to_string(&image_dir.join("guest-sdk.lock"))?;
    let kernel_bytes = resolve_pinned_kernel(workspace_root)?;
    let guest_sdk_rev = quoted_value(&guest_sdk_lock, "rev")?;

    write(out_dir.join("boot.toml"), boot.as_bytes())?;
    write(out_dir.join("harness.toml"), harness.as_bytes())?;
    write(
        out_dir.join("expected-regions.toml"),
        expected_regions.as_bytes(),
    )?;
    write(out_dir.join("bzImage"), &kernel_bytes)?;

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

    let git_rev = match source_rev {
        Some(rev) => rev.to_owned(),
        None => git_rev(workspace_root)?,
    };
    let kernel_hash = blake3_file(&out_dir.join("bzImage"))?;
    let initramfs_hash = blake3_file(&out_dir.join("initramfs.cpio.zst"))?;
    write_workload_manifest(
        &out_dir.join("workload-image.yaml"),
        &git_rev,
        &guest_sdk_rev,
        refwork_emu::EMU_VERSION,
        &kernel_hash,
        &initramfs_hash,
    )?;
    write_unstamped_sidecar(
        &out_dir.join("determinism.unstamped.yaml"),
        &git_rev,
        refwork_emu::EMU_VERSION,
    )?;
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
    let manifest_blake3 = match blake3_file(manifest) {
        Ok(hash) => Some(hash),
        Err(err) => {
            errors.push(format!(
                "cannot hash manifest {}: {err}",
                manifest.display()
            ));
            None
        }
    };
    let root = mapping(&yaml, "root", &mut errors);
    let Some(variant) = resolve_variant(root, &mut errors) else {
        return Err(ImageError::Validation(errors));
    };

    if let Some(root) = root {
        expect_u64(root, "schema_version", 1, "schema_version", &mut errors);
        expect_string(root, "kind", "workload-image", "kind", &mut errors);
    }
    if let Some(meta) = child_map(root, "meta", "meta", &mut errors) {
        match variant {
            ManifestVariant::Game => {
                expect_string(meta, "name", WORKLOAD_NAME, "meta.name", &mut errors);
                expect_string(meta, "version", VERSION, "meta.version", &mut errors);
            }
            ManifestVariant::Generic => {
                if let Some(name) = string_field(meta, "name", "meta.name", &mut errors) {
                    if name.is_empty()
                        || !name
                            .bytes()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
                        || !name.as_bytes()[0].is_ascii_lowercase()
                            && !name.as_bytes()[0].is_ascii_digit()
                    {
                        errors.push("meta.name must match ^[a-z0-9][a-z0-9-]*$".into());
                    }
                }
                if let Some(version) = string_field(meta, "version", "meta.version", &mut errors) {
                    if version.is_empty()
                        || version.contains('@')
                        || version.chars().any(char::is_whitespace)
                    {
                        errors
                            .push("meta.version must be non-empty without @ or whitespace".into());
                    }
                }
            }
        }
        if let Some(built_from) =
            child_map(Some(meta), "built_from", "meta.built_from", &mut errors)
        {
            match variant {
                ManifestVariant::Game => expect_nonempty_string(
                    built_from,
                    "emu_version",
                    "meta.built_from.emu_version",
                    &mut errors,
                ),
                ManifestVariant::Generic => {
                    expect_nonempty_string(built_from, "repo", "meta.built_from.repo", &mut errors);
                    for key in ["git_rev", "guest_sdk_rev"] {
                        let path = format!("meta.built_from.{key}");
                        if let Some(rev) = string_field(built_from, key, &path, &mut errors) {
                            if !is_git_rev_hex(rev) {
                                errors.push(format!("{path} must be 40 lowercase hex chars"));
                            }
                        }
                    }
                }
            }
        }
    }

    validate_artifacts(base, root, &mut errors);
    validate_boot(root, &mut errors);
    validate_machine(root, variant, &mut errors);
    validate_regions(root, variant, &mut errors);
    validate_fps(root, variant, &mut errors);
    validate_pad_layout(root, &mut errors);
    validate_defaults(root, variant, &mut errors);
    match variant {
        ManifestVariant::Game => {
            validate_handoff_files(base, root, manifest_blake3.as_deref(), &mut errors)
        }
        ManifestVariant::Generic => {
            if let Some(determinism) = child_map(root, "determinism", "determinism", &mut errors) {
                expect_nonempty_string(determinism, "suite", "determinism.suite", &mut errors);
            }
            validate_generic_handoff_files(base, root, manifest_blake3.as_deref(), &mut errors);
        }
    }
    validate_no_game_content(base, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ImageError::Validation(errors))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManifestVariant {
    Game,
    Generic,
}

fn resolve_variant(root: Option<&Mapping>, errors: &mut Vec<String>) -> Option<ManifestVariant> {
    let Some(value) = root.and_then(|root| root.get(Value::String("variant".into()))) else {
        return Some(ManifestVariant::Game);
    };
    match value.as_str() {
        Some("game") => Some(ManifestVariant::Game),
        Some("generic") => Some(ManifestVariant::Generic),
        _ => {
            errors.push(format!("variant must be game or generic, got {value:?}"));
            None
        }
    }
}

pub fn manifest_variant(manifest: &Path) -> Result<ManifestVariant, ImageError> {
    let content = read_to_string(manifest)?;
    let yaml: Value = serde_yaml::from_str(&content).map_err(ImageError::Yaml)?;
    let mut errors = Vec::new();
    let root = mapping(&yaml, "root", &mut errors);
    let variant = resolve_variant(root, &mut errors);
    variant.ok_or(ImageError::Validation(errors))
}

pub fn double_build(workspace_root: &Path) -> Result<DoubleBuildReport, ImageError> {
    ensure_clean_git_checkout(workspace_root, "reference-workload")?;
    let source_rev = git_rev(workspace_root)?;
    let control_plane = sibling_checkout(workspace_root, "control-plane")?;
    ensure_clean_git_checkout(&control_plane, "control-plane")?;
    // dh-proto path dep (worker gRPC contract). Only the crate and its proto
    // input gate the build, so the cleanliness check is scoped to them — the
    // hypervisor checkout routinely carries unrelated in-flight work.
    let hypervisor = sibling_checkout(workspace_root, "determinism-hypervisor")?;
    ensure_clean_git_paths(
        &hypervisor,
        "determinism-hypervisor",
        &["crates/dh-proto", "proto"],
    )?;
    // guest-sdk feeds the build twice: detguest-sdk as a harness path dep
    // and detguest-agent built from the pinned rev — gate all crate sources
    // plus the workspace manifests.
    let guest_sdk = sibling_checkout(workspace_root, "guest-sdk")?;
    ensure_clean_git_paths(
        &guest_sdk,
        "guest-sdk",
        &["crates", "Cargo.toml", "Cargo.lock"],
    )?;

    let double_root = workspace_root.join("target").join(DOUBLE_BUILD_ROOT);
    if double_root.exists() {
        remove_dir_all(&double_root)?;
    }
    create_dir_all(&double_root)?;

    let siblings = [
        ("control-plane", control_plane.as_path()),
        ("determinism-hypervisor", hypervisor.as_path()),
        ("guest-sdk", guest_sdk.as_path()),
    ];
    let first_dir = build_from_clean_root(
        workspace_root,
        &siblings,
        &double_root,
        "root-a",
        &source_rev,
    )?;
    let second_dir = build_from_clean_root(
        workspace_root,
        &siblings,
        &double_root,
        "root-b",
        &source_rev,
    )?;

    let first_manifest = first_dir.join("workload-image.yaml");
    let second_manifest = second_dir.join("workload-image.yaml");
    validate_manifest(&first_manifest)?;
    validate_manifest(&second_manifest)?;

    let artifacts = compare_double_build_artifacts(&first_dir, &second_dir)?;
    Ok(DoubleBuildReport {
        first_manifest,
        second_manifest,
        artifacts,
    })
}

pub fn register_image(
    workspace_root: &Path,
    manifest: Option<&Path>,
    require_green_stamp: bool,
) -> Result<RegisterReport, ImageError> {
    let manifest = manifest.map(Path::to_path_buf).unwrap_or_else(|| {
        workspace_root
            .join("dist")
            .join(format!("workload-image-{VERSION}"))
            .join("workload-image.yaml")
    });

    validate_manifest(&manifest)?;
    let base = manifest.parent().ok_or_else(|| {
        ImageError::InvalidInput(format!("manifest has no parent: {}", manifest.display()))
    })?;
    let manifest_blake3 = blake3_file(&manifest)?;
    let green_stamp = base.join(GREEN_STAMP_FILE);
    let unstamped = base.join(UNSTAMPED_FILE);

    if green_stamp.is_file() && unstamped.is_file() {
        return Err(ImageError::InvalidInput(format!(
            "{} must not coexist with {UNSTAMPED_FILE}",
            green_stamp.display()
        )));
    }
    if green_stamp.is_file() {
        return Ok(RegisterReport {
            manifest,
            manifest_blake3,
            mode: RegisterMode::DirectDistStamped,
        });
    }

    if require_green_stamp || green_stamp_support_landed(workspace_root) {
        return Err(ImageError::MissingInput(format!(
            "missing determinism green stamp {}; package 06 registration refuses unstamped manifests",
            green_stamp.display()
        )));
    }
    if !unstamped.is_file() {
        return Err(ImageError::MissingInput(format!(
            "missing determinism sidecar {}; expected {UNSTAMPED_FILE} before package 06",
            unstamped.display()
        )));
    }

    Ok(RegisterReport {
        manifest,
        manifest_blake3,
        mode: RegisterMode::DirectDistUnstamped,
    })
}

fn build_from_clean_root(
    source_workspace: &Path,
    siblings: &[(&str, &Path)],
    double_root: &Path,
    name: &str,
    source_rev: &str,
) -> Result<PathBuf, ImageError> {
    // Build in a FIXED directory and rename to the per-root name afterward:
    // cargo canonicalizes manifest paths and folds the real lexical path of
    // out-of-workspace path deps (../guest-sdk, …) into crate metadata
    // hashes, so building under root-a and root-b directly yields different
    // mangled symbols and breaks byte-for-byte reproducibility. (A symlink
    // alias is not enough — canonicalization sees through it.)
    let build_root = double_root.join("build");
    if build_root.exists() {
        remove_dir_all(&build_root)?;
    }
    let workspace = build_root.join("reference-workload");
    create_dir_all(&build_root)?;
    materialize_tracked_source(source_workspace, &workspace)?;
    for (sibling_name, sibling_path) in siblings {
        symlink_path(sibling_path, &build_root.join(sibling_name))?;
    }
    let out_dir = build_image_with_git_rev(&workspace, None, Some(source_rev))?;
    let rel = out_dir
        .strip_prefix(&build_root)
        .map_err(|_| {
            ImageError::InvalidInput(format!(
                "build output {} escaped the build root {}",
                out_dir.display(),
                build_root.display()
            ))
        })?
        .to_path_buf();
    let root = double_root.join(name);
    if root.exists() {
        remove_dir_all(&root)?;
    }
    std::fs::rename(&build_root, &root).map_err(|source| ImageError::Io {
        path: root.clone(),
        source,
    })?;
    Ok(root.join(rel))
}

fn materialize_tracked_source(source: &Path, dest: &Path) -> Result<(), ImageError> {
    let output = Command::new("git")
        .arg("ls-files")
        .arg("-z")
        .current_dir(source)
        .output()
        .map_err(|source| ImageError::Io {
            path: PathBuf::from("git"),
            source,
        })?;
    if !output.status.success() {
        return Err(ImageError::CommandFailed {
            program: "git ls-files -z".into(),
            status: output.status.to_string(),
        });
    }

    create_dir_all(dest)?;
    for rel in output.stdout.split(|byte| *byte == 0) {
        if rel.is_empty() {
            continue;
        }
        let rel = String::from_utf8_lossy(rel);
        let src = source.join(rel.as_ref());
        let dst = dest.join(rel.as_ref());
        copy_source_entry(&src, &dst)?;
    }
    Ok(())
}

fn copy_source_entry(src: &Path, dst: &Path) -> Result<(), ImageError> {
    if let Some(parent) = dst.parent() {
        create_dir_all(parent)?;
    }
    let metadata = std::fs::symlink_metadata(src).map_err(|source| ImageError::Io {
        path: src.to_owned(),
        source,
    })?;
    if metadata.file_type().is_symlink() {
        let target = std::fs::read_link(src).map_err(|source| ImageError::Io {
            path: src.to_owned(),
            source,
        })?;
        symlink_path(&target, dst)
    } else {
        std::fs::copy(src, dst).map_err(|source| ImageError::Io {
            path: dst.to_owned(),
            source,
        })?;
        std::fs::set_permissions(dst, metadata.permissions()).map_err(|source| ImageError::Io {
            path: dst.to_owned(),
            source,
        })
    }
}

/// Artifact-handoff leg of the kernel/agent split
/// (.agents/decisions/2026-07-02-kernel-agent-artifact-split.md): consume
/// guest-sdk's deterministically-built bzImage, refusing on a BLAKE3
/// mismatch with `image/kernel.lock`.
fn resolve_pinned_kernel(workspace_root: &Path) -> Result<Vec<u8>, ImageError> {
    let kernel_lock = read_to_string(&workspace_root.join("image").join("kernel.lock"))?;
    let pinned = quoted_value(&kernel_lock, "blake3")?;
    let guest_sdk = sibling_checkout(workspace_root, "guest-sdk")?;
    let bzimage = guest_sdk.join("image").join("build").join("bzImage");
    if !bzimage.is_file() {
        return Err(ImageError::MissingInput(format!(
            "pinned kernel artifact missing: {} — run `./image/build.sh kernel` in guest-sdk              (its build key caches the result; see guest-sdk image/KERNEL.md)",
            bzimage.display()
        )));
    }
    verify_pinned_kernel_artifact(&bzimage, &pinned)
}

/// Refusal core of the kernel pin, parameterized for tests: read the
/// artifact and refuse on a BLAKE3 mismatch with the lock.
pub fn verify_pinned_kernel_artifact(bzimage: &Path, pinned: &str) -> Result<Vec<u8>, ImageError> {
    let bytes = read(bzimage)?;
    let actual = blake3::hash(&bytes).to_hex().to_string();
    if actual != pinned {
        return Err(ImageError::InvalidInput(format!(
            "kernel artifact {} has BLAKE3 {actual} but image/kernel.lock pins {pinned};              either rebuild the pinned kernel in guest-sdk or deliberately bump the lock              (kernel_version/build_key/blake3 together)",
            bzimage.display()
        )));
    }
    Ok(bytes)
}

/// Refusal core of the guest-sdk rev pin, parameterized for tests: refuse
/// unless the checkout's HEAD is exactly the pinned rev, naming both revs.
pub fn verify_pinned_rev(guest_sdk: &Path, pinned_rev: &str) -> Result<(), ImageError> {
    let head = Command::new("git")
        .arg("-C")
        .arg(guest_sdk)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|source| ImageError::Io {
            path: PathBuf::from("git"),
            source,
        })?;
    if !head.status.success() {
        return Err(ImageError::CommandFailed {
            program: format!("git -C {} rev-parse HEAD", guest_sdk.display()),
            status: head.status.to_string(),
        });
    }
    let head = String::from_utf8_lossy(&head.stdout).trim().to_owned();
    if head != pinned_rev {
        return Err(ImageError::InvalidInput(format!(
            "guest-sdk checkout is at {head} but image/guest-sdk.lock pins {pinned_rev};              check out the pinned rev or deliberately bump the lock"
        )));
    }
    Ok(())
}

/// Build-from-sibling leg of the kernel/agent split: verify the guest-sdk
/// checkout is at exactly the rev pinned in `image/guest-sdk.lock`, then
/// build detguest-agent for the musl target and return its path.
fn build_agent_from_pinned_sibling(workspace_root: &Path) -> Result<PathBuf, ImageError> {
    let guest_sdk_lock = read_to_string(&workspace_root.join("image").join("guest-sdk.lock"))?;
    let pinned_rev = quoted_value(&guest_sdk_lock, "rev")?;
    let guest_sdk = sibling_checkout(workspace_root, "guest-sdk")?;
    verify_pinned_rev(&guest_sdk, &pinned_rev)?;

    let status = Command::new("cargo")
        .args([
            "build",
            "--locked",
            "--release",
            "--target",
            "x86_64-unknown-linux-musl",
            "-p",
            "detguest-agent",
        ])
        .current_dir(&guest_sdk)
        .status()
        .map_err(|source| ImageError::Io {
            path: PathBuf::from("cargo"),
            source,
        })?;
    if !status.success() {
        return Err(ImageError::CommandFailed {
            program:
                "cargo build --target x86_64-unknown-linux-musl -p detguest-agent (in guest-sdk)"
                    .into(),
            status: status.to_string(),
        });
    }
    Ok(guest_sdk
        .join("target")
        .join("x86_64-unknown-linux-musl")
        .join("release")
        .join("detguest-agent"))
}

fn compare_double_build_artifacts(
    first_dir: &Path,
    second_dir: &Path,
) -> Result<Vec<ArtifactComparison>, ImageError> {
    let mut errors = Vec::new();
    let mut comparisons = Vec::new();
    for file in ["bzImage", "initramfs.cpio.zst", "workload-image.yaml"] {
        let first = read(&first_dir.join(file))?;
        let second = read(&second_dir.join(file))?;
        let first_hash = blake3::hash(&first).to_hex().to_string();
        let second_hash = blake3::hash(&second).to_hex().to_string();
        if first != second {
            errors.push(format!(
                "double-build artifact {file} differs: first bytes={} blake3={}, second bytes={} blake3={}",
                first.len(),
                first_hash,
                second.len(),
                second_hash
            ));
            continue;
        }
        comparisons.push(ArtifactComparison {
            file,
            bytes: first.len() as u64,
            blake3: first_hash,
        });
    }

    if errors.is_empty() {
        Ok(comparisons)
    } else {
        Err(ImageError::Validation(errors))
    }
}

fn sibling_checkout(workspace_root: &Path, name: &str) -> Result<PathBuf, ImageError> {
    let Some(parent) = workspace_root.parent() else {
        return Err(ImageError::MissingInput(format!(
            "workspace has no parent: {}",
            workspace_root.display()
        )));
    };
    let sibling = parent.join(name);
    if sibling.join("Cargo.toml").is_file() {
        Ok(sibling)
    } else {
        Err(ImageError::MissingInput(format!(
            "missing sibling {name} checkout at {}; clean image double-build requires ../{name} or an approved recorded proto source",
            sibling.display()
        )))
    }
}

/// Scoped variant of [`ensure_clean_git_checkout`]: only the named pathspecs
/// must be clean.
fn ensure_clean_git_paths(path: &Path, label: &str, pathspecs: &[&str]) -> Result<(), ImageError> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(path)
        .arg("status")
        .arg("--short")
        .arg("--");
    for spec in pathspecs {
        command.arg(spec);
    }
    let output = command.output().map_err(|source| ImageError::Io {
        path: PathBuf::from("git"),
        source,
    })?;
    if !output.status.success() {
        return Err(ImageError::CommandFailed {
            program: format!("git -C {} status --short -- ...", path.display()),
            status: output.status.to_string(),
        });
    }
    if output.stdout.is_empty() {
        Ok(())
    } else {
        Err(ImageError::InvalidInput(format!(
            "{label} checkout is dirty in {}:
{}",
            pathspecs.join(", "),
            String::from_utf8_lossy(&output.stdout)
        )))
    }
}

fn ensure_clean_git_checkout(path: &Path, label: &str) -> Result<(), ImageError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("status")
        .arg("--short")
        .output()
        .map_err(|source| ImageError::Io {
            path: PathBuf::from("git"),
            source,
        })?;
    if !output.status.success() {
        return Err(ImageError::CommandFailed {
            program: format!("git -C {} status --short", path.display()),
            status: output.status.to_string(),
        });
    }
    if output.stdout.is_empty() {
        Ok(())
    } else {
        Err(ImageError::InvalidInput(format!(
            "{label} checkout is dirty:\n{}",
            String::from_utf8_lossy(&output.stdout)
        )))
    }
}

fn green_stamp_support_landed(workspace_root: &Path) -> bool {
    workspace_root.join(GREEN_STAMP_SENTINEL).is_file()
}

#[cfg(unix)]
fn symlink_path(src: &Path, dst: &Path) -> Result<(), ImageError> {
    if let Some(parent) = dst.parent() {
        create_dir_all(parent)?;
    }
    std::os::unix::fs::symlink(src, dst).map_err(|source| ImageError::Io {
        path: dst.to_owned(),
        source,
    })
}

#[cfg(not(unix))]
fn symlink_path(_src: &Path, dst: &Path) -> Result<(), ImageError> {
    Err(ImageError::InvalidInput(format!(
        "cannot create clean-root sibling symlink on this platform: {}",
        dst.display()
    )))
}

fn build_static_harness(workspace_root: &Path) -> Result<(), ImageError> {
    let bin = workspace_root
        .join("target")
        .join("x86_64-unknown-linux-musl")
        .join("release")
        .join("refwork-harness");
    let rustflags = harness_rustflags(workspace_root);
    let status = Command::new("cargo")
        .arg("build")
        .arg("--locked")
        .arg("--release")
        .arg("--target")
        .arg("x86_64-unknown-linux-musl")
        .arg("-p")
        .arg("refwork-harness")
        .env("RUSTFLAGS", rustflags)
        .current_dir(workspace_root)
        .status()
        .map_err(|source| ImageError::Io {
            path: PathBuf::from("cargo"),
            source,
        })?;
    if status.success() {
        ensure_no_panic_unwind(&bin)
    } else {
        Err(ImageError::CommandFailed {
            program: "cargo build --target x86_64-unknown-linux-musl -p refwork-harness".into(),
            status: status.to_string(),
        })
    }
}

fn harness_rustflags(workspace_root: &Path) -> String {
    let mut flags = vec![
        "-C".to_owned(),
        "panic=abort".to_owned(),
        format!(
            "--remap-path-prefix={}=/reference-workload",
            workspace_root.display()
        ),
    ];
    if let Some(parent) = workspace_root.parent() {
        // Every sibling path dep must be remapped (literal and canonical
        // forms — double-build reaches siblings through per-root symlinks),
        // or the embedded paths differ between clean-root builds and break
        // double-build reproducibility.
        for sibling in ["control-plane", "determinism-hypervisor", "guest-sdk"] {
            let path = parent.join(sibling);
            flags.push(format!("--remap-path-prefix={}=/{sibling}", path.display()));
            if let Ok(canonical) = path.canonicalize() {
                if canonical != path {
                    flags.push(format!(
                        "--remap-path-prefix={}=/{sibling}",
                        canonical.display()
                    ));
                }
            }
        }
    }
    flags.join(" ")
}

fn ensure_no_panic_unwind(bin: &Path) -> Result<(), ImageError> {
    let output = Command::new("nm")
        .arg("-a")
        .arg(bin)
        .output()
        .map_err(|source| ImageError::Io {
            path: PathBuf::from("nm"),
            source,
        })?;
    if !output.status.success() {
        return Err(ImageError::CommandFailed {
            program: format!("nm -a {}", bin.display()),
            status: output.status.to_string(),
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("panic_unwind") {
        Err(ImageError::InvalidInput(format!(
            "static harness {} contains panic_unwind symbols",
            bin.display()
        )))
    } else {
        Ok(())
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
        CpioEntry::dir("dev"),
        CpioEntry::dir("dev/hugepages"),
        CpioEntry::dir("etc"),
        CpioEntry::dir("etc/detguest"),
        CpioEntry::dir("etc/refwork"),
        CpioEntry::dir("proc"),
        CpioEntry::dir("run"),
        CpioEntry::dir("sbin"),
        CpioEntry::dir("sys"),
        CpioEntry::dir("usr"),
        CpioEntry::dir("usr/bin"),
        CpioEntry::file("etc/detguest/boot.toml", 0o100644, boot_toml.to_vec()),
        CpioEntry::file(
            "etc/detguest/expected-regions.toml",
            0o100644,
            expected_regions.to_vec(),
        ),
        CpioEntry::file("etc/refwork/harness.toml", 0o100644, harness_toml.to_vec()),
        // The image's only init path: /init IS the agent (guest-sdk
        // ARCHITECTURE §4; mirrors image/build.sh's recipe). The previous
        // #!/bin/sh init script could never exec — the image ships no
        // shell — leaving the kernel with no working init: the guest
        // spun to HARD_CAP on the first real boot (2026-07-03).
        CpioEntry::symlink("init", "/sbin/detguest-agent"),
        CpioEntry::file("sbin/detguest-agent", 0o100755, agent),
        CpioEntry::file("usr/bin/refwork-harness", 0o100755, harness),
    ];
    write(out, &newc_archive(&entries))
}

fn compress_zstd(input: &Path, output: &Path) -> Result<(), ImageError> {
    enforce_zstd_version()?;
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

fn enforce_zstd_version() -> Result<(), ImageError> {
    let output = Command::new("zstd")
        .arg("--version")
        .output()
        .map_err(|source| ImageError::Io {
            path: PathBuf::from("zstd"),
            source,
        })?;
    if !output.status.success() {
        return Err(ImageError::CommandFailed {
            program: "zstd --version".into(),
            status: output.status.to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains(&format!("v{ZSTD_VERSION}")) {
        Ok(())
    } else {
        Err(ImageError::InvalidInput(format!(
            "zstd version must be {ZSTD_VERSION}, got {}",
            stdout.trim()
        )))
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

    /// newc symlink entry: body bytes are the link target.
    fn symlink(path: &'static str, target: &str) -> Self {
        Self {
            path,
            mode: 0o120777,
            data: target.as_bytes().to_vec(),
        }
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
    emu_version: &str,
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
    emu_version: "{emu_version}"
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
  layout_id: {PAD_LAYOUT_ID}
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

fn write_unstamped_sidecar(
    path: &Path,
    git_rev: &str,
    emu_version: &str,
) -> Result<(), ImageError> {
    let content = format!(
        r#"schema_version: 1
kind: determinism-unstamped
workload_image: {WORKLOAD_NAME}@{VERSION}
git_rev: "{git_rev}"
emu_version: "{emu_version}"
reason: "package 06 owns the full determinism green stamp"
"#
    );
    write(path, content.as_bytes())
}

fn write_dist_readme(path: &Path) -> Result<(), ImageError> {
    let content = format!(
        r#"# Workload Image {VERSION}

Generated by `cargo run --locked -p xtask -- image build`.

This directory is an image handoff bundle for `refwork-demo`. It contains the
guest-sdk-built `bzImage` (hash-pinned by `image/kernel.lock`), a
deterministic `newc` initramfs compressed as `initramfs.cpio.zst` carrying
the detguest-agent (built from the guest-sdk rev pinned by
`image/guest-sdk.lock`) and the refwork harness, `workload-image.yaml`,
`boot.toml`, `harness.toml`, and guest-sdk expected-region handoff data.

No game ROM, SRAM, framebuffer golden, or game-derived bytes are included. The
operator ROM is attached separately by the hypervisor as the read-only
`game-image` block device (`/dev/vdb` inside the guest).

Validate with:

```sh
cargo run --locked -p xtask -- image validate {}/workload-image.yaml
```

Before publishing a package-04 handoff, run:

```sh
cargo run --locked -p xtask -- image double-build
cargo run --locked -p xtask -- image register --manifest {}/workload-image.yaml
```

`image register` is a direct `dist/` handoff/no-op until control-plane artifact
registration exists. Once package 06 green-stamp support lands, registration
refuses unstamped manifests.
"#,
        path.parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or("dist/workload-image"),
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
    for (key, required_file) in [("kernel", "bzImage"), ("initramfs", "initramfs.cpio.zst")] {
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
        if file != required_file {
            errors.push(format!(
                "artifacts.{key}.file must be {required_file}, got {file}"
            ));
            continue;
        }
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
    validate_cmdline(cmdline, errors);
}

fn validate_machine(root: Option<&Mapping>, variant: ManifestVariant, errors: &mut Vec<String>) {
    let Some(machine) = child_map(root, "machine", "machine", errors) else {
        return;
    };
    expect_u64(machine, "vcpus", 1, "machine.vcpus", errors);
    match variant {
        ManifestVariant::Game => expect_u64(machine, "mem_mib", 128, "machine.mem_mib", errors),
        ManifestVariant::Generic => {
            positive_u64_field(machine, "mem_mib", "machine.mem_mib", errors);
        }
    }
    let Some(devices) = seq_field(machine, "devices", "machine.devices", errors) else {
        return;
    };
    if variant == ManifestVariant::Game {
        expect_device(
            devices,
            DeviceSpec {
                kind: "virtio-blk",
                role: Some("game-image"),
                readonly: Some(true),
                required: true,
            },
            errors,
        );
    }
    expect_device(
        devices,
        DeviceSpec {
            kind: "detguest-channel",
            role: None,
            readonly: None,
            required: true,
        },
        errors,
    );
    expect_device(
        devices,
        DeviceSpec {
            kind: "pv-pad",
            role: None,
            readonly: None,
            required: true,
        },
        errors,
    );
    if variant == ManifestVariant::Generic {
        let mut seen = std::collections::HashSet::new();
        for device in devices {
            let Some(map) = mapping(device, "machine.devices[]", errors) else {
                continue;
            };
            let Some(kind) = string_field(map, "kind", "machine.devices[].kind", errors) else {
                continue;
            };
            if kind != "detguest-channel" && kind != "pv-pad" {
                errors.push(format!(
                    "machine.devices unsupported kind {kind} for variant generic"
                ));
            }
            if !seen.insert(kind) {
                errors.push(format!("machine.devices duplicate kind {kind}"));
            }
        }
    }
}

struct DeviceSpec {
    kind: &'static str,
    role: Option<&'static str>,
    readonly: Option<bool>,
    required: bool,
}

fn expect_device(devices: &[Value], spec: DeviceSpec, errors: &mut Vec<String>) {
    let mut found = false;
    for device in devices {
        let Some(map) = mapping(device, "machine.devices[]", errors) else {
            continue;
        };
        let Some(kind) = string_field(map, "kind", "machine.devices[].kind", errors) else {
            continue;
        };
        if kind != spec.kind {
            continue;
        }
        found = true;
        if let Some(role) = spec.role {
            expect_string(
                map,
                "role",
                role,
                &format!("machine.devices.{}.role", spec.kind),
                errors,
            );
        }
        if let Some(readonly) = spec.readonly {
            expect_bool(
                map,
                "readonly",
                readonly,
                &format!("machine.devices.{}.readonly", spec.kind),
                errors,
            );
        }
        expect_bool(
            map,
            "required",
            spec.required,
            &format!("machine.devices.{}.required", spec.kind),
            errors,
        );
    }
    if !found {
        errors.push(format!("machine.devices missing {}", spec.kind));
    }
}

fn validate_cmdline(cmdline: &str, errors: &mut Vec<String>) {
    if !cmdline.is_ascii() || cmdline.contains('\0') {
        errors.push("boot.cmdline must be ASCII text without NUL bytes".into());
        return;
    }
    if cmdline.trim() != cmdline || cmdline.contains("  ") {
        errors.push("boot.cmdline must use single spaces without leading/trailing space".into());
        return;
    }

    let mut quiet_seen = false;
    let mut loglevel_seen = false;
    for token in cmdline.split(' ') {
        match token {
            "quiet" if !quiet_seen => quiet_seen = true,
            "quiet" => errors.push("boot.cmdline duplicates quiet".into()),
            value if value.starts_with("loglevel=") && !loglevel_seen => {
                loglevel_seen = true;
                let level = value.trim_start_matches("loglevel=");
                match level.parse::<u8>() {
                    Ok(0..=7) => {}
                    _ => errors.push(format!("boot.cmdline has invalid loglevel token {value}")),
                }
            }
            value if value.starts_with("loglevel=") => {
                errors.push(format!("boot.cmdline duplicates loglevel token {value}"))
            }
            "" => errors.push("boot.cmdline contains an empty token".into()),
            other => errors.push(format!("boot.cmdline token {other:?} is not whitelisted")),
        }
    }
}

fn validate_regions(root: Option<&Mapping>, variant: ManifestVariant, errors: &mut Vec<String>) {
    let Some(root) = root else {
        return;
    };
    let Some(regions) = seq_field(root, "regions", "regions", errors) else {
        return;
    };

    if variant == ManifestVariant::Generic {
        if regions.is_empty() {
            errors.push("regions must be a non-empty list".into());
        }
        let mut seen = std::collections::HashSet::new();
        for value in regions {
            let Some(region) = mapping(value, "regions[]", errors) else {
                continue;
            };
            let Some(name) = string_field(region, "name", "regions[].name", errors) else {
                continue;
            };
            if name.is_empty() {
                errors.push("regions[].name must not be empty".into());
            }
            if !seen.insert(name) {
                errors.push(format!("regions duplicate name {name}"));
            }
            let optional = if has_key(region, "optional") {
                bool_field(
                    region,
                    "optional",
                    &format!("regions.{name}.optional"),
                    errors,
                )
            } else {
                None
            };
            let size_path = format!("regions.{name}.size");
            if let Some(size) = u64_field(region, "size", &size_path, errors) {
                if size % 4096 != 0 || (size == 0 && optional != Some(true)) {
                    errors.push(format!(
                        "{size_path} must be a multiple of 4096 (zero requires optional: true)"
                    ));
                }
            }
            if name == "framebuffer" {
                expect_nonempty_string(region, "format", "regions.framebuffer.format", errors);
            }
            if has_key(region, "layout_version") {
                errors.push(format!(
                    "regions.{name} must not carry guest-sdk layout_version in workload-image.yaml"
                ));
            }
        }
        return;
    }
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

fn validate_fps(root: Option<&Mapping>, variant: ManifestVariant, errors: &mut Vec<String>) {
    let Some(fps) = child_map(root, "fps", "fps", errors) else {
        return;
    };
    match variant {
        ManifestVariant::Game => {
            expect_u64(fps, "num", FPS_NUM, "fps.num", errors);
            expect_u64(fps, "den", FPS_DEN, "fps.den", errors);
        }
        ManifestVariant::Generic => {
            for key in ["num", "den"] {
                let path = format!("fps.{key}");
                positive_u64_field(fps, key, &path, errors);
            }
        }
    }
}

fn validate_pad_layout(root: Option<&Mapping>, errors: &mut Vec<String>) {
    let Some(pad) = child_map(root, "pad_layout", "pad_layout", errors) else {
        return;
    };
    expect_string(
        pad,
        "layout_id",
        PAD_LAYOUT_ID,
        "pad_layout.layout_id",
        errors,
    );
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

fn validate_defaults(root: Option<&Mapping>, variant: ManifestVariant, errors: &mut Vec<String>) {
    if variant == ManifestVariant::Generic && !root.is_some_and(|r| has_key(r, "defaults")) {
        return;
    }
    let Some(defaults) = child_map(root, "defaults", "defaults", errors) else {
        return;
    };
    if variant == ManifestVariant::Generic {
        for key in ["feature_map", "scoring_program"] {
            let path = format!("defaults.{key}");
            if let Some(value) = string_field(defaults, key, &path, errors) {
                let mut parts = value.split('@');
                if !matches!((parts.next(), parts.next(), parts.next()), (Some(name), Some(version), None) if !name.is_empty() && !version.is_empty())
                {
                    errors.push(format!("{path} must be non-empty name@version"));
                }
            }
        }
        return;
    }
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

fn validate_handoff_files(
    base: &Path,
    root: Option<&Mapping>,
    manifest_blake3: Option<&str>,
    errors: &mut Vec<String>,
) {
    for rel in [
        "boot.toml",
        "harness.toml",
        "expected-regions.toml",
        "README.md",
    ] {
        if !base.join(rel).is_file() {
            errors.push(format!("missing dist handoff file {rel}"));
        }
    }
    let unstamped = base.join(UNSTAMPED_FILE);
    let green_stamp = base.join(GREEN_STAMP_FILE);
    match (unstamped.is_file(), green_stamp.is_file()) {
        (true, false) => {}
        (false, true) => {
            validate_green_stamp_sidecar(
                &green_stamp,
                root,
                manifest_blake3,
                ManifestVariant::Game,
                errors,
            );
        }
        (true, true) => errors.push(format!(
            "{UNSTAMPED_FILE} must not coexist with {GREEN_STAMP_FILE}"
        )),
        (false, false) => errors.push(format!(
            "missing determinism sidecar {UNSTAMPED_FILE} or {GREEN_STAMP_FILE}"
        )),
    }

    match read_to_string(&base.join("expected-regions.toml")) {
        Ok(content) => validate_expected_regions_toml(&content, errors),
        Err(err) => errors.push(format!("cannot read expected-regions.toml: {err}")),
    }
    match read_to_string(&base.join("boot.toml")) {
        Ok(content) => validate_boot_toml(&content, errors),
        Err(err) => errors.push(format!("cannot read boot.toml: {err}")),
    }
    match read_to_string(&base.join("harness.toml")) {
        Ok(content) => validate_harness_toml(&content, errors),
        Err(err) => errors.push(format!("cannot read harness.toml: {err}")),
    }
}

fn validate_generic_handoff_files(
    base: &Path,
    root: Option<&Mapping>,
    manifest_blake3: Option<&str>,
    errors: &mut Vec<String>,
) {
    for rel in ["boot.toml", "expected-regions.toml", "README.md"] {
        if !base.join(rel).is_file() {
            errors.push(format!("missing dist handoff file {rel}"));
        }
    }
    let unstamped = base.join(UNSTAMPED_FILE);
    let green_stamp = base.join(GREEN_STAMP_FILE);
    match (unstamped.is_file(), green_stamp.is_file()) {
        (true, false) => validate_generic_unstamped(&unstamped, root, errors),
        (false, true) => validate_green_stamp_sidecar(
            &green_stamp,
            root,
            manifest_blake3,
            ManifestVariant::Generic,
            errors,
        ),
        (true, true) => errors.push(format!(
            "{UNSTAMPED_FILE} must not coexist with {GREEN_STAMP_FILE}"
        )),
        (false, false) => errors.push(format!(
            "missing determinism sidecar {UNSTAMPED_FILE} or {GREEN_STAMP_FILE}"
        )),
    }
    let regions = manifest_region_sizes(root);
    let boot = match read_to_string(&base.join("boot.toml")) {
        Ok(content) => Some(strip_toml_comments(&content)),
        Err(err) => {
            errors.push(format!("cannot read boot.toml: {err}"));
            None
        }
    };
    let expected = match read_to_string(&base.join("expected-regions.toml")) {
        Ok(content) => Some(strip_toml_comments(&content)),
        Err(err) => {
            errors.push(format!("cannot read expected-regions.toml: {err}"));
            None
        }
    };
    let mut boot_layouts = std::collections::HashMap::new();
    if let Some(boot) = boot.as_deref() {
        if field_u64(boot, "boot_toml_version") != Some(1) {
            errors.push("boot.toml boot_toml_version must be 1".into());
        }
        if toml_table_block(boot, "autostart").is_none() {
            errors.push("boot.toml missing [autostart]".into());
        }
        let units = toml_array_blocks(boot, "unit");
        if units.is_empty() {
            errors.push("boot.toml missing [[unit]]".into());
        }
        if !units
            .iter()
            .any(|unit| field_string(unit, "exec").is_some_and(|s| !s.is_empty()))
        {
            errors.push("boot.toml unit.exec must be non-empty".into());
        }
        let expected_regions = toml_array_blocks(boot, "expected_region");
        if expected_regions.is_empty() {
            errors.push("boot.toml must declare at least one [[expected_region]]".into());
        }
        for block in expected_regions {
            if let Some(name) = field_string(block, "name") {
                if !regions.contains_key(&name) {
                    errors.push(format!(
                        "boot.toml expected_region {name} is not declared in regions"
                    ));
                }
                boot_layouts.insert(name, field_u64(block, "layout_version"));
            } else {
                errors.push("boot.toml expected_region.name must be non-empty".into());
            }
        }
    }
    if let Some(expected) = expected.as_deref() {
        if field_u64(expected, "schema_version") != Some(1) {
            errors.push("expected-regions.toml schema_version must be 1".into());
        }
        for block in toml_array_blocks(expected, "regions") {
            let Some(name) = field_string(block, "name") else {
                errors.push("expected-regions.toml regions.name must be non-empty".into());
                continue;
            };
            match regions.get(&name) {
                Some(size) => {
                    if let Some(actual) = field_u64(block, "size") {
                        if actual != *size {
                            errors.push(format!("expected-regions.toml {name} size {actual} != manifest size {size}"));
                        }
                    } else {
                        errors.push(format!(
                            "expected-regions.toml {name} size missing or invalid"
                        ));
                    }
                }
                None => errors.push(format!(
                    "expected-regions.toml {name} is not declared in regions"
                )),
            }
            if let Some(Some(boot_layout)) = boot_layouts.get(&name) {
                let actual = field_u64(block, "layout_version");
                if actual != Some(*boot_layout) {
                    errors.push(format!(
                        "expected-regions.toml {name} layout_version {} != boot.toml {boot_layout}",
                        actual.map_or("missing".into(), |v| v.to_string())
                    ));
                }
            }
        }
    }
}

fn validate_generic_unstamped(path: &Path, root: Option<&Mapping>, errors: &mut Vec<String>) {
    let content = match read_to_string(path) {
        Ok(content) => content,
        Err(err) => {
            errors.push(format!("cannot read {UNSTAMPED_FILE}: {err}"));
            return;
        }
    };
    let yaml: Value = match serde_yaml::from_str(&content) {
        Ok(yaml) => yaml,
        Err(err) => {
            errors.push(format!("{UNSTAMPED_FILE} yaml parse failed: {err}"));
            return;
        }
    };
    let Some(sidecar) = mapping(&yaml, UNSTAMPED_FILE, errors) else {
        return;
    };
    expect_string(
        sidecar,
        "kind",
        "determinism-unstamped",
        "determinism.unstamped.kind",
        errors,
    );
    if let Some(image_ref) = manifest_image_ref(root) {
        expect_string(
            sidecar,
            "workload_image",
            &image_ref,
            "determinism.unstamped.workload_image",
            errors,
        );
    }
    if let Some(rev) = manifest_built_from(root)
        .and_then(|built_from| built_from.get(Value::String("git_rev".into())))
        .and_then(Value::as_str)
    {
        expect_string(
            sidecar,
            "git_rev",
            rev,
            "determinism.unstamped.git_rev",
            errors,
        );
    }
}

fn manifest_image_ref(root: Option<&Mapping>) -> Option<String> {
    let meta = root?.get(Value::String("meta".into()))?.as_mapping()?;
    let name = meta.get(Value::String("name".into()))?.as_str()?;
    let version = meta.get(Value::String("version".into()))?.as_str()?;
    Some(format!("{name}@{version}"))
}

fn manifest_region_sizes(root: Option<&Mapping>) -> std::collections::HashMap<String, u64> {
    root.and_then(|root| root.get(Value::String("regions".into())))
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(|value| {
            let region = value.as_mapping()?;
            let name = region.get(Value::String("name".into()))?.as_str()?;
            let size = region.get(Value::String("size".into()))?.as_u64()?;
            Some((name.to_owned(), size))
        })
        .collect()
}

fn validate_green_stamp_sidecar(
    path: &Path,
    root: Option<&Mapping>,
    manifest_blake3: Option<&str>,
    variant: ManifestVariant,
    errors: &mut Vec<String>,
) {
    let content = match read_to_string(path) {
        Ok(content) => content,
        Err(err) => {
            errors.push(format!("cannot read {GREEN_STAMP_FILE}: {err}"));
            return;
        }
    };
    let yaml: Value = match serde_yaml::from_str(&content) {
        Ok(yaml) => yaml,
        Err(err) => {
            errors.push(format!("{GREEN_STAMP_FILE} yaml parse failed: {err}"));
            return;
        }
    };
    let Some(stamp) = mapping(&yaml, GREEN_STAMP_FILE, errors) else {
        return;
    };

    expect_u64(
        stamp,
        "schema_version",
        1,
        "determinism.last_green.schema_version",
        errors,
    );
    expect_string(
        stamp,
        "kind",
        "determinism-last-green",
        "determinism.last_green.kind",
        errors,
    );
    let image_ref = match variant {
        ManifestVariant::Game => Some(format!("{WORKLOAD_NAME}@{VERSION}")),
        ManifestVariant::Generic => manifest_image_ref(root),
    };
    if let Some(image_ref) = image_ref {
        expect_string(
            stamp,
            "workload_image",
            &image_ref,
            "determinism.last_green.workload_image",
            errors,
        );
    }

    if let Some(manifest_blake3) = manifest_blake3 {
        expect_string(
            stamp,
            "image_manifest_hash",
            manifest_blake3,
            "determinism.last_green.image_manifest_hash",
            errors,
        );
    }
    let git_rev = match variant {
        ManifestVariant::Game => manifest_git_rev(root, errors),
        ManifestVariant::Generic => manifest_built_from(root)
            .and_then(|built_from| built_from.get(Value::String("git_rev".into())))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    };
    if let Some(git_rev) = git_rev {
        let rev_key = if variant == ManifestVariant::Game {
            "reference_workload_git_rev"
        } else {
            "git_rev"
        };
        expect_string(
            stamp,
            rev_key,
            &git_rev,
            &format!("determinism.last_green.{rev_key}"),
            errors,
        );
    }
    // The stamp must name the emulator epoch the manifest was built at (a
    // stamp without the key is refused: `workload_image` above already pins
    // the stamp to this bundle version, so no pre-0.2.0 stamp reaches here).
    if let Some(emu_version) = (variant == ManifestVariant::Game)
        .then(|| manifest_emu_version(root))
        .flatten()
    {
        expect_string(
            stamp,
            "emu_version",
            &emu_version,
            "determinism.last_green.emu_version",
            errors,
        );
    }

    expect_nonempty_string(
        stamp,
        "suite_version",
        "determinism.last_green.suite_version",
        errors,
    );
    expect_nonempty_string(
        stamp,
        "timestamp",
        "determinism.last_green.timestamp",
        errors,
    );
    if let Some(report_hash) = string_field(
        stamp,
        "suite_report_blake3",
        "determinism.last_green.suite_report_blake3",
        errors,
    ) {
        if !is_blake3_hex(report_hash) {
            errors.push(
                "determinism.last_green.suite_report_blake3 must be 64 lowercase hex chars".into(),
            );
        }
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

fn validate_boot_toml(content: &str, errors: &mut Vec<String>) {
    // The agent's real schema (guest-sdk API.md §7.1): boot_toml_version,
    // [autostart].unit, [[unit]] with exec + [unit.control] refwork-ctl,
    // and the READY gate as [[expected_region]] name + layout_version pairs.
    if field_u64(content, "boot_toml_version") != Some(1) {
        errors.push("boot.toml boot_toml_version must be 1".into());
    }
    let Some(autostart) = table_block(content, "[autostart]") else {
        errors.push("boot.toml missing [autostart]".into());
        return;
    };
    if field_u64(autostart, "unit") != Some(0) {
        errors.push("boot.toml autostart.unit must be 0".into());
    }
    let Some(unit) = table_block(content, "[[unit]]") else {
        errors.push("boot.toml missing [[unit]]".into());
        return;
    };
    if field_u64(unit, "id") != Some(0) {
        errors.push("boot.toml unit.id must be 0".into());
    }
    if field_string(unit, "exec").as_deref() != Some("/usr/bin/refwork-harness") {
        errors.push("boot.toml unit.exec must be /usr/bin/refwork-harness".into());
    }
    let Some(control) = table_block(content, "[unit.control]") else {
        errors.push("boot.toml missing [unit.control]".into());
        return;
    };
    if field_string(control, "protocol").as_deref() != Some("refwork-ctl") {
        errors.push("boot.toml unit.control.protocol must be refwork-ctl".into());
    }
    if field_u64(control, "proto_version") != Some(1) {
        errors.push("boot.toml unit.control.proto_version must be 1".into());
    }
    if field_string(control, "game_dev").as_deref() != Some("/dev/vdb") {
        errors.push("boot.toml unit.control.game_dev must be /dev/vdb".into());
    }
    if field_string(control, "game_source").as_deref() != Some("pv-blk") {
        errors.push("boot.toml unit.control.game_source must be pv-blk".into());
    }
    for spec in REQUIRED_REGIONS {
        let Some(block) = region_array_block(content, "expected_region", spec.name) else {
            errors.push(format!(
                "boot.toml missing [[expected_region]] {}",
                spec.name
            ));
            continue;
        };
        if field_u64(block, "layout_version") != Some(spec.layout_version) {
            errors.push(format!(
                "boot.toml expected_region {} layout_version must be {}",
                spec.name, spec.layout_version
            ));
        }
    }
}

fn validate_harness_toml(content: &str, errors: &mut Vec<String>) {
    if field_u64(content, "schema_version") != Some(1) {
        errors.push("harness.toml schema_version must be 1".into());
    }
    if field_string(content, "schema_owner").as_deref() != Some("reference-workload") {
        errors.push("harness.toml schema_owner must be reference-workload".into());
    }
    let Some(harness) = table_block(content, "[harness]") else {
        errors.push("harness.toml missing [harness]".into());
        return;
    };
    if field_string(harness, "binary").as_deref() != Some("/usr/bin/refwork-harness") {
        errors.push("harness.toml harness.binary must be /usr/bin/refwork-harness".into());
    }
    if field_u64(harness, "control_fd") != Some(3) {
        errors.push("harness.toml harness.control_fd must be 3".into());
    }
    if field_string(harness, "game_image_device").as_deref() != Some("/dev/vdb") {
        errors.push("harness.toml harness.game_image_device must be /dev/vdb".into());
    }
    if field_u64(harness, "protocol_version") != Some(1) {
        errors.push("harness.toml harness.protocol_version must be 1".into());
    }
    let Some(regions) = table_block(content, "[regions]") else {
        errors.push("harness.toml missing [regions]".into());
        return;
    };
    if !regions.contains("required = [\"wram\", \"framebuffer\", \"meta\"]") {
        errors.push("harness.toml regions.required must list wram/framebuffer/meta".into());
    }
    if field_bool_toml(regions, "publish_vram") != Some(false) {
        errors.push("harness.toml regions.publish_vram must be false".into());
    }
    if field_string(regions, "publish_sram").as_deref() != Some("cart-dependent") {
        errors.push("harness.toml regions.publish_sram must be cart-dependent".into());
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

fn positive_u64_field(
    parent: &Mapping,
    key: &str,
    path: &str,
    errors: &mut Vec<String>,
) -> Option<u64> {
    match parent
        .get(Value::String(key.into()))
        .and_then(Value::as_u64)
    {
        Some(value @ 1..) => Some(value),
        _ => {
            errors.push(format!("{path} must be an integer >= 1"));
            None
        }
    }
}

fn bool_field(parent: &Mapping, key: &str, path: &str, errors: &mut Vec<String>) -> Option<bool> {
    let Some(value) = parent.get(Value::String(key.into())) else {
        errors.push(format!("missing {path}"));
        return None;
    };
    match value {
        Value::Bool(value) => Some(*value),
        _ => {
            errors.push(format!("{path} must be a boolean"));
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

fn expect_nonempty_string(parent: &Mapping, key: &str, path: &str, errors: &mut Vec<String>) {
    if let Some(actual) = string_field(parent, key, path, errors) {
        if actual.is_empty() {
            errors.push(format!("{path} must not be empty"));
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

fn expect_bool(parent: &Mapping, key: &str, expected: bool, path: &str, errors: &mut Vec<String>) {
    if let Some(actual) = bool_field(parent, key, path, errors) {
        if actual != expected {
            errors.push(format!("{path} expected {expected}, got {actual}"));
        }
    }
}

fn has_key(parent: &Mapping, key: &str) -> bool {
    parent.contains_key(Value::String(key.into()))
}

/// `meta.built_from` without reporting: `validate_manifest` already reports a
/// missing `meta` / `meta.built_from` once, and the stamp cross-checks below
/// must not repeat that error per field they read.
fn manifest_built_from(root: Option<&Mapping>) -> Option<&Mapping> {
    let mut ignored = Vec::new();
    let meta = child_map(root, "meta", "meta", &mut ignored)?;
    child_map(Some(meta), "built_from", "meta.built_from", &mut ignored)
}

fn manifest_git_rev(root: Option<&Mapping>, errors: &mut Vec<String>) -> Option<String> {
    let built_from = manifest_built_from(root)?;
    string_field(built_from, "git_rev", "meta.built_from.git_rev", errors).map(ToOwned::to_owned)
}

/// Read-only: a missing or empty `emu_version` is `validate_manifest`'s error
/// to report, so the stamp cross-check just skips when it is absent.
fn manifest_emu_version(root: Option<&Mapping>) -> Option<String> {
    let built_from = manifest_built_from(root)?;
    let mut ignored = Vec::new();
    string_field(
        built_from,
        "emu_version",
        "meta.built_from.emu_version",
        &mut ignored,
    )
    .map(ToOwned::to_owned)
}

fn is_blake3_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn is_git_rev_hex(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn strip_toml_comments(content: &str) -> String {
    content
        .lines()
        .map(|line| {
            let mut in_string = false;
            let mut escaped = false;
            for (offset, ch) in line.char_indices() {
                if ch == '"' && !escaped {
                    in_string = !in_string;
                }
                if ch == '#' && !in_string {
                    return &line[..offset];
                }
                escaped = ch == '\\' && !escaped;
                if ch != '\\' {
                    escaped = false;
                }
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn toml_table_block<'a>(content: &'a str, table: &str) -> Option<&'a str> {
    let header = format!("[{table}]");
    let mut start = None;
    for (offset, line) in content.split_inclusive('\n').scan(0, |position, line| {
        let offset = *position;
        *position += line.len();
        Some((offset, line))
    }) {
        if line.trim() == header {
            start = Some(offset + line.len());
            continue;
        }
        if let Some(previous) = start {
            if line.trim_start().starts_with('[') {
                return Some(&content[previous..offset]);
            }
        }
    }
    start.map(|previous| &content[previous..])
}

fn toml_array_blocks<'a>(content: &'a str, array: &str) -> Vec<&'a str> {
    let header = format!("[[{array}]]");
    let mut blocks = Vec::new();
    let mut start = None;
    for (offset, line) in content.split_inclusive('\n').scan(0, |position, line| {
        let offset = *position;
        *position += line.len();
        Some((offset, line))
    }) {
        if line.trim_start().starts_with('[') {
            if let Some(previous) = start.take() {
                blocks.push(&content[previous..offset]);
            }
            if line.trim() == header {
                start = Some(offset + line.len());
            }
        }
    }
    if let Some(previous) = start {
        blocks.push(&content[previous..]);
    }
    blocks
}

fn region_block<'a>(content: &'a str, name: &str) -> Option<&'a str> {
    region_array_block(content, "regions", name)
}

/// Find the `[[array]]` entry whose `name` field matches.
fn region_array_block<'a>(content: &'a str, array: &str, name: &str) -> Option<&'a str> {
    let header = format!("[[{array}]]");
    content
        .split(header.as_str())
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

fn field_bool_toml(block: &str, key: &str) -> Option<bool> {
    field_value(block, key)?.parse().ok()
}

fn field_value<'a>(block: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key} = ");
    block
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(&prefix))
}

fn table_block<'a>(content: &'a str, table: &str) -> Option<&'a str> {
    let start = content.find(table)?;
    let rest = &content[start + table.len()..];
    let end = rest
        .find("\n[")
        .or_else(|| rest.find("\n[["))
        .unwrap_or(rest.len());
    Some(&rest[..end])
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

    /// Gap B negative test: a kernel artifact whose bytes do not match the
    /// lock's BLAKE3 must be refused, naming both hashes and the bump
    /// guidance. (Fails if the mismatch branch is removed.)
    #[test]
    fn kernel_pin_refuses_wrong_content() {
        let tmp = TempDir::new();
        let bzimage = tmp.path.join("bzImage");
        std::fs::write(&bzimage, b"not the pinned kernel").unwrap();
        let pinned = "0000000000000000000000000000000000000000000000000000000000000000";

        let err = verify_pinned_kernel_artifact(&bzimage, pinned)
            .expect_err("wrong-content kernel must be refused");
        let msg = err.to_string();
        let actual = blake3::hash(b"not the pinned kernel").to_hex().to_string();
        assert!(msg.contains(pinned), "must name the pinned hash: {msg}");
        assert!(msg.contains(&actual), "must name the actual hash: {msg}");
        assert!(
            msg.contains("rebuild the pinned kernel in guest-sdk")
                && msg.contains("deliberately bump the lock"),
            "must carry the bump guidance: {msg}"
        );
    }

    /// Gap B negative test: a checkout at a different rev than the lock's
    /// pin must be refused, naming both revs. Uses a scratch git repo —
    /// never the real sibling checkouts.
    #[test]
    fn rev_pin_refuses_wrong_checkout() {
        let tmp = TempDir::new();
        let repo = tmp.path.join("scratch-repo");
        std::fs::create_dir_all(&repo).unwrap();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@t")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@t")
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}: {out:?}");
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        };
        git(&["init", "-q"]);
        git(&["commit", "-q", "--allow-empty", "-m", "scratch"]);
        let head = git(&["rev-parse", "HEAD"]);
        let pinned = "1111111111111111111111111111111111111111";

        verify_pinned_rev(&repo, &head).expect("matching rev must pass");
        let err = verify_pinned_rev(&repo, pinned).expect_err("wrong rev must be refused");
        let msg = err.to_string();
        assert!(msg.contains(&head), "must name the checkout rev: {msg}");
        assert!(msg.contains(pinned), "must name the pinned rev: {msg}");
        assert!(
            msg.contains("deliberately bump the lock"),
            "must carry the bump guidance: {msg}"
        );
    }

    fn valid_dist() -> TempDir {
        let tmp = TempDir::new();
        std::fs::write(tmp.path.join("bzImage"), b"kernel").unwrap();
        std::fs::write(tmp.path.join("initramfs.cpio.zst"), b"initramfs").unwrap();
        std::fs::write(
            tmp.path.join("boot.toml"),
            include_bytes!("../../image/boot.toml"),
        )
        .unwrap();
        std::fs::write(
            tmp.path.join("harness.toml"),
            include_bytes!("../../image/harness.toml"),
        )
        .unwrap();
        std::fs::write(
            tmp.path.join("expected-regions.toml"),
            include_bytes!("../../image/expected-regions.toml"),
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
            refwork_emu::EMU_VERSION,
            &kernel_hash,
            &initramfs_hash,
        )
        .unwrap();
        tmp
    }

    fn green_stamp_text(tmp: &TempDir) -> String {
        std::fs::read_to_string(tmp.path.join("determinism.last_green")).unwrap()
    }

    fn append_green_stamp_line(tmp: &TempDir, line: &str) {
        let text = format!("{}{line}\n", green_stamp_text(tmp));
        std::fs::write(tmp.path.join("determinism.last_green"), text).unwrap();
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
    fn game_error_vectors_are_unchanged() {
        const GAME_DEVICE: &str =
            "    - { kind: virtio-blk, role: game-image, readonly: true, required: true }\n";
        let cases: [(&str, &str, &str, &[&str]); 8] = [
            (
                "vcpus",
                "  vcpus: 1",
                "  vcpus: 2",
                &["machine.vcpus expected 1, got 2"],
            ),
            (
                "artifact",
                "file: bzImage",
                "file: kernel.payload",
                &["artifacts.kernel.file must be bzImage, got kernel.payload"],
            ),
            (
                "cmdline",
                "cmdline: \"quiet\"",
                "cmdline: \"quiet root=/dev/vda\"",
                &["boot.cmdline token \"root=/dev/vda\" is not whitelisted"],
            ),
            (
                "device",
                GAME_DEVICE,
                "    - { kind: virtio-blk, role: host-root, readonly: false, required: false }\n",
                &[
                    "machine.devices.virtio-blk.role expected \"game-image\", got \"host-root\"",
                    "machine.devices.virtio-blk.readonly expected true, got false",
                    "machine.devices.virtio-blk.required expected true, got false",
                ],
            ),
            (
                "fps",
                "  num: 21477272",
                "  num: 60.0",
                &["fps.num must be an unsigned integer"],
            ),
            (
                "pad",
                "{ name: Start, bit: 10 }",
                "{ name: Start, bit: 9 }",
                &["pad_layout.buttons[10].bit expected 10, got 9"],
            ),
            (
                "layout",
                "- { name: wram, size: 131072 }",
                "- { name: wram, size: 131072, layout_version: 1 }",
                &["regions.wram must not carry guest-sdk layout_version in workload-image.yaml"],
            ),
            (
                "missing-game-image",
                GAME_DEVICE,
                "",
                &["machine.devices missing virtio-blk"],
            ),
        ];
        for (name, before, after, expected) in cases {
            let tmp = valid_dist();
            let text = manifest_text(&tmp);
            assert!(text.contains(before), "{name} fixture drift");
            write_manifest_text(&tmp, &text.replace(before, after));
            let errors = validation_errors(&tmp);
            assert_eq!(errors, expected, "{name}");
        }
        let tmp = valid_dist();
        std::fs::remove_file(tmp.path.join(UNSTAMPED_FILE)).unwrap();
        assert_eq!(
            validation_errors(&tmp),
            ["missing determinism sidecar determinism.unstamped.yaml or determinism.last_green"]
        );

        let tmp = valid_dist();
        write_manifest_text(
            &tmp,
            &manifest_text(&tmp)
                .replace("name: refwork-demo", "name: other")
                .replace(GAME_DEVICE, ""),
        );
        assert_eq!(
            validation_errors(&tmp),
            [
                "meta.name expected \"refwork-demo\", got \"other\"",
                "machine.devices missing virtio-blk",
            ]
        );

        let tmp = valid_dist();
        write_manifest_text(
            &tmp,
            &manifest_text(&tmp)
                .replace("  vcpus: 1", "  vcpus: 2")
                .replace("  layout_id: console16-12btn-v1\n", ""),
        );
        assert_eq!(
            validation_errors(&tmp),
            [
                "machine.vcpus expected 1, got 2",
                "missing pad_layout.layout_id",
            ]
        );
    }

    fn generic_fixture_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../image/fixtures/generic")
    }

    fn generic_fixture_copy() -> TempDir {
        let tmp = TempDir::new();
        for entry in std::fs::read_dir(generic_fixture_dir()).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                std::fs::copy(entry.path(), tmp.path.join(entry.file_name())).unwrap();
            }
        }
        tmp
    }

    fn generic_error_after_manifest_replace(before: &str, after: &str) -> Vec<String> {
        let tmp = generic_fixture_copy();
        let original = manifest_text(&tmp);
        assert!(original.contains(before), "fixture missing {before:?}");
        write_manifest_text(&tmp, &original.replacen(before, after, 1));
        validation_errors(&tmp)
    }

    macro_rules! generic_manifest_rejection {
        ($name:ident, $before:expr, $after:expr, $error:expr) => {
            #[test]
            fn $name() {
                let errors = generic_error_after_manifest_replace($before, $after);
                assert!(errors.iter().any(|e| e.contains($error)), "{errors:?}");
            }
        };
    }

    #[test]
    fn generic_fixture_validates_in_place() {
        validate_manifest(&generic_fixture_dir().join("workload-image.yaml")).unwrap();
    }
    #[test]
    fn generic_fixture_copy_validates() {
        let tmp = generic_fixture_copy();
        validate_manifest(&tmp.path.join("workload-image.yaml")).unwrap();
    }
    #[test]
    fn game_manifest_without_game_image_rejected() {
        let tmp = valid_dist();
        let text = manifest_text(&tmp).replace(
            "    - { kind: virtio-blk, role: game-image, readonly: true, required: true }\n",
            "",
        );
        write_manifest_text(&tmp, &text);
        assert!(validation_errors(&tmp)
            .iter()
            .any(|e| e.contains("machine.devices missing virtio-blk")));
    }
    #[test]
    fn explicit_variant_game_without_game_image_rejected() {
        let tmp = valid_dist();
        let text = manifest_text(&tmp)
            .replace(
                "kind: workload-image\n",
                "kind: workload-image\nvariant: game\n",
            )
            .replace(
                "    - { kind: virtio-blk, role: game-image, readonly: true, required: true }\n",
                "",
            );
        write_manifest_text(&tmp, &text);
        assert!(validation_errors(&tmp)
            .iter()
            .any(|e| e.contains("machine.devices missing virtio-blk")));
    }

    generic_manifest_rejection!(
        generic_with_virtio_blk_rejected,
        "    - { kind: pv-pad, required: true }",
        "    - { kind: pv-pad, required: true }\n    - { kind: virtio-blk, required: true }",
        "machine.devices unsupported kind virtio-blk"
    );
    generic_manifest_rejection!(
        generic_with_pv_net_rejected,
        "    - { kind: pv-pad, required: true }",
        "    - { kind: pv-pad, required: true }\n    - { kind: pv-net, required: true }",
        "machine.devices unsupported kind pv-net"
    );
    generic_manifest_rejection!(
        generic_duplicate_pv_pad_rejected,
        "    - { kind: pv-pad, required: true }",
        "    - { kind: pv-pad, required: true }\n    - { kind: pv-pad, required: true }",
        "machine.devices duplicate kind pv-pad"
    );
    generic_manifest_rejection!(
        generic_missing_pv_pad_rejected,
        "    - { kind: pv-pad, required: true }\n",
        "",
        "machine.devices missing pv-pad"
    );
    generic_manifest_rejection!(
        generic_missing_detguest_channel_rejected,
        "    - { kind: detguest-channel, required: true }\n",
        "",
        "machine.devices missing detguest-channel"
    );
    generic_manifest_rejection!(
        generic_name_charset_enforced,
        "name: generic-fixture",
        "name: Generic_Fixture",
        "meta.name"
    );
    generic_manifest_rejection!(
        generic_version_rejects_at_sign,
        "version: \"0.1.0\"",
        "version: \"0.1.0@x\"",
        "meta.version"
    );
    generic_manifest_rejection!(
        generic_git_rev_must_be_40_hex,
        "git_rev: \"0000000000000000000000000000000000000000\"",
        "git_rev: \"000000000000000000000000000000000000000\"",
        "meta.built_from.git_rev"
    );
    generic_manifest_rejection!(
        generic_requires_pad_layout_identity,
        "name: Up, bit: 6",
        "name: UP, bit: 6",
        "pad_layout.buttons[6].name"
    );
    generic_manifest_rejection!(
        generic_fps_must_be_integer_rational,
        "fps: { num: 60, den: 1 }",
        "fps: { num: 60.0, den: 1 }",
        "fps.num"
    );
    generic_manifest_rejection!(
        generic_fps_zero_den_rejected,
        "fps: { num: 60, den: 1 }",
        "fps: { num: 60, den: 0 }",
        "fps.den must be an integer >= 1"
    );
    generic_manifest_rejection!(
        generic_framebuffer_requires_format,
        "  - { name: state, size: 4096 }",
        "  - { name: state, size: 4096 }\n  - { name: framebuffer, size: 4096 }",
        "regions.framebuffer.format"
    );
    generic_manifest_rejection!(
        generic_duplicate_region_rejected,
        "  - { name: state, size: 4096 }",
        "  - { name: state, size: 4096 }\n  - { name: state, size: 4096 }",
        "regions duplicate name state"
    );
    generic_manifest_rejection!(
        generic_region_size_must_be_page_multiple,
        "name: state, size: 4096",
        "name: state, size: 256",
        "regions.state.size must be a multiple of 4096"
    );
    generic_manifest_rejection!(
        generic_mem_mib_zero_rejected,
        "mem_mib: 64",
        "mem_mib: 0",
        "machine.mem_mib must be an integer >= 1"
    );

    #[test]
    fn unknown_variant_rejected() {
        assert_eq!(
            generic_error_after_manifest_replace("variant: generic", "variant: sandbox"),
            ["variant must be game or generic, got String(\"sandbox\")"]
        );
    }
    #[test]
    fn variant_null_rejected() {
        let errors = generic_error_after_manifest_replace("variant: generic", "variant:");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].starts_with("variant must be game or generic"));
    }
    #[test]
    fn generic_framebuffer_with_format_accepted() {
        let tmp = generic_fixture_copy();
        write_manifest_text(&tmp, &manifest_text(&tmp).replace("  - { name: state, size: 4096 }", "  - { name: state, size: 4096 }\n  - { name: framebuffer, size: 4096, format: xrgb8888-16x16-stride64 }"));
        validate_manifest(&tmp.path.join("workload-image.yaml")).unwrap();
    }
    #[test]
    fn generic_optional_zero_size_accepted() {
        let tmp = generic_fixture_copy();
        write_manifest_text(
            &tmp,
            &manifest_text(&tmp).replace(
                "  - { name: state, size: 4096 }",
                "  - { name: state, size: 4096 }\n  - { name: scratch, size: 0, optional: true }",
            ),
        );
        validate_manifest(&tmp.path.join("workload-image.yaml")).unwrap();
    }

    fn generic_file_errors(file: &str, before: &str, after: &str) -> Vec<String> {
        let tmp = generic_fixture_copy();
        let path = tmp.path.join(file);
        let original = std::fs::read_to_string(&path).unwrap();
        assert!(original.contains(before));
        std::fs::write(path, original.replace(before, after)).unwrap();
        validation_errors(&tmp)
    }

    macro_rules! generic_file_rejection {
        ($name:ident, $file:expr, $before:expr, $after:expr, $error:expr) => {
            #[test]
            fn $name() {
                let errors = generic_file_errors($file, $before, $after);
                assert!(errors.iter().any(|e| e.contains($error)), "{errors:?}");
            }
        };
    }

    generic_file_rejection!(
        generic_requires_an_expected_region,
        "boot.toml",
        "[[expected_region]]\nname = \"state\"\nlayout_version = 1\n",
        "",
        "boot.toml must declare at least one [[expected_region]]"
    );
    generic_file_rejection!(
        generic_expected_region_must_be_declared,
        "boot.toml",
        "name = \"state\"",
        "name = \"other\"",
        "boot.toml expected_region other is not declared in regions"
    );
    generic_file_rejection!(
        generic_expected_regions_size_must_match,
        "expected-regions.toml",
        "size = 4096",
        "size = 8192",
        "expected-regions.toml state size 8192 != manifest size 4096"
    );
    generic_file_rejection!(
        generic_layout_version_must_agree,
        "expected-regions.toml",
        "layout_version = 1",
        "layout_version = 2",
        "expected-regions.toml state layout_version 2 != boot.toml 1"
    );
    generic_file_rejection!(
        generic_unstamped_sidecar_keys_checked,
        "determinism.unstamped.yaml",
        "workload_image: generic-fixture@0.1.0",
        "workload_image: other@0.1.0",
        "determinism.unstamped.workload_image"
    );
    generic_file_rejection!(
        generic_boot_toml_comment_does_not_count_as_table,
        "boot.toml",
        "[autostart]",
        "# [autostart]",
        "boot.toml missing [autostart]"
    );

    #[test]
    fn generic_boot_toml_trailing_table_not_attributed_to_region() {
        let tmp = generic_fixture_copy();
        let path = tmp.path.join("boot.toml");
        let original = std::fs::read_to_string(&path).unwrap();
        std::fs::write(path, format!("{original}[extra]\nlayout_version = 9\n")).unwrap();
        validate_manifest(&tmp.path.join("workload-image.yaml")).unwrap();
    }

    #[test]
    fn generic_does_not_require_emu_version() {
        let tmp = generic_fixture_copy();
        let path = tmp.path.join("workload-image.yaml");
        validate_manifest(&path).unwrap();
        write_manifest_text(
            &tmp,
            &manifest_text(&tmp).replace(
                "    repo: guest-sdk",
                "    repo: guest-sdk\n    emu_version: \"x\"",
            ),
        );
        validate_manifest(&path).unwrap();
    }

    fn write_generic_green_stamp(tmp: &TempDir) {
        std::fs::remove_file(tmp.path.join(UNSTAMPED_FILE)).unwrap();
        let hash = blake3_file(&tmp.path.join("workload-image.yaml")).unwrap();
        let content = format!("schema_version: 1\nkind: determinism-last-green\nworkload_image: generic-fixture@0.1.0\nimage_manifest_hash: \"{hash}\"\ngit_rev: \"0000000000000000000000000000000000000000\"\nsuite_version: \"fixture-suite-v1\"\ntimestamp: \"2026-10-15T00:00:00Z\"\nsuite_report_blake3: \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\"\n");
        std::fs::write(tmp.path.join(GREEN_STAMP_FILE), content).unwrap();
    }

    #[test]
    fn generic_green_stamp_accepted() {
        let tmp = generic_fixture_copy();
        write_generic_green_stamp(&tmp);
        validate_manifest(&tmp.path.join("workload-image.yaml")).unwrap();
    }
    #[test]
    fn generic_green_stamp_wrong_workload_image_rejected() {
        let tmp = generic_fixture_copy();
        write_generic_green_stamp(&tmp);
        let path = tmp.path.join(GREEN_STAMP_FILE);
        let content = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            content.replace(
                "workload_image: generic-fixture@0.1.0",
                "workload_image: refwork-demo@0.2.0",
            ),
        )
        .unwrap();
        assert!(validation_errors(&tmp)
            .iter()
            .any(|e| e.contains("determinism.last_green.workload_image")));
    }
    #[test]
    fn generic_green_stamp_requires_git_rev() {
        let tmp = generic_fixture_copy();
        write_generic_green_stamp(&tmp);
        let path = tmp.path.join(GREEN_STAMP_FILE);
        let content = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            content.replace(
                "git_rev: \"0000000000000000000000000000000000000000\"\n",
                "",
            ),
        )
        .unwrap();
        assert!(validation_errors(&tmp)
            .iter()
            .any(|e| e.contains("determinism.last_green.git_rev")));
    }
    #[test]
    fn generic_unknown_top_level_key_ignored() {
        let tmp = generic_fixture_copy();
        write_manifest_text(
            &tmp,
            &format!("{}x_builder_note: hello\n", manifest_text(&tmp)),
        );
        validate_manifest(&tmp.path.join("workload-image.yaml")).unwrap();
    }

    fn write_double_build_artifacts(dir: &Path, initramfs: &[u8]) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("bzImage"), b"kernel").unwrap();
        std::fs::write(dir.join("initramfs.cpio.zst"), initramfs).unwrap();
        std::fs::write(dir.join("workload-image.yaml"), b"manifest").unwrap();
    }

    fn write_valid_green_stamp(tmp: &TempDir) {
        std::fs::remove_file(tmp.path.join("determinism.unstamped.yaml")).unwrap();
        let manifest_hash = blake3_file(&tmp.path.join("workload-image.yaml")).unwrap();
        let content = format!(
            r#"schema_version: 1
kind: determinism-last-green
workload_image: {WORKLOAD_NAME}@{VERSION}
image_manifest_hash: "{manifest_hash}"
reference_workload_git_rev: "0123456789012345678901234567890123456789"
emu_version: "{emu_version}"
suite_version: "refwork-verify-suite-v1"
timestamp: "2026-06-21T23:57:31Z"
suite_report_blake3: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
"#,
            emu_version = refwork_emu::EMU_VERSION,
        );
        std::fs::write(tmp.path.join("determinism.last_green"), content).unwrap();
    }

    fn write_green_stamp_without(tmp: &TempDir, line_prefix: &str) {
        write_valid_green_stamp(tmp);
        let text = green_stamp_text(tmp)
            .lines()
            .filter(|line| !line.starts_with(line_prefix))
            .map(|line| format!("{line}\n"))
            .collect::<String>();
        std::fs::write(tmp.path.join("determinism.last_green"), text).unwrap();
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed with {status}");
    }

    #[test]
    fn validator_accepts_generated_manifest_shape() {
        let tmp = valid_dist();
        validate_manifest(&tmp.path.join("workload-image.yaml")).unwrap();
    }

    #[test]
    fn generated_manifest_carries_emu_version() {
        let tmp = valid_dist();
        let expected = format!("    emu_version: \"{}\"\n", refwork_emu::EMU_VERSION);
        assert!(
            manifest_text(&tmp).contains(&expected),
            "manifest must record refwork_emu::EMU_VERSION under meta.built_from"
        );
    }

    #[test]
    fn validate_rejects_manifest_without_emu_version() {
        let tmp = valid_dist();
        let line = format!("    emu_version: \"{}\"\n", refwork_emu::EMU_VERSION);
        let manifest = manifest_text(&tmp).replace(&line, "");
        assert_ne!(
            manifest,
            manifest_text(&tmp),
            "fixture must contain the field"
        );
        write_manifest_text(&tmp, &manifest);

        let errors = validation_errors(&tmp);

        assert!(
            errors
                .iter()
                .any(|err| err == "missing meta.built_from.emu_version"),
            "errors: {errors:?}"
        );
    }

    #[test]
    fn validate_rejects_empty_emu_version() {
        let tmp = valid_dist();
        let line = format!("    emu_version: \"{}\"\n", refwork_emu::EMU_VERSION);
        let manifest = manifest_text(&tmp).replace(&line, "    emu_version: \"\"\n");
        write_manifest_text(&tmp, &manifest);

        let errors = validation_errors(&tmp);

        assert!(
            errors
                .iter()
                .any(|err| err == "meta.built_from.emu_version must not be empty"),
            "errors: {errors:?}"
        );
    }

    #[test]
    fn generated_manifest_pins_d7_framebuffer_line() {
        // The exact framebuffer line the hypervisor layout contract (D7)
        // expects; `validate_regions` only enforces `size >=`, and the dist
        // bundle is not built in CI, so the generated text is pinned here.
        let tmp = valid_dist();
        assert!(
            manifest_text(&tmp).contains(
                "{ name: framebuffer, size: 229376, format: xrgb8888-256x224-stride1024 }"
            ),
            "generated manifest framebuffer line drifted from the D7 contract"
        );
    }

    #[test]
    fn validate_rejects_manifest_from_another_workspace_version() {
        let tmp = valid_dist();
        let manifest = manifest_text(&tmp).replace(
            &format!("  version: \"{VERSION}\"\n"),
            "  version: \"0.1.0\"\n",
        );
        assert_ne!(manifest, manifest_text(&tmp), "fixture must carry VERSION");
        write_manifest_text(&tmp, &manifest);

        let errors = validation_errors(&tmp);

        assert!(
            errors
                .iter()
                .any(|err| err.starts_with("meta.version expected")),
            "errors: {errors:?}"
        );
    }

    #[test]
    fn missing_built_from_is_reported_once_even_when_stamped() {
        let tmp = valid_dist();
        write_valid_green_stamp(&tmp);
        let manifest = manifest_text(&tmp);
        let start = manifest.find("  built_from:\n").unwrap();
        let end = manifest.find("artifacts:\n").unwrap();
        write_manifest_text(&tmp, &format!("{}{}", &manifest[..start], &manifest[end..]));

        let errors = validation_errors(&tmp);

        assert_eq!(
            errors
                .iter()
                .filter(|err| *err == "missing meta.built_from")
                .count(),
            1,
            "errors: {errors:?}"
        );
    }

    #[test]
    fn unstamped_sidecar_carries_emu_version() {
        let tmp = TempDir::new();
        let path = tmp.path.join("determinism.unstamped.yaml");
        write_unstamped_sidecar(
            &path,
            "0123456789012345678901234567890123456789",
            "refwork-emu test",
        )
        .unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("emu_version: \"refwork-emu test\"\n"),
            "{text}"
        );
        assert!(
            text.contains(&format!("workload_image: {WORKLOAD_NAME}@{VERSION}\n")),
            "{text}"
        );
    }

    #[test]
    fn green_stamp_without_emu_version_rejected() {
        let tmp = valid_dist();
        write_green_stamp_without(&tmp, "emu_version:");

        let err = register_image(
            Path::new("/missing-workspace"),
            Some(&tmp.path.join("workload-image.yaml")),
            true,
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("missing determinism.last_green.emu_version"),
            "{err}"
        );
    }

    #[test]
    fn green_stamp_emu_version_mismatch_rejected() {
        let tmp = valid_dist();
        write_green_stamp_without(&tmp, "emu_version:");
        append_green_stamp_line(&tmp, "emu_version: \"refwork-emu 0.0.0-other\"");

        let err = register_image(
            Path::new("/missing-workspace"),
            Some(&tmp.path.join("workload-image.yaml")),
            true,
        )
        .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("determinism.last_green.emu_version expected"),
            "must name the stamp field: {msg}"
        );
        assert!(
            msg.contains(refwork_emu::EMU_VERSION),
            "must name the manifest's epoch: {msg}"
        );
    }

    #[test]
    fn green_stamp_matching_emu_version_accepted() {
        let tmp = valid_dist();
        write_valid_green_stamp(&tmp);

        let report = register_image(
            Path::new("/missing-workspace"),
            Some(&tmp.path.join("workload-image.yaml")),
            true,
        )
        .unwrap();

        assert_eq!(report.mode, RegisterMode::DirectDistStamped);
    }

    #[test]
    fn generated_manifest_contains_pad_layout_id() {
        let tmp = valid_dist();
        let manifest = manifest_text(&tmp);

        assert!(manifest.contains("  layout_id: console16-12btn-v1"));
    }

    #[test]
    fn double_build_cleanliness_check_rejects_dirty_checkout() {
        let tmp = TempDir::new();
        git(&tmp.path, &["init"]);
        git(
            &tmp.path,
            &["config", "user.email", "codex@example.invalid"],
        );
        git(&tmp.path, &["config", "user.name", "Codex"]);
        std::fs::write(tmp.path.join("tracked.txt"), b"clean").unwrap();
        git(&tmp.path, &["add", "tracked.txt"]);
        git(&tmp.path, &["commit", "-m", "init"]);
        std::fs::write(tmp.path.join("tracked.txt"), b"dirty").unwrap();

        let err = ensure_clean_git_checkout(&tmp.path, "reference-workload").unwrap_err();

        assert!(err
            .to_string()
            .contains("reference-workload checkout is dirty"));
    }

    #[test]
    fn double_build_artifact_compare_accepts_equal_outputs() {
        let tmp = TempDir::new();
        let first = tmp.path.join("first");
        let second = tmp.path.join("second");
        write_double_build_artifacts(&first, b"initramfs");
        write_double_build_artifacts(&second, b"initramfs");

        let artifacts = compare_double_build_artifacts(&first, &second).unwrap();

        assert_eq!(artifacts.len(), 3);
        assert_eq!(artifacts[0].file, "bzImage");
        assert_eq!(artifacts[1].file, "initramfs.cpio.zst");
        assert_eq!(artifacts[2].file, "workload-image.yaml");
    }

    #[test]
    fn double_build_artifact_compare_rejects_mismatch() {
        let tmp = TempDir::new();
        let first = tmp.path.join("first");
        let second = tmp.path.join("second");
        write_double_build_artifacts(&first, b"initramfs-a");
        write_double_build_artifacts(&second, b"initramfs-b");

        let err = compare_double_build_artifacts(&first, &second).unwrap_err();

        match err {
            ImageError::Validation(errors) => assert!(errors
                .iter()
                .any(|err| err.contains("double-build artifact initramfs.cpio.zst differs"))),
            err => panic!("unexpected error: {err}"),
        }
    }

    #[test]
    fn register_accepts_unstamped_direct_handoff_before_package_06() {
        let tmp = valid_dist();

        let report = register_image(
            Path::new("/missing-workspace"),
            Some(&tmp.path.join("workload-image.yaml")),
            false,
        )
        .unwrap();

        assert_eq!(report.mode, RegisterMode::DirectDistUnstamped);
        assert_eq!(report.manifest, tmp.path.join("workload-image.yaml"));
    }

    #[test]
    fn register_rejects_unstamped_manifest_when_green_stamp_is_required() {
        let tmp = valid_dist();

        let err = register_image(
            Path::new("/missing-workspace"),
            Some(&tmp.path.join("workload-image.yaml")),
            true,
        )
        .unwrap_err();

        assert!(err.to_string().contains("missing determinism green stamp"));
    }

    #[test]
    fn register_accepts_green_stamped_manifest() {
        let tmp = valid_dist();
        write_valid_green_stamp(&tmp);

        let report = register_image(
            Path::new("/missing-workspace"),
            Some(&tmp.path.join("workload-image.yaml")),
            true,
        )
        .unwrap();

        assert_eq!(report.mode, RegisterMode::DirectDistStamped);
    }

    #[test]
    fn register_rejects_dummy_green_stamp() {
        let tmp = valid_dist();
        std::fs::remove_file(tmp.path.join("determinism.unstamped.yaml")).unwrap();
        std::fs::write(tmp.path.join("determinism.last_green"), b"green").unwrap();

        let err = register_image(
            Path::new("/missing-workspace"),
            Some(&tmp.path.join("workload-image.yaml")),
            true,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("determinism.last_green must be a mapping"));
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
    fn validator_rejects_artifact_filename_drift() {
        let tmp = valid_dist();
        std::fs::rename(tmp.path.join("bzImage"), tmp.path.join("kernel.payload")).unwrap();
        let manifest = manifest_text(&tmp).replace("file: bzImage", "file: kernel.payload");
        write_manifest_text(&tmp, &manifest);

        let errors = validation_errors(&tmp);

        assert!(errors
            .iter()
            .any(|err| err.contains("artifacts.kernel.file must be bzImage")));
    }

    #[test]
    fn validator_rejects_unsupported_cmdline_token() {
        let tmp = valid_dist();
        let manifest =
            manifest_text(&tmp).replace("cmdline: \"quiet\"", "cmdline: \"quiet root=/dev/vda\"");
        write_manifest_text(&tmp, &manifest);

        let errors = validation_errors(&tmp);

        assert!(errors
            .iter()
            .any(|err| err.contains("boot.cmdline token \"root=/dev/vda\"")));
    }

    #[test]
    fn validator_rejects_device_contract_drift() {
        let tmp = valid_dist();
        let manifest = manifest_text(&tmp).replace(
            "{ kind: virtio-blk, role: game-image, readonly: true, required: true }",
            "{ kind: virtio-blk, role: host-root, readonly: false, required: false }",
        );
        write_manifest_text(&tmp, &manifest);

        let errors = validation_errors(&tmp);

        assert!(errors
            .iter()
            .any(|err| err.contains("machine.devices.virtio-blk.role")));
        assert!(errors
            .iter()
            .any(|err| err.contains("machine.devices.virtio-blk.readonly")));
        assert!(errors
            .iter()
            .any(|err| err.contains("machine.devices.virtio-blk.required")));
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
    fn validator_rejects_pad_layout_id_drift() {
        let tmp = valid_dist();
        let manifest = manifest_text(&tmp).replace(
            "layout_id: console16-12btn-v1",
            "layout_id: console16-12btn-v2",
        );
        write_manifest_text(&tmp, &manifest);

        let errors = validation_errors(&tmp);

        assert!(errors
            .iter()
            .any(|err| err.contains("pad_layout.layout_id expected")));
    }

    #[test]
    fn validator_rejects_missing_pad_layout_id() {
        let tmp = valid_dist();
        let manifest = manifest_text(&tmp).replace("  layout_id: console16-12btn-v1\n", "");
        write_manifest_text(&tmp, &manifest);

        let errors = validation_errors(&tmp);

        assert!(errors
            .iter()
            .any(|err| err.contains("missing pad_layout.layout_id")));
    }

    #[test]
    fn validator_rejects_pad_button_name_casing_drift() {
        let tmp = valid_dist();
        let manifest = manifest_text(&tmp).replace("{ name: Up, bit: 6 }", "{ name: UP, bit: 6 }");
        write_manifest_text(&tmp, &manifest);

        let errors = validation_errors(&tmp);

        assert!(errors
            .iter()
            .any(|err| err.contains("pad_layout.buttons[6].name expected")));
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
    fn validator_rejects_sidecar_contract_drift() {
        let tmp = valid_dist();
        let boot = std::fs::read_to_string(tmp.path.join("boot.toml"))
            .unwrap()
            .replace("protocol = \"refwork-ctl\"", "protocol = \"bogus-ctl\"");
        std::fs::write(tmp.path.join("boot.toml"), boot).unwrap();
        let harness = std::fs::read_to_string(tmp.path.join("harness.toml"))
            .unwrap()
            .replace("protocol_version = 1", "protocol_version = 99");
        std::fs::write(tmp.path.join("harness.toml"), harness).unwrap();

        let errors = validation_errors(&tmp);

        assert!(errors
            .iter()
            .any(|err| err.contains("boot.toml unit.control.protocol")));
        assert!(errors
            .iter()
            .any(|err| err.contains("harness.toml harness.protocol_version")));
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
