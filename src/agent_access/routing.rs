//! Explicit Octos addressing. A saved binding never silently targets ordinary
//! room messages; only commands, replies, or user-selected mentions do so.
use matrix_sdk::ruma::{RoomId, UserId, OwnedUserId, events::room::message::RoomMessageEventContent};
use serde_json::{Map, Value, json};
use super::model::AgentAccessSettings;

pub fn directives(
    settings: &AgentAccessSettings,
    owner: Option<&UserId>,
    room: &RoomId,
    is_direct: bool,
    message: &RoomMessageEventContent,
    reply: Option<&UserId>,
) -> Result<Option<Map<String, Value>>, String> {
    let bots = &settings.bot_settings;
    let parent = bots
        .enabled
        .then(|| bots.resolved_bot_user_id(owner).ok())
        .flatten();
    let bindings = bots.room_bindings_for(room);
    let mut candidates = settings.agent_registry.agent_user_ids();
    candidates.extend(bots.known_bot_user_ids());
    candidates.extend(bindings.iter().map(|b| b.bot_user_id.clone()));
    candidates.extend(parent.clone());
    candidates.sort();
    candidates.dedup();
    if candidates.is_empty() {
        return Ok(None);
    }
    let command = message
        .body()
        .strip_prefix('/')
        .filter(|s| !s.starts_with('/'))
        .and_then(|s| s.split_whitespace().next());
    let management = parent
        .as_ref()
        .is_some_and(|p| bindings.iter().any(|b| &b.bot_user_id == p));
    let mentioned = candidates.iter().any(|u| {
        message
            .mentions
            .as_ref()
            .is_some_and(|m| m.user_ids.contains(u))
            || message.body().contains(u.as_str())
            || message
                .body()
                .split(|c: char| c.is_whitespace() || matches!(c, ',' | '!' | '?' | '(' | ')'))
                .any(|word| word == format!("@{}", u.localpart()))
    });
    let target: Option<OwnedUserId> =
        if let Some((_, local)) = command.and_then(|c| c.split_once('@')) {
            let matches = candidates
                .iter()
                .filter(|u| u.localpart().eq_ignore_ascii_case(local))
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [user] => Some((*user).clone()),
                _ => return Err(format!("Unknown or ambiguous registered agent: @{local}")),
            }
        } else if management && command.is_some_and(|c| super::commands::contains(true, c)) {
            parent.clone()
        } else if !mentioned {
            reply
                .filter(|u| candidates.iter().any(|c| c.as_str() == u.as_str()))
                .map(ToOwned::to_owned)
        } else {
            None
        };
    let mut extra = Map::new();
    if let Some(target) = target {
        extra.insert("org.octos.target_user_id".into(), json!(target));
    } else {
        extra.insert("org.octos.explicit_room".into(), json!(true));
    }
    if management && !is_direct && command == Some("allbots") {
        let mut targets = bindings
            .iter()
            .map(|b| b.bot_user_id.clone())
            .filter(|u| Some(u) != parent.as_ref())
            .collect::<Vec<_>>();
        targets.sort();
        targets.dedup();
        if !targets.is_empty() {
            extra.insert("org.octos.broadcast_targets".into(), json!(targets));
        }
    }
    Ok(Some(extra))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bindings_do_not_implicitly_address_room_messages() {
        let mut settings = AgentAccessSettings::default();
        settings.bot_settings.enabled = true;
        settings.bot_settings.botfather_user_id = "@father:example.org".into();
        let room = matrix_sdk::ruma::room_id!("!r:example.org");
        let father = matrix_sdk::ruma::user_id!("@father:example.org");
        let child = matrix_sdk::ruma::user_id!("@child:example.org");
        for id in [father, child] {
            settings
                .bot_settings
                .set_room_bound(room.to_owned(), Some(id.to_owned()), true);
        }
        let route = |text: &str, reply: Option<&UserId>, direct: bool| {
            directives(
                &settings,
                None,
                room,
                direct,
                &RoomMessageEventContent::text_plain(text),
                reply,
            )
            .unwrap()
            .unwrap()
        };
        assert_eq!(route("hello", None, false)["org.octos.explicit_room"], true);
        assert_eq!(
            route("hello", Some(child), false)["org.octos.target_user_id"],
            child.as_str()
        );
        assert_eq!(
            route("@child hello", Some(father), false)["org.octos.explicit_room"],
            true
        );
        assert_eq!(
            route("/listbots", None, false)["org.octos.target_user_id"],
            father.as_str()
        );
        assert_eq!(
            route("/status@child", None, false)["org.octos.target_user_id"],
            child.as_str()
        );
        assert_eq!(
            route("/allbots hello", None, false)["org.octos.broadcast_targets"],
            json!([child])
        );
        assert!(!route("/allbots hello", None, true).contains_key("org.octos.broadcast_targets"));
        assert!(
            directives(
                &settings,
                None,
                room,
                false,
                &RoomMessageEventContent::text_plain("/status@missing"),
                None
            )
            .is_err()
        );
    }
}
