# Rinx

English | [简体中文](README.zh-CN.md)

A native Matrix messenger from [Upstream Labs](https://github.com/upstreamlabs), with a WeChat-style interface, English/Chinese support, Moments, and scoped mini apps.

Rinx is an independent continuation of [`OctoSense-org/robrix2`'s `wechat-ui` branch](https://github.com/OctoSense-org/robrix2/tree/wechat-ui), starting at `16913e0c6f0397b4ed1272b7dcd421935521d5f3`. That branch's commit history is preserved here. Development now lives on this repository's **`main`** branch.

## Choosing a server and registering

The native desktop and mobile sign-in screen offers **matrix.org**, **tchncs.de**, and **mozilla.org**, alongside a custom server address. Select a server, then Continue. Rinx discovers its actual backend and current authentication capabilities before presenting registration options; the catalog does not hardcode API hosts or contact every listed server.

Servers advertising browser registration (currently matrix.org and tchncs.de) open their own SSO registration flow and return to Rinx after completion. Mozilla community onboarding opens its [official website](https://chat.mozilla.org); return to Rinx and use single sign-on afterward. Legacy servers use their own Matrix `/_matrix/client/v3/register` endpoint; native registration supports dummy and invitation-token authentication. Servers decide whether registration is open. Other legacy verification stages, such as email or CAPTCHA, are not yet supported in the native form. OAuth `registration_endpoint` metadata describes client registration and is never used to submit a user's password.

Switching servers clears registration details and invalidates any older discovery result. Credentials go only to the selected server's discovered backend. Public suggestions are separate from device-local history.

For native UI validation on macOS, run `tools/wechat-ux/live/native_server_catalog.py --binary target/debug/rinx --output <new-output-directory>` with Python 3 and Swift available. Add `--live-public` to check all three public servers' current discovery and registration actions; the test never submits public account credentials. It uses isolated profiles and captures both desktop and phone-width layouts. See [desktop](docs/screenshots/registration-server-desktop.png), [phone width](docs/screenshots/registration-server-phone.png), and [validation evidence](docs/screenshots/registration-server-validation.json).

### Homeserver history

The sign-in server field shows recent homeservers when clicked or focused. Typing filters the list; choosing an entry fills the field, and Continue checks it again. Successful Matrix discovery records up to 20 deduplicated destinations, most recent first, in device-local `homeserver_history.json`. Existing saved login sessions seed the list on upgrade. The file contains server addresses only and retains history across restarts, even if login fails after server discovery.

## Features

- Chats, Contacts, Discover, and Me, backed by Matrix messaging and encrypted rooms.
- Room discovery, optional Spaces, per-room search and attachments, hidden chats, and message forwarding.
- Matrix-backed Moments, separate from self-DM file transfer.
- A Markdown article editor with images, covers, themes, full preview, and publish/retract workflows.
- Native HTML/CSS article previews through [makepad-html](https://github.com/OctoSense-org/makepad-html) and Blitz, rendered into Makepad without a WebView.
- Mini-app consent scoped to the signed-in user, with a bundled Palpo app for Hagency connections, project and agent requests, and coordinator approvals.
- Shared desktop and mobile settings, including appearance, language, account and privacy controls.

The renderer remains experimental: grayscale/sepia filters have known failures; arbitrary WeChat HTML import and complete WeChat compatibility are not claimed. See the [integration evidence](lab/article-html-integration/README.md) and [mini-app authority design](docs/adr/0002-octoscript-mini-app-authority.md).

## Built-in apps

Rinx bundles native and OctoScript apps through `system-apps.json`. The native article editor lives under `apps/article-editor/`; both deployments share its catalog and host-service contracts. See the [app development guide](apps/README.md) and [ADR 0008](docs/adr/0008-rinx-system-app-catalog.md).

## Hagency quick start

Open **Mini apps → Palpo** (under **Discover** on mobile) using your current Matrix account. The Palpo app handles server engagement approvals, resources, projects and agents. Hagency runs separately on the machine with your signed-in Codex installation.

1. In Hagency, request a **New server engagement** with the Matrix server address and existing resource-owner and coordinator accounts.
2. Confirm it as the resource owner in Rinx's **Palpo → Inbox**, then approve it as the Palpo administrator. Hagency receives the configuration automatically; the owner clicks **Verify connection** and waits for **Connection verified**.
3. In Hagency's **My resources → New resource configuration**, create resources for that engagement with a model, budget and eligible project managers. Each resource has one budget; no second pool allocation is needed.
4. In Rinx, an eligible manager selects **Resources → Request project here**, then **Projects → Request agent** after project approval. The coordinator reviews both requests in the Palpo Inbox.
5. Accept the agent's DM invitation when it appears, then wait for **Execution · ready** before sending a message. Owner DMs need no @mention; shared project rooms do.

See the [Hagency quick-start guide](docs/hagency-quickstart.md) for roles, first-resource setup, existing project rooms, encryption and troubleshooting. [Agent-chat support](docs/agent-chat.md) describes the client protocol and developer checks.

## Settings and appearance

Desktop and mobile use the same **Settings** screen with **Account**, **Preferences**, **Privacy** and **About**. On mobile, open it from **Me → Settings**.

Use **Settings → Preferences → App appearance** to choose light/dark appearance, follow the system where supported, change the accent, or open **Customize appearance**. Hosted Rinx may instead show **Appearance is managed by OctoSense**. The **Hagency** section in Preferences controls workflow-command suggestions; approval cards work with that toggle off.

## Build and run

Install Rust and CMake. The repository pins Rust 1.98.0. On macOS:

```sh
brew install cmake
git clone https://github.com/hagency-org/rinx.git Rinx
cd Rinx
cargo run --locked --features agent_chat
```

The executable is `rinx`, and the macOS app is `Rinx.app`. HTML/CSS preview and the `agent_chat` feature are enabled by default. **Settings → Preferences → Hagency** enables optional workflow-command suggestions, not the Palpo app or approval cards. Rinx does not start the Hagency service itself.

Rinx needs a Matrix homeserver supporting native Sliding Sync. Enter account credentials directly in the app. On Linux, install the native dependencies listed in the [inherited build guide](docs/robrix-upstream-readme.md#building--running-robrix-on-desktop); use `rinx` wherever that historical guide names the package or executable `robrix`. Mobile packaging scripts have been renamed for Rinx but require your own signing configuration and device validation.

On Windows, install Rust with the MSVC toolchain, Visual Studio Build Tools with
the **Desktop development with C++** workload and a Windows SDK, Git, and CMake
(the CMake installer, or `winget install Kitware.CMake`). Then run:

```powershell
git clone https://github.com/hagency-org/rinx.git Rinx
cd Rinx
cargo run --locked --features agent_chat
```

The executable is `rinx.exe`. The local assistant also needs its Octos kernel.
With Python 3 installed, follow the [Windows runtime staging steps](packaging/README-octos.md#windows).
On x86-64 MSVC, the helper builds `octos-x86_64-pc-windows-msvc.exe` under
`dist/runtime/`; Rinx discovers it as **`octos.exe` beside `rinx.exe`** (or at
`RINX_OCTOS_BIN`). Stage it beside the executable for the profile you run.
Without it, `Assistant (this device)` reports "no packaged assistant runtime".

Two things to expect once it is running:

- A development kernel's console can take focus. If typing stops reaching Rinx
  when it opens, click the Rinx window to restore focus.
- If requests time out only in a proxied shell, check its proxy settings. When
  direct access to your homeserver is intended, add that hostname to `NO_PROXY`
  for the current PowerShell session (for example, `$env:NO_PROXY = "localhost,127.0.0.1,matrix.example.org"`).

HTTP(S) link previews follow Robrix's native card implementation: the homeserver's
`/_matrix/client/v1/media/preview_url` endpoint supplies the title, description,
and thumbnail. The homeserver must permit URL previews. If Palpo returns
`403 M_FORBIDDEN` with `URL is not allowed to be previewed`, add the desired domains
to its [URL-preview configuration](https://github.com/palpo-im/palpo/blob/v0.4.0/palpo-example.toml):

```toml
[url_preview]
domain_explicit_allowlist = ["github.com", "example.org"]
```

Apply that configuration on the homeserver, then restart Rinx to clear failed
preview requests cached by open timelines. Failed requests are logged as
`Homeserver link preview failed`; the original URL remains usable.

Chat links and preview cards open in an in-app web reader on macOS, iOS, and
Android. On desktop, the reader opens in a separate Rinx window so the conversation
remains usable; on mobile, it opens as an in-app panel. Its SVG toolbar provides
Close, Back, Forward, Reopen link, and Open in browser, with hover labels. Closing
the reader leaves the chat open. Each opened chat link gets its own tab, preserving
its page, scroll position, and navigation history when switching tabs. A tab's
close button closes only that tab; closing the last tab closes the reader.
The header identifies the selected tab's original opened link, and Reopen link
returns to it. The embedded reader uses
the platform browser engine and does not depend on the homeserver preview API.

Markdown attachments (`.md`, `.markdown`, or a Markdown MIME type) have an Open
button alongside Download and Share. They use Matrix's authenticated media
download, including decryption, and open as native document tabs in the same
reader. Shared article cards also open there. Documents keep their own scroll
position; their HTTP links open web tabs. The article editor remains separate.

Open reader tabs and the selected tab are saved per account and restored after
restarting Rinx. Closing a tab or the reader explicitly removes it from the saved
session. Restored webpages load their original chat links; browser navigation
history and scroll positions are retained only while Rinx stays running.
Markdown and article tabs reload through Matrix, without saving document bodies
in the reader session file.

```sh
cargo test --locked --features agent_chat --lib
cargo test --locked --manifest-path crates/article-core/Cargo.toml
python3 tools/wechat-ux/check_i18n.py
```

The main CI workflow builds and tests the native app and portable article core.
The inherited multi-platform build and license-refresh workflows are available
by manual dispatch; release signing and publishing require Rinx-specific secrets.

## Multiple accounts

Use the bottom-left account menu on desktop, or Settings → Account on mobile, to add accounts and switch between saved Matrix sessions. Only the selected account syncs. Switching retains each account's device and encryption database; logging out revokes only the selected account. Saved accounts are also available on the login screen. See [account flows and data isolation](docs/multi-account.md) for migration, storage boundaries, and experimental TSP limitations.

## Data and compatibility

Rinx has its own application identity, `org.octosense.rinx`, and its own default data directory. It does not automatically migrate an existing Robrix login or profile. `RINX_DATA_DIR` selects an absolute path for an isolated profile; legacy `ROBRIX_DATA_DIR` remains a fallback for existing test tooling. `RINX_DATA_DIR` takes precedence.

Existing `rs.robius.robrix.*` Matrix event types are retained so shared articles, mini apps, forwarded messages, and Moments remain interoperable. The app's sign-in callback uses `rinx://login`.

## 致敬 Robrix · Acknowledgements

Rinx 致敬 [Robrix](https://github.com/project-robius/robrix)、[Robrix2](https://github.com/Project-Robius-China/robrix2)、Kevin Boos、Project Robius 及所有贡献者。感谢他们为原生 Rust Matrix 客户端打下的基础。

Rinx builds on their work and on [Makepad](https://github.com/makepad/makepad), [Robius](https://github.com/project-robius), [Matrix Rust SDK](https://github.com/matrix-org/matrix-rust-sdk), [Blitz](https://github.com/DioxusLabs/blitz), and [Octoscript](https://github.com/OctoSense-org/Octoscript). Original authorship and commit history are preserved.

## License

Rinx is distributed under **[Apache-2.0](LICENSE)**. Inherited Robrix code retains its original **[MIT copyright and permission notice](LICENSE-MIT)**. See [NOTICE](NOTICE) for attribution and [third-party notices](licenses/THIRD-PARTY-NOTICES.html) for bundled dependencies; their licenses remain in force.
