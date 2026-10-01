# ADR 0006: Shared OctoSense App Hub distribution in Rinx

- Date: 2026-09-26
- Status: Accepted; implemented, with headless native and catalog verification
- Extends ADR 0005's runtime and replaces its local-import-only distribution.

## Problem

Discover → Mini apps exposes a bundle directory, raw room ID and core connection
fields. People cannot browse, install or reopen published apps. The runtime
already understands OctoSense's `main.splash` and L0 `page.card` bundles, but its
developer importer deliberately rejects publisher signatures. Sharing a runtime
does not automatically provide the App Hub installation lifecycle or every host
service.

## Decision

Use the OctoSense App Hub format and publication contract without introducing a
Rinx package extension, a separate catalog or a second publisher identity.
`manifest.json` defines identity, version and authority; `listing.json` supplies
store presentation; the entry is `main.splash` or `page.card` with its kit. Assets
travel in the existing `.pack.json` artifact. Source repository and commit are
provenance, never executable download instructions.

Discover → Mini apps preserves the built-in app catalog. Its App Hub entry
opens a native library with Recent, My apps and Browse.
The built-in article editor is also reachable there. An app's details show its
publisher, version, source and manifest-derived permissions. Add/Update installs
the reviewed version; Open grants a new runtime session to the current Matrix
account and the room selected by name. Developer opens the existing folder import
and standalone Octos configuration; Import an app also remains in the built-in
catalog. Running an app hides the library and
uses the modal's available screen; Back returns to the library and revokes the
instance, following ADR 0005.

Reuse `octosense-app-hub` for catalog signatures, publisher keys, freshness,
listing, pack format, install admission and withdrawal checks. Reuse
`octosense-app-policy` for digest and runtime limits. `rinx-miniapp-catalog` is a
headless Rinx adapter for local persistence, atomic replacement, recents and
compatibility; it owns no signing scheme or script engine. A serial worker does
network/disk operations away from the UI thread. Its data root is account scoped.
No network response or worker result carries authority into another account.

**Amended for Rinx 1.1.0 (OctoSense ADR 0005, the app contract).** Rinx no
longer links `octosense-app-hub` or `octosense-app-policy`, so an App Hub change
does not force a Rinx release. The manifest, policy resolution, digest and
publisher-signature checks and package running come from
`octosense-app-contract` 1.x on crates.io. The catalog client (catalog and pack
formats, anchor and working-key signatures, publisher keys, freshness, install
admission, withdrawal) is Rinx's own `rinx-miniapp-catalog::hub`, reading App
Hub's unchanged publication format and anchor; it verifies a catalog over its
bytes as received. The Splash sandbox is Rinx's own (`src/miniapps/sandbox.rs`),
built from the contract's `AppPolicy` and never wider than it.

Catalog acceptance persists the verified sequence before installation; stale or
unverifiable refreshes do not replace trusted cached state. A verified withdrawal
is honored even when persistence fails. Install consent binds the entire catalog
entry, rechecked before and after downloading. Verify the complete manifest,
publisher signature and bundle digest, then replace the live bundle using a
recoverable rename. Verify again when opening and on the frozen runtime snapshot.
Failed upgrades preserve the previous installed bytes. Removing an app removes
its executable and recent entry; account-scoped app documents remain, as stated
in the UI.

The runtime retains Rinx's account/room leases and Matrix/Octos adapters. An app
requesting unsupported services or agent profiles is listed as unavailable with
a reason. A catalog signature does not grant a missing host service. Built-in
`os.*` apps continue to ship with their host and cannot be installed as store
apps. The article editor is a native built-in, not a fabricated Hub publication.

## Publishing and upstream coordination

App authors use Hub's existing `stamp`, `check`, `scan`, `sign-manifest` and
submission process. The current Hub process is a tagged public source commit
and a submission issue; a maintainer publishes admitted artifacts and the signed
catalog. Do not document a nonexistent release Action. Published versions are
immutable; updates use a new version and withdrawals arrive in a later catalog.

Rinx pins Hub and policy to the same published revision as its built-in catalog,
`362d832f`. Capability admission remains with the shared policy, and Rinx lists
unsupported host services as unavailable. This change neither publishes packages
nor grants access to system app services. Keep both dependencies on the same
revision when coordinating upstream.

Reference publication contract:
https://github.com/OctoSense-org/OctoSense-App-Hub/blob/59004274ef0334b4fc25cd1ebac0caf80547c64a/docs/PUBLISHING.md

## Verification

Use an isolated test Hub with disposable signing keys and real canonical packs
to cover install, signed launch, tampering, changed consent, failed updates,
withdrawal, rollback, offline restart, recents and unsupported services. Test
Rinx's native library navigation, permissions, room selection and Back. An empty
production catalog must display an honest empty state; fixtures must never be
presented as production publications. Record desktop/mobile verification
separately. Matrix app sharing/deep links and additional service adapters remain
separate work; a local install does not share grants or install an app for peers.

## Implementation and validation record

- `crates/miniapp-catalog` wraps the shared Hub client. It persists verified
  catalogs and recents, excludes concurrent writers, checks host compatibility,
  binds consent to the complete entry, stages and verifies downloads, and
  recovers interrupted bundle replacements. Installed apps removed from a later
  catalog remain visible and removable, but cannot run.
- `src/miniapps/library.rs` implements the native library, details,
  permissions and conversation picker. `catalog_worker.rs` owns account-specific
  background work. Source paths and Octos configuration are under Developer.
- `package.rs` admits signed Hub packages through a separate verified path and
  verifies their frozen runtime copies. Unsigned developer import remains
  distinct. `ui.rs` and the app-level Back handler return from an app to the
  library without exiting Rinx or the host shell.
- The author workflow is documented in `examples/miniapps/README.md`; the
  distribution mechanism is the existing App Hub submission and publication
  process. This change does not publish a sample or alter the production catalog.

Initial implementation validation on the MacBook, 2026-09-26 (before the current-main integration):

| Check | Result |
| --- | --- |
| `cargo test --manifest-path crates/miniapp-catalog/Cargo.toml --offline` | 12 passed; live-network test ignored by default |
| Explicit `production_catalog_verifies_with_the_shared_anchor` test | Passed; actual signed production catalog has 0 entries |
| `cargo clippy --manifest-path crates/miniapp-catalog/Cargo.toml --all-targets --offline -- -D warnings` | Passed |
| `cargo test --offline --locked --profile fast --lib octoscript_apps -- --test-threads=1` | 13 passed, including signed frozen launch, existing L0/script tests and Back/navigation |
| Native Makepad headless drawing | Library, details and empty catalog fit 360- and 1024-point widths; script registration errors are captured and checked |
| Standalone library compile | Passed |
| OctoSense module compile | Passed with `--no-default-features --features octosense-module` |

The layout test caught an invalid attempt to hide a PortalList directly. The
library now switches a containing View and completes empty-list drawing through
the standard filler template. Tests exercise both details and the empty public
catalog state. These are headless Makepad drawing checks, not a claim of new
OnePlus touch/keyboard or GPU screenshot verification. No APK was installed by
this change; Matrix/Octos live-service integration is retained from ADR 0005.

## Integration with current main

`src/host/service.rs` owns deployment selection, and `src/host/octos.rs` scopes
requests through the host-owned Octos service. The library
lives under `src/miniapps`, alongside the existing built-in app catalog. Signed
Hub launch and built-in launch both preserve frozen package verification; Hub
packages cannot impersonate a reserved system-app identity. Back unwinds app,
library details, Hub library, and the built-in catalog before closing the modal.
