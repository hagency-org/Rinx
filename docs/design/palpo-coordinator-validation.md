# Coordinator workflow integration

This records implementation and acceptance for [ADR 0011](../adr/0011-hagency-server-engagements.md).
The current checkpoint below supersedes the older dated partial results.
See also the [implementation overview](palpo-miniapp-implementation.md).
It replaces the earlier resource-contribution action: resources are contributed
by their Hagency owner, while Rinx handles requests and coordinator decisions.

The UI labels Matrix administrator and engagement coordinator separately. It
renders only navigation granted by the current server session. Rust Palpo accepts
the reviewed manifest but grants only services already implemented, so the app
can connect during migration without advertising unsupported operations.
Approval state and execution state are displayed separately. Contribution drafts
from the earlier workflow are retained locally but cannot be resumed.

Projects and Agents use the Rust service's role-scoped, paginated read models.
Approved agents appear while allocation is still pending. Missing consumption is
explicitly unreported; reported consumption is a lower bound with freshness,
never an invented remaining balance. Stale readiness becomes unknown. Matrix
administrators do not inherit access to other users' agent lists.

The agent owner can request additional tokens from a current allocation. Rust
binds the form to that agent, project and resource grant; the coordinator reviews
the exact increase in Inbox. Hagency execution is still required before the
displayed allocation increases. A retry returns the original request after the
allocation has changed. Stale provider observations hide the top-up action.

The production OctoScript form sends a decision intent and stable command ID.
Rust Palpo constructs the authority envelope from its authenticated Matrix
session and frozen request. Retrying cannot create another Hagency delivery.

## Validated locally

On 2026-10-04, the actual Makepad instrument host, production Splash bundle and
Rinx HTTP adapter ran against the Rust `palpo-operations` executable. Matrix
authentication and existing engagement/project authority were explicit fixtures.
Three hidden windows used isolated manager, coordinator and administrator data
directories. No existing user session or deployment was opened.

Run `6d7e0802975f493e96e167b94de1b01c` passed all seven checks:

- The project manager can read the agent request but cannot approve it.
- The Matrix administrator has no implicit agent-approval authority.
- The assigned coordinator approves through the real OctoScript form.
- Light, dark and custom themes preserve draft contents, script heap and call count.
- Repeating the command creates exactly one outbound Hagency work item.
- The manager sees `approved` alongside `Execution · pending`, not a live agent.
- Resource contribution is absent from the mini app.

The extended run `532acbbcffc4484288443239c3d1f04e` passed eleven checks,
adding Projects/Agents navigation, pending allocation/unknown usage, current
lower-bound usage, and stale observations. Provider updates are explicit fixtures
sent through the real authenticated machine HTTP route. The pending, current and
stale agent cards were visually inspected. Four Rust Palpo host tests also pass.

Run `ef8d10a573c5479a868109c266b2d5b2` passed all fourteen checks. It adds
native owner top-up submission, native coordinator approval, and replay after
an authenticated provider fixture reports execution. The form, approval and
120,000-token result captures were inspected. This proves the UI/HTTP path;
the provider fixture does not reserve resources or execute a real Hagency agent.

Real captures were inspected: the narrow owner result, desktop coordinator dark
form and administrator Inbox. Runtime logs had no Splash evaluation/callback
errors. Evidence remains under `target/palpo-coordinator-validation/<run>/`,
including the report, binary hashes, input traces, widget captures and screenshots.

```sh
# Build the paired Palpo Rust branch first.
cargo build --profile fast --locked --example palpo_miniapp
python3 tools/wechat-ux/live/native_palpo.py \
  --backend /path/to/palpo/target/debug/palpo-operations \
  --binary target/fast/examples/palpo_miniapp
```

`tools/octo check` restamped the manifest and resolved its declared capabilities.
Its store-publication check refused the missing `listing.json`, as expected for
this built-in app with no publisher/platform submission. No listing or identity
was fabricated. Rinx's own bundle packaging/build validation passed.

## Current acceptance checkpoint, 2026-10-05

The Rust migration and agent lifecycle are implemented. Full ADR acceptance still
requires the outstanding platform/deployment work; passing a desktop fixture
alone does not establish it. Matrix-room notices and durable Inbox recovery are
verified. Suspended-app OS push delivery remains open: the checkout has no
configured native push provider/gateway, and a configuration location has been
requested before choosing that platform integration.

- Native production-Splash regression `6517e8356fa444e2b98e93c95b129d67` passes
  all 26 checks, including role isolation, decision/top-up replay, interrupted
  drafts, live themes, notification preferences and the My Actions board.
- Desktop native Save/Cancel/profile-import/probe run
  `fab3aeaa83224af4a8ba65825b4f7fed` passes 19 checks. The operator explicitly
  handed over the desktop after the earlier interrupted dialog attempts.
- Live Rust Palpo, the real Matrix homeserver and native Hagency ran in the
  isolated `rinx-adr0011.test` deployment. Two simultaneous same-server TLS
  engagements have distinct coordinators, profiles, grants, projects and agents.
  All project and agent approvals used the actual Rinx forms. No second console
  approval was needed.
- The first agent replied in its encrypted DM and project room from Android.
  After live rotation, the second agent also replied in its encrypted DM and to
  an actual project-room mention. Screenshots were captured and inspected.
  Work is executed by the deterministic native MCP fixture; no paid-provider
  measurement or model-quality claim is made.
- Native rotation run `a119fe7f636f49008b2209d6b052800b` moves only the second
  transport to generation 4; registration remains generation 1. The first stays
  online/Ready. Live import reattaches the second without a process restart.
  A bug in one-shot warm-runtime custody was fixed in Hagency `29e44e8`; the
  live report is `target/adr0011-live/two-engagement-rotation-v5-report.json`
  in the Palpo worktree.
- The live top-up increases the same agent from 100,000 to 120,000 tokens exactly
  once. Replay preserves the ledger. Reallocating engagement capacity is explicit:
  a request exceeding the shared parent was refused before owner rebalancing.
- Native retirement run `e4cb06c8f8d74a7a85bf093bb92a5a15` verifies runtime and
  Matrix cleanup. Unknown usage remains reserved. Android My Actions opens the
  latest retired result, including cleanup and usage-settlement status.
- Actual Node-to-Rust migration, native receipt adoption, restored-copy replay and
  writer fences pass. A prior compatible Rust build opens a copy of the latest
  database without changing its state or delivery rows; post-cutover approvals
  and the top-up survive rollback compatibility checks.
- Palpo passes 46 workflow tests, nine library tests and clippy. Hagency passes
  nine provision-runtime tests and all 21 inline-factory tests serially, including
  reconnecting over the same domain writer. One parallel waiter-loss run hit a
  pre-existing timing-dependent intermediate-state assertion; its isolated and
  serial runs pass. Spec validation reports 1,258 bindings with none missing;
  production caller validation reports no missing/unresolved callers and retains
  its existing G8 bootstrap debt.
- Android builds and runs the production app on the connected device. Consent,
  actual My Actions/history navigation, latest retirement and real agent chats
  have been checked. Native document-picker Cancel, Save and background/resume
  pass; the exported JSON exactly matches the active generation-4 profile. A real
  manager-to-owner logout/login required fresh consent and showed owner data.
  Native guard tests separately verify revocation immediately before writing.
  Test exports were removed from the phone afterward.
- The narrowed native room picker run `fec9b0bc30c14a31a3a63e824a22092c`
  passes eligible-room selection, theme reload, cancellation and account
  invalidation without submitting a project.
- The actual OctoSense shell (`59e7dd5`) hosts Rinx snapshot `ac402c3b` and passes
  shared Matrix login/Palpo consent, a separate mini-app window, authoritative
  Ready status, live light/dark theme changes, My Actions navigation, remembered
  consent on reopen and host-window close. Captures were visually inspected.
  The shell build and dependency-graph check pass. This is an isolated local
  dependency override, not a fabricated Rinx release tag or production pin update.
- OpenHarmony Rust and ArkTS/HAP builds pass with the new native document-picker
  bridge. Device installation is awaiting a Rinx signing profile for the connected
  phone.

Android captures are under `target/adr0011-device-validation/`, notably
`android-second-rotation-final-dm.png`, `android-second-project-after-send.png`,
`android-actions-board-real.png` and `android-board-latest-retired.png`.
The Android export report is `android-export-report.json` in that directory.
Hosted evidence is `target/adr0011-hosted/report.json` in the isolated
`OctoSense-palpo-adr0011` worktree, with `palpo-shared-login.png`,
`agent-dark-theme.png`, `my-actions-room.png`,
`reopened-remembered-consent.png` and `closed-miniapp-main-room.png`.
Live fixture accounts, keys, profiles and databases remain private and untracked.

Companion changes: [Palpo #508](https://github.com/palpo-im/palpo/pull/508) and
[Hagency #29](https://github.com/hagency-org/hagency-rs/pull/29).

## Continued implementation, 2026-10-04

The manager's forms now distinguish the engagement allocation from its parent
resource. Rust Palpo prepares owner-authorized Matrix rooms with durable recovery
and freezes the definition before coordinator review. Its HTTP tests cover a
lost createRoom reply, a lost agent event response followed by a new Matrix login,
conflicting retries, funding/role checks and automatic command queueing. The
paired Rust notification worker adds private My Actions delivery, reminders,
quiet hours, privacy revalidation and a pinned Inbox summary.

The desktop window host from commit `9927786e` is integrated here. Real Makepad
window input verified opening, OS close, Back, reopening, theme reapplication and
preservation of the main chat draft/display context. The light catalog and dark
import screenshots were inspected. Evidence:
`target/miniapp-window-validation/51e566f3bcdc4f78a7a3a8a8e128a3e0/report.json`.

The coordinator UI scenario also passed against the rebuilt Rust process,
including the original fourteen checks. Its evidence is
`target/palpo-coordinator-validation/5e9b1bdbeb974031a58b7f039b051f68/report.json`.
The coordinator decision screenshot was inspected. These runs use a Matrix HTTP
fixture; they do not claim combined live Hagency or mobile acceptance.

## Owner association and native connection probe

The paired Rust services now implement owner initiation, one designated Matrix
admin decision, resumable appservice installation, explicitly scoped profile
retrieval and generation-bound connection verification. The Rinx association
action exposes setup retry and authorized export; Engagements separates the last
proof time from current heartbeat connectivity.

`native_palpo_association.py` runs actual Rinx, Rust Palpo and native Hagency
processes against an isolated Matrix HTTP fixture. Hagency's CLI persists and
retries the association, the real OctoScript admin form approves it, native
Hagency imports the matching profile, and the owner's Rinx Verify action sends
the probe. Hagency consumes both delivery lanes and publishes the authenticated
receipt. All eight checks passed; both screenshots were inspected at
`target/palpo-association-validation/93b07861d5bb4ca5b0bc4ddfa0a950b9/report.json`.
This covers native connection proof, not agent execution, live Matrix or a native
save-dialog interaction: the harness retrieves the authorized profile through
the API and writes its private import file.

## Delivered refusals and current delegation

The agent list now retains the coordinator's approval and shows Hagency's
terminal allocation refusal separately, with a readable reason and no pending
allocation claim. Association-only buttons are guarded on non-association
records. The native scenario drives a second approval and its authenticated
capacity refusal, then scrolls the narrow owner list to inspect that result.
All fifteen checks passed and the refusal screenshot was reviewed:
`target/palpo-coordinator-validation/7c195bb18a92451a9b0385a806c090cb/report.json`.

The paired Hagency console now edits the single engagement resource ledger
from both Resources and Server engagements, and displays delivered agent
approvals before Matrix admission. It also records owner delegation revisions,
with suspension/revocation, explicit export recipients and durable publication.
Palpo applies those revisions to current approval/export authority and notices.
These validations remain isolated fixture evidence, not production cutover or
device acceptance.

## Scoped agent lifecycle

The owner can pause, resume, remove and retry definitively failed cleanup through
native forms when the runtime advertises `coordinatorAgentControlV1`. A pending
command, accepted retirement and verified cleanup are separate projections.
Retired history retains its allocation and usage; unknown consumption is not
refunded by retirement. Final accounted usage is labelled separately from runtime
lower-bound observations.

The eighteen-check instrument run passed at
`target/palpo-coordinator-validation/bba39c472bba48da8e45cc259d889739/report.json`.
It drives real native forms and Rust Palpo with authenticated provider fixtures;
the retired-history screenshot was inspected. An earlier hidden-window run
missed the initial Waiting input before reaching the lifecycle flow; failed runs
remain recorded. This evidence does not claim live executor termination.

## Account notification preferences

Notifications now exposes account-scoped delivery/reminder switches, reminder
times and quiet hours, with the native device time zone as an optional suggestion.
The Rust service retains revisioned settings and checks daylight-saving wall
time. Disabled delivery does not decide a pending action; overdue reminders are
combined into one delivery when service resumes.

The native run passed twenty checks at
`target/palpo-coordinator-validation/a66cfbd69adb479aa3dfa58e65d9f7ac/report.json`.
The settings screenshot was inspected. This run saves the coordinator's settings,
reopens them and verifies that the owner's settings remain independent. The
startup harness now waits for the initial Inbox HTTP request to settle before
clicking Waiting, fixing the previously recorded missed input while busy.

## Verified room board and native navigation

My Actions mounts the installed bundle only after exact-bundle consent and a
current private-room/account verification. The native host rechecks the binding
on each service call and on refresh; account changes revoke it and restore chat
history. Agent chat navigation accepts only the service-derived room after both
the user and agent are joined. Signup navigation retains the original approval
event through an invitation join, with account and expiry checks.

The combined native run passed all 24 checks at
`target/palpo-coordinator-validation/cf7023ce7a614e1d802911d2ae987f15/report.json`.
The actual embedded board, theme switch and account-switch screenshots were
inspected. The host suite passed 25 checks and shell navigation passed six.
The instrument uses real Rinx and Rust Palpo processes with isolated Matrix and
provider fixtures; it does not establish live Hagency execution or mobile
acceptance. The scroll helper now requires a visible hit target before clicking.

## Rust signup approval worker

The Signups screen now opens the original verified Matrix approval event through
its explicit native navigation grant. A navigation call sends no verdict. The
Rust worker consumes the bound Matrix decision and projects registration only
after a confirmed ordinary account or original-device reconciliation.

All five native checks passed at
`target/palpo-signup-validation/ffd93453b2d64a028adf42d8cfe0aec5/report.json`;
the pending handoff and terminal registration screenshots were inspected. This
uses the real Rust worker and native Palpo adapter with an isolated Matrix HTTP
fixture. It verifies the source-event handoff, not a live Matrix SDK room join.
The Rust suite separately covers eight signup scenarios, including lost replies,
restart, private-room changes, stale/forged verdicts and revocation during UIAA.

## Credential controls and independent profiles

Administrators can pause, resume, revoke and renew an engagement's Matrix or
transport credentials through native forms. Pending changes retain their exact
intent after a lost reply. Owners see the resulting association state and
notices; a resumed or rotated connection requires a new authenticated probe.
Credential revocation does not assert that an offline runtime stopped.

The combined native run passed all 16 checks at
`target/palpo-association-validation/7a2dc86faa514493888fba9425eefc35/report.json`.
The paused and two-engagement screenshots were inspected. The run pins all three
executables before and after acceptance. One native Hagency process imports two
profiles for the same homeserver, retains both proofs after restart, rejects an
old credential generation, and preserves the first profile when adding the second.
An earlier run completed its functional checks but failed the executable-hash
gate during an overlapping build; it is not counted as passing evidence.

This uses actual Rinx, Rust Palpo and Hagency processes with a Matrix HTTP
fixture. Profile retrieval uses the authorized API, so native save-dialog and
real Matrix/agent/device acceptance remain separate gates.

### Scoped Matrix display-name changes (2026-10-05)

Native run `db6e9f0cfe45422c801846e432a3bdda` passed 21 checks. The actual
Rinx form submits a frozen rename command, shows Matrix verification pending,
and displays the confirmed name after an authenticated observation. Chat remains
usable while only a label update is pending. Captures `owner-rename-pending.png`
and `owner-rename-verified.png` were inspected. This run uses the Rust Palpo
process and a provider fixture; Hagency store and HTTPS-client tests separately
cover real rename execution and read-back. It is not live-provider acceptance.

### Approved project setup recovery (2026-10-05)

Run `0a97a9126f214590a91825d332085946` passed 22 checks with binary hashes
held constant through acceptance. The Projects page shows a failed private-room
join, sends a frozen retry through the native form, and enables Request agent
only after the approved project becomes ready. The original decision and grant
are unchanged. Failure and ready screenshots were inspected. The first run
caught missing optional fields for older project projections; the Rust DTO now
supplies explicit null/false defaults and association views do not read a
project-only property. Evidence uses Rust Palpo plus provider/Matrix fixtures,
not a live provider deployment.

### Private configuration file custody (2026-10-05)

The host now selects a desktop destination before writing any credential bytes.
It creates a private temporary file (0600 on Unix), syncs it, rechecks the
account lease, then atomically replaces the selected regular file. Existing
public permissions and hard links cannot expose the new credentials; symlink
destinations are refused. Cancel returns only `saved:false`. macOS uses the
same native panel/main-run-loop pattern as Makepad's document dialogs.
Mobile continues to use its native document provider.

Two focused tests pass for private replacement/hard-link isolation and lease
revocation before commit. The association harness has `--native-export` to drive
Cancel, Save, direct Hagency import and credential-leak checks. The first native
acceptance attempts were interrupted by concurrent panel interaction. The user
subsequently handed over the desktop; the completed run below supersedes those
failed attempts, which do not count as acceptance. One isolated fixture
profile saved to Documents during that attempt was verified against its test
database and removed; existing user profiles were left untouched.


Native credential export acceptance completed in run
`fab3aeaa83224af4a8ba65825b4f7fed` (19 checks). The owned AppKit Save panel
was captured and driven through Accessibility by PID; Cancel writes no file,
Save selects an explicit destination and writes mode 0600, and the resulting
profile imports into native Hagency and passes the authenticated probe. Export
bytes are absent from Splash, its jail and logs. The same run covers credential
pause/resume, rotation, stale-profile refusal, restart and two independent
same-server engagements. Panel and saved/cancelled screen captures were visually
inspected. All three executable hashes remained unchanged during acceptance.
Matrix remains an isolated HTTP fixture; this does not claim live agent chat or
mobile device acceptance.
