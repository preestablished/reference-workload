use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
struct RegionRecord {
    name: String,
    size: u64,
    format: Option<String>,
    layout_version: u64,
    required: bool,
    writable: bool,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate has a workspace parent")
        .to_owned()
}

fn read_workspace_file(path: &str) -> String {
    let path = repo_root().join(path);
    std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("cannot read {}: {err}", path.display());
    })
}

#[test]
fn required_image_inputs_exist() {
    for path in [
        "image/kernel.lock",
        "image/kernel.config",
        "image/builder.lock",
        "image/guest-sdk.lock",
        "image/boot.toml",
        "image/harness.toml",
        "image/expected-regions.toml",
        "image/README.md",
    ] {
        assert!(repo_root().join(path).is_file(), "missing {path}");
    }
}

#[test]
fn boot_toml_names_autostart_and_required_regions() {
    let boot = read_workspace_file("image/boot.toml");

    assert!(boot.contains("refwork-harness"));
    assert!(boot.contains("expected_regions = [\"wram\", \"framebuffer\", \"meta\"]"));
    assert_eq!(regions_from_toml(&boot), required_region_records());
}

#[test]
fn expected_regions_include_sizes_and_layout_versions() {
    let expected = read_workspace_file("image/expected-regions.toml");

    assert_eq!(regions_from_toml(&expected), required_region_records());
}

#[test]
fn docs_assign_boot_schema_to_guest_sdk() {
    let readme = read_workspace_file("image/README.md");

    assert!(readme.contains("The guest-sdk owns the"));
    assert!(readme.contains("`boot.toml` schema"));
}

#[test]
fn placeholder_lock_hashes_match_payloads() {
    for path in [
        "image/kernel.lock",
        "image/builder.lock",
        "image/guest-sdk.lock",
    ] {
        let lock = read_workspace_file(path);
        let payload = quoted_value(&lock, "placeholder_payload");
        let expected = quoted_value(&lock, "blake3");
        let actual = blake3::hash(payload.as_bytes()).to_hex().to_string();
        assert_eq!(actual, expected, "{path} placeholder hash mismatch");
    }
}

#[test]
fn no_game_payload_or_workload_image_is_committed_under_image_inputs() {
    let image_dir = repo_root().join("image");
    let forbidden_names = [
        "workload-image.yaml",
        "workload-image.yml",
        "game.rom",
        "game.sfc",
        "game.smc",
    ];

    for name in forbidden_names {
        assert!(
            !image_dir.join(name).exists(),
            "{name} must be generated or supplied outside image inputs"
        );
    }

    assert_no_rom_like_files(&image_dir);
}

fn quoted_value(content: &str, key: &str) -> String {
    let prefix = format!("{key} = \"");
    let line = content
        .lines()
        .find(|line| line.starts_with(&prefix))
        .unwrap_or_else(|| panic!("missing {key}"));
    let value = line
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or_else(|| panic!("malformed quoted value for {key}"));
    value.replace("\\n", "\n")
}

fn required_region_records() -> Vec<RegionRecord> {
    vec![
        RegionRecord {
            name: "wram".into(),
            size: 131_072,
            format: None,
            layout_version: 1,
            required: true,
            writable: false,
        },
        RegionRecord {
            name: "framebuffer".into(),
            size: 229_376,
            format: Some("xrgb8888-256x224-stride1024".into()),
            layout_version: 1,
            required: true,
            writable: false,
        },
        RegionRecord {
            name: "meta".into(),
            size: 4_096,
            format: None,
            layout_version: 1,
            required: true,
            writable: false,
        },
    ]
}

fn regions_from_toml(content: &str) -> Vec<RegionRecord> {
    content
        .split("[[regions]]")
        .skip(1)
        .map(parse_region_record)
        .collect()
}

fn parse_region_record(block: &str) -> RegionRecord {
    RegionRecord {
        name: field_string(block, "name"),
        size: field_u64(block, "size"),
        format: field_optional_string(block, "format"),
        layout_version: field_u64(block, "layout_version"),
        required: field_bool(block, "required"),
        writable: field_bool(block, "writable"),
    }
}

fn field_string(block: &str, key: &str) -> String {
    let value = field_value(block, key);
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or_else(|| panic!("field {key} is not a quoted string"))
        .to_owned()
}

fn field_optional_string(block: &str, key: &str) -> Option<String> {
    let line = block
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with(&format!("{key} = ")))?;
    let value = line
        .split_once('=')
        .map(|(_, value)| value.trim())
        .unwrap_or_else(|| panic!("malformed {key} line"));
    Some(
        value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .unwrap_or_else(|| panic!("field {key} is not a quoted string"))
            .to_owned(),
    )
}

fn field_u64(block: &str, key: &str) -> u64 {
    field_value(block, key)
        .parse()
        .unwrap_or_else(|err| panic!("field {key} is not u64: {err}"))
}

fn field_bool(block: &str, key: &str) -> bool {
    field_value(block, key)
        .parse()
        .unwrap_or_else(|err| panic!("field {key} is not bool: {err}"))
}

fn field_value<'a>(block: &'a str, key: &str) -> &'a str {
    let prefix = format!("{key} = ");
    block
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("missing field {key} in region block:\n{block}"))
}

fn assert_no_rom_like_files(dir: &Path) {
    for entry in std::fs::read_dir(dir).expect("read image dir") {
        let entry = entry.expect("read image entry");
        let path = entry.path();
        if path.is_dir() {
            assert_no_rom_like_files(&path);
            continue;
        }

        let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        assert!(
            !matches!(ext, "rom" | "sfc" | "smc" | "srm" | "sav" | "bin"),
            "game-like payload file is not allowed in image inputs: {}",
            path.display()
        );
    }
}

/// The hypervisor enforces D7 framebuffer geometry from `layout_version 1`
/// since determinism-hypervisor `5698d7e`: exactly 229,376 bytes, XRGB8888,
/// 256x224, stride 1024 — wrong length is a `FailedPrecondition` at
/// `GetFramebuffer`/`CaptureSpec` time. Pin the image contract to it
/// explicitly (phase3-m4-first-room-unblock step 03) rather than trusting
/// `FB_BYTES` transitively.
#[test]
fn framebuffer_region_matches_hypervisor_layout_contract() {
    const D7_FB_BYTES: u64 = 229_376; // 1024 stride * 224 rows

    for source in ["image/expected-regions.toml", "image/boot.toml"] {
        let records = regions_from_toml(&read_workspace_file(source));
        let fb = records
            .iter()
            .find(|r| r.name == "framebuffer")
            .unwrap_or_else(|| panic!("{source} lacks a framebuffer region"));
        assert_eq!(fb.size, D7_FB_BYTES, "{source} framebuffer size");
        assert_eq!(fb.layout_version, 1, "{source} framebuffer layout_version");
        assert_eq!(
            fb.format.as_deref(),
            Some("xrgb8888-256x224-stride1024"),
            "{source} framebuffer format"
        );
    }

    // The dist manifest the operator consumes must carry the same size.
    let manifest = read_workspace_file("dist/workload-image-0.1.0/workload-image.yaml");
    assert!(
        manifest.contains("name: framebuffer, size: 229376, format: xrgb8888-256x224-stride1024"),
        "dist manifest framebuffer line drifted from the D7 contract"
    );
}
