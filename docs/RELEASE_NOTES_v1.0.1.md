# Rinx @VERSION@

A maintenance release: standalone Rinx now runs on the same Makepad as OctoSense, and the release packages build again. It includes everything in [1.0.0](RELEASE_NOTES_v1.0.0.md).

## Changed

- **One Makepad with OctoSense:** standalone Rinx now builds on OctoSense-org/makepad `a5a3cf5b`, the revision OctoSense pins, with the matching Octoscript-Makepad runtime (`515acb49`). Every Makepad crate Rinx names, under any URL, resolves to that revision, including the article editor's Markdown crates. Before, standalone Rinx ran on an older Makepad (`3c818b16`) than Rinx inside OctoSense.
- **No local Makepad patch:** the Metal fix for dropped draw lists is in Makepad itself, so Rinx no longer carries a patch for it.

## Fixed in this release

- **Release packaging:** the 1.0.0 release jobs failed before building.
  - Android and iOS: installing the pinned `cargo-makepad` stopped with "a member of the wrong workspace". It is now built from the pinned Makepad checkout on its own.
  - Linux: Makepad now links `libdrm`, so the build installs `libdrm-dev`.
  - Windows: the packaging command is passed to `cmd` as one line.

## Tested

- **Devices:** standalone on a MacBook (macOS) and a OnePlus 6T (Android 11). Both open to sign-in with no crashes, and Back at the sign-in screen on Android returns to the launcher.
- **CI:** Rinx CI passes (native build and unit tests, portable article core).
- **In OctoSense's build:** there is still one Makepad and one App Hub.

## Downloads

The packages for each platform are attached below as assets.
