//! The device's side of the catalog: verify, list, install, remove, and
//! refuse to run what has been withdrawn. Refusal-first, as App Hub's own
//! device client: a catalog that does not verify is not a catalog, and a
//! bundle whose bytes do not match its entry is never installed.
use super::index::{About, Catalog, Entry, Status, CATALOG_SCHEMA};
use super::signing::{verify_catalog, PublisherKeys};
use octosense_app_contract::{
    check_reserved_id, digest_dir, policy, AppManifest, AppPolicy, HostLimits, SignatureVerifier,
    MANIFEST_FILE,
};
use std::path::{Path, PathBuf};

/// How stale a held catalog may be before installs stop. Running apps are
/// unaffected: a device kept offline must not become a place a withdrawal
/// never reaches.
pub const CATALOG_FRESHNESS_DAYS: u64 = 14;

/// Days from a catalog's `published` date (ISO 8601, date part) to `today`.
/// None when either date does not parse: an unreadable date is stale.
pub fn days_between(published: &str, today: &str) -> Option<u64> {
    fn ordinal(date: &str) -> Option<i64> {
        let mut parts = date.get(..10)?.split('-');
        let y: i64 = parts.next()?.parse().ok()?;
        let m: i64 = parts.next()?.parse().ok()?;
        let d: i64 = parts.next()?.parse().ok()?;
        if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
            return None;
        }
        let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * m + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        Some(era * 146_097 + doe)
    }
    let a = ordinal(published)?;
    let b = ordinal(today)?;
    Some((b - a).max(0) as u64)
}

/// A catalog that verified, with the text it verified as.
struct Held {
    catalog: Catalog,
    json: String,
    signing_bytes: Vec<u8>,
}

pub struct Store {
    /// The anchor this build trusts.
    anchor_public_hex: String,
    /// Where installed apps live: one directory per app id.
    app_data_root: PathBuf,
    limits: HostLimits,
    held: Option<Held>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Availability {
    Installable,
    Installed { version: String },
    /// Installed, but the catalog no longer offers this version.
    Withdrawn { reason: String },
}

#[derive(Clone, Debug)]
pub struct Listing {
    pub app_id: String,
    pub name: String,
    pub version: String,
    pub publisher: String,
    /// From the manifest: what the app will be allowed to do.
    pub permissions: Vec<String>,
    /// What the publisher wrote, as far as Rinx shows it. None without a
    /// listing, or with one Rinx cannot read ([`Entry::about`] says why).
    pub about: Option<About>,
    /// The hub's artifact path for this version, relative to the catalog.
    pub artifact: String,
    pub availability: Availability,
}

impl Store {
    pub fn new(anchor_public_hex: &str, app_data_root: &Path, limits: HostLimits) -> Self {
        Store {
            anchor_public_hex: anchor_public_hex.to_string(),
            app_data_root: app_data_root.to_path_buf(),
            limits,
            held: None,
        }
    }

    /// Accept a catalog if it verifies and is not older than the one held.
    /// The sequence check stops a replayed catalog un-withdrawing an app.
    pub fn accept_catalog(&mut self, json: &str) -> Result<(), String> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("catalog is not valid: {e}"))?;
        let schema = value.get("schema").and_then(serde_json::Value::as_u64);
        if schema != Some(u64::from(CATALOG_SCHEMA)) {
            return Err(format!(
                "catalog schema {} is not {CATALOG_SCHEMA}",
                value.get("schema").cloned().unwrap_or_default()
            ));
        }
        verify_catalog(&value, &self.anchor_public_hex)?;
        let signing_bytes = super::index::signing_bytes_of(&value)?;
        let catalog: Catalog =
            serde_json::from_value(value).map_err(|e| format!("catalog is not valid: {e}"))?;
        if let Some(held) = &self.held
            && catalog.sequence < held.catalog.sequence
        {
            return Err(format!(
                "refusing catalog {} older than the one held ({})",
                catalog.sequence, held.catalog.sequence
            ));
        }
        self.held = Some(Held { catalog, json: json.to_string(), signing_bytes });
        Ok(())
    }

    pub fn catalog(&self) -> Option<&Catalog> {
        self.held.as_ref().map(|h| &h.catalog)
    }

    /// The held catalog's text, exactly as it verified.
    pub fn catalog_json(&self) -> Option<&str> {
        self.held.as_ref().map(|h| h.json.as_str())
    }

    /// The bytes the held catalog's signature covers.
    pub fn catalog_signing_bytes(&self) -> Option<&[u8]> {
        self.held.as_ref().map(|h| h.signing_bytes.as_slice())
    }

    /// How old the held catalog is on `today` (ISO date). None without one.
    pub fn catalog_age_days(&self, today: &str) -> Option<u64> {
        let catalog = self.catalog()?;
        Some(days_between(&catalog.published, today).unwrap_or(u64::MAX))
    }

    /// Whether installs are allowed on `today`: a catalog must be held and
    /// within the freshness window.
    pub fn installs_allowed(&self, today: &str) -> Result<(), String> {
        match self.catalog_age_days(today) {
            None => Err("no catalog has been accepted".into()),
            Some(age) if age > CATALOG_FRESHNESS_DAYS => Err(format!(
                "the catalog is {age} days old; installs pause until the hub is reachable again"
            )),
            Some(_) => Ok(()),
        }
    }

    /// Everything to show, newest entry per app, with what is installed.
    pub fn listings(&self) -> Vec<Listing> {
        let Some(catalog) = self.catalog() else {
            return Vec::new();
        };
        let mut out: Vec<Listing> = Vec::new();
        // Newest entry per app: the catalog appends, so walk it backwards.
        for entry in catalog.entries.iter().rev() {
            if out.iter().any(|l| l.app_id == entry.app_id()) {
                continue;
            }
            let installed = self.installed_version(entry.app_id());
            let availability = match (&entry.status, installed) {
                (Status::Withdrawn(reason), _) => Availability::Withdrawn { reason: reason.clone() },
                (_, Some(version)) => Availability::Installed { version },
                (_, None) => Availability::Installable,
            };
            out.push(Listing {
                app_id: entry.app_id().to_string(),
                name: entry.manifest.name.clone(),
                version: entry.version().to_string(),
                publisher: entry.publisher.clone(),
                permissions: entry.permissions_summary(),
                about: entry.about().ok().flatten(),
                artifact: entry.artifact.clone(),
                availability,
            });
        }
        out
    }

    /// The publisher keys the held catalog carries. The catalog is signed,
    /// so they are as trustworthy as the catalog itself.
    pub fn publisher_keys(&self) -> PublisherKeys {
        let mut keys = PublisherKeys::new();
        if let Some(catalog) = self.catalog() {
            for entry in &catalog.entries {
                if !entry.publisher_key.is_empty() {
                    keys = keys.with(&entry.publisher, &entry.publisher_key);
                }
            }
        }
        keys
    }

    /// The newest entry for an app.
    pub fn entry(&self, app_id: &str) -> Option<&Entry> {
        self.catalog()?.entries.iter().rev().find(|e| e.app_id() == app_id)
    }

    pub fn install_dir(&self, app_id: &str) -> PathBuf {
        self.app_data_root.join(app_id).join("bundle")
    }

    /// The installed version, read from the app's own copy of its manifest.
    pub fn installed_version(&self, app_id: &str) -> Option<String> {
        let json = std::fs::read_to_string(self.install_dir(app_id).join(MANIFEST_FILE)).ok()?;
        AppManifest::parse(&json).ok().map(|m| m.version)
    }

    /// Admit a bundle unpacked at `staged`, then move it into place. Returns
    /// what the app will be allowed to do.
    ///
    /// The order matters: the entry must offer this version, the staged
    /// bytes must hash to what the entry says, the manifest must be the
    /// entry's manifest, and only then does the policy resolve.
    pub fn install_staged(
        &self,
        app_id: &str,
        staged: &Path,
        verifier: &dyn SignatureVerifier,
        today: &str,
    ) -> Result<AppPolicy, String> {
        self.installs_allowed(today)?;
        if app_id.starts_with("os.") {
            return Err(format!("{app_id} names a system app, which no store may install"));
        }
        check_reserved_id(app_id)?;
        let entry = self
            .entry(app_id)
            .ok_or_else(|| format!("{app_id} is not in the catalog"))?;
        if let Status::Withdrawn(reason) = &entry.status {
            return Err(format!("{app_id} has been withdrawn: {reason}"));
        }
        let digest = digest_dir(staged)?;
        if !digest.eq_ignore_ascii_case(&entry.manifest.integrity.bundle_blake3) {
            return Err(format!(
                "the downloaded bundle hashes to {digest}, the catalog says {}",
                entry.manifest.integrity.bundle_blake3
            ));
        }
        let staged_manifest = std::fs::read_to_string(staged.join(MANIFEST_FILE))
            .map_err(|e| format!("the bundle has no manifest: {e}"))?;
        let staged_manifest = AppManifest::parse(&staged_manifest)?;
        if staged_manifest.id != entry.manifest.id || staged_manifest.version != entry.manifest.version {
            return Err("the bundle's manifest is not the one the catalog admitted".into());
        }
        if let Some(signature) = &entry.manifest.integrity.signature {
            verifier.verify(&signature.key_id, &signature.value, &entry.manifest.signing_bytes()?)?;
        }
        let policy = policy::resolve(&entry.manifest, &self.limits)?;

        let target = self.install_dir(app_id);
        if target.exists() {
            std::fs::remove_dir_all(&target)
                .map_err(|e| format!("cannot replace the installed app: {e}"))?;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        copy_tree(staged, &target)?;
        Ok(policy)
    }

    /// Remove an app and everything stored under its directory.
    pub fn remove(&self, app_id: &str) -> Result<(), String> {
        let dir = self.app_data_root.join(app_id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| format!("cannot remove {app_id}: {e}"))?;
        }
        Ok(())
    }

    /// May this installed app run now? A withdrawal stops an app that is
    /// already on the device.
    pub fn may_run(&self, app_id: &str) -> Result<AppPolicy, String> {
        let entry = self
            .entry(app_id)
            .ok_or_else(|| format!("{app_id} is not in this catalog"))?;
        if let Status::Withdrawn(reason) = &entry.status {
            return Err(format!("{app_id} was withdrawn: {reason}"));
        }
        let installed = self
            .installed_version(app_id)
            .ok_or_else(|| format!("{app_id} is not installed"))?;
        if installed != entry.version() {
            return Err(format!(
                "{app_id} {installed} is installed but the catalog offers {}; update before running",
                entry.version()
            ));
        }
        policy::resolve(&entry.manifest, &self.limits)
    }
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(from).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let target = to.join(entry.file_name());
        if kind.is_symlink() {
            return Err("a bundle may not hold a symlink".into());
        }
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
