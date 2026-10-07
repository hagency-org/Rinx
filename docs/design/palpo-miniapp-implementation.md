# ADR 0010 / 0011 implementation checkpoint

The current implementation follows [ADR 0011](../adr/0011-hagency-server-engagements.md).
Palpo Operations is now a Rust service; Hagency owns engagement resources,
reservations, provisioning and runtime observations. Rinx supplies native
credential custody and the production OctoScript screens. The dated sections
below retain earlier evidence and do not describe the current architecture.

## Current implementation

- Hagency initiates an association. The designated Matrix administrator decides
  it in Rinx; the assigned engagement coordinator decides projects and agents.
  Matrix administration and project ownership do not confer coordinator authority.
- Independent engagement profiles, transport generations and supervised workers
  support multiple engagements with the same homeserver. Rotation preserves the
  appservice registration generation, approved requests and agent identities.
- Resources have explicit parent capacity and engagement allocations. Concurrent
  decisions, top-ups and replays use the runtime's transactional ledger. Unknown
  consumption retains its reservation, including after retirement.
- An approved agent is provisioned automatically through the native factory.
  Runtime ownership and current DM/project routes are required for Ready. The
  UI distinguishes pending delivery, provisioning, uncertain/failed setup,
  unavailable, paused and retired agents. Pause, resume, rename, top-up and
  verified runtime/Matrix cleanup use scoped commands and durable receipts.
- Palpo's Rust service supplies borrowed Matrix sessions, exact capability
  grants, current role checks, durable Inbox/audit/outbox, signup reconciliation,
  association administration, profile export, connection probes, notifications,
  reminders, quiet hours and personal My Actions rooms.
- My Actions embeds the installed app only after account/bundle consent and
  backend verification of the private room. It retains a chat-history fallback.
  Reading or dismissing a notice does not decide its request. HTTPS action links
  must match the current homeserver; custom Rinx links carry only an action ID.
- Project creation can prepare a new room or use the host-owned selector for an
  eligible existing room. The selector filters current joined rooms by creator
  and privacy; Palpo rechecks authoritative state when submitting. Splash never
  receives the Matrix credential.
- Native export uses an account lease through the final write. Desktop uses a
  private atomic replacement; Android checks current authorization before its
  document provider is opened; OpenHarmony uses a native picker/descriptor bridge.
  Splash receives saved/cancelled status, never profile bytes.
- Legacy Node state is retained with stable IDs and immutable history. Native
  receipts support adoption without reallocating approved work. Legacy project
  continuation uses the original request and current explicit resource grants.
  Old contribution and agent-identity bypass routes are retired under ADR 0011.

`PALPO_PROJECT_APPROVAL_REQUIRED` and implicit grandfathered project authority are
not the new workflow's security model. Rust enforces the coordinator/grant gates
on every supported path. The Node writer is fenced during cutover; rollback must
retain decisions made after cutover, not restore an old database snapshot.

## Acceptance evidence and limits

The [coordinator validation record](palpo-coordinator-validation.md) contains
native UI reports and the live isolated Palpo/Hagency lifecycle evidence. The
latest native regression exercises actual Splash callbacks, account isolation,
themes/drafts, notifications, approved-agent state and My Actions. The desktop
Save/Cancel/import/probe acceptance passed 19 checks.

Live acceptance uses the private `rinx-adr0011.test` homeserver and two independent
TLS Hagency engagements. Both have coordinator-approved projects/agents and real
Matrix replies. A live transport rotation reattaches the second agent without
restarting the first. Top-up replay preserves one increase; retirement completes
runtime and Matrix cleanup while keeping unknown usage reserved. Agent work uses
a deterministic native MCP peer, so this proves the execution/Matrix path, not
paid-model quality or provider-reported token usage.

Migration checks exercise the actual Node writer, Rust cutover, native receipt
adoption, restored copies, lock fencing and exact replay. A prior compatible Rust
binary opened a copy of the current database without changing any state or
leased-delivery rows; post-cutover decisions remained present.

Android device checks pass for shared login/consent, live DM/project replies,
My Actions and native profile Save/Cancel/background cancellation. The saved
profile was verified against its active generation and removed from the phone.
Account changes require fresh consent; native guard tests also cover revocation
before the final write. Hosted OctoSense also passes shared login, separate
mini-app windows, live themes, agent status, My Actions and close/reopen with
remembered consent. Its test uses an isolated source override and leaves the
production release tag unchanged.

The native library and complete OpenHarmony HAP build, but installation needs a
signing profile for Rinx and the connected phone. Matrix-room notices and Inbox
recovery are implemented; OS alerts while the mobile app is suspended are not
validated. No native push provider/gateway configuration was found. That
integration and device acceptance, OpenHarmony installation, and production
cutover remain open; the ADR is not fully release-accepted.

## Reproduce the native checks

```sh
cargo build --profile fast --locked --features palpo-instrument \
  --example palpo_miniapp --example palpo_action_room --example palpo_local
python3 tools/wechat-ux/live/native_palpo_coordinator.py \
  --backend /path/to/palpo/target/debug/palpo-operations \
  --binary target/fast/examples/palpo_miniapp \
  --board-binary target/fast/examples/palpo_action_room
```

`native_palpo_local.py` drives the production MiniAppsPanel against the isolated
live server and verifies the result with independent authenticated reads. It
requires a private acceptance account file and only permits the test server.
Evidence and credentials stay under ignored `target/` directories. Freeze live
executables before testing; do not rebuild an executable while it is running.

After generating an OpenHarmony project with cargo-makepad, run
`python3 tools/package-openharmony.py --project <generated-project>` before hvigor.
The installer adds the native document picker hooks and preserves signing
settings. On incremental builds, copy the new cdylib into `entry/libs/arm64-v8a/`
and repeat the hook installer; regenerating the whole project replaces local
project configuration.

App Hub publication is not attempted. The built-in bundle is stamped, but the
store gate still refuses its absent publisher listing. No publisher or platform
claim is fabricated. Companion App Contract and Robius picker changes are vendored
with provenance and licenses pending their upstream releases.

The [archived PR #60 checkpoint](palpo-miniapp-legacy-checkpoint.md) preserves
the earlier Node workflow and native/worker validation history. Its commands and
acceptance claims apply only to the pinned historical revision.

## Checkpoint evidence (2026-10-03)

- Companion Palpo commit: `291b438` (`feat/rinx-miniapp`), 79 backend tests passed.
- Companion App Hub commit: `9854614` (`feat/palpo-miniapp-contract`), 162
  contract/policy/hub tests including doctests passed.
- Rinx catalog: 17 passed, one existing ignored test; origin/action-link tests:
  two passed. `cargo build --release --locked` completed successfully.
- Native run `d23fe8f8a25147b6970b103fb626ff4a`: all five scenario checks passed;
  binary SHA-256 `496df362b08aafdd82624bf3bf9aeddfd922dba283ecce6445a506c7af790c59`.
- Design Flow stamp: `25e82095e7e9a68b300e1553cda7b53702cdfd531e6fee73cde6b417aab3da0a`.
  `octo check --allow-unsigned` reports the expected missing store listing as
  its sole refusal, plus an unsigned-publisher warning. This is not a gate pass.

## Live member validation on mini1 (2026-10-03)

SSH succeeds with the configured key using macOS `UseKeychain=yes`. The public
Matrix endpoint is `https://crew.ominix.io:19443`; the existing admin web service
is `https://crew.ominix.io:19444`. The admin service and PostgreSQL are Docker
containers. The Docker homeserver service name currently resolves to a socat
forwarder targeting the native Palpo process at host port 18010. Both public
Matrix traffic and the admin service reach that active Palpo instance.

`tools/wechat-ux/live/live_palpo.py` ran the actual production Splash app through
Makepad with the current member's real saved Matrix session. It uploaded an
isolated validation backend to mini1 and reached it over SSH port forwarding.
Matrix identity and role checks went to the real running Palpo. Workflow records
went into a new private SQLite database, not the production admin database;
notification workers stayed off. Existing public services were not reconfigured.

Live run `edd7f85cdf9f4917846d20793c4dc7c0` passed:

- Real Matrix whoami and the existing admin web service authenticated the same
  session; a deliberately nonexistent fleet returned 404 for the member and
  401 for an invalid bearer.
- The new app-session exchange succeeded. A member's administrator operation
  returned 403, and administrator decision buttons were absent.
- Native input submitted a contribution under the real member identity into
  the isolated workflow store. Live dark-theme reapply preserved heap and calls.
- Mini-app disconnect left the original Matrix session valid.

Screenshots were inspected, and the native log contains no script/render errors.
The sidecar was stopped after the test. The same updated native binary also
passed the original fixture administrator/owner scenario in run
`f324de6560f44d3a933a4edcd71a7f98`.

This member run did not cover privileged approvals. The saved server-side
administrator/bot credentials found during deployment inspection belonged to an
older test server and returned M_UNKNOWN_TOKEN. The subsequent operator test
below resolves the validation credential problem without changing existing users.

## Live administrator/owner validation on mini1 (2026-10-03)

`tools/wechat-ux/live/live_palpo_admin.py` provisions temporary admin, owner and
notification-bot identities through Palpo's supported no-server operator CLI.
It overrides auto-join rooms for that one-shot process and does not restart the
running homeserver or reset existing users. Credentials stay in private files
and native memory. The production workflow database and public routing stay
unchanged; the sidecar uses an isolated SQLite store and SSH port forwarding.

```sh
python3 tools/wechat-ux/live/live_palpo_admin.py \
  --palpo /path/to/palpo-rinx-miniapp \
  --binary /path/to/target/fast/examples/palpo_miniapp
```

The live native scenario verifies:

- Real admin/member role checks and refusal of owner self-approval.
- Native contribution submission, administrator approval, actual Matrix App
  Service registration and the owner's configuration handoff screen.
- Export authorization: only the owner receives configuration; even an unrelated
  server administrator gets 404. No credentials enter notification cards.
- Private My Actions rooms receive minimal Matrix notices. Marking a notice
  seen leaves the action pending and a later reminder arrives. Only the test
  runner uses accelerated reminder intervals.
- A stale decision returns 409, while an old action reference reads the latest
  approved state. Native rejection returns to owner history.
- Disconnecting the mini app leaves its underlying Matrix login valid.

Run `admin-144a5d328f504fce9046289f033f9f7f` passed all five report checks and
cleanup with the corrected error handling. The native binary SHA-256 was
`965245bde972eaa0e8b89694dce1c24b9ff00424ed20964bdf3a139e09c4a124`.
Admin review, owner handoff and rejection-history screenshots were inspected.
The native logs contain no script/render errors. Cleanup removed the test App
Service, left/forgot the fixture rooms, deactivated and locked test users, removed
their devices and confirmed all test tokens returned 401. Historical room events
remain on Matrix; cleanup does not claim to erase that history. Both public
Matrix and admin-web health checks returned 200 afterwards.

The test exposed a misleading native error: a valid adapter's object-level 404
was reported as a missing adapter. Rinx now distinguishes typed operation errors
from missing routes while keeping upstream message/header text out of diagnostics.
All four Rinx Palpo adapter tests and all eight Palpo mini-app backend tests pass.
The companion live-server runner and documentation are in Palpo commit `9d640bb`.

Public mini-app routing is still undeployed. Native save-dialog completion,
Hagency import/connection, live project/agent lifecycle and OS push entry are
separate outstanding gates; these results do not claim full ADR completion.
