[English](hagency-quickstart.md) | [简体中文](hagency-quickstart.zh-CN.md)

# Hagency in Rinx: quick start

Use Rinx to approve a Hagency connection, request a project and agent, and chat
with that agent. Hagency runs Codex on the operator's machine; Rinx is the
Matrix client. These steps describe this checkout's Palpo mini app and the
matching Hagency server-engagement workflow.

## Prepare the accounts and local runtime

| Role | What this account does |
| --- | --- |
| Resource owner / Hagency operator | Runs Hagency, confirms the connection, creates resources and delegates coordination. |
| Palpo administrator | Approves the server engagement. |
| Coordinator | Reviews project, agent and additional-token requests for the engagement. |
| Project / agent owner | Requests the project and agent, chats with it and approves protected operations. |

One person may hold several roles. Use existing accounts on the same Matrix
server, with full IDs such as `@owner:example.org`. The coordinator is a Matrix
user, not a bot you need to create through Hagency. Use separate Rinx profiles
when testing different accounts on one machine; see [Data and compatibility](../README.md#data-and-compatibility).

On the machine that will run the agent, the operator installs and signs in to
Codex, starts Hagency, and opens its local console:

```sh
codex login
hagency start
```

Use **Setup → Coding agents** to check the detected Codex installation and
runtime configuration. The machine and service must remain running for agents
to work. Installation, service mode and first-source setup are covered in the
[Hagency user guide](https://github.com/hagency-org/hagency-rs/blob/main/docs/user-guide/README.md).

In Rinx, sign in to Palpo and set up cross-signing / verify your session before
using encrypted approval rooms. Open **Mini apps → Palpo**; on mobile, **Mini
apps** is under **Discover**. Review its permissions and click **Run** on first
use. The app uses the current Matrix account; enter no password or Codex key in
the mini app.

The main tabs are **Inbox, Projects, Agents and Resources**. Open **More** for
**Engagements**, **Notifications** and **My Actions**; administrator tools appear
there only when your account has access.

## 1. Establish the server engagement

1. **Operator, in Hagency:** click **New server engagement** under **Setup →
   Connect Palpo** or **Server engagements**. Enter the HTTPS Matrix server
   address, resource-owner and coordinator IDs, connection name and delegation
   duration. Only fill in **Separate Palpo address** if the administrator
   supplies a separate operations address. Click **Request connection**.
2. **Resource owner, in Rinx:** open **Palpo → Inbox**, review the connection
   you initiated and approve it. Hagency's **Open request in Rinx** link opens
   that same request.
3. **Administrator, in Rinx:** approve the server setup in your own Palpo Inbox.
4. Hagency receives the approved configuration automatically. **A new connection
   requested this way does not need JSON download/import.**
5. **Resource owner, in Rinx:** open the approved request and click **Verify
   connection** once. Wait while the disabled button shows verification in
   progress, then check for **Connection verified** in both applications.

To add resources to an existing verified engagement, continue below. You do not
need to revoke and reconnect it. **Cancel** in a delegation form discards the
unsaved edit; revocation is a separate change and does not delete existing
agents, history or reservations.

## 2. Create a resource in Hagency

1. Open **My resources → New resource configuration**.
2. Select the verified **Server engagement** and a local **Source
   configuration**. The source supplies the framework, provider and account;
   the search box filters model/framework/reasoning, not agent or project names.
3. Select the model and reasoning level, then enter the monthly token budget.
4. Enter **Eligible Matrix project managers** as full same-server IDs separated
   by spaces or newlines, for example `@project-owner:example.org`.
5. Click **Create resource** and wait for it to appear in the eligible user's
   Rinx **Palpo → Resources** catalog.

Each resource has one budget assigned to its engagement. One engagement can
have several resources; there is no second resource-pool allocation afterward.
An unknown shared-account quota is not a measured zero or a provider guarantee.

On a fresh Hagency state directory, the wizard currently needs the operator to
create the first local source through the documented
[operator API](https://github.com/hagency-org/hagency-rs/blob/main/README.md#create-a-resource-with-the-operator-api).
Setup prepares the Codex runtime but does not seed that source. Once it exists,
create engagement resources through the same wizard above.

## 3. Request a project and agent in Rinx

1. **Project owner:** in **Palpo → Resources**, click **Request project here**
   on a resource. Enter a name and purpose.
2. Let Palpo create a new room, or use **Choose an existing room**. Existing
   rooms must be private (invite-only), unencrypted, created by your account,
   and not Spaces. Palpo verifies the selection and invites the engagement's
   representative.
3. Submit. **Coordinator:** approve or reject the project in **Palpo → Inbox**.
   **Owner:** wait for project setup to complete in **Projects**.
4. **Owner:** click **Request agent** on the project. Select the resource and
   role, then enter the agent name, initial tokens and daily rate. Submit.
5. **Coordinator:** review and approve or reject the requested allocation in
   your Palpo Inbox. Hagency checks authorization and capacity when it receives
   the decision.
6. **Owner:** follow **Agents → Open latest result**. **Approved** is the
   decision; **Waiting for Hagency / Preparing agent** means runtime setup is
   still in progress. Wait for **Ready to chat** before testing chat.

Project approval grants resource access; it does not reserve an agent's tokens.
The project room is for shared discussion and agent @mentions. The local agent
workspace, encrypted owner DM and encrypted approval room are separate.

## 4. Chat and approve operations

- Accept the agent's DM invitation and send a short message. **Owner DMs need
  no @mention, including the first message.** Keep the DM between you and the
  agent.
- In the shared project room, @mention the agent and keep follow-ups in that
  thread. Agents do not work in encrypted group rooms shared with other people.
- Answer protected-operation cards in the owner's approval room using the
  buttons. A text reply is not an approval. These decisions are separate from
  project and token approvals in the Palpo Inbox.
- Use **Palpo → Agents → Request more tokens** when needed; the coordinator
  reviews the request. Open **Manage agent** for authorized rename, pause,
  resume and remove controls. Read their latest runtime result.

## Check problems without duplicating requests

| What you see | What to check |
| --- | --- |
| Empty Inbox | Current Matrix account and Inbox filter. Owner confirmation comes before admin approval; projects and agents go to the coordinator. |
| Empty Resources | Verified connection, published resource, eligible-manager ID and active delegation. |
| Project not ready | Open its setup explanation. Use **Retry project setup** only when the server offers it. |
| Approved, Preparing agent | Setup is still running. Accept the DM invitation when it appears and follow the execution status. |
| Needs attention, setup outcome uncertain | Ask the Hagency operator to inspect the original attempt, logs and Matrix state. Account/room creation may already have happened and tokens may remain reserved; do not create duplicates. |
| Needs attention, runtime unavailable | The agent exists but its runtime or Matrix connection needs attention on the Hagency host. |
| DM silent while Ready to chat | Confirm you are its owner and the DM has no other people; ask the operator to inspect key exchange and message intake. |
| Approval card cannot decrypt | Verify the current Rinx session or use another verified session. |

## Settings and themes

Desktop and mobile share **Settings → Account / Preferences / Privacy / About**.
On mobile, enter through **Me → Settings**. Appearance is under **Preferences →
App appearance**; workflow suggestions are under **Preferences → Hagency**.
Approval cards and the Palpo app do not depend on enabling workflow suggestions.
See [Settings and appearance](../README.md#settings-and-appearance).
