use super::*;

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
    generic_name_leading_hyphen_rejected,
    "name: generic-fixture",
    "name: -fixture",
    "meta.name"
);
generic_manifest_rejection!(
    generic_name_empty_rejected,
    "name: generic-fixture",
    "name: \"\"",
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
    generic_boot_layout_version_required,
    "boot.toml",
    "layout_version = 1\n",
    "",
    "boot.toml expected_region layout_version must be an integer"
);
generic_file_rejection!(
    generic_boot_layout_version_must_be_integer,
    "boot.toml",
    "layout_version = 1",
    "layout_version = \"invalid\"",
    "boot.toml expected_region layout_version must be an integer"
);
generic_file_rejection!(
    generic_boot_root_version_must_be_root,
    "boot.toml",
    "boot_toml_version = 1\n[autostart]",
    "[autostart]\nboot_toml_version = 1",
    "boot.toml boot_toml_version must be 1"
);
generic_file_rejection!(
    generic_expected_root_version_must_be_root,
    "expected-regions.toml",
    "schema_version = 1\n",
    "",
    "expected-regions.toml schema_version must be 1"
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
fn generic_compact_toml_assignments_accepted() {
    let tmp = generic_fixture_copy();
    for file in ["boot.toml", "expected-regions.toml"] {
        let path = tmp.path.join(file);
        let text = std::fs::read_to_string(&path).unwrap().replace(" = ", "=");
        std::fs::write(path, text).unwrap();
    }
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
