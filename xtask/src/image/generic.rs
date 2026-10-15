use super::*;

pub(super) fn validate_handoff_files(
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
        if generic_root_u64(boot, "boot_toml_version") != Some(1) {
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
            .any(|unit| generic_field_string(unit, "exec").is_some_and(|s| !s.is_empty()))
        {
            errors.push("boot.toml unit.exec must be non-empty".into());
        }
        let expected_regions = toml_array_blocks(boot, "expected_region");
        if expected_regions.is_empty() {
            errors.push("boot.toml must declare at least one [[expected_region]]".into());
        }
        for block in expected_regions {
            if let Some(name) = generic_field_string(block, "name") {
                if !regions.contains_key(&name) {
                    errors.push(format!(
                        "boot.toml expected_region {name} is not declared in regions"
                    ));
                }
                if let Some(layout) = generic_field_u64(block, "layout_version") {
                    boot_layouts.insert(name, layout);
                } else {
                    errors
                        .push("boot.toml expected_region layout_version must be an integer".into());
                }
            } else {
                errors.push("boot.toml expected_region.name must be non-empty".into());
            }
        }
    }
    if let Some(expected) = expected.as_deref() {
        if generic_root_u64(expected, "schema_version") != Some(1) {
            errors.push("expected-regions.toml schema_version must be 1".into());
        }
        for block in toml_array_blocks(expected, "regions") {
            let Some(name) = generic_field_string(block, "name") else {
                errors.push("expected-regions.toml regions.name must be non-empty".into());
                continue;
            };
            match regions.get(&name) {
                Some(size) => {
                    if let Some(actual) = generic_field_u64(block, "size") {
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
            if let Some(boot_layout) = boot_layouts.get(&name) {
                let actual = generic_field_u64(block, "layout_version");
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

pub(super) fn manifest_image_ref(root: Option<&Mapping>) -> Option<String> {
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

fn generic_field_value<'a>(block: &'a str, key: &str) -> Option<&'a str> {
    block
        .lines()
        .filter_map(|line| {
            let (actual_key, value) = line.split_once('=')?;
            (actual_key.trim() == key).then_some(value.trim())
        })
        .next()
}

fn generic_field_string(block: &str, key: &str) -> Option<String> {
    let value = generic_field_value(block, key)?;
    Some(value.strip_prefix('"')?.strip_suffix('"')?.to_owned())
}

fn generic_field_u64(block: &str, key: &str) -> Option<u64> {
    generic_field_value(block, key)?.parse().ok()
}

fn generic_root_u64(content: &str, key: &str) -> Option<u64> {
    let root_end = content
        .lines()
        .scan(0, |offset, line| {
            let start = *offset;
            *offset += line.len() + 1;
            Some((start, line))
        })
        .find_map(|(offset, line)| line.trim_start().starts_with('[').then_some(offset))
        .unwrap_or(content.len());
    generic_field_u64(&content[..root_end], key)
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
