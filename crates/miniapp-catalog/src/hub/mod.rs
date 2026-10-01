//! Rinx's own client for the App Hub catalog.
//!
//! The rules a mini app runs under (its manifest, the policy it gets, the
//! integrity checks) come from the app contract, `octosense-app-contract`
//! 1.x on crates.io (OctoSense ADR 0005). The catalog around them is Rinx's:
//! the wire format App Hub publishes (`catalog.json`, `<artifact>.pack.json`)
//! and the anchor that signs it, read and verified here without linking any
//! App Hub crate, so an App Hub change does not force a Rinx release.
//!
//! What this module keeps from App Hub's device client, unchanged in effect:
//!
//! - a catalog is accepted only with an anchor-certified working key's
//!   signature over its canonical bytes, and never older than the one held;
//! - installs pause when the held catalog is older than
//!   [`CATALOG_FRESHNESS_DAYS`]; running what is installed does not;
//! - a bundle is installed only when its digest, manifest and publisher
//!   signature match the catalog entry and its policy resolves;
//! - a withdrawn entry is neither installable nor runnable.
//!
//! The signature is checked over the catalog exactly as received, so an
//! entry carrying fields this build does not display (a newer listing, a
//! newer optional manifest field) still verifies; anything this build
//! cannot read is then refused or shown as unavailable, never widened.
mod index;
mod pack;
mod remote;
mod signing;
mod store;
mod words;

pub use index::{About, AboutPublisher, Catalog, Entry, Source, Status, WorkingKey, CATALOG_SCHEMA};
pub use pack::{pack_dir, unpack, Pack, PACK_SCHEMA};
pub use remote::{today, Remote};
pub use signing::{sign_manifest, verify_catalog, HubKey, PublisherKeys};
pub use store::{days_between, Availability, Listing, Store, CATALOG_FRESHNESS_DAYS};
