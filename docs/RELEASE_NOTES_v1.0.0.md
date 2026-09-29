# Rinx @VERSION@

The first tagged release of Rinx, the Matrix chat, Moments and articles app with mini apps and an on-device assistant. It ships standalone (desktop and Android) and as a native app inside OctoSense, which pins Rinx only by release tag ([#37](https://github.com/hagency-org/Rinx/issues/37)).

## Highlights

- **Matrix chat** with Robrix2 Hagency and AppService feature parity ([#38](https://github.com/hagency-org/Rinx/pull/38)), server-aware account registration, and Moments.
- **Articles** with the native editor, SVG actions and compact confirmations.
- **Mini apps:** a catalog, deployment modes, and Octoscript mini apps with scoped Matrix and shared Octos services.
- **Assistant (Octos):**
  - host-owned app peers per ADR 0007;
  - Rinx's own consent;
  - Privacy › Assistant access, with per-room grants that you can revoke.

  Standalone Rinx packages its own pinned Octos runtime.
- **Inside OctoSense:**
  - Rinx runs as a hosted module;
  - its layouts respond to the pane size ([#39](https://github.com/hagency-org/Rinx/pull/39));
  - Back at Rinx's root returns to the host ([#42](https://github.com/hagency-org/Rinx/pull/42)).

## Fixed in this release

- **Back:** it is no longer swallowed at the sign-in screen or room list, whether Rinx runs inside OctoSense or standalone on Android.
- **First launch on Android:** sign-in shows in about 5 s after a fresh install (it was about 36 s), and there is no location prompt at launch.

## Tested

- **Devices:** standalone and inside OctoSense on a MacBook (macOS) and a OnePlus 6T (Android 11). Both open to sign-in with no crashes.
- **CI:** Rinx CI passes (native build and unit tests, portable article core).
- **In OctoSense's build:** there is one Makepad and one App Hub.

## Downloads

The packages for each platform are attached below as assets.
