# Archived Palpo mini-app validation before ADR 0011

This is the implementation record from Rinx PR #60 at
`d7a43ffe50bc13116c86e2d24487a1b10986396a`. Its commands, test counts, backend
contracts, and outstanding gates describe that historical revision only.

The current authority model and Rust backend are defined by
[ADR 0011](../adr/0011-hagency-server-engagements.md). Use the
[current implementation checkpoint](palpo-miniapp-implementation.md) and
[coordinator validation record](palpo-coordinator-validation.md) for current
behavior and reproduction commands.

The conflict resolution retains main's replacements for the earlier code:

- Separate desktop mini-app windows and verified My Actions boards.
- Scoped signup and agent-room navigation, reminders, and quiet hours.
- Coordinator approval and Rust-projected lifecycle and resource state.
- Guided bilingual forms, shared themes, account-bound export, and room selection.

The old Node DTO normalizer and its fixture-only connection screen are not
reintroduced. Their fields and authority rules do not match the Rust service.
The original branch commits remain available for reproducing historical checks.

---

# ADR 0010 implementation checkpoint

The user-facing main build remains separate from this development branch. This
implementation builds on the shared Rinx theme branch and the latest reviewed
OctoScript App Design Flow (0e59346e). The bundle was created with `octo new`;
the new host services were implemented in their owning repositories before
the bundle used them. The shared contract starts from App Hub main 2bcb898.

## Full implementation ledger — 2026-10-04

ADR 0010 remains in progress. The role correction is implemented locally; the
running server was restored to its original image after an unsuccessful rollout.
No production activation or client restart is claimed by the local checks below.
Historical contribution tests later in this document describe the earlier model;
they are not acceptance evidence for the corrected Hagency-originated flow.

| Requirement | Current evidence | Remaining acceptance |
| --- | --- | --- |
| One designated Palpo administrator approves projects | Palpo role checks, strict-create migration, distinct native manager/admin controls | Activate strict mode and validate on the live deployment |
| Hagency owns resource contributions | Rinx forms removed; native contribution calls refused; Hagency `e911aae` adds authenticated operator contribution controls and bounded publication | Hagency-originated association/review/handoff; live validation |
| Capacity is reserved before a project becomes allocated | Finite project forms, explicit administrators, prepared owner-bound room, atomic decision/reservation commands, exact receipt projection and partial recovery; native UI fixtures plus actual Palpo HTTP/Rust worker integration | Full runtime/Matrix and live acceptance; explicit legacy migration |
| Assigned project admins decide agents and top-ups | Scoped commands, current-role leases, atomic debit/receipts, assigned-admin Inbox and native forms; actual Palpo HTTP/Rust worker decisions and replay | Actual model execution, live Matrix room readiness and full native workflow acceptance |
| Revocation/expiry fence provisioning and execution | Scoped removal commands; local runtime custody and independent Matrix retirement proof; response-loss, restart and uncertain-cleanup tests | Actual cross-service runtime/Matrix cleanup; live validation |
| Named agent appears and works in its project room | Existing provisioning, earlier live agent setup and ready-agent navigation with native fixture coverage | Corrected grant-based end-to-end approval through actual room readiness |
| Usage and lifecycle management | Same-agent top-ups, scoped stale/unknown usage, owner removal/retry UI and verified cleanup projection; slot/rate release retains lifetime token debit | Live lifecycle and final usage/refund policy acceptance |
| Durable actions and notifications | Inbox/outbox, private notices, stale-card checks; durable per-account preferences, quiet hours, snooze, coalesced reminder delivery and a verified native room board | Cancel/resubmit/expiry parity, room-list pending badge/default pin, richer age/deadline metadata, live delivery and OS entry |
| Identity, permissions and secrets | Native account binding, exact bundle consent, owner-only save boundary | Full logout/account-switch/bundle-update/revocation tests and native save/import acceptance |
| Signup decisions | Authorized original-event navigation, current-role/source verification, account-bound invitation continuation | Full native timeline and real Matrix approve/reject acceptance |
| Shared theme and desktop windows | Native theme-preservation and separate-window tests, including budget, assignment, agent and top-up forms | Hosted OctoSense acceptance |
| Android and OpenHarmony | No device acceptance claimed | Actual build, touch/back/keyboard, background notification and file-picker tests |
| Shared contract and App Hub release | Companion contract implementation and local provenance | Reviewed contract release, remove vendor patch, authentic publisher/platform evidence and publication |

No catalog item, connection proof, queued command or admin click is sufficient to
claim reserved capacity, running agent readiness, or complete cleanup. Each UI
state must follow its owning service's durable receipt. Existing projects and
agents retain their original owners; migration into grants must be explicit.

## Verified My Actions room board — 2026-10-04

Opening a joined, server-verified My Actions room mounts the installed Palpo
bundle in `ActionRoomBoard` / `MiniAppsPanel`. Needs my action, Waiting and
History use the same authorized paginated Inbox as the separate app window.
There is no message-derived task database and no execution of room-supplied
Splash. The normal RoomScreen timeline remains behind the Chat history toggle;
returning to the board revokes the previous app instance and fetches fresh work.
A board polls its binding and, while showing the Inbox, current records every
30 seconds. Its count is a pending-work count, separate from Matrix reads.

The exact `palpo.actions.room.get` grant is read-only. The separate
`palpo.actions.room.ensure` grant, requiring `palpo-actions-room-v1`, explicitly
sets up, joins and navigates to the current account's room. The server checks
bot identity, private membership, power levels, purpose and a saved revision.
Leaving stops delivery. Explicit setup can replace an invalid room; revisioned
aliases recover a lost create response without making another valid room or
losing pending work. Frozen notification envelopes cannot keep targeting the
abandoned binding. App Hub companion `d9c790d` owns these additive grants; older
Inbox read permissions do not acquire room setup authority.

Rinx probes only after the current account consented to the exact installed
bundle digest. Every board operation revalidates its server binding, and account
changes revoke queued requests and discard the board. Theme reload retains the
active view. The board does not overwrite the assistant's separate foreground
app context. Regular mini-app navigation keeps the existing desktop window.

Validation: **148 backend tests**, **31 contract tests** (also rerun at
App Hub `6180376`), eight Palpo adapter tests and the native board lifecycle
unit check pass. The ordinary optimized Rinx release build passes without
the instrument-only feature. Hagency's two selected integration checks pass against Palpo
`8ccc7b0e52f723e2c931946c9350a02a0824decd`, including the actual HTTP/SQLite
worker receipt and restart path. The final Makepad run
`938208777f8149b3b842f3bb78890d7f` passes **16 checks**. The light, dark and
chat-fallback captures were inspected. Its report records both example hashes
and the exact production bundle digests. It explicitly verifies that History
remains visible, pending detail opens, theme reload survives and account switch
removes the board. Normal history-search navigation and the room-info button
return to the ordinary timeline instead of being swallowed by the board.

The initial instrument failures exposed a dynamic Size scope error and a
fixture widget-ID collision; both were fixed. Long quiet-hour forms are tested
by scrolling to their real widgets. A separate run hit the existing hidden-window
script startup budget; the successful full run did not bypass that budget or
replay input. This evidence supersedes the unsuccessful runs.

The instrument-only `palpo_action_room` example mounts the actual native board
and panel with a fixed local SDK account, the actual HTTP adapter and production
bundle. Its ordinary timeline is an explicit fixture label, so these checks do
not prove a live Matrix timeline, OS notification delivery or a mobile build.
The feature is absent from ordinary Rinx builds. Production and visible user
sessions remain unchanged.

## Account notification preferences and quiet hours — 2026-10-04

Inbox now opens Notification settings inside the same themed mini app. Users can
turn workflow notifications or reminders off, edit the reminder offsets, and
save quiet hours with an explicit timezone. The host suggests its detected IANA
timezone without silently saving it. Server-side preferences belong only to the
current account; explicit read/write grants from App Hub `8ff0654` keep read
access separate from settings changes. Revision checks prevent another device's
edits being overwritten; exact retries after a lost response remain idempotent.

Quiet hours follow local clock changes, including both daylight-saving boundaries,
and are rechecked after asynchronous room verification. Disabled recipients do
not starve enabled users in a delivery batch. A delivered message coalesces the
cadence points missed during downtime, snooze, quiet hours or retries. Toggling
settings does not restart a completed schedule. The server stores the exact
message envelope before delivery, so response-loss retries preserve their Matrix
transaction, room and content. Action-required messages mention their recipient;
informational messages stay quiet. Neither includes configuration secrets.

Inbox cards show overdue reminder schedules and snooze status independently of
workflow completion. Reading, disabling notifications and reaching the last
reminder leave the action pending. Explicit snooze works after the default
schedule ends, subject to notification preferences. Workflow expiry remains outstanding. The later room-board checkpoint above
adds a native projection without changing reminder completion semantics.

Backend validation passes **141 tests**, followed by **21 targeted checks** after
the final notification lookup optimization. The shared contract passes **30
tests**; seven Rinx Palpo adapter tests and the optimized release build pass.
The actual Hagency worker/Palpo HTTP integration passes both selected tests
again at backend `9d769b27d4125888f98137bf6d6168ef12ca1d7a`.

Final Makepad run `cb3cf6ae115f4446ad54be6567588212` passes **15 checks**, including
editing and saving cadence/quiet hours, reopening the settings, and verifying
that the other account remains unchanged. Both quiet-hour captures were inspected.
The existing role, project, agent, top-up, removal, recovery, signup, theme, draft
and disconnect cases also pass. Example SHA-256:
`ea30ec35a5f8a764bb1bf5760e909d9078ba4efe5dc470930c986c96476ba633`.
The report pins both bundle files. The harness scrolls long content into view
and retries only the SDK's explicit transient read-only frame-capture failure;
inputs and submissions are never replayed by that retry.

These are local HTTP/Matrix fixtures and hidden macOS Makepad windows. Production
and visible user sessions remain unchanged; no mobile OS notification delivery
or complete ADR acceptance is claimed.

## Open a ready agent's project room — 2026-10-04

My agents now offers **Open agent chat** only after the server verifies readiness.
The new exact `palpo.requests.open` grant requires `palpo-agent-navigation-v1`
(App Hub `8c4035f`). The script supplies only its request ID. Palpo refreshes that
request's provider status and Matrix membership, rechecks current scope and
returns an account-bound room/agent destination. Stale observations, paused
fleets, expired grants, changed ownership/revisions and removal refuse opening.
Approval or transport receipts alone cannot enable the button.

Rinx validates the closed destination and original request ID, then rechecks the
current account and joined-room state. Success closes the mini-app window and
selects the project conversation. An unsynced/invited room leaves the app open
with a retry notice; this action does not join rooms, send messages or approvals.
Older request DTOs default to no navigation permission.

Validation: optimized Rinx and instrument builds pass. Palpo `3fe92d7` passes
all **134 backend tests**; the shared contract passes **29 tests**. Rinx's
agent-chat filter passes **67 tests** with one existing ignored, including the
new app-shell account/membership guard and closed-target validation. All **seven Palpo adapter tests** also pass. The real Hagency worker/Palpo HTTP
integration passes both selected tests again at the exact `3fe92d7` revision.

Makepad hidden-window run `4f378746da2b44eaadfd58c9b1e19147` passes **14 checks**.
The new case first confirms that an applied approval has no chat button, then
publishes an explicit readiness/Matrix-membership fixture and clicks the actual
production Splash button. The native adapter resolves the correct account,
room and agent with zero additional Matrix mutations. Its ready-agent capture
was inspected. Example SHA-256:
`af4f310bff11a3081bed58b6a0b58fbe2e921662a12c77984e420353f44ed5cb`.
The local report also pins both bundle files. Existing role, budget, theme,
draft, signup, top-up, removal and reservation-recovery checks remain passing.

The instrument records the trusted destination; it has no logged-in SDK timeline
or actual model runtime. Live conversation/rendering and full ADR acceptance
remain open. Production and visible user sessions were not changed.

## Actual Palpo HTTP / Hagency worker integration — 2026-10-04

Hagency `c506b58` runs its production command consumer and status publisher
against the actual Palpo HTTP/session/workflow/SQLite modules at `06b888e`.
Contributions, reservations, admission/rejection, token increases, revocation and
unused release use Hagency's real ledger and business receipts. This bridges the
previous split between native UI tests with fake Hagency results and Rust tests
with fake Palpo responses.

The final opt-in run passes both selected worker tests. Eleven commands produce
nine applied receipts and two current-authority refusals. Transport ACKs do not
allocate capacity. A dropped committed publication response followed by a
Hagency store restart replays identical bytes without extra sequence advancement
or notices. Duplicate admission/top-up deliveries preserve one engagement and one
increase from 80,000 to 120,000 tokens. Partial retry and unused release leave
1,600,000 cumulatively reserved tokens and 400,000 verified unused release, with
no pending delivery or legacy Hagency human-approval request.

Real status publication preserves unknown usage and never reports a ready agent
without provisioning. Removal before provisioning uses durable no-cleanup/no-
identity evidence; it does not assert Matrix deactivation. The peer explicitly
fixtures Matrix rooms/identities and opts into the withheld workflow capability.
No model process or real Matrix homeserver runs in this test. Full runtime/Matrix
acceptance and the broader ledger remain open; production is unchanged. The
command and pinned source checks are in Hagency's project-grants review document.

## Failed project allocation recovery — 2026-10-04

The designated Palpo administrator can recover a partially refused allocation
after every original reservation has a result. Retry sends new commands only for
refused reservations and preserves the original owner, room, limits, expiry and
assigned administrators. Release closes a failed allocation only after Hagency
confirms its accepted grants have never funded an agent. Pending or refused
release keeps capacity held. The owner sees the closed result in History and can
request a new project; the previous room and history remain intact.

This uses the separate `palpo.inbox.recover` grant from App Hub `3862c36`.
Palpo `7af9ee2` rechecks current designated authority, original scope and fresh
capacity, commits recovery/commands/notices atomically, and keeps exact retries
idempotent. Hagency `1e40151` adds the typed unused-release receipt and schema-64
marker. Any lifetime agent debit refuses release, even after removal. Grant rows
remain permanent admission/replay fences. Cumulative reserved/released counters
are monotonic; their difference is held capacity. The optional peer extension
`projectWorkflow.unusedRelease: true` gates release commands.

Approval details now name each resource beside its budget and refusal result.
Readable explanations replace raw reservation refusal codes. Older DTOs remain
readable without enabling recovery controls.

Validation: **130 Palpo backend tests, six Rinx adapter tests and 28 shared
contract tests pass**. Hagency store/Palpo regressions pass **645 tests** with
50 existing ignored; the final 12 project-command tests, shared eight-vector
wire corpus, production host check and schema-64 console asset build also pass.
Optimized Rinx and instrument example builds pass.

Final Makepad hidden-window run `09c5cfe34129477da76b3bdadd797d81` passes all
13 checks, including partial retry, pending release, exact fixture receipt,
owner preservation and return to current resources. It also retains the earlier
three-role, budget/assignment, top-up, removal, signup handoff, draft, theme and
disconnect checks. Resource-name and pending/completed release captures were
inspected. Example SHA-256:
`dbb9388baa25ec4b9372e4f337b5ff1719ef20f7731e52d71f2c14618840facf`.
The report pins both production bundle files. An earlier 13-check run predates
the resource labels and readable errors; the final run includes both.

These tests exercise the actual Splash, native transport and local Palpo
HTTP/SQLite with explicit Matrix/Hagency fixtures. Actual cross-service/live
recovery, same-project migration, allocation increases, broader cancellation and
the full acceptance ledger remain. Hagency still withholds workflow capability;
production and visible user sessions were not changed.

## Signup approval navigation — 2026-10-04

Signups now offers Open signup request to configured account approvers. The new
exact `palpo.accounts.open` grant requires `palpo-account-navigation-v1`; an older
host or a read-only signup grant does not acquire this capability. The bundle
supplies only a request ID. Palpo verifies the private approval room, current
approver role and membership, original bot event, request/tool/digest/approvers,
and that the source remains current after asynchronous checks.

Rinx closes the mini-app window and navigates to the exact trusted Matrix event
under the same signed-in account. For an invitation, it preserves the event while
the user joins. An expired continuation, account change or selection of another room drops
the pending event and its matching generic room waiter. A new signup navigation
also replaces an older room waiter. Opening neither joins automatically nor sends
a verdict; existing trusted Matrix controls and the account worker still own
approve/reject. Account approval is separate from project approval authority.

Validation: all 125 Palpo backend tests pass; the final source-check ordering
passes the 26 focused account/mini-app tests. Six Rinx adapter tests plus the
app-shell invitation continuation regression pass, as do all 28 shared contract
tests. Optimized Rinx and instrument example builds pass. Makepad hidden-window
run `ff509030317f46a183685c646d32d5ef` passes all eleven checks, including original
signup-event resolution without a verdict or account creation, and the earlier
budget, assignment, top-up, removal, draft, theme and disconnect flows. The actual
signup captures were inspected. Example SHA-256:
`620ee142411abe2ec2840a991f3f7554f27ea30481551e98b1397432453ff348`.
The report also pins the production Splash and manifest digests.

The isolated native example records the validated host handoff without an SDK
login. It cannot prove real timeline rendering, invitation acceptance or a Matrix
signup verdict. Those acceptance checks remain open; production is unchanged.

## Agent removal, cleanup receipts and usage — 2026-10-04

Owners can remove an allocated agent from My agents, with a reason and stable
request ID. Current assigned project administrators have the same scoped backend
authority. This does not ask Hagency for another human approval. A queued or
applied command keeps the action pending until runtime cleanup and Matrix
retirement are independently verified. Definite failures permit one fresh cleanup
attempt; uncertain outcomes ask for operator inspection. Old cards resolve to the
latest action, and a stale failure cannot offer another retry after a new attempt.

Hagency publishes its recorded Matrix identity, local effect attempt/custody,
independent retirement receipt, allocation and observed usage lower bound. Its
fixed retirement call verifies the response and journals the exact target under
a current-registration transaction. Restart/lost replies preserve idempotency.
Rinx distinguishes unknown usage and stale observations. Verified removal releases
concurrency and daily-rate allowance; lifetime token allocations are not refunded
from incomplete usage evidence.

Validation: Palpo's 122 backend tests, five Rinx adapter tests and 27 shared
contract tests pass. Hagency's store/Palpo regression passed 642 tests (50 existing
ignored); the final focused lifecycle/command/TLS run passed 25 tests. Both
optimized Rinx binaries build. Final Makepad hidden-window run
`15aae70b2e0b4e68b49babddaf6496bf` passed all ten checks through the production
Splash and native adapter, including owner removal, pending cleanup, definite
failure, a single retry, verified completion, three role sessions, budget/top-up
forms, drafts, themes and disconnect. Captures were inspected.

The final example SHA-256 is
`cfb047d7c78b154d562f3352084935e8e09612c36db71280443f4492d68e1941`;
main.splash SHA-256 is
`48d048da3b5f5d9f0fb53d4cbea4886241ee1a8aa823ba34a84c05444bbddada`.
The report also pins the manifest. An earlier ten-check run passed before the
final retry/refusal refinements. All new UI evidence uses isolated local
HTTP/SQLite and explicit Matrix/Hagency fixtures, not a real runtime lifecycle.

The full ADR remains open: actual cross-service/live acceptance, Hagency-originated
association, partial-refusal recovery, signup decisions, richer My Actions and
notification preferences, hosted/mobile validation and release are unfinished.
Production and visible user sessions remain unchanged; Hagency still withholds
the workflow capability.

## Agent decisions and same-agent token increases — 2026-10-04

Finite-grant agent requests now become durable Inbox actions for explicitly
assigned project administrators. The original Matrix request event is retained;
the legacy Hagency human-approval queue receives no duplicate request. Project
owners can inspect the result, and unassigned server admins cannot inspect or
decide it. Decisions recheck active identity, current assignment/self-approval
policy, room binding, contribution and remaining capacity, then atomically commit
the source-bound command, audit and notices. Applied admission does not claim
runtime readiness. Independently paginated status updates wait for the exact
decision receipt rather than preventing earlier receipts from being delivered.

The owner's Request more tokens form keeps the original engagement. Its Inbox
action allows the assigned admin to approve a smaller positive increase or reject
it. Confirmed tokens and amounts awaiting Hagency's business receipt are shown
separately. Exact request, decision and receipt retries preserve one increase;
rejected increases enqueue no remote work. Grant capacity remains held until a
defined, verified cleanup releases it. Assigned-recipient notifications also
recheck account and grant authority. Shared consent wording now describes project
and token requests, rather than asking a manager to contribute resources.

Validation: all 117 Palpo backend tests and five Rinx adapter tests pass. Makepad
hidden-window run `3b04a89353644c85b2cd4c2cbe4b3265` passed all nine checks,
including the earlier budget/theme/draft checks, assigned-admin agent approval,
and a 50,000-token request approved for 40,000, moving the same agent from 80,000
to 120,000 confirmed tokens. Actual manager/admin screenshots were inspected.
The test uses the production Splash file and host adapter with local HTTP/SQLite;
Matrix and Hagency receipts are explicit fixtures, not live lifecycle evidence.

The native report pins both runtime bundle files and the binary, SHA-256
`a1d24c8e5ba9918f3e0c3be147d5cec00a8c3a8ece98e5feb727f74494f0b4d4`.
One earlier unoptimized run exceeded Makepad's 64 ms script-entry limit during a
concurrent build. The passing run kept the same runtime limits. A subsequent
driver failure came from a fixed click-height cutoff; it now uses the actual
window height. Failed startups preserve evidence and use activity-guarded cleanup,
and budget errors fail validation even when Makepad logs them at info level.

The final optimized Rinx and instrument example also build successfully. Release
smoke run `e2081aca9be54863a33dbd288d5ad151` passed startup, legacy-draft,
role-control and live theme/focus/selection/undo checks without budget errors;
example SHA-256 `126dd8d710ab7c7f92421d7c35fdeb58abd461a86c89531ed447ae1b707cd2bf`.

The capability remains disabled in Hagency pending actual cross-service and
lifecycle acceptance. Production and visible user sessions remain unchanged.

## Finite project approval and reservation — 2026-10-04

The request form selects a current Hagency contribution and asks for a token
budget, maximum agents, aggregate daily rate and duration. Palpo prepares the
requester's actual project room before review, using their current Matrix token.
This is a proposal, not an allocated project. Stable request/room IDs recover
lost Matrix responses without transferring ownership or creating another room.

The designated administrator explicitly assigns active local project admins and
chooses the self-approval policy. Palpo rechecks the room's owner, privacy,
binding, contribution revision and remaining capacity. Human decision, finite
reservation commands, audit and notices commit together. Queued promises count
against availability; transport ACKs do not allocate. Only exact applied business
receipts for all resources enable new agent requests. Refused/partial reservations
stay unallocated, with held capacity preserved. Existing unbudgeted projects gain
no implicit grant; an exact historical operation can still resume its own retry.

Validation: all 106 Palpo backend tests pass, including room response loss,
invalid/stale budgets, missing administrator policy, changed ownership, queue
rollback, competing approvals, transport rotation, receipt replay and refusal.
Five Rinx Palpo adapter tests pass, including older-server DTO compatibility.
Full native Rinx and the instrument example build successfully.

Makepad hidden-window run `df4979918dcd415f8480a9f5c3111e30` passed seven checks
through the production Splash bundle and native host: manager/admin controls,
finite-budget submission, explicit assignment, pending versus applied result,
draft restart, old approval-draft compatibility, theme/focus/selection/undo and
Matrix-login-preserving disconnect. Captures were inspected. The example's SHA-256
is `f5b56cf8efc48e52e830e27232082f8de86b7ea84311d726157f091a616d7bc7`.

Matrix and Hagency in this run are explicit local fixtures. The test releases a
synthetic business receipt after capturing the pending state; it does not prove
live Hagency reservation or agent admission. Assigned-admin agent/top-up Inbox
integration remains unfinished, so Hagency still does not advertise the full
workflow capability. No production activation, visible client restart, mobile
device acceptance, or full ADR completion is claimed.

## Implemented

- A built-in Palpo Splash app in `apps/palpo`: member/admin navigation, persistent
  drafts, Inbox views and pagination, project requests and designated-admin decisions,
  owner activation, named-agent requests, fleet and Matrix-identity operations.
- Shared Rinx semantic colors, fonts, controls and live theme reapply. Theme
  changes preserve the script heap, forms, focus, selection, undo and request IDs.
- Exact `palpo.*` service grants in App Contract 1.2, consent bound to the active
  account and exact build-owned bundle digest, and revocable instance leases.
- A native adapter bound to the authenticated homeserver origin. Matrix/app
  credentials stay outside Splash. The owner-only configuration result goes
  directly to a native save dialog; the script receives only saved/cancelled.
- Palpo's passwordless app-session adapter, durable SQLite approval records,
  resource grants, audit/outbox, private My Actions notification rooms and reminders.
  Same-origin action links in Rinx open the latest server-authorized action.

Palpo shares the browser's existing backend and serial mutation queue. It checks
current roles and ownership on the server. Strict project approval across all
frontends is an explicit deployment migration, `PALPO_PROJECT_APPROVAL_REQUIRED=1`;
existing projects and owners remain intact. New agent requests need finite grants;
exact legacy operation retries remain resumable. See Palpo's `web-admin/MINIAPP.md`.

## Role correction validated on 2026-10-03

Project creation approval now belongs to one configured Palpo administrator.
The mini app presents Project approvals to that account and My Inbox / My projects /
My agents / Request a project to managers. Rinx no longer submits contributions
or registers fleets; older native calls are rejected by Palpo. Historical
contribution records and connected resources are preserved. An older Palpo server
without `canApproveProjects` does not implicitly grant approval UI to a Matrix admin.

Palpo `f895acdd20` adds the role check, including queued decisions and notification
recipients. Deployment requires `PALPO_PROJECT_APPROVAL_REQUIRED=1` so existing
browser/API routes cannot bypass approval for new projects. Existing projects
remain owned by their original requester; this does not transfer littlewhite.

Validation: 85 backend tests (14 mini-app workflow/role cases), 323 Rinx library
tests passed (2 ignored), full native build, and Makepad hidden-window run
`e1e1bc6c37c64f199a81d0917417eb89`. The native run covers distinct manager/admin
controls, project approval and owner activation, named-agent submission, draft
recovery, theme/focus/selection/undo preservation and disconnect. Real captures
were inspected at 430px manager width and desktop admin width. Connection
regression `85f0e3372cbd4630a79851e66f1c7afa` passes pending-proof polling,
navigation cancellation and refusal handling with admin connection maintenance.

These are native/macOS checks, not Android or OpenHarmony device acceptance.
Hagency-originated automatic pairing and assigned-project-admin agent admission
remain the backend gaps recorded in ADR 0010.

## Hagency reservation foundation — 2026-10-04

Companion Hagency commit `86099fc` adds finite contribution/project grants,
explicit project-admin policy and revision changes, atomic agent/top-up debits,
financial replay receipts and grant retirement. It counts provider reservations
once across projects and agents, including resources sharing a provider seat.
Expired/revoked grants fence queued or late provisioning and runtime capability
use. Legacy console approval cannot bypass a grant. Held resources cannot move
to another account implicitly, and lower ceilings still constrain new decisions.

Evidence: 595 store regressions passed (50 ignored), followed by 46 focused tests
on the reviewed changes, including 15 grant scenarios. The native Hagency build
check passes. The expiry test crosses an actual SQLite lock; financial replay is
checked after restart and removal of the rolling display-history receipt.

This is the accounting foundation. The operator contribution UI, authenticated
Palpo command/receipt consumer, role synchronization, Rinx approval/top-up screens
and full live lifecycle still require integration. No workflow support capability
is advertised by this commit, and no production update or Makepad/mobile result
is claimed for these backend tests.

## Authenticated command/receipt transport — 2026-10-04

Hagency `6771862` and Palpo `bb593b6` add seven closed operations for project
reservation/assignment, agent approval/rejection, top-up and project/agent
revocation. Palpo's enqueue API requires the human decision's SQLite transaction;
Inbox still needs to invoke it. A fixed authenticated machine route returns a
short lease after reading the current Matrix actor and assigned role. Hagency
checks that lease after acquiring its writer lock and refreshes it after slow
Matrix observations. This bounds the distributed authorization window; it does
not claim instantaneous revocation of a previously issued lease.

Admission, budget debit, provision effect and immutable business receipt commit
together. Transport custody ACKs do not allocate a project. Lost-response/restart
retries preserve the original publication bytes and acknowledge only included
receipts. Replayed historical receipts cannot restore an older assignment or
revoked grant. Statuses use durable workflow evidence and acknowledged pages
scoped to the current fleet registration; later agents are no longer omitted by
a fixed first-100 scan.

Evidence: 667 Rust core/store/Palpo tests passed (74 existing ignored); final
strict wire-corpus test and native executable check passed. Palpo's full suite
passed 93 tests, then 23 focused tests passed after the final transaction guard.
The shared corpus pins all seven command digests and receipt shapes in Rust/Node.
See Hagency `docs/reviews/2026-10-04-palpo-project-grants.md`.

This remains a development backend. Contribution controls/snapshots, explicit
project budget/administrator forms and assigned-admin Inbox command/result
projection must be connected before advertising support. No production update,
visible client restart or new Makepad/device acceptance is claimed. Full ADR
0010 remains active, including the broader acceptance ledger above.

## Operator contribution controls and publication — 2026-10-04

Hagency `e911aae` adds finite contribution creation/revocation on its own resource
page for an already connected Palpo registration. It checks the original console
session, resource revision, current registration and account. Saved exact requests
recover from response loss/reload without reserving twice. Revocation leaves
capacity held through cleanup. Rinx still has no resource-contribution form.

Sixteen-row contribution pages travel through frozen outbound updates. A lost
acknowledgement/restart resends the original bytes before advancing its cursor.
Pages exclude foreign fleets and old registrations. Palpo `b45b3fd` commits
validated snapshots with the update, refuses changed budgets/decreasing held
reservations/retired-state restoration, and fences old accepted project grants
after registration rotation. An empty page is not a refund or deletion.

Validation: 26 focused SQLite store tests, eight TLS catalog tests, four actual
HTTP/browser contribution tests and all 98 Palpo backend tests passed. Production
console assets built; desktop and 430px captures were inspected. The browser walk
uses an isolated real SQLite store and drops a committed response before reload
and exact retry. This is Hagency browser evidence, not a new Makepad/device run.

Remaining: Hagency-originated association, Palpo/Rinx project budgets and explicit
administrator forms, assigned-admin Inbox commands/results, full live lifecycle
and the broader gates above. No workflow capability, production activation or
visible session restart is claimed by this checkpoint.

## Validation

The native fixture uses the production Splash file and Rinx HTTP adapter with
real Palpo HTTP handlers and SQLite, plus explicit fake Matrix/Hagency services.
It drives a 430 × 820 owner window and a desktop admin window with Makepad
remote input and real captures. It does not use the signed-in user's profile.

```sh
cargo build --profile fast --locked --example palpo_miniapp
python3 tools/wechat-ux/live/native_palpo.py \
  --palpo /path/to/palpo-rinx-miniapp \
  --node /path/to/node24 \
  --binary target/fast/examples/palpo_miniapp
```

Reports, input traces, widget trees and screenshots are written under
`target/palpo-validation/<run>/`. The native scenario covers manager/admin UI separation, project approval and owner activation,
named-agent submission, draft recovery after process restart, light/dark/custom
themes, selection/undo, and disconnect without Matrix logout. Screenshots were
inspected. A Makepad hidden-window frame-confirmation failure is handled by
separating input from a read-only screenshot barrier, never replaying a click.

Additional checks: Palpo backend regression suite, Rinx origin/action-link unit
tests, catalog permission/admission tests, shared contract/policy/hub tests,
and the full Rinx release build. The validation report is the evidence for a
specific native run; this document is not a mobile or live-server acceptance claim.

## Remaining ADR gates

This is **not the complete ADR**. The full implementation ledger above is the
current acceptance checklist. Signup navigation is implemented; actual native
timeline/verdict acceptance, configuration saving/import and full account-
switch/revocation integration remain. Earlier live contribution checks below
used the superseded Rinx-originated model and do not validate Hagency-originated
association. My Actions now has a verified native room board, private notices
and quiet hours. Room-list pending badges/default pinning, richer age/deadline
metadata and OS-notification mini-app entry remain.

Delegated decisions, top-ups, scoped usage/lifecycle projection and verified
retirement now have local backend and native fixture coverage. Actual
cross-service runtime/Matrix lifecycle and recovery acceptance, allocation
increases, explicit migration and broader cancel/resubmit/expiry remain.
Android, OpenHarmony and hosted OctoSense require their own build/device checks.

App Hub publication is not attempted. The template listing with invented
publisher/platform data was removed. See `apps/palpo/PUBLISHING.md`. The stock
card-host lacks Rinx controls and Palpo services, so validation uses the actual
Rinx host fixture. Store admission awaits publisher metadata, device evidence
and release of the shared contract; there is no claimed store gate pass.

The temporary `vendor/octosense-app-contract` patch contains the companion
App Hub additive extension and provenance. Replace it with the published 1.2
crate after cross-repository review; no private App Hub git dependency is added.

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


## Desktop window and verification follow-up (2026-10-03)

Desktop standalone Rinx now hosts MiniAppsPanel in a lazily created native
window. OctoSense uses its injected window host; Android, iOS and OpenHarmony
retain the in-app modal. Closing the native window revokes its instance, and
closing the main window or clearing the account closes the mini-app window.
Theme reapply preserves the panel and the main conversation's display context.

Palpo's contribution detail and fleet cards read the server's connection proof
and show **Connection verified** only for a ready fleet with verified event
delivery and a verification timestamp. Verify performs one mutation and polls
only fleet reads, at two-second intervals for up to 30 reads. Navigation cancels
the watch; a failed or pending proof is never reported as complete. Project
cards distinguish waiting for approval, approved/ready to create, and created.

Validation evidence (local `target/` artifacts are intentionally untracked):

- `miniapp-window-validation/5fe7a341ebfb4ff1b11f93f0f6beabd7`: real hidden
  Makepad windows; lazy open, text input, dark-theme reapply, main draft/display
  preservation, OS close, reopen, and panel Back close passed. Captures inspected.
- `palpo-connection-validation/7fb4401c0447441298015591eddf6a06`: production
  Splash/host with explicit HTTP fixtures; delayed proof, read-only polling,
  navigation cancellation, persistent verified label and server refusal passed.
- `palpo-validation/8fbb736c16eb4448a04da9811a2f8378`: existing native
  contribution/project/named-agent submission, draft restart, theme and logout
  isolation scenarios passed against the local Palpo fixture.
- Library tests: 323 passed, two existing ignored. Account cleanup tests passed
  again after wiring main-window close. No-default-features OctoSense module
  compilation passed. Android/OpenHarmony device validation remains outstanding.
- Rebuilt full Rinx and restarted the isolated owner/admin profiles against
  `crew.ominix.io`. Each has its main chat and a separate Palpo window. The actual
  user's contribution displays Connection verified; both mini-app sessions have
  no Splash errors. No additional resource/agent approvals were submitted by
  this window/status validation.

The live `octosense-dev` project exists and its owner is joined. The diagnostic
at this checkpoint found no agent request in Palpo and no Hagency engagement;
project approval alone had not provisioned an agent. The product owner's new
requirement is recorded in the ADR's project-administrator amendment: assigned
project administrators decide agents within accepted budgeted grants, and
Hagency applies those decisions automatically. That grant/role/protocol work
is **not implemented by this UI fix**.
