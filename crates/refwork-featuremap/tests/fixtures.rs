//! Integration tests for the negative fixture suite (plan 03).
//!
//! Tests:
//! 1. Manifest ↔ directory bijection — every `*.yaml` (excluding `*.scoring.yaml`)
//!    in `fixtures/invalid/` appears in `expected.json` and vice-versa.
//! 2. Each manifest entry fails validation with the expected rule id.
//! 3. At least one test exercises the binary via `env!("CARGO_BIN_EXE_refwork-featuremap")`.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("invalid")
}

/// Entry from expected.json.
#[derive(Debug)]
struct Expectation {
    rule: String,
    scoring: Option<String>,
}

fn load_manifest() -> HashMap<String, Expectation> {
    let path = fixtures_dir().join("expected.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read expected.json: {}", e));
    let value: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("cannot parse expected.json: {}", e));
    let obj = value.as_object().expect("expected.json must be an object");
    obj.iter()
        .map(|(k, v)| {
            let rule = v["rule"]
                .as_str()
                .expect("rule must be a string")
                .to_string();
            let scoring = v
                .get("scoring")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            (k.clone(), Expectation { rule, scoring })
        })
        .collect()
}

/// Collect all `*.yaml` files in fixtures/invalid/ excluding `*.scoring.yaml`.
fn collect_standalone_yaml_files() -> HashSet<String> {
    let dir = fixtures_dir();
    std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read fixtures dir: {}", e))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().into_string().ok()?;
            if name.ends_with(".yaml") && !name.ends_with(".scoring.yaml") {
                Some(name)
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn manifest_directory_bijection() {
    let manifest = load_manifest();
    let files = collect_standalone_yaml_files();

    // Every manifest entry must exist as a file
    for name in manifest.keys() {
        assert!(
            files.contains(name),
            "manifest entry {:?} has no corresponding fixture file",
            name
        );
    }

    // Every standalone yaml file must appear in the manifest
    for name in &files {
        assert!(
            manifest.contains_key(name.as_str()),
            "fixture file {:?} is not listed in expected.json",
            name
        );
    }

    // Every *.scoring.yaml on disk must be referenced by some manifest entry,
    // or it is dead weight that is never exercised.
    let referenced: HashSet<&str> = manifest
        .values()
        .filter_map(|e| e.scoring.as_deref())
        .collect();
    for entry in std::fs::read_dir(fixtures_dir()).unwrap().flatten() {
        let name = entry.file_name().into_string().unwrap_or_default();
        if name.ends_with(".scoring.yaml") {
            assert!(
                referenced.contains(name.as_str()),
                "orphan scoring fixture {:?} is not referenced by expected.json",
                name
            );
        }
    }
}

#[test]
fn all_fixtures_fail_with_expected_rule() {
    let manifest = load_manifest();
    let dir = fixtures_dir();

    for (filename, expectation) in &manifest {
        let map_path = dir.join(filename);
        let map_yaml = std::fs::read_to_string(&map_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {}", filename, e));

        let errors = if let Some(scoring_file) = &expectation.scoring {
            // Cross-file: validate map + scoring pair
            let scoring_path = dir.join(scoring_file);
            let scoring_yaml = std::fs::read_to_string(&scoring_path)
                .unwrap_or_else(|e| panic!("cannot read scoring file {}: {}", scoring_file, e));

            let (map, _) = refwork_featuremap::parse_feature_map(&map_yaml)
                .unwrap_or_else(|e| panic!("map parse error for {}: {}", filename, e));
            let (sp, _) = refwork_featuremap::parse_scoring_program(&scoring_yaml)
                .unwrap_or_else(|e| panic!("scoring parse error for {}: {}", scoring_file, e));
            refwork_featuremap::validate_pair(&map, &sp)
        } else {
            // Map-only validation (includes preamble checks via parse_feature_map)
            match refwork_featuremap::parse_feature_map(&map_yaml) {
                Ok((_, errors)) => errors,
                Err(e) => panic!("parse fatal error for {}: {}", filename, e),
            }
        };

        assert!(
            !errors.is_empty(),
            "fixture {} should have produced errors but validated cleanly",
            filename
        );

        let found = errors.iter().any(|e| e.rule == expectation.rule.as_str());
        assert!(
            found,
            "fixture {} expected rule {:?} but got: {:?}",
            filename,
            expectation.rule,
            errors.iter().map(|e| &e.rule).collect::<Vec<_>>()
        );
    }
}

/// Test that runs the actual binary for at least one fixture.
#[test]
fn binary_rejects_bad_schema_version() {
    let bin = env!("CARGO_BIN_EXE_refwork-featuremap");
    let fixture = fixtures_dir().join("10-bad-schema-version.yaml");

    let output = std::process::Command::new(bin)
        .arg("validate")
        .arg(&fixture)
        .output()
        .unwrap_or_else(|e| panic!("failed to run binary: {}", e));

    assert_ne!(
        output.status.code(),
        Some(0),
        "binary should exit non-zero for bad-schema-version fixture"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("preamble/version"),
        "expected preamble/version in stderr, got: {}",
        stderr
    );
}

/// Test that the binary accepts the demo map (validates cleanly, exit 0).
#[test]
fn binary_accepts_demo_map() {
    let bin = env!("CARGO_BIN_EXE_refwork-featuremap");
    let demo_map = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("feature-maps")
        .join("demo-game.yaml");

    let output = std::process::Command::new(bin)
        .arg("validate")
        .arg(&demo_map)
        .output()
        .unwrap_or_else(|e| panic!("failed to run binary: {}", e));

    assert_eq!(
        output.status.code(),
        Some(0),
        "binary should exit 0 for demo-game.yaml, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Test that the binary accepts the demo map + scoring pair (exit 0).
#[test]
fn binary_accepts_demo_map_with_scoring() {
    let bin = env!("CARGO_BIN_EXE_refwork-featuremap");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let demo_map = root.join("feature-maps").join("demo-game.yaml");
    let demo_scoring = root.join("scoring").join("demo-game.yaml");

    let output = std::process::Command::new(bin)
        .arg("validate")
        .arg(&demo_map)
        .arg("--scoring")
        .arg(&demo_scoring)
        .output()
        .unwrap_or_else(|e| panic!("failed to run binary: {}", e));

    assert_eq!(
        output.status.code(),
        Some(0),
        "binary should exit 0 for demo-game.yaml + scoring, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
