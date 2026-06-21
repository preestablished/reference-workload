use std::path::{Path, PathBuf};

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
    for region in ["wram", "framebuffer", "meta"] {
        assert!(boot.contains(&format!("name = \"{region}\"")));
    }
}

#[test]
fn expected_regions_include_sizes_and_layout_versions() {
    let expected = read_workspace_file("image/expected-regions.toml");

    for (name, size) in [("wram", 131_072), ("framebuffer", 229_376), ("meta", 4_096)] {
        let name_index = expected
            .find(&format!("name = \"{name}\""))
            .unwrap_or_else(|| panic!("missing expected region {name}"));
        let region_block = &expected[name_index..];
        assert!(
            region_block.contains(&format!("size = {size}")),
            "{name} missing size {size}"
        );
        assert!(
            region_block.contains("layout_version = 1"),
            "{name} missing layout_version"
        );
    }
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
