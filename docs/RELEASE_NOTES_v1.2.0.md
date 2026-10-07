# Rinx @VERSION@

Rinx 1.2 adds saved Matrix accounts, recent homeservers, customizable shared
themes, and guided Hagency project and agent workflows in native mini apps.

## Changes

- Save multiple Matrix accounts and switch between them while retaining each
  account's device and encrypted local store. Logging out removes only the
  active account. Only the active account syncs and receives notifications.
- Choose a previously verified homeserver from the sign-in picker. Search,
  scrolling and restart persistence work on desktop and narrow layouts.
- Use shared light/dark themes across Rinx and supported mini apps, including
  bundled presets and imported theme packages.
- Follow resource, project and agent workflows through the Hagency mini app,
  with capacity information, capability filters, English/Chinese labels and
  private My Actions boards. Approval and runtime readiness remain separate.
- Agent-request forms submit the edited name, token budget and daily rate.
  Rejected submissions retain the form and show inline guidance.
- Read web links and shared Markdown in a separate tabbed desktop reader.
  Link-preview cards require the homeserver's URL-preview endpoint to allow
  the destination.
- Updated Windows build and assistant-runtime staging instructions.

## Validation and limits

The combined account/history implementation passed 349 library tests and 15
Makepad hidden-window UI checks using isolated Matrix fixtures. Native and
portable CI passed for the merged feature PRs. These checks do not establish
live SSO/E2EE interoperability or physical mobile acceptance for multi-account
switching. Experimental TSP builds refuse account switching.

The native Hagency request-form regression passed on narrow and desktop
layouts. Its visual score remains below the existing 9.5 all-screen target.
The Hagency workflow requires a compatible Rust Operations backend and native
Hagency runtime; installing Rinx alone does not deploy either service.

## Packages

- macOS downloads ending in `unnotarized.dmg` use an ad-hoc signature and have
  no Apple Developer ID signature or notarization ticket. macOS may require
  approval in **System Settings → Privacy & Security** to open them.
- Windows installers are not Authenticode signed.
- Android APKs are for sideloading and use Makepad's bundled development
  signing key. They are not Google Play releases.
- iOS and OpenHarmony binaries are not included in this release.

Download the assets attached below. SHA-256 checksums accompany the packages.
