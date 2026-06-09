//! `fetch-test-roms`: download test ROM archives listed in `xtask/test-roms.lock`,
//! verify BLAKE3 checksums, and unpack into `target/test-roms/<name>/`.

use std::path::Path;
use std::process::Command;

use serde::Deserialize;

/// One entry in the lock file.
#[derive(Debug, Deserialize)]
pub struct LockEntry {
    /// Short name used as subdirectory: `target/test-roms/<name>/`.
    pub name: String,
    /// Direct download URL (GitHub archive or similar).
    pub url: String,
    /// BLAKE3 hex digest of the downloaded archive.
    pub blake3: String,
    /// Archive format: "tar.gz", "zip", or "tar.xz".
    pub format: String,
}

/// Parse `xtask/test-roms.lock` (JSON array of `LockEntry`).
pub fn load_lock(workspace_root: &Path) -> Result<Vec<LockEntry>, String> {
    let lock_path = workspace_root.join("xtask/test-roms.lock");
    let content = std::fs::read_to_string(&lock_path)
        .map_err(|e| format!("cannot read {}: {}", lock_path.display(), e))?;
    serde_json::from_str(&content).map_err(|e| format!("cannot parse lock file: {}", e))
}

/// Run the fetch-test-roms command.
pub fn run_fetch(workspace_root: &Path) -> Result<(), String> {
    let entries = load_lock(workspace_root)?;
    let target_dir = workspace_root.join("target/test-roms");
    std::fs::create_dir_all(&target_dir)
        .map_err(|e| format!("cannot create {}: {}", target_dir.display(), e))?;

    for entry in &entries {
        println!("fetch: {} ...", entry.name);
        fetch_entry(entry, &target_dir).map_err(|e| format!("{}: {}", entry.name, e))?;
    }

    Ok(())
}

fn fetch_entry(entry: &LockEntry, target_dir: &Path) -> Result<(), String> {
    // Check for TBD placeholder (operator must fill in real hash/URL).
    if entry.blake3.contains("TBD") || entry.url.contains("TBD") {
        println!(
            "fetch: SKIPPING '{}' — lock entry has placeholder values.\n\
             The operator must fill in a real commit hash and BLAKE3 digest.\n\
             See xtask/test-roms.lock for instructions.",
            entry.name
        );
        return Ok(());
    }

    let out_dir = target_dir.join(&entry.name);
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("cannot create {}: {}", out_dir.display(), e))?;

    let archive_name = format!(
        "{}.{}",
        entry.name,
        if entry.format == "tar.gz" {
            "tar.gz"
        } else {
            &entry.format
        }
    );
    let archive_path = target_dir.join(&archive_name);

    // Download with curl -L.
    println!("  downloading {} ...", entry.url);
    let status = Command::new("curl")
        .args(["-L", "-o", archive_path.to_str().unwrap(), &entry.url])
        .status();

    match status {
        Err(e) => {
            return Err(format!("curl failed to launch (offline?): {}", e));
        }
        Ok(s) if !s.success() => {
            return Err(format!(
                "curl exited with status {} (possibly offline or bad URL)",
                s
            ));
        }
        Ok(_) => {}
    }

    // Verify BLAKE3.
    println!("  verifying blake3 ...");
    let data = std::fs::read(&archive_path).map_err(|e| format!("cannot read archive: {}", e))?;
    let computed = blake3::hash(&data);
    let computed_hex = computed.to_hex().to_string();
    if computed_hex != entry.blake3 {
        return Err(format!(
            "BLAKE3 mismatch for {}: expected {} got {}",
            entry.name, entry.blake3, computed_hex
        ));
    }
    println!("  blake3 OK");

    // Unpack.
    println!("  unpacking into {} ...", out_dir.display());
    let unpack_status = match entry.format.as_str() {
        "tar.gz" | "tar.xz" => Command::new("tar")
            .args([
                "xf",
                archive_path.to_str().unwrap(),
                "-C",
                out_dir.to_str().unwrap(),
                "--strip-components=1",
            ])
            .status(),
        "zip" => Command::new("unzip")
            .args([
                "-o",
                archive_path.to_str().unwrap(),
                "-d",
                out_dir.to_str().unwrap(),
            ])
            .status(),
        fmt => return Err(format!("unsupported archive format '{}'", fmt)),
    };

    match unpack_status {
        Err(e) => Err(format!("unpack command failed to launch: {}", e)),
        Ok(s) if !s.success() => Err(format!("unpack exited with status {}", s)),
        Ok(_) => {
            println!("  done: {}", out_dir.display());
            Ok(())
        }
    }
}
