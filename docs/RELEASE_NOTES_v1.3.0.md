# Rinx @VERSION@

Rinx 1.3 adds guided Matrix server selection and native email verification for
Palpo registration, improves Android scrolling and search, and refines chat and
text-field shapes.

## Changes

- Choose matrix.org, tchncs.de or mozilla.org from the server picker, or enter
  your own homeserver. Rinx checks that server's capabilities and offers its
  native or browser registration flow.
- On a Palpo server configured for email verification, request a six-digit code,
  verify it, and create an account entirely inside Rinx. Desktop and mobile share
  the same registration screen, resend timer, error handling and English/Chinese
  labels. Existing invitation-token requirements are retained.
- Changing the email address or homeserver discards the previous verification
  proof. Correcting an invitation token retains the current registration session.
- Improve Android text rendering and avoid repeated OpenGL uniform uploads and
  unchanged link/pill updates while scrolling.
- Collapse the mobile search row when scrolling away from the list beginning.
  The header search button returns to the top, stops a fling and focuses the
  field while keeping the query. Chats and Spaces use their own list position.
- Reduce excessive box rounding, keep text inside its input area, and remove
  the outer chat border.

## Validation and known limits

Native macOS Rinx completed real email registration against the deployed Palpo
on mini2: AgentMail delivery, recipient-supplied code verification, account
creation, automatic sign-in, stored email retrieval and initial Matrix sync.
Expired-code rejection, resend and a server restart during verification were
also exercised. Isolated native checks covered desktop and phone-size layouts,
incorrect codes, changing email, Chinese copy and invitation stages.

Android scrolling and mobile search were exercised on OnePlus 6 and Redmi Note
12. Scrolling can still hitch and is not consistently 60 FPS. A saved dark theme
can initially appear light on Android until appearance is changed. Physical
Android/iOS email keyboard and autofill behavior has not been validated.

Native email registration requires a compatible Palpo deployment with its
`registration_email` configuration enabled. Other homeservers retain their own
registration policy. This release does not add email password recovery or the
separate administrator-approval workflow.

## Packages

- macOS downloads ending in `unnotarized.dmg` use an ad-hoc signature and have
  no Apple Developer ID signature or notarization ticket. macOS may require
  approval in **System Settings → Privacy & Security** to open them.
- Windows installers are not Authenticode signed.
- Android APKs are for sideloading and use Makepad's bundled development signing
  key. They are not Google Play releases.
- iOS and OpenHarmony binaries are not included in this release.

Download the attached packages. `SHA256SUMS` lists their checksums, and
`BUILD_PROVENANCE.json` records the source commit and build run for each package.
