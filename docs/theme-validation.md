# Shared customer themes: implementation and validation

This extends the [MVP record](theme-mvp.md) and implements the Rinx portions of
[ADR 0009](adr/0009-shared-reloadable-themes.md). The [customer guide](customer-themes.md)
documents file format, UI and compatibility boundaries. The complete ADR's
cross-platform release criteria are not all satisfied by this macOS run.

## Delivered behavior

- A renderer-independent versioned theme contract validates data, resolves
  inherited tokens/references and light/dark variants, computes revisions, and
  emits trusted Makepad definitions or owned-HTML CSS variables.
- Standalone Rinx supports customer `.octotheme` files, basic/JSON editing,
  duplicate, preview, cancel, apply, undo, reset, import, export and Matrix
  attachment sharing/review. An interrupted apply recovers the known-good theme.
- Hosted instances retain OctoSense authority. The existing complete stylesheet
  path still works; a validated data-snapshot receive API is also available.
- Native pages, shared controls and cached drawing values consume semantic
  roles. Retained cards, popups, reactions, send indicators, reader tabs and
  secondary windows refresh on reapply. A source guard rejects unexplained
  interface color literals.
- Splash and opted-in L0 kits inherit the theme without replacing their
  instances, replaying guarded startup work or modifying bundle identity.
- URL-card titles/descriptions and native body text share 11 logical font units
  at normal scale (12.65 at 115%). Titles remain bold; metadata uses 9.5.
- Plain Markdown follows app paper/ink/link/code defaults. Authored articles
  preserve document style, and external WebKit pages retain their own CSS.
  Native readers use the shared bounded reading width and balanced gutters.
  Explicit table layout defaults and a code-ink uniform keep table geometry
  and code contrast correct after a live theme change.

## Reproduction

The native tests use the actual Makepad remote instrumentation, hidden macOS
windows, native rendering, pointer/keyboard input, widget measurements and
screenshots/OCR. They are not screenshots generated from mock HTML. Each run
uses an isolated offline profile; no production Matrix account or room send is
needed. `target/fast`, not an older `target/debug`, is the tested binary path.

```sh
cargo test --locked --manifest-path crates/theme-contract/Cargo.toml
cargo test --profile fast --locked --features octosense-module --lib
cargo check --profile fast --locked --no-default-features --features octosense-module
cargo check --profile fast --locked --features tsp
cargo build --profile fast --locked --example theme_mvp --example chat_web_browser --example chat_link_preview --bin rinx
python3 tools/wechat-ux/live/native_theme.py
python3 tools/wechat-ux/live/native_theme_packages.py
python3 tools/wechat-ux/live/native_chat_link_preview.py --binary target/fast/examples/chat_link_preview
python3 tools/wechat-ux/live/native_chat_web_browser.py --binary target/fast/examples/chat_web_browser --desktop
python3 tools/check_theme_literals.py
python3 tools/wechat-ux/check_i18n.py
cargo build --release --locked
python3 tools/wechat-ux/live/native_theme_release.py
```

The scripts write a result/report JSON, binary hashes where relevant, screenshots,
native logs and input traces under the corresponding `target/*validation` or
`target/*regressions` directory. Native prerequisites are documented beside the
existing [probe](../tools/wechat-ux/live/native_probe.py).

## Popular palettes: macOS, 2026-10-06

The bundled catalog now includes Catppuccin, Nord, Dracula, Gruvbox and Tokyo
Night adaptations, each with coordinated light/dark variants. Appearance mode
remains independent of palette selection. Source revisions, adaptations and
license notices are recorded in the [customer guide](customer-themes.md).

| Check | Result |
| --- | --- |
| `cargo test --offline --profile fast --locked --lib theme::` | 11 passed, including all bundled palettes in both modes against macOS, Windows, iOS and Android base styles |
| Fast Rinx, `theme_mvp` and `chat_theme` builds | Passed |
| Production chat components, five new palettes × two modes × desktop/narrow | 20 captures passed text/fill contrast, rendered canvas/initials and retained draft checks |
| Native appearance controls, desktop/narrow | Menu bounds and pointer selection, all five light/dark pairs, persisted selection, System preservation, restart and return to defaults passed in a separate controls-only diagnostic |
| Full native/embedded palette suite | Failed: embedded theme registration exceeded Makepad's script time budget; an L0 preview retained stale or missing ink |
| Unchanged external pixel UX scorer | 8.09–9.22/10; mean 8.679. The 9.5 target is **not met** |
| Source guards | Theme literals, translation catalog and diff whitespace passed |

Local evidence: `target/chat-theme-review/bd3e35a0edfc4adc9d1b88620c83ece8/`
contains the chat report, whole native captures and HTML review. The full
embedded suite's failures are preserved under
`target/theme-preset-validation/{e212b69c2eaf47249c281554287772c1,194c3e8a508f43ca92187506618e62fa}/`.
The separate native-controls diagnostic is under
`target/popular-palette-controls/6cbe45084afe4dfebd72ab4cca096e89/`;
it reports embedded mismatches separately and does not establish embedded
restyling correctness. These are macOS windows at desktop and phone widths,
not physical mobile-device runs.

The pinned Makepad revision `1f3b1dedfbb81424eb8dbf69e5e2c634fa73dc54`
runs trusted framework/theme registration inside Splash's 64 ms script budget
in `widgets/src/splash.rs::eval_styled_body_with_apply`. The failure reproduced
in concurrent and serial test runs while this machine was under heavy load.
Separating trusted registration from budgeted app evaluation remains unresolved;
this change neither increases nor disables the app's execution limits.

## Selection readability: macOS, 2026-10-06

Opaque theme selection fills previously covered chat glyphs. Plain/rich message
renderers now reserve the selection background before emitting text. Text inputs
also use an explicit selection draw-call group: a later caret draw can no longer
prevent reuse of the background call and move the highlight above the letters.

Native GPU comparisons verify that selected glyphs retain their ink, that the
highlight is present, and that selection still produces the expected text.
The message checks cover all 26 bundled light/dark combinations (light at desktop
width, dark at phone width). The complete selection suite passed 15 checks in
each default mode, and the timeline suite passed seven drag/scroll/link checks.
Quoted and code-formatted text remained readable in the light and dark probes.
The production desktop/mobile composers passed all four mode/layout cases,
including select-all, replacement and undo. These are native macOS window tests,
not physical mobile-device tests.

```sh
cargo build --offline --profile fast --locked --example chat_text_selection --example chat_timeline_selection --example chat_theme --bin rinx
python3 tools/wechat-ux/live/native_chat_selection.py --binary target/fast/examples/chat_text_selection
python3 tools/wechat-ux/live/native_chat_selection.py --binary target/fast/examples/chat_text_selection --appearance dark --narrow
python3 tools/wechat-ux/live/native_chat_selection.py --binary target/fast/examples/chat_text_selection --visual-only --palette resources/themes/community/catppuccin.octotheme
python3 tools/wechat-ux/live/native_chat_selection.py --binary target/fast/examples/chat_text_selection --visual-only --blocks --appearance dark --narrow
python3 tools/wechat-ux/live/native_chat_timeline_selection.py --binary target/fast/examples/chat_timeline_selection
python3 tools/wechat-ux/live/native_composer_selection.py
```

The existing native selection probe now checks pixels as well as copy state;
the new composer probe includes screenshots and binary hashes. Both wait for a
rendered unselected baseline rather than comparing against an initial clear-only
frame. Local evidence is under `target/chat-selection-regressions/`,
`target/selection-palette-validation/`, `target/chat-timeline-selection-regressions/`
and `target/composer-selection-validation/a3de6258c1064372adfec42943681b6d/`.
The separate embedded-app reload timeout reported above is unaffected.

## Results: macOS, 2026-10-03

The [machine-readable record](theme-validation.json) contains the run directories,
reported binary SHA-256 values and screenshot hashes.

| Check | Result |
| --- | --- |
| Rinx library with `octosense-module` | 318 passed, 2 ignored |
| Portable theme contract | 7 passed |
| Module-only and optional TSP builds | Passed `cargo check` |
| Original native theme suite | Standalone and hosted revisions, drafts, selection/focus/undo, stable isolates, one startup request/timer, catalog/article flow, persistence and narrow scroll passed |
| Customer-theme native suite | Import without apply, both previews, contrast rejection, cancel/apply/undo, retained notifications, restart, interrupted-apply recovery, narrow editor and typed hosted delivery passed |
| Actual chat-card fixture | All 8 layout, content, navigation and reuse checks passed |
| Actual tabbed-reader fixture | All 21 checks passed, including retained tabs/scroll/website state, balanced bounded reading width, tables after reapply, and sampled code foreground/background pixels |
| Optimized release | Built; actual signed-out Rinx started with isolated light/dark custom-theme profiles, retained the selected package and produced native frames without script/shader errors |
| Source checks | Theme-literal guard and diff whitespace checks passed; 1,229 i18n entries, 1,160 translated call sites, no missing entries |

The native package fixture measured UI body, card title and card description at
`[11, 11, 11]`, then `[12.65, 12.65, 12.65]` after 115% text scaling. It also
checks that the same widget identities and edited values survive the change.

![Custom theme preview in light appearance](screenshots/theme-custom-light.png)
![The same theme editor in dark appearance](screenshots/theme-custom-dark.png)
![URL cards using shared body typography](screenshots/theme-url-cards.png)
![Retained Markdown tabs with themed tables and readable code](screenshots/theme-reader-dark.png)

## Remaining integration and release gates

- The portable crate lives in Rinx's source tree and is ready for other hosts
  to depend on. OctoSense has not yet adopted it in its own dependency graph.
  Two isolated hosts are covered in Rust tests; native fixtures cover hosted
  reapply, not a deployed OctoSense shell running two signed-in Rinx accounts.
- No separate Android APK/OpenHarmony process snapshot/subscription transport
  is shipped here. `theme::host::receive` is the consumer boundary, not IPC.
- Desktop System detection is implemented. Android configuration events have
  a source adapter but require a device build/run. System adapters for
  iOS/OpenHarmony/Web remain open; those builds expose explicit appearance.
- The macOS run does not prove Windows/Linux/Android/iOS/OpenHarmony/Web UI,
  mobile pickers/sharing, device touch interaction, accessibility or IME
  composition. Text/selection/focus and Chinese/English fixtures are narrower
  evidence than a platform accessibility review.
- Matrix theme sharing uses the SDK attachment/encryption path, but a real
  encrypted-room send/download/decrypt roundtrip was not performed. Tests inject
  the same import action after validation, and verify receiving does not apply.
- Legacy literal design kits keep their authored palettes. New semantic kits
  must opt in. The generic renderer and installed bundle bytes are unchanged.
- V1 admits host `system`/`mono` fonts and retains host SVG resources; it does not
  load uploaded font/icon binaries. Marketplace publication, account theme sync
  and organization policy remain separate ADR scope.

Do not interpret this delivery or a green macOS fixture as completion of these
remaining platform/host release gates.
