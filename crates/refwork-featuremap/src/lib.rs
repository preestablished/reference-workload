#![forbid(unsafe_code)]

pub const FEATURE_MAP_FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Feature {
    pub name: String,
    pub region: String,
    pub offset: u32,
    pub width: u8,
}

pub fn validate(feature: &Feature) -> Result<(), &'static str> {
    if feature.name.is_empty() {
        return Err("feature name is required");
    }
    if feature.region.is_empty() {
        return Err("feature region is required");
    }
    if !matches!(feature.width, 1 | 2 | 4 | 8) {
        return Err("feature width must be 1, 2, 4, or 8 bytes");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_minimal_feature() {
        assert!(validate(&Feature {
            name: "x".to_string(),
            region: "wram".to_string(),
            offset: 0,
            width: 1,
        })
        .is_ok());
    }
}
