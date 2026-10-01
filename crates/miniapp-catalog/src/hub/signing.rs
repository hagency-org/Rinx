//! The hub's signatures, as Rinx checks them.
//!
//! Rinx trusts one key, the hub's offline anchor ([`crate::HUB_ANCHOR`]). The
//! anchor certifies the hub's working key, which signs every catalog; a
//! catalog carries the publishers' keys, which sign their manifests.
use super::index::{signing_bytes_of, Catalog, WorkingKey};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use octosense_app_contract::{AppManifest, SignatureVerifier};
use serde_json::Value;

/// A signing key. Rinx devices never hold one: this is for tests and tools
/// that build a catalog to read.
pub struct HubKey(SigningKey);

impl HubKey {
    pub fn generate() -> Self {
        let mut seed = [0u8; 32];
        rand_core::TryRngCore::try_fill_bytes(&mut rand_core::OsRng, &mut seed)
            .expect("the OS has no randomness");
        HubKey(SigningKey::from_bytes(&seed))
    }

    pub fn public_hex(&self) -> String {
        hex::encode(self.0.verifying_key().to_bytes())
    }

    pub fn sign_hex(&self, message: &[u8]) -> String {
        hex::encode(self.0.sign(message).to_bytes())
    }

    /// The anchor certifying a working key: a signature over its public bytes.
    pub fn certify(&self, working_public_hex: &str) -> Result<String, String> {
        let bytes = hex::decode(working_public_hex)
            .map_err(|e| format!("working key is not hex: {e}"))?;
        Ok(self.sign_hex(&bytes))
    }

    /// Sign a catalog in place with this working key, recording the anchor's
    /// certificate.
    pub fn sign_catalog(&self, catalog: &mut Catalog, anchor_certificate: &str) -> Result<(), String> {
        catalog.signature = None;
        catalog.key = None;
        let bytes = catalog.signing_bytes()?;
        catalog.signature = Some(self.sign_hex(&bytes));
        catalog.key = Some(WorkingKey {
            public: self.public_hex(),
            anchor_certificate: anchor_certificate.to_string(),
        });
        Ok(())
    }
}

fn verifying_key(hex_key: &str) -> Result<VerifyingKey, String> {
    let raw = hex::decode(hex_key).map_err(|e| format!("key is not hex: {e}"))?;
    let bytes: [u8; 32] = raw
        .as_slice()
        .try_into()
        .map_err(|_| "key is not 32 bytes".to_string())?;
    VerifyingKey::from_bytes(&bytes).map_err(|e| format!("key is not a valid ed25519 key: {e}"))
}

fn signature(hex_sig: &str) -> Result<Signature, String> {
    let raw = hex::decode(hex_sig).map_err(|e| format!("signature is not hex: {e}"))?;
    let bytes: [u8; 64] = raw
        .as_slice()
        .try_into()
        .map_err(|_| "signature is not 64 bytes".to_string())?;
    Ok(Signature::from_bytes(&bytes))
}

/// Verify a catalog, as received, against the anchor this build ships.
///
/// Both links are checked: the anchor certified the working key, and that
/// key signed these bytes. An unsigned catalog is refused: the catalog is
/// also how a withdrawal reaches the device.
pub fn verify_catalog(catalog: &Value, anchor_public_hex: &str) -> Result<(), String> {
    let key: WorkingKey = serde_json::from_value(
        catalog
            .get("key")
            .filter(|k| !k.is_null())
            .cloned()
            .ok_or("catalog names no signing key")?,
    )
    .map_err(|e| format!("catalog signing key is not valid: {e}"))?;
    let signature_hex = catalog
        .get("signature")
        .and_then(Value::as_str)
        .ok_or("catalog is not signed")?;

    let anchor = verifying_key(anchor_public_hex)?;
    let working_raw =
        hex::decode(&key.public).map_err(|e| format!("working key is not hex: {e}"))?;
    anchor
        .verify(&working_raw, &signature(&key.anchor_certificate)?)
        .map_err(|_| "the anchor did not certify this working key".to_string())?;

    verifying_key(&key.public)?
        .verify(&signing_bytes_of(catalog)?, &signature(signature_hex)?)
        .map_err(|_| "the catalog's signature does not match its contents".to_string())
}

/// Publisher keys by the identity the catalog records. A manifest signed by
/// a key the catalog does not name is refused.
#[derive(Default)]
pub struct PublisherKeys {
    keys: Vec<(String, String)>,
}

impl PublisherKeys {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, key_id: &str, public_hex: &str) -> Self {
        self.keys.push((key_id.to_string(), public_hex.to_string()));
        self
    }
}

impl SignatureVerifier for PublisherKeys {
    fn verify(&self, key_id: &str, signature_hex: &str, signed_bytes: &[u8]) -> Result<(), String> {
        let (_, public) = self
            .keys
            .iter()
            .find(|(id, _)| id == key_id)
            .ok_or_else(|| format!("publisher key {key_id:?} is not registered with this hub"))?;
        verifying_key(public)?
            .verify(signed_bytes, &signature(signature_hex)?)
            .map_err(|_| format!("the signature from key {key_id:?} does not match the manifest"))
    }
}

/// Sign a manifest as a publisher would, for tests.
pub fn sign_manifest(key: &HubKey, manifest: &mut AppManifest, key_id: &str) -> Result<(), String> {
    manifest.integrity.signature = None;
    let bytes = manifest.signing_bytes()?;
    manifest.integrity.signature = Some(octosense_app_contract::Signature::new(
        key_id,
        key.sign_hex(&bytes),
    ));
    Ok(())
}
