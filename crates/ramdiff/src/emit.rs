//! `ramdiff emit` — append a feature entry to a feature-map YAML file.
//!
//! Links `refwork-featuremap` for its serde types and validator — never
//! re-implements the schema. After appending, runs the validator on the
//! resulting map and reports any errors.
//!
//! Refuses to overwrite an existing entry with the same name unless `--force`.

use refwork_featuremap::{
    parse_feature_map, validate_map, Discretize, Feature, FeatureType, IntOrHex, Semantics,
    Stability,
};

/// Options for `ramdiff emit`.
pub struct EmitOpts {
    pub map: std::path::PathBuf,
    pub name: String,
    pub offset: u32,
    pub feature_type: FeatureType,
    pub stability: Stability,
    pub discretize: Option<Discretize>,
    /// Region name (default `"wram"`).
    pub region: String,
    pub description: Option<String>,
    pub semantics: Semantics,
    pub force: bool,
}

/// Parse `FeatureType` from CLI string.
pub fn parse_feature_type(s: &str) -> Result<FeatureType, String> {
    match s {
        "u8" => Ok(FeatureType::U8),
        "u16le" => Ok(FeatureType::U16le),
        "u16be" => Ok(FeatureType::U16be),
        "u32le" => Ok(FeatureType::U32le),
        "u32be" => Ok(FeatureType::U32be),
        "i8" => Ok(FeatureType::I8),
        "i16le" => Ok(FeatureType::I16le),
        "i16be" => Ok(FeatureType::I16be),
        "i32le" => Ok(FeatureType::I32le),
        "i32be" => Ok(FeatureType::I32be),
        "bitflags8" => Ok(FeatureType::Bitflags8),
        "bitflags16le" => Ok(FeatureType::Bitflags16le),
        "bitflags32le" => Ok(FeatureType::Bitflags32le),
        "bcd8" => Ok(FeatureType::Bcd8),
        "bcd16le" => Ok(FeatureType::Bcd16le),
        "bytes" => Ok(FeatureType::Bytes),
        other => Err(format!("unknown feature type {:?}", other)),
    }
}

/// Parse `Stability` from CLI string.
pub fn parse_stability(s: &str) -> Result<Stability, String> {
    match s {
        "stable" => Ok(Stability::Stable),
        "volatile" => Ok(Stability::Volatile),
        other => Err(format!(
            "unknown stability {:?}, expected stable or volatile",
            other
        )),
    }
}

/// Parse `Semantics` from CLI string.
pub fn parse_semantics(s: &str) -> Result<Semantics, String> {
    match s {
        "counter" => Ok(Semantics::Counter),
        "position_x" => Ok(Semantics::PositionX),
        "position_y" => Ok(Semantics::PositionY),
        "room_id" => Ok(Semantics::RoomId),
        "health" => Ok(Semantics::Health),
        "resource" => Ok(Semantics::Resource),
        "flags" => Ok(Semantics::Flags),
        "mode" => Ok(Semantics::Mode),
        "progress_flag" => Ok(Semantics::ProgressFlag),
        "timer" => Ok(Semantics::Timer),
        "opaque" => Ok(Semantics::Opaque),
        other => Err(format!("unknown semantics {:?}", other)),
    }
}

/// Append (or overwrite with `--force`) a feature entry in the map YAML.
pub fn run_emit(opts: &EmitOpts) -> Result<(), String> {
    let map_path = &opts.map;
    let yaml_text = std::fs::read_to_string(map_path)
        .map_err(|e| format!("cannot read {:?}: {}", map_path.display(), e))?;

    let (mut map, parse_errors) =
        parse_feature_map(&yaml_text).map_err(|e| format!("cannot parse feature map: {}", e))?;

    // Report parse errors but don't abort — the validator will catch them.
    if !parse_errors.is_empty() {
        for err in &parse_errors {
            eprintln!("emit: warning: {}", err);
        }
    }

    // Check for existing entry.
    let existing_idx = map.features.iter().position(|f| f.name == opts.name);
    if let Some(idx) = existing_idx {
        if !opts.force {
            return Err(format!(
                "feature {:?} already exists in {:?}; use --force to overwrite",
                opts.name,
                map_path.display()
            ));
        }
        map.features.remove(idx);
    }

    // Build the new feature entry.
    let feature = Feature {
        name: opts.name.clone(),
        region: opts.region.clone(),
        offset: IntOrHex(opts.offset as i64),
        feature_type: opts.feature_type.clone(),
        width: None, // derived from type
        semantics: opts.semantics.clone(),
        description: opts.description.clone(),
        stability: opts.stability.clone(),
        discretize: opts.discretize.clone(),
        valid_when: None,
    };

    map.features.push(feature);

    // Validate.
    let errors = validate_map(&map);
    if !errors.is_empty() {
        for err in &errors {
            eprintln!("emit: validation error: {}", err);
        }
        return Err(format!(
            "emit: {} validation error(s) — map not written",
            errors.len()
        ));
    }

    // Serialize and write back.
    let new_yaml =
        serde_yaml::to_string(&map).map_err(|e| format!("cannot serialize map: {}", e))?;
    std::fs::write(map_path, new_yaml)
        .map_err(|e| format!("cannot write {:?}: {}", map_path.display(), e))?;

    eprintln!(
        "emit: appended feature {:?} at offset 0x{:05X} to {:?}",
        opts.name,
        opts.offset,
        map_path.display()
    );
    println!("OK — map validated cleanly");
    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal valid feature-map YAML with no features.
    const EMPTY_MAP_YAML: &str = r#"schema_version: 1
kind: feature-map
meta:
  name: test-map
  workload: test-workload
  game_revision: "test-rev"
  version: 1
regions:
  - name: wram
    size: 131072
features: []
"#;

    fn write_temp_map(dir: &std::path::Path) -> std::path::PathBuf {
        let path = dir.join("test-map.yaml");
        std::fs::write(&path, EMPTY_MAP_YAML).unwrap();
        path
    }

    #[test]
    fn emit_appends_valid_entry() {
        use crate::filter::tempfile_shim::TempDir;
        let tmp = TempDir::new();
        let map_path = write_temp_map(&tmp.path);

        let opts = EmitOpts {
            map: map_path.clone(),
            name: "frame_counter".to_owned(),
            offset: 0x0010,
            feature_type: FeatureType::U16le,
            stability: Stability::Stable,
            discretize: Some(Discretize::Identity),
            region: "wram".to_owned(),
            description: Some("Synthetic ROM frame counter at WRAM 0x0010".to_owned()),
            semantics: Semantics::Counter,
            force: false,
        };

        run_emit(&opts).unwrap();

        // Parse back and verify.
        let yaml = std::fs::read_to_string(&map_path).unwrap();
        let (map, errors) = parse_feature_map(&yaml).unwrap();
        assert!(errors.is_empty(), "validation errors: {:?}", errors);
        let feat = map
            .features
            .iter()
            .find(|f| f.name == "frame_counter")
            .unwrap();
        assert_eq!(feat.offset.0, 0x0010);
    }

    #[test]
    fn emit_refuses_duplicate_without_force() {
        use crate::filter::tempfile_shim::TempDir;
        let tmp = TempDir::new();
        let map_path = write_temp_map(&tmp.path);

        let opts = EmitOpts {
            map: map_path.clone(),
            name: "frame_counter".to_owned(),
            offset: 0x0010,
            feature_type: FeatureType::U16le,
            stability: Stability::Stable,
            discretize: Some(Discretize::Identity),
            region: "wram".to_owned(),
            description: None,
            semantics: Semantics::Counter,
            force: false,
        };

        run_emit(&opts).unwrap();
        let err = run_emit(&opts).unwrap_err();
        assert!(err.contains("already exists"), "error was: {}", err);
    }

    #[test]
    fn emit_force_overwrites() {
        use crate::filter::tempfile_shim::TempDir;
        let tmp = TempDir::new();
        let map_path = write_temp_map(&tmp.path);

        let opts = EmitOpts {
            map: map_path.clone(),
            name: "frame_counter".to_owned(),
            offset: 0x0010,
            feature_type: FeatureType::U16le,
            stability: Stability::Stable,
            discretize: Some(Discretize::Identity),
            region: "wram".to_owned(),
            description: None,
            semantics: Semantics::Counter,
            force: false,
        };

        run_emit(&opts).unwrap();

        let opts_force = EmitOpts {
            map: map_path.clone(),
            name: "frame_counter".to_owned(),
            offset: 0x0012, // different offset
            feature_type: FeatureType::U16le,
            stability: Stability::Stable,
            discretize: Some(Discretize::Identity),
            region: "wram".to_owned(),
            description: None,
            semantics: Semantics::Counter,
            force: true,
        };

        run_emit(&opts_force).unwrap();

        let yaml = std::fs::read_to_string(&map_path).unwrap();
        let (map, errors) = parse_feature_map(&yaml).unwrap();
        assert!(errors.is_empty());
        let feat = map
            .features
            .iter()
            .find(|f| f.name == "frame_counter")
            .unwrap();
        assert_eq!(feat.offset.0, 0x0012);
    }
}
