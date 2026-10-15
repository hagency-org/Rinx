# Native activity pointers

`org.rinx.native.activity` uses `activity` with version 1, app, resource, room,
title, start/end Unix seconds and location. It carries a locator, never a login,
permission, URL to execute, queue rule or enrollment action. Unknown versions
and fields are rejected. An old client can show the message's fallback body.

The native timeline supplies source room, server event ID and sender outside
the card JSON. Clicking emits `MiniAppAction::OpenNative` with the current SDK
account and that source. Pending local echoes without a server event ID cannot
open an activity. A standalone client gives an explicit unsupported-host notice.

A consuming host must fetch the source event again, compare its sender and
pointer, check the current account and target membership, then ask the native
application to verify its authoritative resource. Opening must only focus or
display that resource; joining, enrollment and sending need separate approval.
Repeated opens should focus the existing account/app/resource instance.

The portable parser tests cover valid pointers, unsupported versions, malformed
rooms and ranges, and forged identity/grant fields. Whole-client compilation
and interactive card rendering are pending CI; this proposal does not claim
official adoption or automatic permission grants.
