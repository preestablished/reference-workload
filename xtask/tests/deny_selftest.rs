//! Deny-gate self-test: continuously demonstrates the M0 acceptance clause
//! "CI deny-gates demonstrably fail a PR that adds `std::thread` to
//! `refwork-emu`" — by running the real scanner over a planted-token tree on
//! every PR, instead of a one-off poisoned demo PR.

use std::path::PathBuf;

use xtask::deny::scan_tree;

/// Build a temp tree mimicking a crate src dir with one planted file.
fn plant(dir_name: &str, contents: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("deny-selftest")
        .join(dir_name);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("lib.rs"), contents).unwrap();
    root
}

#[test]
fn planted_tokens_fail_the_scan() {
    // One case per banned token class, in code position (not comments).
    let cases: &[(&str, &str)] = &[
        ("thread", "pub fn f() { std::thread::spawn(|| {}); }\n"),
        ("tokio", "use tokio::runtime::Runtime;\n"),
        ("rand", "use rand::Rng;\n"),
        ("float", "pub fn f() -> f32 { 0.0 }\n"),
        (
            "instant",
            "pub fn f() { let _ = std::time::Instant::now(); }\n",
        ),
        ("hashmap", "use std::collections::HashMap;\n"),
    ];
    for (name, code) in cases {
        // Real refwork-emu source plus the planted file shape: the scanner
        // operates per-file, so a single planted file suffices to model
        // "a PR adds this to refwork-emu".
        let root = plant(name, code);
        let findings = scan_tree(&root);
        assert!(
            !findings.is_empty(),
            "planted `{name}` token was NOT flagged: {code:?}"
        );
        assert!(
            findings.iter().any(|f| f.line == 1),
            "finding for `{name}` should point at line 1: {findings:?}"
        );
    }
}

#[test]
fn comment_only_mentions_are_not_flagged() {
    let root = plant(
        "comments",
        "// std::thread is banned (D1); rand and f32 are banned too.\n\
         /// Doc comment: never use HashMap or Instant::now here.\n\
         pub fn clean() {}\n",
    );
    let findings = scan_tree(&root);
    assert!(
        findings.is_empty(),
        "comment-only mentions must not be flagged: {findings:?}"
    );
}

#[test]
fn real_tree_is_clean() {
    // The actual gate over the real workspace must pass (mirrors
    // `cargo xtask deny`, kept here so `cargo test --workspace` exercises it
    // even if the CI step ordering changes).
    let ws = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    for dir in [
        "crates/refwork-emu/src",
        "crates/refwork-harness/src",
        "crates/refwork-protocol/src",
    ] {
        let findings = scan_tree(&ws.join(dir));
        assert!(findings.is_empty(), "{dir} has findings: {findings:?}");
    }
}
