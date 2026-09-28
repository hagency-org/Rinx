//! Shared account/session approval state, including legacy bridge compatibility.
use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};
use matrix_sdk::{
    Client,
    ruma::{RoomId, OwnedUserId, OwnedRoomId, OwnedEventId},
};
use matrix_sdk_ui::timeline::EventTimelineItem;
use serde_json::Value;
use super::{
    approval::{
        ApprovalRequest, ApprovalAction, ApprovalDecisionState, Namespace, current_unix_time_millis,
    },
    approval_state::{self as model, ApprovalSession, CanonicalApprovalState},
};

#[derive(Clone, Debug)]
pub struct ApprovalStateChanged {
    pub room: OwnedRoomId,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    namespace: &'static str,
    room: OwnedRoomId,
    request: String,
}
#[derive(Clone, Debug)]
struct Entry {
    request: ApprovalRequest,
    sender: String,
    modern: bool,
    conflicted: bool,
    delivery: Option<ApprovalDecisionState>,
}
#[derive(Default)]
struct Runtime {
    epoch: u64,
    owner: Option<OwnedUserId>,
    sessions: HashMap<&'static str, ApprovalSession>,
    entries: HashMap<Key, Entry>,
}
static STATE: LazyLock<Mutex<Runtime>> = LazyLock::new(|| Mutex::new(Runtime::default()));

/// Called on every client replacement, including a same-account re-login.
pub fn reset(owner: Option<OwnedUserId>) {
    let mut state = STATE.lock().unwrap();
    let epoch = state.epoch.wrapping_add(1);
    *state = Runtime {
        epoch,
        owner,
        ..Default::default()
    };
}

pub(super) fn normalize(namespace: Namespace, content: &Value) -> Value {
    let mut content = content.clone();
    if let Some(detail) = content.get(namespace.event_key()).cloned() {
        content
            .as_object_mut()
            .unwrap()
            .insert("com.agentchat.approval".into(), detail);
    }
    if let Some(typ) = content
        .get("msgtype")
        .and_then(Value::as_str)
        .map(str::to_owned)
    {
        if let Some(suffix) = typ.strip_prefix(namespace.base()) {
            content["msgtype"] = Value::String(format!("com.agentchat{suffix}"));
        }
    }
    content
}
fn key(room: &RoomId, request: &ApprovalRequest) -> Key {
    Key {
        namespace: request.namespace.base(),
        room: room.to_owned(),
        request: request.request_id.clone(),
    }
}
fn claim_key(owner: &str, key: &Key, digest: &str) -> model::ApprovalClaimKey {
    model::ApprovalClaimKey {
        account_mxid: owner.into(),
        approval_room_id: key.room.to_string(),
        request_id: key.request.clone(),
        input_digest: digest.into(),
    }
}

/// Original event contents only. Edited bodies, local echoes and arbitrary
/// agent registrations cannot establish approval authority.
pub fn ingest(client: &Client, room: &RoomId, event: &EventTimelineItem) {
    if !crate::matrix_context::is_current(client) {
        return;
    }
    let Some(owner) = client.user_id() else {
        return;
    };
    let Some(event_id) = event.event_id() else {
        return;
    };
    let Some(content) = event
        .original_json()
        .and_then(|raw| raw.get_field::<Value>("content").ok())
        .flatten()
    else {
        return;
    };
    let Some(namespace) = content
        .get("msgtype")
        .and_then(Value::as_str)
        .and_then(Namespace::of_msgtype)
    else {
        return;
    };
    if content
        .get("m.relates_to")
        .and_then(|r| r.get("rel_type"))
        .and_then(Value::as_str)
        == Some("m.replace")
    {
        return;
    }
    let mut state = STATE.lock().unwrap();
    if state.owner.as_deref() != Some(owner) {
        return;
    }
    let now = current_unix_time_millis();
    let normalized = normalize(namespace, &content);
    let session = state.sessions.entry(namespace.base()).or_default();
    let previous_epoch = session.room_epoch(owner.as_str(), room.as_str());
    let _ = session.ingest_message(
        owner.as_str(),
        room.as_str(),
        event_id.as_str(),
        event.sender().as_str(),
        &normalized,
        now,
    );
    let mut changed = previous_epoch != session.room_epoch(owner.as_str(), room.as_str());
    if let Some(request) = super::approval::parse_approval_request(&content) {
        let detail = &content[namespace.event_key()];
        let modern = ["owner_mxid", "publisher_mxid", "revision"]
            .iter()
            .any(|field| detail.get(*field).is_some());
        let key = key(room, &request);
        if let Some(entry) = state.entries.get_mut(&key) {
            if entry.request != request
                || entry.sender != event.sender().as_str()
                || entry.modern != modern
            {
                changed |= !entry.conflicted;
                entry.conflicted = true;
            }
        } else if state.entries.len() < 1024 {
            state.entries.insert(
                key,
                Entry {
                    request,
                    sender: event.sender().to_string(),
                    modern,
                    conflicted: false,
                    delivery: None,
                },
            );
        }
    }
    drop(state);
    if changed {
        makepad_widgets::Cx::post_action(ApprovalStateChanged {
            room: room.to_owned(),
        });
    }
}

fn decision_locked(
    state: &Runtime,
    key: &Key,
    request: &ApprovalRequest,
    now: u64,
) -> ApprovalDecisionState {
    let Some(owner) = &state.owner else {
        return ApprovalDecisionState::Unavailable;
    };
    let Some(entry) = state.entries.get(key) else {
        return ApprovalDecisionState::Unavailable;
    };
    if entry.conflicted || entry.request != *request {
        return ApprovalDecisionState::Unavailable;
    }
    // A migration status can retire an older request. It must bind to the
    // original publisher and digest, not merely reuse its request ID.
    if !entry.modern
        && let Some(view) = state
            .sessions
            .get(key.namespace)
            .and_then(|s| s.approval(owner.as_str(), key.room.as_str(), &key.request))
    {
        if view.binding.publisher_mxid == entry.sender
            && view.binding.input_digest == request.input_digest
            && view.binding.owner_mxid == owner.as_str()
            && view.legacy_read_only
        {
            return ApprovalDecisionState::Unavailable;
        }
    }
    if entry.modern {
        let Some(session) = state.sessions.get(key.namespace) else {
            return ApprovalDecisionState::Unavailable;
        };
        let Some(view) = session.approval(owner.as_str(), key.room.as_str(), &key.request) else {
            return ApprovalDecisionState::Unavailable;
        };
        if view.conflicted || view.legacy_read_only || view.binding.owner_mxid != owner.as_str() {
            return ApprovalDecisionState::Unavailable;
        }
        if view.state != CanonicalApprovalState::Pending {
            return ApprovalDecisionState::Confirmed(
                match view.state {
                    CanonicalApprovalState::Approved => "Approved",
                    CanonicalApprovalState::Denied => "Denied",
                    CanonicalApprovalState::Expired => "Expired",
                    CanonicalApprovalState::Consumed => "Consumed",
                    _ => unreachable!(),
                }
                .into(),
            );
        }
        if entry.delivery.is_none()
            && !session.is_actionable(owner.as_str(), key.room.as_str(), &key.request, now)
        {
            return if request.is_expired(now) {
                ApprovalDecisionState::Expired
            } else {
                ApprovalDecisionState::Unavailable
            };
        }
    }
    if let Some(delivery) = &entry.delivery {
        return delivery.clone();
    }
    if request.is_expired(now) {
        ApprovalDecisionState::Expired
    } else {
        ApprovalDecisionState::Pending
    }
}

pub fn decision(room: &RoomId, request: &ApprovalRequest, now: u64) -> ApprovalDecisionState {
    let owner = crate::sliding_sync::current_user_id();
    let state = STATE.lock().unwrap();
    if owner != state.owner {
        return ApprovalDecisionState::Unavailable;
    }
    decision_locked(&state, &key(room, request), request, now)
}

#[derive(Clone, Debug)]
pub struct Claim {
    owner: OwnedUserId,
    epoch: u64,
    key: Key,
    pub transaction_id: String,
    pub source_event_id: OwnedEventId,
    digest: String,
    modern: bool,
}
pub fn claim(
    room: &RoomId,
    event: &EventTimelineItem,
    request: &ApprovalRequest,
    action: &ApprovalAction,
) -> Result<Claim, &'static str> {
    let owner = crate::sliding_sync::current_user_id().ok_or("Session ended")?;
    if !crate::sliding_sync::get_client()
        .and_then(|c| c.get_room(room))
        .is_some_and(|r| r.state() == matrix_sdk::RoomState::Joined)
    {
        return Err("Approval room is no longer joined");
    }
    let mut state = STATE.lock().unwrap();
    let now = current_unix_time_millis();
    let key = key(room, request);
    if state.owner.as_ref() != Some(&owner)
        || !matches!(
            decision_locked(&state, &key, request, now),
            ApprovalDecisionState::Pending
        )
        || request.action(&action.id) != Some(action)
    {
        return Err("This request is expired, unavailable, or already has a decision in progress.");
    }
    let entry = state.entries.get(&key).unwrap();
    if event.sender().as_str() != entry.sender {
        return Err("Approval publisher changed");
    }
    let modern = entry.modern;
    let source_event_id = event.event_id().ok_or("Missing request event")?.to_owned();
    // Namespace-specific, deterministic transaction IDs also protect an SDK
    // retry of the same verdict from creating a second Matrix event.
    let transaction_id = format!(
        "rinx.{}.{}.{}.{}",
        key.namespace,
        request.request_id,
        action.id,
        &request.input_digest[..16]
    );
    if modern {
        let choice = match action.id.as_str() {
            "approve_once" => model::ApprovalAction::ApproveOnce,
            "deny" => model::ApprovalAction::Deny,
            _ => return Err("Unsupported approval action"),
        };
        state
            .sessions
            .get_mut(key.namespace)
            .unwrap()
            .try_claim(
                claim_key(owner.as_str(), &key, &request.input_digest),
                choice,
                now,
            )
            .map_err(|_| "Approval already claimed or unavailable")?;
    }
    state.entries.get_mut(&key).unwrap().delivery =
        Some(ApprovalDecisionState::Sending(action.clone()));
    let claim = Claim {
        owner,
        epoch: state.epoch,
        key,
        transaction_id,
        source_event_id,
        digest: request.input_digest.clone(),
        modern,
    };
    drop(state);
    makepad_widgets::Cx::post_action(ApprovalStateChanged {
        room: room.to_owned(),
    });
    Ok(claim)
}
impl Claim {
    pub fn is_current(&self, client: &Client) -> bool {
        if !crate::matrix_context::is_current(client) || client.user_id() != Some(&self.owner) {
            return false;
        }
        if !client
            .get_room(&self.key.room)
            .is_some_and(|r| r.state() == matrix_sdk::RoomState::Joined)
        {
            return false;
        }
        let state = STATE.lock().unwrap();
        state.epoch == self.epoch
            && state.owner.as_ref() == Some(&self.owner)
            && state
                .entries
                .get(&self.key)
                .is_some_and(|e| matches!(e.delivery, Some(ApprovalDecisionState::Sending(_))))
    }
    pub(crate) fn complete(&self, result: model::SendResult) {
        let mut state = STATE.lock().unwrap();
        if state.epoch != self.epoch || state.owner.as_ref() != Some(&self.owner) {
            return;
        }
        if self.modern {
            if let Some(session) = state.sessions.get_mut(self.key.namespace) {
                session.record_send_result(
                    &claim_key(self.owner.as_str(), &self.key, &self.digest),
                    result,
                    current_unix_time_millis(),
                );
            }
        }
        if let Some(entry) = state.entries.get_mut(&self.key)
            && let Some(ApprovalDecisionState::Sending(action)) = entry.delivery.clone()
        {
            entry.delivery = match result {
                model::SendResult::ConfirmedPreSendFailure => None,
                model::SendResult::OutcomeUnknown => {
                    Some(ApprovalDecisionState::OutcomeUnknown(action))
                }
                model::SendResult::Sent => Some(ApprovalDecisionState::Sent(action)),
            };
        }
        drop(state);
        makepad_widgets::Cx::post_action(ApprovalStateChanged {
            room: self.key.room.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture(namespace: Namespace, modern: bool) -> (Runtime, Key, Value) {
        let owner = matrix_sdk::ruma::user_id!("@owner:test").to_owned();
        let room = matrix_sdk::ruma::room_id!("!approval:test");
        let mut content: Value =
            serde_json::from_str(include_str!("testdata/palpo-request.json")).unwrap();
        let mut detail = content["com.agentchat.approval"].clone();
        detail["request_id"] = json!("approval_0123456789abcdef0123456789abcdef");
        detail["expires_at"] = json!(5000);
        if modern {
            detail["owner_mxid"] = json!(owner.as_str());
            detail["publisher_mxid"] = json!("@bridge:test");
            detail["revision"] = json!(1);
            detail["actions"] = json!([{"id":"approve_once","label":"Approve once","style":"primary"},{"id":"deny","label":"Deny","style":"danger"}]);
        }
        content
            .as_object_mut()
            .unwrap()
            .remove("com.agentchat.approval");
        content[namespace.event_key()] = detail;
        content["msgtype"] = json!(namespace.request_msgtype());
        let request = super::super::approval::parse_approval_request(&content).unwrap();
        let key = key(room, &request);
        let mut state = Runtime {
            owner: Some(owner.clone()),
            ..Default::default()
        };
        state
            .sessions
            .entry(namespace.base())
            .or_default()
            .ingest_message(
                owner.as_str(),
                room.as_str(),
                "$request",
                "@bridge:test",
                &normalize(namespace, &content),
                1000,
            );
        state.entries.insert(
            key.clone(),
            Entry {
                request,
                sender: "@bridge:test".into(),
                modern,
                conflicted: false,
                delivery: None,
            },
        );
        (state, key, content)
    }
    #[test]
    fn canonical_status_overrides_local_delivery_in_every_namespace() {
        for namespace in Namespace::ALL {
            let (mut state, key, mut content) = fixture(namespace, true);
            let request = state.entries[&key].request.clone();
            let chosen = request.action("approve_once").unwrap().clone();
            assert_eq!(
                decision_locked(&state, &key, &request, 1000),
                ApprovalDecisionState::Pending
            );
            state.entries.get_mut(&key).unwrap().delivery =
                Some(ApprovalDecisionState::OutcomeUnknown(chosen));
            let detail = &mut content[namespace.event_key()];
            detail["kind"] = json!("status");
            detail["state"] = json!("approved");
            detail["decision"] = json!("allow");
            detail["revision"] = json!(2);
            detail.as_object_mut().unwrap().remove("actions");
            content["msgtype"] = json!(namespace.status_msgtype());
            let normalized = normalize(namespace, &content);
            state
                .sessions
                .get_mut(namespace.base())
                .unwrap()
                .ingest_message(
                    "@owner:test",
                    "!approval:test",
                    "$spoof",
                    "@other:test",
                    &normalized,
                    1100,
                );
            assert!(matches!(
                decision_locked(&state, &key, &request, 1100),
                ApprovalDecisionState::OutcomeUnknown(_)
            ));
            state
                .sessions
                .get_mut(namespace.base())
                .unwrap()
                .ingest_message(
                    "@owner:test",
                    "!approval:test",
                    "$status",
                    "@bridge:test",
                    &normalized,
                    1100,
                );
            assert_eq!(
                decision_locked(&state, &key, &request, 1100),
                ApprovalDecisionState::Confirmed("Approved".into())
            );
        }
    }
    #[test]
    fn legacy_scopes_remain_supported_and_shared_across_views() {
        let (mut state, key, _) = fixture(Namespace::DEFAULT, false);
        let request = state.entries[&key].request.clone();
        let chosen = request.action("approve_task").unwrap().clone();
        state.entries.get_mut(&key).unwrap().delivery =
            Some(ApprovalDecisionState::Sending(chosen.clone()));
        for _ in 0..4 {
            assert_eq!(
                decision_locked(&state, &key, &request, 1000),
                ApprovalDecisionState::Sending(chosen.clone())
            );
        }
        state.entries.get_mut(&key).unwrap().conflicted = true;
        assert_eq!(
            decision_locked(&state, &key, &request, 1000),
            ApprovalDecisionState::Unavailable
        );
    }
    #[test]
    fn partial_modern_bindings_never_fall_back_to_legacy_approval() {
        let (mut state, key, _) = fixture(Namespace::DEFAULT, false);
        state.entries.get_mut(&key).unwrap().modern = true;
        assert_eq!(
            decision_locked(&state, &key, &state.entries[&key].request, 1000),
            ApprovalDecisionState::Unavailable
        );
    }
}
