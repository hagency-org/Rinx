//! Untrusted native activity pointers. Parsing never issues a lease or performs a write.
use serde::{Deserialize, Serialize};

pub const MSGTYPE: &str = "org.rinx.native.activity";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityCard {
    pub version: u8,
    pub app: String,
    pub resource: String,
    pub room: String,
    pub title: String,
    pub start: u64,
    pub end: u64,
    pub location: String,
}

impl ActivityCard {
    pub fn parse(value: serde_json::Value) -> Result<Self, String> {
        let card: Self = serde_json::from_value(value).map_err(|_| "Invalid activity fields")?;
        card.validate()?;
        Ok(card)
    }
    pub fn validate(&self) -> Result<(), String> {
        let identifier = |s: &str| !s.is_empty() && s.len() <= 128
            && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b));
        let line = |s: &str, limit: usize| s.chars().count() <= limit && !s.chars().any(char::is_control);
        if self.version != 1 || !identifier(&self.app) || !identifier(&self.resource)
            || self.room.len() > 255 || !self.room.starts_with('!')
            || !self.room[1..].split_once(':').is_some_and(|(local, server)| !local.is_empty() && !server.is_empty() && !server.contains(['/', '?', '#', '\\']))
            || self.room.chars().any(|c| c.is_whitespace() || c.is_control())
            || self.title.trim().is_empty() || !line(&self.title, 120)
            || !line(&self.location, 200) || self.start >= self.end || self.end > 253402300799
        { return Err("Unsupported or invalid native activity card".into()); }
        Ok(())
    }
}

/// Filled by the SDK timeline, outside the deserialized card. The receiving
/// host must re-fetch this event and validate current identity, membership,
/// source and resource before opening. No card field can grant permission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageSource {
    pub room: String,
    pub event: String,
    pub sender: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wire() -> serde_json::Value {
        serde_json::json!({"version":1,"app":"org.example.bookclub","resource":"activity-42",
            "room":"!club:example.org","title":"Reading club","start":1900000000u64,
            "end":1900007200u64,"location":"Library"})
    }
    #[test] fn round_trip_keeps_only_a_pointer() {
        let card = ActivityCard::parse(wire()).unwrap();
        assert_eq!(ActivityCard::parse(serde_json::to_value(&card).unwrap()).unwrap(), card);
    }
    #[test] fn future_protocol_and_malformed_fields_are_rejected() {
        for (field, value) in [("version",serde_json::json!(2)), ("room",serde_json::json!("https://example.org")),
            ("resource",serde_json::json!("../private")), ("app",serde_json::json!("file://local")),
            ("title",serde_json::json!("broken\nline")), ("end",serde_json::json!(1900000000u64))] {
            let mut value_wire = wire(); value_wire[field] = value;
            assert!(ActivityCard::parse(value_wire).is_err(), "{field}");
        }
    }
    #[test] fn identity_and_grants_cannot_be_smuggled_into_cards() {
        for field in ["account", "sender", "token", "authorization", "action", "source_event"] {
            let mut value = wire(); value[field] = serde_json::json!("forged");
            assert!(ActivityCard::parse(value).is_err(), "{field}");
        }
    }
    #[test] fn empty_room_parts_and_url_components_are_rejected() {
        for room in ["!:example.org", "!room:", "!room:example.org/path", "!room:example.org?query", "!room:example.org#fragment"] {
            let mut value = wire(); value["room"] = serde_json::json!(room);
            assert!(ActivityCard::parse(value).is_err(), "{room}");
        }
    }
    #[test] fn room_server_may_include_an_ipv6_address_and_port() {
        let mut value = wire(); value["room"] = serde_json::json!("!opaque:[2001:db8::1]:8448");
        assert!(ActivityCard::parse(value).is_ok());
    }
}
