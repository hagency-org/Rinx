//! Bot identity hints from the exact configured BotFather in a bound room.
use matrix_sdk::{
    Client,
    ruma::{OwnedServerName, OwnedRoomId, OwnedUserId, UserId, RoomId},
};
use matrix_sdk_ui::timeline::EventTimelineItem;
use super::model::BotSettingsState;
#[derive(Clone, Debug)]
pub struct DiscoveredBots {
    pub owner: OwnedUserId,
    pub room: OwnedRoomId,
    pub sender: OwnedUserId,
    pub users: Vec<OwnedUserId>,
}
pub fn ingest(client: &Client, room: &RoomId, event: &EventTimelineItem) {
    if !crate::matrix_context::is_current(client) {
        return;
    }
    let Some(owner) = client.user_id() else {
        return;
    };
    let settings = super::current();
    let bots = &settings.bot_settings;
    if !bots.enabled
        || !bots.is_room_bound(room)
        || bots.resolved_bot_user_id(Some(owner)).ok().as_deref() != Some(event.sender())
    {
        return;
    }
    let Some(content) = event
        .original_json()
        .and_then(|r| r.get_field::<serde_json::Value>("content").ok())
        .flatten()
    else {
        return;
    };
    if content
        .get("m.relates_to")
        .and_then(|r| r.get("rel_type"))
        .and_then(serde_json::Value::as_str)
        == Some("m.replace")
    {
        return;
    }
    let Some(body) = content.get("body").and_then(serde_json::Value::as_str) else {
        return;
    };
    if body.len() > 65536 {
        return;
    }
    let users =
        extract_bot_user_ids_from_listbots_reply(body, Some(&owner.server_name().to_owned()))
            .into_iter()
            .filter(|id| {
                id != owner && id != event.sender() && !bots.known_bot_user_ids.contains(id)
            })
            .take(256)
            .collect::<Vec<_>>();
    if !users.is_empty() {
        makepad_widgets::Cx::post_action(DiscoveredBots {
            owner: owner.to_owned(),
            room: room.to_owned(),
            sender: event.sender().to_owned(),
            users,
        });
    }
}
fn extract_bot_user_ids_from_listbots_reply(
    text: &str,
    default_server_name: Option<&OwnedServerName>,
) -> Vec<OwnedUserId> {
    let mut bot_user_ids = Vec::<OwnedUserId>::new();

    let mut push_bot = |bot_user_id: OwnedUserId| {
        if !bot_user_ids
            .iter()
            .any(|existing_bot_user_id| existing_bot_user_id.as_str() == bot_user_id.as_str())
        {
            bot_user_ids.push(bot_user_id);
        }
    };

    for token in text.split(|ch: char| {
        !(ch.is_ascii_alphanumeric() || matches!(ch, '@' | ':' | '_' | '-' | '.'))
    }) {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }

        if token.starts_with('@') && token.contains(':') {
            if let Ok(bot_user_id) = UserId::parse(token).map(|user_id| user_id.to_owned()) {
                if BotSettingsState::is_valid_known_bot_user_id(bot_user_id.as_ref()) {
                    push_bot(bot_user_id);
                }
            }
            continue;
        }

        if token.contains(':') && !token.starts_with('@') {
            let full_user_id = format!("@{token}");
            if let Ok(bot_user_id) = UserId::parse(&full_user_id).map(|user_id| user_id.to_owned())
            {
                if BotSettingsState::is_valid_known_bot_user_id(bot_user_id.as_ref()) {
                    push_bot(bot_user_id);
                }
            }
            continue;
        }

        let localpart_lc = token.to_ascii_lowercase();
        let is_likely_bot_localpart = (localpart_lc == "bot"
            || localpart_lc.starts_with("bot_")
            || localpart_lc.starts_with("bot-")
            || localpart_lc.starts_with("bot."))
            && localpart_lc != "bots"
            && localpart_lc != "botfather"
            && token
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.');
        if !is_likely_bot_localpart {
            continue;
        }

        let Some(default_server_name) = default_server_name else {
            continue;
        };
        let full_user_id = format!("@{token}:{default_server_name}");
        if let Ok(bot_user_id) = UserId::parse(&full_user_id).map(|user_id| user_id.to_owned()) {
            push_bot(bot_user_id);
        }
    }

    bot_user_ids
}
#[cfg(test)]
mod tests {
    #[test]
    fn service_urls_are_not_bot_ids() {
        let ids = super::extract_bot_user_ids_from_listbots_reply(
            "Service: http://127.0.0.1:8010 Bots: @helper:example.org",
            None,
        );
        assert_eq!(
            ids,
            vec![matrix_sdk::ruma::user_id!("@helper:example.org").to_owned()]
        );
    }
}
