# Pending shared contract release

Exact source from companion App Hub commit `9854614` on
`feat/palpo-miniapp-contract`, based on App Hub 2bcb898. Adds the ADR 0010 exact service grants and their
consent language as version 1.2.0. No admission or integrity rule is relaxed.
Remove the root, standalone miniapp-catalog, system-apps and miniapp-package
Cargo patches when the same shared contract is published. All four consumers
require at least version 1.2 so build, admission and packaging agree.

The additive `palpo.agents.control` grant and consent text match companion App
Hub commit `6180376` on `fix/palpo-project-consent`. It authorizes bounded
project-scoped lifecycle intents; the backend still checks current authority.

The notification read/settings grants match App Hub commit `8ff0654`. The native
host supplies a device time-zone suggestion; only an explicit settings save
changes the account preference.

The vendored manifest, Palpo service registry, policy and growth tests now match
App Hub `6180376`, including the closed native signup/agent navigation targets
and verified My Actions room feature gates. Native integration was adapted from
Rinx `a9aaa2f0`, `8d2ffbd4` and `ee3836bb`; the superseded Node workflow-view
normalizer is intentionally not used with the ADR 0011 Rust projections.

Scoped rename consent now matches App Hub `a93073d`; the same lifecycle grant
is checked for the project owner/resource owner/current coordinator. Runtime
`coordinatorAgentProfileV1` negotiation is required before displaying rename.

Project setup recovery consent matches App Hub `2e2ad26`. The Rust endpoint
checks current project/engagement roles and requires an existing approved grant;
it cannot create a replacement approval.

Native project room selection and its required host feature match App Hub
`978a28f`. Only the selected joined room crosses the Rinx picker boundary;
Palpo independently checks current owner authority before binding it.

Windows bundle hashing now uses forward slashes for the relative file name,
matching the existing Unix digest and manifest values. File bytes, length,
ordering, symlink rejection and integrity enforcement remain unchanged. This
portability correction is local to Rinx pending a shared contract release.
