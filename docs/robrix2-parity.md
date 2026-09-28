# Robrix2 Hagency integration parity

Baseline: Rinx `aa08870c`, compared with Robrix2
`da375f241df3368a24318c94a5ed4865009f7529`.

This ports the missing client features into Rinx's native desktop/mobile UI.
Matrix AppService administration is separate from the existing local/hosted
Octos assistant. Registering an agent is a presentation preference and never
approval authority. The Agent Operations producer-contract release gate remains
in place; this change does not turn on its unreleased server protocol.

## Feature map

| Area | Rinx behavior after this change |
| --- | --- |
| Rooms and spaces | Explore → Create a room or space. Private/public creation, optional encrypted rooms and parent space; manage space name/topic and add/remove child links. Permissions are checked against authenticated state. Removing a link does not remove membership. |
| Agent Access | Account-scoped registry, exact Matrix profile lookup/re-check, framework selection (Octos AppService, Octos Direct, Hermes, OpenClaw, unknown), open chat, and confirmed unbind. Existing Robrix2 registry/settings fields deserialize. |
| Agent identity | Registered-agent badges in chat and member details; `/invitebot` opens the invitation picker with registered agents. Ordinary users keep the SDK's encrypted-DM default. New explicitly registered-bot DMs are unencrypted, as described in the panel; existing rooms retain encryption. |
| Matrix AppService | Enable/configure BotFather and service URL; health check; room/bot bindings with remarks; create/list/delete/help commands; scheduling command suggestions; discovery from authenticated BotFather replies in bound rooms. |
| Command routing | Exact target metadata for management commands, replies to registered agents and `/command@agent`; `/allbots` uses persisted child bindings. Ordinary room messages do not acquire an implicit bot target from a saved binding. |
| Octos interactive messages | Up to six action buttons, generic edited menus, immutable approval metadata, expiry and authorized-approver checks, original-event response relations, duplicate-send protection and uncertain-delivery state. |
| Hagency approvals | Shared state across panes, canonical server revisions/status, conflict detection, atomic claims, stable transaction IDs and conservative handling of unknown delivery. Existing four approval scopes and all three namespaces remain supported. |
| Private approval rooms | Authenticated v1/v2 marker discovery; namespace isolation, generation/protocol downgrade protection, account-scoped persisted hints with fresh validation on every scan, joined-room checks, bounds/timeouts/cancellation. Public notices open a project-filtered verified-room picker. |
| Access and language | Desktop Preferences, mobile General and Chat Info entries, Back navigation, English/Chinese panel strings, and confirmed destructive controls. |
| Standard build | `agent_chat` is included by default; minimal builds still work with `--no-default-features`. |

Existing Hagency roles, coordinator workflow commands, bridge companion invites,
long/streaming agent replies, mini-apps and the Octos assistant are retained.

## Validation

The default library suite passes **287 tests** (2 existing ignored tests).
The feature-enabled suite, embedded/minimal builds and i18n checks are also
checked as described below.

Run tests with a separate English-language profile because several existing UI
assertions expect English labels. Do not change a personal profile for tests.

```sh
mkdir -p /tmp/rinx-parity-tests
printf '"en"\n' > /tmp/rinx-parity-tests/ui-language.json
RINX_DATA_DIR=/tmp/rinx-parity-tests cargo test --locked --lib
RINX_DATA_DIR=/tmp/rinx-parity-tests cargo test --locked --features agent_ops_dev --lib
cargo check --locked --no-default-features
cargo check --locked --no-default-features --features agent_chat
cargo build --locked
python3 tools/wechat-ux/check_i18n.py
git diff --check
```

Native validation used the Makepad input bridge with hidden test windows,
isolated Matrix accounts/profile, and a local Palpo fixture. No personal Matrix
accounts or existing deployment were changed. Desktop windows were 1000×800;
mobile layouts were exercised at 375×812 (screenshots are at 2× scale).

Verified through real UI input and Matrix event/state reads:

- Agent lookup/registration, persistence across restart, OpenClaw badges,
  unbind confirmation/cancel, and English/Chinese mobile Agent Access.
- BotFather `/listbots`, `/createbot` (including escaped prompt arguments),
  `/deletebot` confirmation and delivery, and management-command target metadata.
- Service health feedback against an isolated HTTP fixture.
- Space creation, name/topic changes, child link addition and confirmed removal.
  Newly created spaces with incomplete SDK metadata use fresh authenticated
  state in the management panel.
- Private v2 approval-room discovery on desktop/mobile, public-notice navigation,
  original Hagency verdict namespace/bindings, and canonical `Denied` status
  after a bridge-authored update. A marker rollback remained rejected after
  restarting the client; a newer valid generation restored discovery.
- Octos action buttons on mobile and an exact `org.octos.action_response` with
  original event ID and sender target observed in Matrix history.

Local screenshots and input traces are under the ignored
`lab/wechat-ux/evidence/live/parity/` directory. Useful receipts include
`space-linked-desktop.png`, `approval-confirmed-desktop.png`,
`approval-open-mobile.png`, `public-approval-discovery-desktop.png`,
`octos-response-mobile.png`, `agent-access-mobile-zh.png` and
`delete-bot-confirmation-mobile-zh.png`.

These checks validate the Matrix client contract, not a deployed BotFather's
creation/deletion implementation. Health checks used an HTTP fixture; mobile
checks used the native macOS mobile layout, not iOS/Android hardware. Production
AppService/Hagency deployment and physical-device certification remain separate
release checks. The local Palpo fixture sometimes omitted newly-created room
metadata from sliding sync; management operations fetch current state, while
ordinary sidebar metadata still depends on the SDK's sync stream.
