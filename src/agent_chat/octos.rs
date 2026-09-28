//! Octos AppService action messages. Approval metadata is immutable; ordinary
//! action menus may be edited. The service validates responses server-side.
use std::{
    collections::{HashMap, HashSet},
    sync::{LazyLock, Mutex},
};
use matrix_sdk::{
    Client,
    ruma::{OwnedEventId, OwnedRoomId, OwnedUserId, UserId},
};
use matrix_sdk_ui::timeline::EventTimelineItem;
use serde_json::{Value, json};
use super::approval::{ApprovalAction, ActionStyle, current_unix_time_millis, is_approval_msgtype};

const MAX_ACTIONS: usize = 6;
#[derive(Clone, Debug, PartialEq)]
pub struct Approval {
    request_id: String,
    digest: String,
    pub title: String,
    pub summary: String,
    approvers: Vec<OwnedUserId>,
    pub expires: u64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Payload {
    pub approval: Option<Approval>,
    pub actions: Vec<ApprovalAction>,
}
fn nonempty(value: &Value, key: &str) -> Option<String> {
    let value = value.get(key)?.as_str()?.trim();
    (!value.is_empty()).then(|| value.to_owned())
}
impl Payload {
    pub fn parse(original: &Value, latest: &Value) -> Option<Self> {
        // Never let Octos actions bypass a Hagency request/status parser.
        if [original, latest].into_iter().any(|c| {
            c.get("msgtype")
                .and_then(Value::as_str)
                .is_some_and(is_approval_msgtype)
        }) {
            return None;
        }
        let approval = if let Some(value) = original.get("org.octos.approval_request") {
            nonempty(value, "tool_name")?;
            if !matches!(value.get("risk_level")?.as_str()?, "normal" | "critical")
                || value.get("on_timeout")?.as_str()? != "notify"
            {
                return None;
            }
            let approvers = value
                .get("authorized_approvers")?
                .as_array()?
                .iter()
                .map(|v| OwnedUserId::try_from(v.as_str()?).ok())
                .collect::<Option<Vec<_>>>()?;
            if approvers.is_empty() {
                return None;
            }
            let expires = chrono::DateTime::parse_from_rfc3339(value.get("expires_at")?.as_str()?)
                .ok()?
                .timestamp_millis()
                .try_into()
                .ok()?;
            Some(Approval {
                request_id: nonempty(value, "request_id")?,
                digest: nonempty(value, "tool_args_digest")?,
                title: nonempty(value, "title")?,
                summary: nonempty(value, "summary")?,
                approvers,
                expires,
            })
        } else {
            // An edit cannot upgrade an ordinary message into an approval.
            if latest.get("org.octos.approval_request").is_some() {
                return None;
            }
            None
        };
        let content = if approval.is_some() { original } else { latest };
        let mut ids = HashSet::new();
        let actions = content
            .get("org.octos.actions")?
            .as_array()?
            .iter()
            .filter_map(|v| {
                let id = nonempty(v, "id")?;
                let label = nonempty(v, "label")?;
                if !ids.insert(id.clone())
                    || (approval.is_some() && !matches!(id.as_str(), "approve" | "deny"))
                {
                    return None;
                }
                let style = match v.get("style").and_then(Value::as_str) {
                    Some("primary") => ActionStyle::Primary,
                    Some("danger") => ActionStyle::Danger,
                    _ => ActionStyle::Secondary,
                };
                Some(ApprovalAction { id, label, style })
            })
            .take(MAX_ACTIONS)
            .collect::<Vec<_>>();
        (!actions.is_empty()).then_some(Self { approval, actions })
    }
    pub fn available(&self, user: &UserId, now: u64) -> bool {
        self.approval
            .as_ref()
            .is_none_or(|a| now < a.expires && a.approvers.iter().any(|id| id == user))
    }
    fn response(
        &self,
        action: &ApprovalAction,
        event: &OwnedEventId,
        sender: &OwnedUserId,
    ) -> Value {
        let mut content = json!({"msgtype":"m.text", "org.octos.target_user_id":sender,
            "m.relates_to":{"m.in_reply_to":{"event_id":event}}});
        if let Some(approval) = &self.approval {
            content["body"] = json!(format!("[Approval: {}] {}", action.id, approval.title));
            content["org.octos.approval_response"] = json!({"request_id":approval.request_id,"decision":action.id,"source_event_id":event,"tool_args_digest":approval.digest});
        } else {
            content["body"] = json!(format!("[Action: {}]", action.label));
            content["org.octos.action_response"] =
                json!({"action_id":action.id,"source_event_id":event});
        }
        content
    }
}

#[derive(Clone, Debug)]
pub struct Context {
    client: Client,
    key: (OwnedRoomId, OwnedEventId),
    sender: OwnedUserId,
    pub payload: Payload,
}
impl Context {
    pub fn from_event(room: &matrix_sdk::ruma::RoomId, event: &EventTimelineItem) -> Option<Self> {
        let original = event
            .original_json()?
            .get_field::<Value>("content")
            .ok()??;
        let edited = event
            .latest_edit_json()
            .and_then(|r| r.get_field::<Value>("content").ok())
            .flatten();
        let latest = edited
            .as_ref()
            .map(|c| c.get("m.new_content").unwrap_or(c))
            .unwrap_or(&original);
        let payload = Payload::parse(&original, latest)?;
        Some(Self {
            client: crate::sliding_sync::get_client()?,
            key: (room.to_owned(), event.event_id()?.to_owned()),
            sender: event.sender().to_owned(),
            payload,
        })
    }
    pub fn status(&self) -> Option<&'static str> {
        if !crate::matrix_context::is_current(&self.client) {
            return Some("Approval unavailable");
        }
        if let Some(status) = STATE.lock().unwrap().claims.get(&self.key).copied() {
            return Some(status);
        }
        self.payload.approval.as_ref().and_then(|a| {
            if current_unix_time_millis() >= a.expires {
                Some("Expired")
            } else if !self
                .client
                .user_id()
                .is_some_and(|u| self.payload.available(u, current_unix_time_millis()))
            {
                Some("Approval unavailable")
            } else {
                None
            }
        })
    }
    pub fn send(&self, action: &ApprovalAction) {
        if self.status().is_some() || !self.payload.actions.contains(action) {
            return;
        }
        let context = self.clone();
        let action = action.clone();
        let epoch = {
            let mut state = STATE.lock().unwrap();
            if state.claims.contains_key(&self.key) || state.claims.len() >= 4096 {
                return;
            }
            state.claims.insert(self.key.clone(), "Sending…");
            state.epoch
        };
        makepad_widgets::Cx::post_action(Changed);
        crate::sliding_sync::spawn_async_task(async move {
            let prepare = async {
                crate::matrix_context::ensure_current(&context.client)?;
                let room = context
                    .client
                    .get_room(&context.key.0)
                    .ok_or_else(|| anyhow::anyhow!("The room is unavailable."))?;
                anyhow::ensure!(
                    room.state() == matrix_sdk::RoomState::Joined,
                    "Join the room first."
                );
                if room.latest_encryption_state().await?.is_encrypted() {
                    context
                        .client
                        .encryption()
                        .request_user_identity(&context.sender)
                        .await?;
                    room.discard_room_key().await?;
                }
                crate::matrix_context::ensure_current(&context.client)?;
                anyhow::ensure!(
                    context
                        .client
                        .user_id()
                        .is_some_and(|u| context.payload.available(u, current_unix_time_millis())),
                    "This approval has expired."
                );
                Ok::<_, anyhow::Error>(room)
            }
            .await;
            let result = match prepare {
                Ok(room) => {
                    let transaction = matrix_sdk::ruma::TransactionId::new();
                    let content =
                        context
                            .payload
                            .response(&action, &context.key.1, &context.sender);
                    Some(
                        room.send_raw("m.room.message", content)
                            .with_transaction_id(&transaction)
                            .await
                            .is_ok(),
                    )
                }
                Err(_) => None,
            };
            let mut state = STATE.lock().unwrap();
            if state.epoch != epoch {
                return;
            }
            match result {
                Some(true) => {
                    state.claims.insert(context.key, "Sent · awaiting bridge");
                }
                Some(false) => {
                    state
                        .claims
                        .insert(context.key, "Delivery unknown · awaiting bridge");
                }
                None => {
                    state.claims.remove(&context.key);
                }
            }
            drop(state);
            if result.is_none() {
                crate::shared::popup_list::enqueue_popup_notification(
                    crate::i18n::tr(
                        "Action could not be sent. Check your connection and try again.",
                    ),
                    crate::shared::popup_list::PopupKind::Error,
                    Some(6.0),
                );
            }
            makepad_widgets::Cx::post_action(Changed);
        });
    }
}
#[derive(Default)]
struct State {
    epoch: u64,
    claims: HashMap<(OwnedRoomId, OwnedEventId), &'static str>,
}
static STATE: LazyLock<Mutex<State>> = LazyLock::new(Default::default);
#[derive(Clone, Debug)]
pub struct Changed;
pub fn reset() {
    let mut state = STATE.lock().unwrap();
    state.epoch = state.epoch.wrapping_add(1);
    state.claims.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    fn menu() -> Value {
        json!({"msgtype":"m.text","org.octos.actions":[{"id":"approve","label":"Approve","style":"primary"},{"id":"deny","label":"Deny","style":"danger"}]})
    }
    fn approval() -> Value {
        let mut value = menu();
        value["org.octos.approval_request"] = json!({"request_id":"req1","tool_name":"Bash","tool_args_digest":"sha256:abc","title":"Run tests","summary":"cargo test","risk_level":"normal","authorized_approvers":["@owner:example.org"],"expires_at":"2030-01-01T00:00:00Z","on_timeout":"notify"});
        value
    }
    #[test]
    fn approval_bindings_and_buttons_cannot_be_edited() {
        let original = approval();
        let mut edited = original.clone();
        edited["org.octos.approval_request"]["tool_args_digest"] = json!("forged");
        edited["org.octos.actions"] = json!([]);
        let parsed = Payload::parse(&original, &edited).unwrap();
        assert_eq!(parsed.actions.len(), 2);
        assert_eq!(parsed.approval.unwrap().digest, "sha256:abc");
        assert!(Payload::parse(&menu(), &approval()).is_none());
    }
    #[test]
    fn malformed_approval_cannot_fall_back_to_actions() {
        for field in [
            "request_id",
            "tool_args_digest",
            "title",
            "summary",
            "authorized_approvers",
            "expires_at",
            "risk_level",
            "on_timeout",
        ] {
            let mut bad = approval();
            bad["org.octos.approval_request"]
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(Payload::parse(&bad, &bad).is_none(), "{field}");
        }
        for ns in super::super::approval::Namespace::ALL {
            let mut bad = menu();
            bad["msgtype"] = json!(ns.status_msgtype());
            assert!(Payload::parse(&bad, &bad).is_none());
        }
    }
    #[test]
    fn expiry_authorization_and_wire_response() {
        let parsed = Payload::parse(&approval(), &approval()).unwrap();
        let user = OwnedUserId::try_from("@owner:example.org").unwrap();
        let stranger = OwnedUserId::try_from("@stranger:example.org").unwrap();
        assert!(parsed.available(&user, 1));
        assert!(!parsed.available(&stranger, 1));
        assert!(!parsed.available(&user, u64::MAX));
        let event = OwnedEventId::try_from("$source").unwrap();
        let response = parsed.response(&parsed.actions[1], &event, &stranger);
        assert_eq!(response["org.octos.approval_response"]["decision"], "deny");
        assert_eq!(
            response["org.octos.approval_response"]["tool_args_digest"],
            "sha256:abc"
        );
        assert_eq!(response["org.octos.target_user_id"], stranger.as_str());
        assert_eq!(
            response["m.relates_to"]["m.in_reply_to"]["event_id"],
            "$source"
        );
    }
    #[test]
    fn generic_edits_and_limits() {
        let original = menu();
        let mut edited = menu();
        edited["org.octos.actions"] = json!([{"id":"retry","label":"Retry"}]);
        let parsed = Payload::parse(&original, &edited).unwrap();
        assert_eq!(parsed.actions[0].id, "retry");
        edited["org.octos.actions"] = json!(
            (0..9)
                .map(|i| json!({"id":i.to_string(),"label":"Click"}))
                .collect::<Vec<_>>()
        );
        assert_eq!(Payload::parse(&original, &edited).unwrap().actions.len(), 6);
    }
}
