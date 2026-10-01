//! The catalog App Hub publishes, as Rinx reads it.
//!
//! The field names and shapes are App Hub's catalog schema 1. An entry
//! embeds the manifest so that what the hub signed and what Rinx enforces
//! are the same bytes.
use octosense_app_contract::AppManifest;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Whether this version is still offered, and why not.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "state", content = "reason")]
pub enum Status {
    /// Offered for install.
    Offered,
    /// No longer offered and no longer runnable. The reason is shown to the
    /// person.
    Withdrawn(String),
}

impl Status {
    pub fn is_offered(&self) -> bool {
        matches!(self, Status::Offered)
    }
}

/// One app version in the catalog.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// The manifest exactly as the publisher signed it, read by the contract
    /// (a manifest written for a newer contract 1.x is read by its rules).
    #[serde(deserialize_with = "contract_manifest")]
    pub manifest: AppManifest,
    /// The store listing as App Hub reviewed it. Kept as written: Rinx shows
    /// a few of its fields ([`Entry::about`]) and never interprets the rest.
    #[serde(default)]
    pub listing: Option<Value>,
    /// The app's tool manifest as reviewed. Rinx runs no app agents, so it
    /// keeps the field only to read and re-serialise the entry faithfully.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Value>,
    /// Where the hub's copy of the bundle lives, relative to the catalog.
    pub artifact: String,
    /// The publisher's key identity, as the hub knows it.
    pub publisher: String,
    /// The publisher's public key, hex. Empty when admitted unsigned.
    #[serde(default)]
    pub publisher_key: String,
    /// Where the source lives, for transparency. Never fetched.
    pub source: Source,
    pub status: Status,
    /// When the hub admitted it, as an ISO 8601 date.
    pub admitted: String,
}

fn contract_manifest<'de, D: Deserializer<'de>>(deserializer: D) -> Result<AppManifest, D::Error> {
    let value = Value::deserialize(deserializer)?;
    AppManifest::parse(&value.to_string()).map_err(serde::de::Error::custom)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub repository: String,
    pub commit: String,
}

/// The listing fields Rinx shows. Read leniently from [`Entry::listing`]:
/// fields Rinx does not show are ignored, a field it does show must have
/// the right shape.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct About {
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub icon: Option<String>,
    /// The platforms the publisher supports. Required, as in App Hub's
    /// listing: an entry that does not say is not offered on any.
    pub platforms: Vec<String>,
    pub publisher: AboutPublisher,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AboutPublisher {
    pub name: String,
}

impl Entry {
    pub fn app_id(&self) -> &str {
        &self.manifest.id
    }

    pub fn version(&self) -> &str {
        &self.manifest.version
    }

    /// The listing fields Rinx shows. `Ok(None)` when the entry has no
    /// listing; an error when it has one Rinx cannot read, which makes the
    /// app unavailable rather than shown without its publisher's limits.
    pub fn about(&self) -> Result<Option<About>, String> {
        self.listing
            .as_ref()
            .map(|listing| {
                serde_json::from_value(listing.clone())
                    .map_err(|e| format!("The app's store listing cannot be read: {e}"))
            })
            .transpose()
    }

    /// What the person is told this app may do, in plain words, derived from
    /// the manifest rather than from anything the app says about itself.
    pub fn permissions_summary(&self) -> Vec<String> {
        super::words::permissions(&self.manifest)
    }
}

/// The signed list Rinx reads. `signature` covers the canonical form of the
/// catalog without `signature` and `key`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema: u32,
    /// Increases with every publish; a replayed older catalog is refused.
    pub sequence: u64,
    /// When this catalog was signed, ISO 8601.
    pub published: String,
    pub entries: Vec<Entry>,
    /// Hex ed25519 signature by the hub's working key.
    #[serde(default)]
    pub signature: Option<String>,
    /// The working key that signed it, and the anchor's certificate for it.
    #[serde(default)]
    pub key: Option<WorkingKey>,
}

/// The hub's day-to-day signing key, certified by the offline anchor.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkingKey {
    /// Hex ed25519 public key.
    pub public: String,
    /// Hex ed25519 signature by the anchor over the working public key bytes.
    pub anchor_certificate: String,
}

pub const CATALOG_SCHEMA: u32 = 1;

impl Catalog {
    pub fn new(sequence: u64, published: &str, entries: Vec<Entry>) -> Self {
        Catalog {
            schema: CATALOG_SCHEMA,
            sequence,
            published: published.to_string(),
            entries,
            signature: None,
            key: None,
        }
    }

    /// The bytes the hub signs, from this value: used to sign (tests and
    /// tools). A received catalog is verified over its bytes as received
    /// ([`signing_bytes_of`]).
    pub fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        let value = serde_json::to_value(self).map_err(|e| e.to_string())?;
        signing_bytes_of(&value)
    }

    /// The offered entry for an app id, if any.
    pub fn offered(&self, app_id: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|e| e.app_id() == app_id && e.status.is_offered())
    }
}

/// The signing bytes of a catalog as JSON: with its signature and key set to
/// `null` (as the hub serialises an unsigned catalog), in canonical form
/// (sorted keys, no insignificant whitespace).
pub fn signing_bytes_of(catalog: &Value) -> Result<Vec<u8>, String> {
    let mut bare = catalog.clone();
    let object = bare.as_object_mut().ok_or("catalog is not a JSON object")?;
    object.insert("signature".into(), Value::Null);
    object.insert("key".into(), Value::Null);
    Ok(canonical(&bare).into_bytes())
}

/// Canonical JSON: sorted keys, no insignificant whitespace.
fn canonical(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let body: Vec<String> = keys
                .iter()
                .map(|k| format!("{}:{}", Value::String((*k).clone()), canonical(&map[*k])))
                .collect();
            format!("{{{}}}", body.join(","))
        }
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(canonical).collect::<Vec<_>>().join(",")
        ),
        other => other.to_string(),
    }
}
