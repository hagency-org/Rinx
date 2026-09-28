// Adapted from Project-Robius-China/robrix2 at da375f241df3368a24318c94a5ed4865009f7529.
//! Persisted Matrix agent identity metadata. Registration does not confer authority.
use std::collections::BTreeMap;
use makepad_widgets::log;
use matrix_sdk::ruma::{OwnedMxcUri, OwnedRoomId, OwnedUserId, RoomId, UserId};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentAccessSettings {
    pub agent_registry: AgentRegistry,
    pub bot_settings: BotSettingsState,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentFramework {
    /// Framework not yet identified.
    #[default]
    Unknown,
    /// Octos app-service backed agent.
    Octos,
    /// Octos added as a direct agent (user-account mode; NOT App Service /
    /// BotFather). robrix only interacts with it over Matrix via its MXID; its
    /// deployment location is invisible to robrix.
    OctosDirect,
    /// Hermes external client integration.
    Hermes,
    /// OpenClaw external client integration.
    OpenClaw,
}

/// How much the local user trusts an agent. The lowest tier is the default;
/// higher tiers are added by the slice that lets the user grant trust.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum TrustTier {
    /// Default, least-privileged tier.
    #[default]
    Untrusted,
}

/// A single capability an agent advertises (free-form tag).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentCapability(pub String);

/// A registered agent's metadata, keyed in [`AgentRegistry`] by its Matrix user ID.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AgentEntry {
    /// Human-readable name, if known.
    pub display_name: Option<String>,
    /// The framework / runtime backing this agent.
    pub framework: AgentFramework,
    /// Avatar MXC URI, if known.
    pub avatar: Option<OwnedMxcUri>,
    /// Capabilities the agent advertises.
    pub capabilities: Vec<AgentCapability>,
    /// Local trust level for this agent.
    pub trust_tier: TrustTier,
}

/// Global source of truth for agent identities, persisted per Matrix account
/// as part of [`AgentAccessSettings`]. Keyed by agent MXID for dedup and deterministic order.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct AgentRegistry {
    agents: BTreeMap<OwnedUserId, AgentEntry>,
}

impl AgentRegistry {
    /// Returns `true` if no agents are registered.
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }

    /// Returns the number of registered agents.
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Returns `true` if an agent with the given MXID is registered.
    pub fn contains(&self, user_id: &UserId) -> bool {
        self.agents
            .keys()
            .any(|registered| registered.as_str() == user_id.as_str())
    }

    /// Returns the entry for the given MXID, if registered.
    pub fn get(&self, user_id: &UserId) -> Option<&AgentEntry> {
        self.agents
            .iter()
            .find(|(registered, _)| registered.as_str() == user_id.as_str())
            .map(|(_, entry)| entry)
    }

    /// Registers an agent, keeping any existing entry for the same MXID (idempotent).
    ///
    /// Returns `true` if a new entry was inserted, `false` if one already existed.
    pub fn register(&mut self, user_id: OwnedUserId, entry: AgentEntry) -> bool {
        use std::collections::btree_map::Entry;
        match self.agents.entry(user_id) {
            Entry::Vacant(vacant) => {
                vacant.insert(entry);
                true
            }
            Entry::Occupied(_) => false,
        }
    }

    /// Removes the agent registered under `user_id`.
    ///
    /// Returns `true` if an entry was removed, `false` if none was registered.
    pub fn unregister(&mut self, user_id: &UserId) -> bool {
        let key = self
            .agents
            .keys()
            .find(|registered| registered.as_str() == user_id.as_str())
            .cloned();
        match key {
            Some(k) => {
                self.agents.remove(&k);
                true
            }
            None => false,
        }
    }

    /// All registered agent MXIDs, in deterministic (sorted) order.
    pub fn agent_user_ids(&self) -> Vec<OwnedUserId> {
        self.agents.keys().cloned().collect()
    }

    /// Iterates all registered agents with their entries, in deterministic
    /// (sorted) order — one O(n) pass, no per-id lookups.
    pub fn agents(&self) -> impl Iterator<Item = (&OwnedUserId, &AgentEntry)> {
        self.agents.iter()
    }
}

impl AgentAccessSettings {
    pub fn normalize(&mut self) {
        self.bot_settings
            .room_bindings
            .sort_by(|a, b| (&a.room_id, &a.bot_user_id).cmp(&(&b.room_id, &b.bot_user_id)));
        self.bot_settings
            .room_bindings
            .dedup_by(|a, b| a.room_id == b.room_id && a.bot_user_id == b.bot_user_id);
        self.seed_agent_registry_from_known_bots();
    }

    /// Returns `true` if a new DM room with `target_user_id` should be encrypted.
    ///
    /// Combines [`BotSettingsState::should_create_encrypted_dm`] with the
    /// [`AgentRegistry`]: any registered agent is treated as a bot and gets an
    /// unencrypted DM; every other user gets an encrypted DM.
    pub fn should_create_encrypted_dm(
        &self,
        target_user_id: &UserId,
        current_user_id: Option<&UserId>,
    ) -> bool {
        let is_agent = self.agent_registry.contains(target_user_id);
        let is_bot = self
            .bot_settings
            .is_identified_bot(target_user_id, current_user_id);
        let encrypted = !is_agent && !is_bot;
        log!(
            "DM encryption decision for {target_user_id}: encrypted={encrypted} \
            (registered_agent={is_agent}, identified_bot={is_bot}, \
            agents={}, known_bots={}, bound_bots={}, botfather={:?})",
            self.agent_registry.len(),
            self.bot_settings.known_bot_user_ids.len(),
            self.bot_settings.room_bindings.len(),
            self.bot_settings.resolved_bot_user_id(current_user_id).ok(),
        );
        encrypted
    }

    /// Migration: if the agent registry is empty, seed it from the legacy
    /// per-account known-bot list so upgraded users keep bot identification.
    ///
    /// Existing registry entries are never overwritten, and the legacy
    /// `known_bot_user_ids` list is left intact (other flows still rely on it).
    pub fn seed_agent_registry_from_known_bots(&mut self) {
        self.bot_settings.prune_malformed_known_bot_user_ids();
        if self.agent_registry.is_empty() {
            for bot_user_id in self.bot_settings.known_bot_user_ids() {
                self.agent_registry.register(
                    bot_user_id,
                    AgentEntry {
                        framework: AgentFramework::Unknown,
                        ..Default::default()
                    },
                );
            }
        }
    }

    /// Removes an AgentLab registration and the bot identity state that was
    /// derived from that registration.
    ///
    /// Octos registration writes the same MXID into several legacy app-service
    /// fields so slash commands and room binding keep working. Unbind must clear
    /// those fields too, otherwise list/member bot markers keep rendering from
    /// stale `known_bot_user_ids`, room bindings, or the configured BotFather ID.
    pub fn unregister_agent_and_clear_bot_identity(
        &mut self,
        user_id: &UserId,
        current_user_id: Option<&UserId>,
    ) -> bool {
        let removed_agent = self.agent_registry.unregister(user_id);
        let removed_known_bot = self.bot_settings.remove_known_bot_user_id(user_id);
        let removed_room_bindings = self
            .bot_settings
            .remove_room_bindings_where(|_, bot_user_id| bot_user_id == user_id);
        let cleared_configured_bot = self
            .bot_settings
            .clear_configured_bot_if_matches(user_id, current_user_id);

        removed_agent || removed_known_bot || removed_room_bindings > 0 || cleared_configured_bot
    }
}

/// Local bot integration settings persisted per Matrix account.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct BotSettingsState {
    /// Whether bot-assisted room binding is enabled in the UI.
    pub enabled: bool,
    /// The configured botfather user, either as a full MXID or localpart.
    pub botfather_user_id: String,
    /// The Octos service base URL used for health checks.
    pub octos_service_url: String,
    /// Bots discovered from BotFather `/listbots` replies.
    pub known_bot_user_ids: Vec<OwnedUserId>,
    /// Rooms that Robrix currently considers bot-bound,
    /// paired with the exact bot MXID used for that room.
    pub room_bindings: Vec<RoomBotBindingState>,
}

/// A persisted room-level bot binding.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoomBotBindingState {
    pub room_id: OwnedRoomId,
    pub bot_user_id: OwnedUserId,
    #[serde(default)]
    pub remark: String,
}

impl Default for BotSettingsState {
    fn default() -> Self {
        Self {
            enabled: false,
            botfather_user_id: Self::DEFAULT_BOTFATHER_LOCALPART.to_string(),
            octos_service_url: Self::DEFAULT_OCTOS_SERVICE_URL.to_string(),
            known_bot_user_ids: Vec::new(),
            room_bindings: Vec::new(),
        }
    }
}

impl BotSettingsState {
    pub const DEFAULT_BOTFATHER_LOCALPART: &'static str = "bot";
    pub const DEFAULT_OCTOS_SERVICE_URL: &'static str = "http://127.0.0.1:8010";

    fn is_numeric_host_fragment(value: &str) -> bool {
        value.contains('.') && value.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
    }

    fn is_port_only_server_name(value: &str) -> bool {
        !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit())
    }

    pub(crate) fn is_valid_known_bot_user_id(bot_user_id: &UserId) -> bool {
        let localpart = bot_user_id.localpart();
        if localpart.is_empty() || Self::is_numeric_host_fragment(localpart) {
            return false;
        }

        !Self::is_port_only_server_name(bot_user_id.server_name().as_str())
    }

    fn prune_malformed_known_bot_user_ids(&mut self) -> bool {
        let original_len = self.known_bot_user_ids.len();
        self.known_bot_user_ids
            .retain(|bot_user_id| Self::is_valid_known_bot_user_id(bot_user_id.as_ref()));
        original_len != self.known_bot_user_ids.len()
    }

    pub fn resolved_octos_service_url(&self) -> &str {
        let raw = self.octos_service_url.trim();
        if raw.is_empty() {
            Self::DEFAULT_OCTOS_SERVICE_URL
        } else {
            raw
        }
    }

    pub fn validate_octos_service_url(service_url: &str) -> Result<(), String> {
        let service_url = service_url.trim();
        if service_url.is_empty() {
            return Err("Octos service URL cannot be empty.".into());
        }

        let parsed_url =
            Url::parse(service_url).map_err(|e| format!("Invalid Octos service URL: {e}"))?;

        match parsed_url.scheme() {
            "http" | "https" => {}
            scheme => {
                return Err(format!(
                    "Unsupported Octos service URL scheme `{scheme}`. Use http or https."
                ));
            }
        }

        if parsed_url.host_str().is_none() {
            return Err("Octos service URL must include a host.".into());
        }

        if !parsed_url.username().is_empty()
            || parsed_url.password().is_some()
            || parsed_url.query().is_some()
            || parsed_url.fragment().is_some()
        {
            return Err("Service URL must not contain credentials, a query or a fragment.".into());
        }
        Ok(())
    }

    pub fn validate_botfather_user_id(
        botfather_user_id: &str,
        current_user_id: Option<&UserId>,
    ) -> Result<(), String> {
        let botfather_user_id = botfather_user_id.trim();
        if botfather_user_id.is_empty() {
            return Err("BotFather user ID cannot be empty.".into());
        }

        Self {
            botfather_user_id: botfather_user_id.to_string(),
            ..Self::default()
        }
        .resolved_bot_user_id(current_user_id)
        .map(|_| ())
    }

    fn room_binding_index(&self, room_id: &RoomId, bot_user_id: &UserId) -> Result<usize, usize> {
        self.room_bindings.binary_search_by(|binding| {
            (binding.room_id.as_str(), binding.bot_user_id.as_str())
                .cmp(&(room_id.as_str(), bot_user_id.as_str()))
        })
    }

    fn room_binding_range(&self, room_id: &RoomId) -> std::ops::Range<usize> {
        let start = self
            .room_bindings
            .partition_point(|binding| binding.room_id.as_str() < room_id.as_str());
        let end = self
            .room_bindings
            .iter()
            .skip(start)
            .position(|binding| binding.room_id.as_str() != room_id.as_str())
            .map_or(self.room_bindings.len(), |offset| start + offset);
        start..end
    }

    /// Returns `true` if the given room is currently marked as bound locally.
    pub fn is_room_bound(&self, room_id: &RoomId) -> bool {
        !self.bound_bot_user_ids(room_id).is_empty()
    }

    /// Returns the persisted BotFather MXID for the given room, if any.
    pub fn bound_bot_user_id(&self, room_id: &RoomId) -> Option<&UserId> {
        let room_binding_range = self.room_binding_range(room_id);
        self.room_bindings
            .get(room_binding_range.start)
            .map(|binding| binding.bot_user_id.as_ref())
    }

    /// Returns all persisted bot MXIDs for the given room.
    pub fn bound_bot_user_ids(&self, room_id: &RoomId) -> Vec<OwnedUserId> {
        self.room_bindings[self.room_binding_range(room_id)]
            .iter()
            .map(|binding| binding.bot_user_id.clone())
            .collect()
    }

    /// Returns all bot bindings for the given room.
    pub fn room_bindings_for(&self, room_id: &RoomId) -> Vec<RoomBotBindingState> {
        self.room_bindings[self.room_binding_range(room_id)].to_vec()
    }

    /// Returns all known bound bot MXIDs across every room, deduplicated.
    pub fn all_bound_bot_user_ids(&self) -> Vec<OwnedUserId> {
        let mut all_bots = self
            .room_bindings
            .iter()
            .map(|binding| binding.bot_user_id.clone())
            .collect::<Vec<_>>();
        all_bots.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        all_bots.dedup_by(|a, b| a.as_str() == b.as_str());
        all_bots
    }

    /// Returns bot MXIDs discovered from BotFather `/listbots` replies.
    pub fn known_bot_user_ids(&self) -> Vec<OwnedUserId> {
        self.known_bot_user_ids.clone()
    }

    /// Merges the given discovered bot IDs into the known bot list.
    ///
    /// Returns `true` if the list changed.
    pub fn record_known_bot_user_ids(
        &mut self,
        discovered_bot_user_ids: impl IntoIterator<Item = OwnedUserId>,
    ) -> bool {
        let mut changed = self.prune_malformed_known_bot_user_ids();
        for bot_user_id in discovered_bot_user_ids {
            if !Self::is_valid_known_bot_user_id(bot_user_id.as_ref()) {
                continue;
            }
            if !self
                .known_bot_user_ids
                .iter()
                .any(|existing| existing.as_str() == bot_user_id.as_str())
            {
                self.known_bot_user_ids.push(bot_user_id);
                changed = true;
            }
        }
        if changed {
            self.known_bot_user_ids
                .sort_by(|lhs, rhs| lhs.as_str().cmp(rhs.as_str()));
            self.known_bot_user_ids
                .dedup_by(|lhs, rhs| lhs.as_str() == rhs.as_str());
        }
        changed
    }

    pub fn remove_known_bot_user_id(&mut self, bot_user_id: &UserId) -> bool {
        let original_len = self.known_bot_user_ids.len();
        self.known_bot_user_ids
            .retain(|known_bot_user_id| known_bot_user_id.as_str() != bot_user_id.as_str());
        original_len != self.known_bot_user_ids.len()
    }

    pub fn clear_configured_bot_if_matches(
        &mut self,
        bot_user_id: &UserId,
        current_user_id: Option<&UserId>,
    ) -> bool {
        let matches_configured_bot =
            self.resolved_bot_user_id(current_user_id)
                .ok()
                .is_some_and(|resolved_bot_user_id| {
                    resolved_bot_user_id.as_str() == bot_user_id.as_str()
                });
        if !matches_configured_bot {
            return false;
        }

        let changed =
            self.enabled || self.botfather_user_id.trim() != Self::DEFAULT_BOTFATHER_LOCALPART;
        self.enabled = false;
        self.botfather_user_id = Self::DEFAULT_BOTFATHER_LOCALPART.to_string();
        changed
    }

    /// Updates the local bound/unbound state for the given room.
    pub fn set_room_bound(
        &mut self,
        room_id: OwnedRoomId,
        bot_user_id: Option<OwnedUserId>,
        bound: bool,
    ) {
        if bound {
            let Some(bot_user_id) = bot_user_id else {
                return;
            };
            match self.room_binding_index(room_id.as_ref(), bot_user_id.as_ref()) {
                Ok(_) => {}
                Err(insert_index) => {
                    self.room_bindings.insert(
                        insert_index,
                        RoomBotBindingState {
                            room_id,
                            bot_user_id,
                            remark: String::new(),
                        },
                    );
                }
            }
        } else {
            if let Some(bot_user_id) = bot_user_id {
                if let Ok(existing_index) =
                    self.room_binding_index(room_id.as_ref(), bot_user_id.as_ref())
                {
                    self.room_bindings.remove(existing_index);
                }
            } else {
                self.room_bindings
                    .retain(|binding| binding.room_id != room_id);
            }
        }
    }

    /// Auto-binds a DM room when it targets the configured app-service bot or a known bot.
    ///
    /// Returns `true` if a bot binding should exist for this room/target pair.
    pub fn bind_dm_target_if_needed(
        &mut self,
        room_id: OwnedRoomId,
        target_user_id: &UserId,
        current_user_id: Option<&UserId>,
    ) -> bool {
        if !self.enabled {
            return false;
        }

        let matches_configured_bot =
            self.resolved_bot_user_id(current_user_id)
                .ok()
                .is_some_and(|configured_bot_user_id| {
                    configured_bot_user_id.as_str() == target_user_id.as_str()
                });
        let matches_known_bot = self
            .known_bot_user_ids
            .iter()
            .any(|known_bot_user_id| known_bot_user_id.as_str() == target_user_id.as_str());

        if !(matches_configured_bot || matches_known_bot) {
            return false;
        }

        self.set_room_bound(room_id, Some(target_user_id.to_owned()), true);
        true
    }

    /// Updates the remark for a specific room bot binding.
    ///
    /// Returns `true` if a binding existed and was updated.
    pub fn set_room_bot_remark(
        &mut self,
        room_id: &RoomId,
        bot_user_id: &UserId,
        remark: String,
    ) -> bool {
        if let Ok(index) = self.room_binding_index(room_id, bot_user_id) {
            self.room_bindings[index].remark = remark;
            true
        } else {
            false
        }
    }

    pub fn remove_room_bindings_where(
        &mut self,
        mut predicate: impl FnMut(&RoomId, &UserId) -> bool,
    ) -> usize {
        let original_len = self.room_bindings.len();
        self.room_bindings
            .retain(|binding| !predicate(binding.room_id.as_ref(), binding.bot_user_id.as_ref()));
        original_len.saturating_sub(self.room_bindings.len())
    }

    /// Returns the configured botfather user ID, resolving a localpart against
    /// the current user's homeserver when needed.
    pub fn resolved_bot_user_id(
        &self,
        current_user_id: Option<&UserId>,
    ) -> Result<OwnedUserId, String> {
        let raw = self.botfather_user_id.trim();
        if raw.starts_with('@') || raw.contains(':') {
            let full_user_id = if raw.starts_with('@') {
                raw.to_string()
            } else {
                format!("@{raw}")
            };
            return UserId::parse(&full_user_id)
                .map(|user_id| user_id.to_owned())
                .map_err(|_| format!("Invalid bot user ID: {full_user_id}"));
        }

        let Some(current_user_id) = current_user_id else {
            return Err(
                "Current user ID is unavailable, so the bot homeserver cannot be resolved.".into(),
            );
        };

        let localpart = if raw.is_empty() {
            Self::DEFAULT_BOTFATHER_LOCALPART
        } else {
            raw
        };
        let full_user_id = format!("@{localpart}:{}", current_user_id.server_name());
        UserId::parse(&full_user_id)
            .map(|user_id| user_id.to_owned())
            .map_err(|_| format!("Invalid bot user ID: {full_user_id}"))
    }

    /// Returns the BotFather MXID that should be used for a room action.
    ///
    /// If the room already has a persisted binding, that exact MXID wins.
    /// Otherwise, the current global configuration is resolved.
    pub fn resolved_bot_user_id_for_room(
        &self,
        room_id: &RoomId,
        current_user_id: Option<&UserId>,
    ) -> Result<OwnedUserId, String> {
        if let Some(bot_user_id) = self.bound_bot_user_id(room_id) {
            return Ok(bot_user_id.to_owned());
        }

        self.resolved_bot_user_id(current_user_id)
    }

    /// Returns `true` if the target user is a positively identified bot:
    /// the resolved BotFather MXID, a bot discovered via `/listbots`, or a
    /// bot MXID bound to some room.
    pub fn is_identified_bot(
        &self,
        target_user_id: &UserId,
        current_user_id: Option<&UserId>,
    ) -> bool {
        if self.enabled
            && self
                .resolved_bot_user_id(current_user_id)
                .is_ok_and(|bot_user_id| bot_user_id.as_str() == target_user_id.as_str())
        {
            return true;
        }
        if self
            .known_bot_user_ids
            .iter()
            .any(|bot_user_id| bot_user_id.as_str() == target_user_id.as_str())
        {
            return true;
        }
        self.room_bindings
            .iter()
            .any(|binding| binding.bot_user_id.as_str() == target_user_id.as_str())
    }

    /// Returns `true` if new DM rooms for this target user should be encrypted.
    ///
    /// Ordinary users always get an encrypted DM. Only DMs with a positively
    /// identified bot (see [`Self::is_identified_bot`]) are created unencrypted,
    /// because appservice bots typically cannot participate in E2EE rooms.
    /// If the BotFather MXID cannot be resolved, the target is treated as an
    /// ordinary user (encrypted).
    pub fn should_create_encrypted_dm(
        &self,
        target_user_id: &UserId,
        current_user_id: Option<&UserId>,
    ) -> bool {
        !self.is_identified_bot(target_user_id, current_user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restored_bindings_are_normalized_before_binary_search() {
        let mut settings = AgentAccessSettings::default();
        for id in ["!z:test", "!a:test", "!z:test"] {
            settings
                .bot_settings
                .room_bindings
                .push(RoomBotBindingState {
                    room_id: id.try_into().unwrap(),
                    bot_user_id: "@bot:test".try_into().unwrap(),
                    remark: String::new(),
                });
        }
        settings.normalize();
        assert_eq!(settings.bot_settings.room_bindings.len(), 2);
        assert!(
            settings
                .bot_settings
                .is_room_bound(matrix_sdk::ruma::room_id!("!a:test"))
        );
        let serialized = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            serde_json::from_str::<AgentAccessSettings>(&serialized).unwrap(),
            settings
        );
    }
    #[test]
    fn endpoint_checks_do_not_accept_embedded_credentials() {
        assert!(
            BotSettingsState::validate_octos_service_url("https://user:secret@service.test")
                .is_err()
        );
        assert!(BotSettingsState::validate_octos_service_url("file:///tmp/agent").is_err());
        assert!(BotSettingsState::validate_octos_service_url("http://127.0.0.1:8010").is_ok());
    }
}
