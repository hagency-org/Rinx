# Rinx @VERSION@

A maintenance release. Standalone Rinx now builds on the framework OctoSense uses from its Terminal release onward. It includes everything in [1.0.1](RELEASE_NOTES_v1.0.1.md), including the release-packaging fixes.

## Changed

- **Makepad:** standalone Rinx now builds on OctoSense-org/makepad `92a19b67` (it was `a5a3cf5b`). OctoSense pins this revision, so standalone Rinx and Rinx inside OctoSense still run on the same Makepad.
  - Every Makepad crate Rinx names moves to this revision, whatever URL it uses. That includes the article editor's Markdown crates.
  - The new framework fixes are in the Makepad runtime. Among them: hosted modules can now veto a close, and a window close is confirmed by the app first.
- **Octoscript:** the Makepad runtime moves to Octoscript-Makepad `ecab2b98` and the node model to Octoscript `dbd48cfb`. These are the revisions OctoSense's `native-runtime.lock.json` names.
- **App Hub:** unchanged at `e8601b80`, the same revision as OctoSense.

## Tested

- **Devices:** standalone on a MacBook (macOS) and a OnePlus 6T (Android 11).
  - Both open to sign-in with no crashes.
  - On Android, Back at the sign-in screen leaves Rinx.
- **Release tooling:** the pinned `cargo-makepad` installs from this Makepad revision.
- **CI:** Rinx CI passes (native build and unit tests, portable article core).
- **In OctoSense's build:** there is still one Makepad and one App Hub.

## Downloads

The packages for each platform are attached below as assets.
