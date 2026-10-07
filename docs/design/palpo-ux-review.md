# Palpo mini-app UX review

The navigation and cards now use a compact, shared Rinx design. Primary tabs
are Inbox, Projects, Agents and Resources. More contains engagements,
notifications, My Actions and authorized administrator tools. Agent lifecycle
controls are under Manage agent; identifiers are under Details.

Cards use Rinx body typography, theme colors, clear state badges and consistent
spacing, including gaps between wrapped buttons. A bounded desktop content width
and wrapping controls support the narrow layout. Approval remains distinct from
runtime readiness, and usage remains a reported lower bound rather than a
fabricated remaining balance.

Forms name the action and preserve a summary of the target in saved drafts.
Cancel returns to the originating page and retains the selected project/resource
when cancelling an agent request. Submit and Cancel share an action row. Theme
changes retain the draft and script heap. Both quick-start guides match the new
navigation and status labels.

## Acceptance, 2026-10-06

**The requested 9.5/10 target is not met across the captured screens.**
The unmodified pixel scorer reports a mean of **9.40**, minimum **9.10** and
maximum **9.78** for twenty after-screens; seven meet 9.5. The visual command
intentionally exits 1 and reports `passed: false`, separately from
`functional_passed: true`. Do not describe the whole app as scoring 9.5.

| Screen | Narrow, 430 × 820 | Desktop, 1000 × 800 |
| --- | ---: | ---: |
| Agents, light | 9.24 | 9.28 |
| Agent controls, light | 9.60 | 9.53 |
| Rename form, dark | 9.24 | 9.10 |
| Agent controls, dark | 9.11 | 9.29 |
| Notifications | 9.53 | 9.48 |
| Projects | 9.38 | 9.31 |
| Resources | 9.45 | 9.77 |
| Inbox | 9.78 | 9.60 |
| Request details | 9.52 | 9.10 |
| Approval form | 9.37 | 9.33 |

The original bundle was also captured with the same fixtures and host:
Inbox scored 8.77 narrow / 8.71 desktop; Agents scored 9.04 / 9.27.
All images are whole native captures; no image was edited or cropped before
scoring. The scorer performs its own fixed system-bar crop and normalization.
Its content-density metric penalizes sparse forms, and pixel scores alone do
not establish task usability. Agents, forms and several list/detail layouts
still need improvement to satisfy the strict per-screen target.

Validation:

- Native coordinator regression passed all 23 checks, including role isolation,
  actual approval/top-up/control forms, replay safety, automatic setup/readiness
  updates, rename observation, retirement/settlement and notification persistence.
- Visual navigation checks passed on both widths: project/resource cancellation,
  decision cancellation, primary/overflow navigation and theme/draft retention.
- Visible controls passed horizontal-overflow and shared 44-point target checks
  (allowing one pixel for snapshot rounding; partially clipped bottom controls
  are excluded from full-height measurement).
- Instrument example build, seven system-app package tests, theme literal guard,
  Python compilation and diff whitespace checks passed.

Matrix authentication, authority and provider observations are isolated fixtures.
The Rinx renderer, production Splash source, HTTP adapter and Rust Palpo service
are real. These runs do not execute Codex/Hagency agents or prove physical-phone
behavior. Existing user accounts, running apps and the mini1 deployment were not
changed. The association runner's navigation was updated but that scenario was
not rerun for this UI change.

## Reproduce

Build Rust Palpo's `palpo-operations` binary separately. From the Rinx root:

```sh
cargo run --quiet --manifest-path tools/miniapp-package/Cargo.toml -- apps/palpo/bundle
cargo build --profile fast --locked --features palpo-instrument --example palpo_miniapp
mkdir -p target/palpo-ux-review
git show d6d2bf16:apps/palpo/bundle/main.splash > target/palpo-ux-review/before.splash
python3 tools/wechat-ux/live/native_palpo_ux.py \
  --backend /path/to/palpo/target/debug/palpo-operations \
  --scorer /path/to/Octoscript-OH/tools/uxscore.py \
  --before target/palpo-ux-review/before.splash
python3 tools/wechat-ux/live/native_palpo_coordinator.py \
  --backend /path/to/palpo/target/debug/palpo-operations \
  --binary target/fast/examples/palpo_miniapp
```

Complete builds before running native acceptance; source and binary hashes must
stay unchanged throughout the run. The visual runner writes `review.html`, all
screenshots, raw snapshots and `report.json` under `target/palpo-ux-review/<run>`.
The default threshold is 9.5 for every after-screen. `--report-only` records an
unmet score without failing the shell command; the report still says it failed.
Review packets remain outside git and the app bundle.

The App Design Flow check resolved capabilities and restamped the bundle. Its
store-admission stage rejected missing `listing.json`, as expected for the Rinx
built-in app described in `apps/palpo/PUBLISHING.md`; no store identity or listing
was fabricated. Rinx's package validation passed.

## Evidence provenance

- Visual run: `36427d580c304f53b3c07ea502afa96b`.
- Workflow run: `ebf02ebb85af45dc9a9e211dbf16e720`.
- Scorer: [Octoscript-OH uxscore.py](https://github.com/OctoSense-org/Octoscript-OH/blob/f69ba147acc930213337e85db16afb504543a907/tools/uxscore.py), unchanged.
- Splash source SHA-256: `8d60c86b7460aa16a5d39d6508c6cc91dc2a209806f0b75959929122a20249e1`.
- Instrument binary SHA-256: `a6cede0ab1333c932278ae47391081341d3a81572c5b5b5c965e00ea2a1eda15`.
- Scorer SHA-256: `14d9018b310ac885f87493367ff813581a1c303bc4c4ee209c4ca90b4b8ee286`.
