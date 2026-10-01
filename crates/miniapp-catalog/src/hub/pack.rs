//! A bundle as one JSON document (`<artifact>.pack.json`), as App Hub serves
//! it. The pack carries no authority: Rinx unpacks it into a staging
//! directory and hashes that.
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Component, Path};

pub const PACK_SCHEMA: u32 = 1;

#[derive(Serialize, Deserialize)]
pub struct Pack {
    pub schema: u32,
    /// Relative path to base64 bytes, sorted so a pack is deterministic.
    pub files: BTreeMap<String, String>,
}

/// Pack every file under `root`, the manifest included.
pub fn pack_dir(root: &Path) -> Result<Pack, String> {
    let mut files = BTreeMap::new();
    collect(root, root, &mut files)?;
    Ok(Pack { schema: PACK_SCHEMA, files })
}

fn collect(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            return Err(format!("{}: a bundle may not hold a symlink", path.display()));
        }
        if kind.is_dir() {
            collect(root, &path, out)?;
            continue;
        }
        let relative = path.strip_prefix(root).map_err(|e| e.to_string())?;
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        out.insert(
            relative.to_string_lossy().replace('\\', "/"),
            base64::engine::general_purpose::STANDARD.encode(bytes),
        );
    }
    Ok(())
}

/// Unpack into `into`, refusing any path that is not a plain relative path.
pub fn unpack(pack: &Pack, into: &Path) -> Result<(), String> {
    if pack.schema != PACK_SCHEMA {
        return Err(format!("pack schema {} is not {}", pack.schema, PACK_SCHEMA));
    }
    std::fs::create_dir_all(into).map_err(|e| e.to_string())?;
    for (name, encoded) in &pack.files {
        let relative = Path::new(name);
        if name.is_empty() || relative.components().any(|c| !matches!(c, Component::Normal(_))) {
            return Err(format!("pack names a path it may not: {name:?}"));
        }
        let target = into.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| format!("{name}: not base64: {e}"))?;
        std::fs::write(&target, bytes).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pack_round_trips_to_the_same_digest() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("bundle");
        std::fs::create_dir_all(bundle.join("kit")).unwrap();
        std::fs::write(bundle.join("manifest.json"), b"{}").unwrap();
        std::fs::write(bundle.join("kit/kit.json"), b"{\"k\":1}").unwrap();
        let before = octosense_app_contract::digest_dir(&bundle).unwrap();
        let out = dir.path().join("out");
        unpack(&pack_dir(&bundle).unwrap(), &out).unwrap();
        assert_eq!(before, octosense_app_contract::digest_dir(&out).unwrap());
    }

    #[test]
    fn a_pack_may_not_write_outside_its_staging_directory() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["../escape.txt", "/abs.txt", ""] {
            let mut pack = Pack { schema: PACK_SCHEMA, files: BTreeMap::new() };
            pack.files.insert(name.into(), base64::engine::general_purpose::STANDARD.encode(b"x"));
            assert!(unpack(&pack, &dir.path().join("out")).is_err(), "{name:?}");
        }
    }
}
