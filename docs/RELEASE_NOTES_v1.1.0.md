# Rinx @VERSION@

The first Rinx release independent of App Hub commits: Rinx depends on `octosense-app-contract` 1.x from crates.io (OctoSense ADR 0005), not on any App Hub commit. An App Hub change no longer forces a Rinx release, and inside OctoSense, Rinx and the shell link one copy of the contract. It includes everything in [1.0.2](RELEASE_NOTES_v1.0.2.md).

## Changed

- **The app contract:** Rinx reads, checks and runs mini apps with `octosense-app-contract` 1.x. That covers the manifest, the policy an app gets, the bundle digest and publisher signature, and the entry script and assets.
  - It replaces `octosense-app-policy` and `octosense-app-hub` (App Hub `0f332112`) in Rinx, its system-apps and mini-app catalog crates, and the `miniapp-package` tool. No App Hub git dependency is left in Rinx or its lock files.
  - A manifest written for a newer contract 1.x is read by the contract's rules: optional new fields are ignored, and an app that needs a feature Rinx does not know is refused ("needs a newer host").
- **Rinx's own mini-app catalog client:** the App Hub catalog is now read by Rinx's own code (`rinx-miniapp-catalog`).
  - The format is unchanged: App Hub's catalog, packs, anchor and publisher keys. Nothing changes for publishers or for apps already installed.
  - The checks are unchanged: the signed catalog, no replayed older catalog, the 14-day freshness window for installs, digest and publisher signature before install, and withdrawal stops an installed app.
  - A catalog's signature is checked over the catalog as received, so a newer App Hub listing field does not stop Rinx reading the catalog. A listing Rinx cannot read makes that app unavailable rather than shown without its platform list.
- **Rinx's own Splash sandbox:** Rinx builds each mini app's isolate settings from the contract's `AppPolicy`, as App Hub's adapter did for 1.0.x. The capabilities, network hosts, storage quota, instruction budget, heap ceiling and prompt right are the policy's, never more. As before, the app may also reach the local server for its own bundle's files.

Makepad (`1f3b1ded`), Octoscript, octos and app-peers stay at the 1.0.2 revisions.

## Tested

- **Devices:** standalone on a MacBook (macOS) and a OnePlus 6T (Android 11).
  - Both open to sign-in with no crashes.
  - On Android, Back at the sign-in screen leaves Rinx.
- **Release tooling:** the pinned `cargo-makepad` installs from the pinned Makepad revision.
- **CI:** Rinx CI passes (native build and unit tests, portable article core).
- **Catalog:** the production App Hub catalog verifies against the shipped anchor, and a catalog signed by App Hub's own code verifies in Rinx's client.

## Downloads

The packages for each platform are attached below as assets.
