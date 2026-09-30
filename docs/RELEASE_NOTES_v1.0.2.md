# Rinx @VERSION@

A maintenance release. Standalone Rinx now builds on the same framework, App Hub and assistant runtime as OctoSense. It includes everything in [1.0.1](RELEASE_NOTES_v1.0.1.md), including the release-packaging fixes.

## Changed

- **Makepad:** standalone Rinx now builds on OctoSense-org/makepad `1f3b1ded` (it was `a5a3cf5b`). OctoSense pins this revision, so standalone Rinx and Rinx inside OctoSense still run on the same Makepad.
  - Every Makepad crate Rinx names moves to this revision, whatever URL it uses. That includes the article editor's Markdown crates.
  - The update brings the framework fixes from OctoSense's Terminal work. Among them:
    - hosted modules can veto a close;
    - a window close asks the app first;
    - text is shaped with OpenType features;
    - Android survives a WebView renderer crash.
- **Octoscript:** Rinx now uses the revisions OctoSense's `native-runtime.lock.json` names:
  - the Makepad runtime moves to Octoscript-Makepad `cb66de07`;
  - the node model moves to Octoscript `dbd48cfb`;
  - the assistant's search engines now run on that same Octoscript.
- **App Hub:** moves to `0f332112`, App Hub on the same Makepad (`1f3b1ded`) and the revision OctoSense pins with it. This applies to Rinx and to its system-apps and mini app catalog crates.
- **Assistant (Octos):** the app-peers contract now comes from current OctoSense.
  - Standalone Rinx's local runtime moves to octos `fe08d8e6`, the revision OctoSense's kernel uses.
  - The packaged `octos` runtime is built from that same revision.

## Tested

- **Devices:** standalone on a MacBook (macOS) and a OnePlus 6T (Android 11).
  - Both open to sign-in with no crashes.
  - On Android, Back at the sign-in screen leaves Rinx.
- **Release tooling:** the pinned `cargo-makepad` installs from this Makepad revision.
- **CI:** Rinx CI passes (native build and unit tests, portable article core).
- **In OctoSense's build:** there is still one Makepad, one App Hub and one octos.

## Downloads

The packages for each platform are attached below as assets.
