// Adapted from robrix2 da375f241d; protocol and adversarial tests retained.
//! Pure, account-scoped state for agent-chat approval projections.

use std::collections::{BTreeSet, HashMap, HashSet};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const KEY: &str = "com.agentchat.approval";
const REQUEST_TYPE: &str = "com.agentchat.approval.request.v1";
const STATUS_TYPE: &str = "com.agentchat.approval.status.v1";
const MARKER_TYPE: &str = "com.agentchat.approval.room.v1";
const MARKER_TYPE_V2: &str = "com.agentchat.approval.room.v2";
pub(crate) const MARKER_FRESHNESS_MS: u64 = 300_000;
const MAX_ACCOUNTS: usize = 16;
const MAX_APPROVALS: usize = 1024;
const MAX_MARKERS: usize = 256;
const MAX_V1_ASSOCIATIONS: usize = 128;
const MAX_V2_ASSOCIATIONS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CanonicalApprovalState {
    Pending,
    Approved,
    Denied,
    Expired,
    Consumed,
}

impl CanonicalApprovalState {
    fn terminal(self) -> bool {
        self != Self::Pending
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ApprovalDecision {
    Allow,
    Deny,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ApprovalAction {
    ApproveOnce,
    Deny,
}

impl ApprovalAction {
    fn name(self) -> &'static str {
        match self {
            Self::ApproveOnce => "approve_once",
            Self::Deny => "deny",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct ApprovalRequestKey {
    pub(crate) account_mxid: String,
    pub(crate) approval_room_id: String,
    pub(crate) request_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ApprovalClaimKey {
    pub(crate) account_mxid: String,
    pub(crate) approval_room_id: String,
    pub(crate) request_id: String,
    pub(crate) input_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ApprovalBinding {
    pub(crate) request_id: String,
    pub(crate) agent: String,
    pub(crate) project: String,
    pub(crate) project_room_id: String,
    pub(crate) input_digest: String,
    pub(crate) owner_mxid: String,
    pub(crate) publisher_mxid: String,
    pub(crate) approval_room_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ApprovalView {
    pub(crate) binding: ApprovalBinding,
    pub(crate) state: CanonicalApprovalState,
    pub(crate) decision: Option<ApprovalDecision>,
    pub(crate) latest_revision: u64,
    pub(crate) expires_at: Option<u64>,
    pub(crate) source_event_ids: Vec<String>,
    pub(crate) legacy_read_only: bool,
    pub(crate) legacy_original_event_id: Option<String>,
    pub(crate) conflicted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChangedApproval {
    pub(crate) key: ApprovalRequestKey,
    pub(crate) room_epoch: u64,
    pub(crate) source_event_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ApprovalModelError {
    InvalidContext,
    InvalidContent,
    BindingMismatch,
    RevisionConflict,
    InvalidTransition,
    CapacityExceeded,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum IngestResult {
    Changed(ChangedApproval),
    Duplicate,
    Rejected(ApprovalModelError),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ClaimGrant {
    pub(crate) transaction_id: String,
    pub(crate) action: ApprovalAction,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SendResult {
    ConfirmedPreSendFailure,
    OutcomeUnknown,
    Sent,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClaimDisposition {
    Released,
    Retained,
    Missing,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ClaimError {
    NotActionable,
    AlreadyClaimed,
    BindingMismatch,
    CapacityExceeded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClaimState {
    Claimed,
    OutcomeUnknown,
    Sent,
}
#[derive(Clone, Debug)]
struct Claim {
    state: ClaimState,
}
#[derive(Clone, Debug)]
struct Record {
    binding: ApprovalBinding,
    state: CanonicalApprovalState,
    decision: Option<ApprovalDecision>,
    revision: u64,
    expires_at: Option<u64>,
    sources: BTreeSet<String>,
    has_request: bool,
    legacy: bool,
    legacy_original_event_id: Option<String>,
    conflicted: bool,
}
impl Record {
    fn view(&self) -> ApprovalView {
        ApprovalView {
            binding: self.binding.clone(),
            state: self.state,
            decision: self.decision,
            latest_revision: self.revision,
            expires_at: self.expires_at,
            source_event_ids: self.sources.iter().cloned().collect(),
            legacy_read_only: self.legacy,
            legacy_original_event_id: self.legacy_original_event_id.clone(),
            conflicted: self.conflicted,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ApprovalSession {
    records: HashMap<ApprovalRequestKey, Record>,
    claims: HashMap<ApprovalClaimKey, Claim>,
    epochs: HashMap<(String, String), u64>,
}

#[derive(Clone, Debug)]
struct Parsed {
    binding: ApprovalBinding,
    state: CanonicalApprovalState,
    decision: Option<ApprovalDecision>,
    revision: u64,
    expires_at: Option<u64>,
    request: bool,
    legacy: bool,
    legacy_original_event_id: Option<String>,
}

impl ApprovalSession {
    pub(crate) fn ingest_message(
        &mut self,
        account: &str,
        room: &str,
        event: &str,
        sender: &str,
        content: &Value,
        _now: u64,
    ) -> IngestResult {
        if !mxid(account) || !room_id(room) || !event_id(event) || !mxid(sender) {
            return IngestResult::Rejected(ApprovalModelError::InvalidContext);
        }
        let parsed = match parse_projection(room, sender, content) {
            Ok(v) => v,
            Err(e) => return IngestResult::Rejected(e),
        };
        let key = ApprovalRequestKey {
            account_mxid: account.into(),
            approval_room_id: room.into(),
            request_id: parsed.binding.request_id.clone(),
        };
        if !self.records.contains_key(&key) {
            if (self.account_count() >= MAX_ACCOUNTS && !self.has_account(account))
                || self
                    .records
                    .keys()
                    .filter(|k| k.account_mxid == account)
                    .count()
                    >= MAX_APPROVALS
            {
                return IngestResult::Rejected(ApprovalModelError::CapacityExceeded);
            }
            let mut sources = BTreeSet::new();
            if parsed.request {
                sources.insert(event.into());
            }
            self.records.insert(
                key.clone(),
                Record {
                    binding: parsed.binding,
                    state: parsed.state,
                    decision: parsed.decision,
                    revision: parsed.revision,
                    expires_at: parsed.expires_at,
                    sources,
                    has_request: parsed.request,
                    legacy: parsed.legacy,
                    legacy_original_event_id: parsed.legacy_original_event_id,
                    conflicted: false,
                },
            );
            return IngestResult::Changed(self.changed(&key));
        }
        let binding_mismatch = self
            .records
            .get(&key)
            .is_some_and(|r| r.binding != parsed.binding);
        if binding_mismatch {
            if let Some(record) = self.records.get_mut(&key) {
                record.conflicted = true;
            }
            let _ = self.changed(&key);
            return IngestResult::Rejected(ApprovalModelError::BindingMismatch);
        }
        if parsed.request {
            let Some(record) = self.records.get_mut(&key) else {
                return IngestResult::Rejected(ApprovalModelError::InvalidTransition);
            };
            // A status-only legacy baseline never becomes a native request.
            if record.legacy {
                return IngestResult::Rejected(ApprovalModelError::InvalidTransition);
            }
            if record.has_request {
                if record.expires_at == parsed.expires_at {
                    return IngestResult::Duplicate;
                }
                record.conflicted = true;
                let _ = self.changed(&key);
                return IngestResult::Rejected(ApprovalModelError::RevisionConflict);
            }
            record.has_request = true;
            record.expires_at = parsed.expires_at;
            record.sources.insert(event.into());
            record.legacy |= parsed.legacy;
            return IngestResult::Changed(self.changed(&key));
        }
        let Some(record) = self.records.get(&key) else {
            return IngestResult::Rejected(ApprovalModelError::InvalidTransition);
        };
        if (record.legacy || parsed.legacy)
            && (record.legacy != parsed.legacy
                || record.legacy_original_event_id != parsed.legacy_original_event_id)
        {
            if let Some(record) = self.records.get_mut(&key) {
                record.conflicted = true;
            }
            let _ = self.changed(&key);
            return IngestResult::Rejected(ApprovalModelError::RevisionConflict);
        }
        let (old_rev, old_state, old_decision, old_legacy) = (
            record.revision,
            record.state,
            record.decision,
            record.legacy,
        );
        if parsed.revision < old_rev {
            return IngestResult::Duplicate;
        }
        if parsed.revision == old_rev {
            if old_state == parsed.state
                && old_decision == parsed.decision
                && old_legacy == parsed.legacy
            {
                return IngestResult::Duplicate;
            }
            if let Some(record) = self.records.get_mut(&key) {
                record.conflicted = true;
            }
            let _ = self.changed(&key);
            return IngestResult::Rejected(ApprovalModelError::RevisionConflict);
        }
        if !transition(old_state, old_decision, parsed.state, parsed.decision) {
            if let Some(record) = self.records.get_mut(&key) {
                record.conflicted = true;
            }
            let _ = self.changed(&key);
            return IngestResult::Rejected(ApprovalModelError::InvalidTransition);
        }
        if let Some(record) = self.records.get_mut(&key) {
            record.revision = parsed.revision;
            record.state = parsed.state;
            record.decision = parsed.decision;
            record.legacy |= parsed.legacy;
        }
        if parsed.state.terminal() {
            self.claims.retain(|c, _| {
                !(c.account_mxid == account
                    && c.approval_room_id == room
                    && c.request_id == key.request_id)
            });
        }
        IngestResult::Changed(self.changed(&key))
    }

    pub(crate) fn approval(
        &self,
        account: &str,
        room: &str,
        request: &str,
    ) -> Option<ApprovalView> {
        self.records
            .get(&ApprovalRequestKey {
                account_mxid: account.into(),
                approval_room_id: room.into(),
                request_id: request.into(),
            })
            .map(Record::view)
    }
    pub(crate) fn room_epoch(&self, account: &str, room: &str) -> u64 {
        self.epochs
            .get(&(account.into(), room.into()))
            .copied()
            .unwrap_or(0)
    }
    pub(crate) fn is_actionable(&self, account: &str, room: &str, request: &str, now: u64) -> bool {
        let key = ApprovalRequestKey {
            account_mxid: account.into(),
            approval_room_id: room.into(),
            request_id: request.into(),
        };
        let Some(r) = self.records.get(&key) else {
            return false;
        };
        r.has_request
            && !r.legacy
            && !r.conflicted
            && r.state == CanonicalApprovalState::Pending
            && r.binding.owner_mxid == account
            && r.expires_at.is_some_and(|e| now < e)
            && !self.claims.keys().any(|c| {
                c.account_mxid == account
                    && c.approval_room_id == room
                    && c.request_id == request
                    && c.input_digest == r.binding.input_digest
            })
    }
    pub(crate) fn try_claim(
        &mut self,
        key: ApprovalClaimKey,
        action: ApprovalAction,
        now: u64,
    ) -> Result<ClaimGrant, ClaimError> {
        let rk = ApprovalRequestKey {
            account_mxid: key.account_mxid.clone(),
            approval_room_id: key.approval_room_id.clone(),
            request_id: key.request_id.clone(),
        };
        let Some(r) = self.records.get(&rk) else {
            return Err(ClaimError::NotActionable);
        };
        if r.binding.input_digest != key.input_digest {
            return Err(ClaimError::BindingMismatch);
        }
        if !self.is_actionable(
            &key.account_mxid,
            &key.approval_room_id,
            &key.request_id,
            now,
        ) {
            return Err(if self.claims.contains_key(&key) {
                ClaimError::AlreadyClaimed
            } else {
                ClaimError::NotActionable
            });
        }
        if self.claims.len() >= MAX_ACCOUNTS * MAX_APPROVALS {
            return Err(ClaimError::CapacityExceeded);
        }
        let transaction_id = stable_txn(&key.request_id, action, &key.input_digest)
            .ok_or(ClaimError::BindingMismatch)?;
        self.claims.insert(
            key.clone(),
            Claim {
                state: ClaimState::Claimed,
            },
        );
        let _ = self.bump_room_epoch(&key.account_mxid, &key.approval_room_id);
        Ok(ClaimGrant {
            transaction_id,
            action,
        })
    }
    pub(crate) fn record_send_result(
        &mut self,
        key: &ApprovalClaimKey,
        result: SendResult,
        now: u64,
    ) -> ClaimDisposition {
        let Some(claim) = self.claims.get(key) else {
            return ClaimDisposition::Missing;
        };
        match result {
            SendResult::ConfirmedPreSendFailure => {
                let rk = ApprovalRequestKey {
                    account_mxid: key.account_mxid.clone(),
                    approval_room_id: key.approval_room_id.clone(),
                    request_id: key.request_id.clone(),
                };
                let release = claim.state == ClaimState::Claimed
                    && self.records.get(&rk).is_some_and(|r| {
                        r.state == CanonicalApprovalState::Pending
                            && !r.conflicted
                            && r.expires_at.is_some_and(|e| now < e)
                    });
                if release {
                    self.claims.remove(key);
                    let _ = self.bump_room_epoch(&key.account_mxid, &key.approval_room_id);
                    ClaimDisposition::Released
                } else {
                    ClaimDisposition::Retained
                }
            }
            SendResult::OutcomeUnknown => {
                if let Some(claim) = self.claims.get_mut(key) {
                    claim.state = ClaimState::OutcomeUnknown;
                }
                let _ = self.bump_room_epoch(&key.account_mxid, &key.approval_room_id);
                ClaimDisposition::Retained
            }
            SendResult::Sent => {
                if let Some(claim) = self.claims.get_mut(key) {
                    claim.state = ClaimState::Sent;
                }
                let _ = self.bump_room_epoch(&key.account_mxid, &key.approval_room_id);
                ClaimDisposition::Retained
            }
        }
    }
    pub(crate) fn claim_state(&self, key: &ApprovalClaimKey) -> Option<ClaimState> {
        self.claims.get(key).map(|c| c.state)
    }
    pub(crate) fn remove_room(&mut self, account: &str, room: &str) {
        self.records
            .retain(|k, _| k.account_mxid != account || k.approval_room_id != room);
        self.claims
            .retain(|k, _| k.account_mxid != account || k.approval_room_id != room);
        self.epochs.remove(&(account.into(), room.into()));
    }
    pub(crate) fn clear_account(&mut self, account: &str) {
        self.records.retain(|k, _| k.account_mxid != account);
        self.claims.retain(|k, _| k.account_mxid != account);
        self.epochs.retain(|(a, _), _| a != account);
    }
    fn changed(&mut self, key: &ApprovalRequestKey) -> ChangedApproval {
        let epoch = self
            .epochs
            .entry((key.account_mxid.clone(), key.approval_room_id.clone()))
            .or_default();
        *epoch = epoch.saturating_add(1);
        ChangedApproval {
            key: key.clone(),
            room_epoch: *epoch,
            source_event_ids: self
                .records
                .get(key)
                .map(|r| r.sources.iter().cloned().collect())
                .unwrap_or_default(),
        }
    }
    fn bump_room_epoch(&mut self, account: &str, room: &str) -> u64 {
        let epoch = self
            .epochs
            .entry((account.into(), room.into()))
            .or_default();
        *epoch = epoch.saturating_add(1);
        *epoch
    }
    fn has_account(&self, a: &str) -> bool {
        self.records.keys().any(|k| k.account_mxid == a)
    }
    fn account_count(&self) -> usize {
        self.records
            .keys()
            .map(|k| &k.account_mxid)
            .collect::<HashSet<_>>()
            .len()
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ApprovalMarkerIndex {
    version: u8,
    markers: Vec<Marker>,
    protocol_floors: Vec<ProtocolFloor>,
    #[serde(skip)]
    validated: HashSet<(String, String, u64)>,
}

impl Default for ApprovalMarkerIndex {
    fn default() -> Self {
        Self {
            version: 2,
            markers: Vec::new(),
            protocol_floors: Vec::new(),
            validated: HashSet::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
struct Association {
    agent: String,
    project_room_id: String,
    active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
struct Marker {
    account: String,
    room: String,
    event: String,
    publisher: String,
    owner: String,
    /// Present only in persisted cache format v1. New records keep this empty.
    agent: String,
    protocol: u8,
    generation: u64,
    associations: Vec<Association>,
    validated_at: u64,
    conflicted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
struct ProtocolFloor {
    account: String,
    room: String,
    minimum: u8,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ApprovalMarkerIndexWire {
    version: u8,
    markers: Vec<Marker>,
    protocol_floors: Vec<ProtocolFloor>,
}

impl<'de> Deserialize<'de> for ApprovalMarkerIndex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut wire = ApprovalMarkerIndexWire::deserialize(deserializer)?;
        match wire.version {
            1 => {
                for marker in &mut wire.markers {
                    if marker.agent.is_empty()
                        || marker.associations.len() > MAX_V1_ASSOCIATIONS
                        || marker.associations.iter().any(|association| {
                            !association.agent.is_empty() || !room_id(&association.project_room_id)
                        })
                    {
                        return Ok(Self::default());
                    }
                    for association in &mut marker.associations {
                        association.agent = marker.agent.clone();
                    }
                    marker.agent.clear();
                    marker.protocol = 1;
                }
                wire.protocol_floors = wire
                    .markers
                    .iter()
                    .map(|marker| ProtocolFloor {
                        account: marker.account.clone(),
                        room: marker.room.clone(),
                        minimum: 1,
                    })
                    .collect();
            }
            2 => {}
            _ => return Ok(Self::default()),
        }
        if wire.markers.len() > MAX_ACCOUNTS.saturating_mul(MAX_MARKERS)
            || wire.protocol_floors.len() > MAX_ACCOUNTS.saturating_mul(MAX_MARKERS)
        {
            return Ok(Self::default());
        }
        Ok(Self {
            version: 2,
            markers: wire.markers,
            protocol_floors: wire.protocol_floors,
            validated: HashSet::new(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MarkerResolution {
    Unique(String),
    Missing,
    Ambiguous,
    NotJoined,
    Stale,
}

impl ApprovalMarkerIndex {
    /// A refresh must authenticate every candidate again while retaining
    /// protocol and generation floors from earlier observations.
    pub(crate) fn begin_revalidation(&mut self) {
        self.validated.clear();
    }

    pub(crate) fn retain_joined(&mut self, account: &str, joined: &HashSet<String>) {
        let removed: Vec<_> = self
            .protocol_floors
            .iter()
            .filter(|floor| floor.account == account && !joined.contains(&floor.room))
            .map(|floor| floor.room.clone())
            .collect();
        for room in removed {
            self.remove_room(account, &room);
        }
    }

    pub(crate) fn observe_v2(
        &mut self,
        account: &str,
        room: &str,
    ) -> Result<(), ApprovalModelError> {
        if !mxid(account) || !room_id(room) {
            return Err(ApprovalModelError::InvalidContext);
        }
        self.mark_room_unavailable(account, room);
        if let Some(floor) = self
            .protocol_floors
            .iter_mut()
            .find(|floor| floor.account == account && floor.room == room)
        {
            floor.minimum = 2;
            return Ok(());
        }
        let account_rooms = self
            .protocol_floors
            .iter()
            .filter(|floor| floor.account == account)
            .count();
        let accounts = self
            .protocol_floors
            .iter()
            .map(|floor| &floor.account)
            .collect::<HashSet<_>>()
            .len();
        if account_rooms >= MAX_MARKERS
            || (accounts >= MAX_ACCOUNTS
                && !self
                    .protocol_floors
                    .iter()
                    .any(|floor| floor.account == account))
        {
            return Err(ApprovalModelError::CapacityExceeded);
        }
        self.protocol_floors.push(ProtocolFloor {
            account: account.into(),
            room: room.into(),
            minimum: 2,
        });
        Ok(())
    }

    fn protocol_floor(&self, account: &str, room: &str) -> u8 {
        self.protocol_floors
            .iter()
            .find(|floor| floor.account == account && floor.room == room)
            .map_or(1, |floor| floor.minimum)
    }

    pub(crate) fn ingest_marker(
        &mut self,
        account: &str,
        room: &str,
        event: &str,
        event_type: &str,
        sender: &str,
        state_key: &str,
        content: &Value,
        now: u64,
    ) -> Result<(), ApprovalModelError> {
        if !state_key.is_empty()
            || !mxid(account)
            || !room_id(room)
            || !event_id(event)
            || !mxid(sender)
        {
            return Err(ApprovalModelError::InvalidContext);
        }
        let detail = content.get(KEY).unwrap_or(content);
        let protocol = detail
            .get("version")
            .and_then(Value::as_u64)
            .and_then(|version| u8::try_from(version).ok())
            .ok_or(ApprovalModelError::InvalidContent)?;
        if !matches!(
            (event_type, protocol),
            (MARKER_TYPE, 1) | (MARKER_TYPE_V2, 2)
        ) {
            return Err(ApprovalModelError::InvalidContent);
        }
        if protocol == 2 {
            self.observe_v2(account, room)?;
        } else if self.protocol_floor(account, room) >= 2 {
            self.mark_room_unavailable(account, room);
            return Err(ApprovalModelError::RevisionConflict);
        }
        if content
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|found| found != event_type)
        {
            return Err(ApprovalModelError::InvalidContent);
        }
        let publisher = req(detail, "publisher_mxid")?;
        let owner = req(detail, "owner_mxid")?;
        let generation = detail
            .get("binding_generation")
            .and_then(Value::as_u64)
            .filter(|generation| *generation > 0)
            .ok_or(ApprovalModelError::InvalidContent)?;
        if !mxid(&publisher) || !mxid(&owner) || publisher != sender || owner != account {
            return Err(ApprovalModelError::InvalidContext);
        }
        let items = detail
            .get("project_room_associations")
            .and_then(Value::as_array)
            .ok_or(ApprovalModelError::InvalidContent)?;
        let association_limit = if protocol == 1 {
            MAX_V1_ASSOCIATIONS
        } else {
            MAX_V2_ASSOCIATIONS
        };
        if items.len() > association_limit || (protocol == 1 && items.is_empty()) {
            return Err(ApprovalModelError::CapacityExceeded);
        }
        let legacy_agent = if protocol == 1 {
            Some(req(detail, "agent")?)
        } else {
            if detail.get("agent").is_some() {
                return Err(ApprovalModelError::InvalidContent);
            }
            None
        };
        let mut seen = HashSet::new();
        let mut associations = Vec::with_capacity(items.len());
        for item in items {
            let agent = match &legacy_agent {
                Some(agent) => agent.clone(),
                None => req(item, "agent")?,
            };
            let project_room_id = req(item, "project_room_id")?;
            let active = item
                .get("active")
                .and_then(Value::as_bool)
                .ok_or(ApprovalModelError::InvalidContent)?;
            if agent.len() > 128
                || !room_id(&project_room_id)
                || !seen.insert((agent.clone(), project_room_id.clone()))
            {
                return Err(ApprovalModelError::InvalidContent);
            }
            associations.push(Association {
                agent,
                project_room_id,
                active,
            });
        }
        associations.sort_by(|left, right| {
            (&left.agent, &left.project_room_id).cmp(&(&right.agent, &right.project_room_id))
        });
        let candidate = Marker {
            account: account.into(),
            room: room.into(),
            event: event.into(),
            publisher,
            owner,
            agent: String::new(),
            protocol,
            generation,
            associations,
            validated_at: now,
            conflicted: false,
        };
        let validation = (account.into(), room.into(), generation);
        if let Some(old) = self
            .markers
            .iter_mut()
            .find(|marker| marker.account == account && marker.room == room)
        {
            if protocol < old.protocol || generation < old.generation {
                self.validated
                    .remove(&(account.into(), room.into(), old.generation));
                return Err(ApprovalModelError::RevisionConflict);
            }
            if generation == old.generation {
                let same = old.protocol == candidate.protocol
                    && old.publisher == candidate.publisher
                    && old.owner == candidate.owner
                    && old.associations == candidate.associations;
                if same {
                    old.validated_at = now;
                    old.event = event.into();
                    self.validated.insert(validation);
                    return Ok(());
                }
                old.conflicted = true;
                self.validated
                    .remove(&(account.into(), room.into(), generation));
                return Err(ApprovalModelError::RevisionConflict);
            }
            self.validated.retain(|(found_account, found_room, _)| {
                found_account != account || found_room != room
            });
            *old = candidate;
            self.validated.insert(validation);
            return Ok(());
        }
        let accounts = self
            .markers
            .iter()
            .map(|marker| &marker.account)
            .collect::<HashSet<_>>()
            .len();
        if self
            .markers
            .iter()
            .filter(|marker| marker.account == account)
            .count()
            >= MAX_MARKERS
            || (accounts >= MAX_ACCOUNTS
                && !self.markers.iter().any(|marker| marker.account == account))
        {
            return Err(ApprovalModelError::CapacityExceeded);
        }
        if !self
            .protocol_floors
            .iter()
            .any(|floor| floor.account == account && floor.room == room)
        {
            self.protocol_floors.push(ProtocolFloor {
                account: account.into(),
                room: room.into(),
                minimum: protocol,
            });
        }
        self.markers.push(candidate);
        self.validated.insert(validation);
        Ok(())
    }

    pub(crate) fn candidates(
        &self,
        account: &str,
        project: Option<&str>,
        joined: &HashSet<String>,
        now: u64,
    ) -> Vec<(String, String, String)> {
        let mut candidates = BTreeSet::new();
        for marker in &self.markers {
            for association in &marker.associations {
                if association.active && project.is_none_or(|p| p == association.project_room_id) {
                    if let MarkerResolution::Unique(room) = self.resolve(
                        account,
                        &association.agent,
                        &association.project_room_id,
                        joined,
                        now,
                    ) {
                        candidates.insert((
                            room,
                            association.agent.clone(),
                            association.project_room_id.clone(),
                        ));
                    }
                }
            }
        }
        candidates.into_iter().collect()
    }

    pub(crate) fn resolve(
        &self,
        account: &str,
        agent: &str,
        project: &str,
        joined: &HashSet<String>,
        now: u64,
    ) -> MarkerResolution {
        let candidates: Vec<_> = self
            .markers
            .iter()
            .filter(|marker| {
                marker.account == account
                    && marker.owner == account
                    && marker.associations.iter().any(|association| {
                        association.active
                            && association.agent == agent
                            && association.project_room_id == project
                    })
            })
            .collect();
        if candidates.is_empty() {
            return MarkerResolution::Missing;
        }
        if candidates.iter().any(|marker| marker.conflicted) {
            return MarkerResolution::Ambiguous;
        }
        if candidates.iter().any(|marker| {
            !self.validated.contains(&(
                marker.account.clone(),
                marker.room.clone(),
                marker.generation,
            )) || marker.validated_at == 0
                || now.saturating_sub(marker.validated_at) > MARKER_FRESHNESS_MS
        }) {
            return MarkerResolution::Stale;
        }
        if candidates.len() != 1 {
            return MarkerResolution::Ambiguous;
        }
        if !joined.contains(&candidates[0].room) {
            MarkerResolution::NotJoined
        } else {
            MarkerResolution::Unique(candidates[0].room.clone())
        }
    }

    pub(crate) fn mark_room_unavailable(&mut self, account: &str, room: &str) {
        self.validated.retain(|(found_account, found_room, _)| {
            found_account != account || found_room != room
        });
    }

    pub(crate) fn remove_room(&mut self, account: &str, room: &str) {
        self.markers
            .retain(|marker| marker.account != account || marker.room != room);
        self.protocol_floors
            .retain(|floor| floor.account != account || floor.room != room);
        self.mark_room_unavailable(account, room);
    }

    pub(crate) fn clear_account(&mut self, account: &str) {
        self.markers.retain(|marker| marker.account != account);
        self.protocol_floors
            .retain(|floor| floor.account != account);
        self.validated
            .retain(|(found_account, _, _)| found_account != account);
    }
}

fn parse_projection(room: &str, sender: &str, c: &Value) -> Result<Parsed, ApprovalModelError> {
    let typ = c
        .get("msgtype")
        .and_then(Value::as_str)
        .ok_or(ApprovalModelError::InvalidContent)?;
    let d = c.get(KEY).ok_or(ApprovalModelError::InvalidContent)?;
    if d.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(ApprovalModelError::InvalidContent);
    }
    let kind = if typ == REQUEST_TYPE {
        "request"
    } else if typ == STATUS_TYPE {
        "status"
    } else {
        return Err(ApprovalModelError::InvalidContent);
    };
    if d.get("kind").and_then(Value::as_str) != Some(kind) {
        return Err(ApprovalModelError::InvalidContent);
    }
    let request_id = req(d, "request_id")?;
    let agent = req(d, "agent")?;
    let project = req(d, "project")?;
    let project_room_id = req(d, "project_room_id")?;
    let input_digest = req(d, "input_digest")?;
    let owner_mxid = req(d, "owner_mxid")?;
    let publisher_mxid = req(d, "publisher_mxid")?;
    let revision = d
        .get("revision")
        .and_then(Value::as_u64)
        .filter(|v| *v > 0)
        .ok_or(ApprovalModelError::InvalidContent)?;
    if !request_id
        .strip_prefix("approval_")
        .is_some_and(|s| hex(s, 32))
        || !room_id(&project_room_id)
        || !hex(&input_digest, 64)
        || !mxid(&owner_mxid)
        || !mxid(&publisher_mxid)
        || publisher_mxid != sender
    {
        return Err(ApprovalModelError::InvalidContent);
    }
    let binding = ApprovalBinding {
        request_id,
        agent,
        project,
        project_room_id,
        input_digest,
        owner_mxid,
        publisher_mxid,
        approval_room_id: room.into(),
    };
    if typ == REQUEST_TYPE {
        let expires_at = d
            .get("expires_at")
            .and_then(Value::as_u64)
            .filter(|v| *v > 0)
            .ok_or(ApprovalModelError::InvalidContent)?;
        if revision != 1 || !actions(d.get("actions")) {
            return Err(ApprovalModelError::InvalidContent);
        }
        return Ok(Parsed {
            binding,
            state: CanonicalApprovalState::Pending,
            decision: None,
            revision,
            expires_at: Some(expires_at),
            request: true,
            legacy: false,
            legacy_original_event_id: None,
        });
    }
    if d.get("actions").is_some() {
        return Err(ApprovalModelError::InvalidContent);
    }
    let state = match d.get("state").and_then(Value::as_str) {
        Some("pending") => CanonicalApprovalState::Pending,
        Some("approved") => CanonicalApprovalState::Approved,
        Some("denied") => CanonicalApprovalState::Denied,
        Some("expired") => CanonicalApprovalState::Expired,
        Some("consumed") => CanonicalApprovalState::Consumed,
        _ => return Err(ApprovalModelError::InvalidContent),
    };
    let decision = match d.get("decision") {
        None | Some(Value::Null) => None,
        Some(Value::String(v)) if v == "allow" => Some(ApprovalDecision::Allow),
        Some(Value::String(v)) if v == "deny" => Some(ApprovalDecision::Deny),
        _ => return Err(ApprovalModelError::InvalidContent),
    };
    if !state_decision(state, decision) {
        return Err(ApprovalModelError::InvalidContent);
    }
    let legacy = d.get("migration_kind").and_then(Value::as_str) == Some("legacy_v1");
    let legacy_original_event_id = if legacy {
        if c.get("m.new_content").is_some() {
            return Err(ApprovalModelError::InvalidContent);
        }
        match c.get("m.relates_to") {
            None => None,
            Some(relation) => {
                if relation.get("rel_type").is_some() {
                    return Err(ApprovalModelError::InvalidContent);
                }
                let original = relation
                    .get("m.in_reply_to")
                    .and_then(|reply| reply.get("event_id"))
                    .and_then(Value::as_str)
                    .filter(|id| event_id(id))
                    .ok_or(ApprovalModelError::InvalidContent)?;
                Some(original.into())
            }
        }
    } else {
        None
    };
    Ok(Parsed {
        binding,
        state,
        decision,
        revision,
        expires_at: None,
        request: false,
        legacy,
        legacy_original_event_id,
    })
}
fn transition(
    a: CanonicalApprovalState,
    ad: Option<ApprovalDecision>,
    b: CanonicalApprovalState,
    bd: Option<ApprovalDecision>,
) -> bool {
    matches!(
        (a, b),
        (
            CanonicalApprovalState::Pending,
            CanonicalApprovalState::Approved
                | CanonicalApprovalState::Denied
                | CanonicalApprovalState::Expired
                | CanonicalApprovalState::Consumed
        )
    ) || matches!(
        (a, b),
        (
            CanonicalApprovalState::Approved
                | CanonicalApprovalState::Denied
                | CanonicalApprovalState::Expired,
            CanonicalApprovalState::Consumed
        )
    ) && ad == bd
}
fn state_decision(s: CanonicalApprovalState, d: Option<ApprovalDecision>) -> bool {
    match s {
        CanonicalApprovalState::Pending => d.is_none(),
        CanonicalApprovalState::Approved => d == Some(ApprovalDecision::Allow),
        CanonicalApprovalState::Denied | CanonicalApprovalState::Expired => {
            d == Some(ApprovalDecision::Deny)
        }
        CanonicalApprovalState::Consumed => d.is_some(),
    }
}
fn stable_txn(request: &str, action: ApprovalAction, digest: &str) -> Option<String> {
    let suffix = request.strip_prefix("approval_")?;
    if !hex(suffix, 32) || !hex(digest, 64) {
        return None;
    }
    let s = format!(
        "robrix.approval.{suffix}.{}.{}",
        action.name(),
        &digest[..16]
    );
    (s.len() <= 128).then_some(s)
}
fn actions(v: Option<&Value>) -> bool {
    let Some(a) = v.and_then(Value::as_array) else {
        return false;
    };
    let e = [("approve_once", "primary"), ("deny", "danger")];
    a.len() == 2
        && a.iter().zip(e).all(|(v, (id, style))| {
            v.get("id").and_then(Value::as_str) == Some(id)
                && v.get("style").and_then(Value::as_str) == Some(style)
                && v.get("label")
                    .and_then(Value::as_str)
                    .is_some_and(|s| !s.trim().is_empty())
        })
}
fn req(v: &Value, k: &str) -> Result<String, ApprovalModelError> {
    v.get(k)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .map(str::to_owned)
        .ok_or(ApprovalModelError::InvalidContent)
}
fn mxid(s: &str) -> bool {
    sigil(s, '@')
}
fn room_id(s: &str) -> bool {
    sigil(s, '!')
}
fn event_id(s: &str) -> bool {
    s.starts_with('$') && s.len() > 1 && s.len() <= 255 && !s.chars().any(char::is_whitespace)
}
fn sigil(s: &str, c: char) -> bool {
    s.starts_with(c)
        && s.len() <= 255
        && s[1..]
            .split_once(':')
            .is_some_and(|(a, b)| !a.is_empty() && !b.is_empty())
        && !s.chars().any(char::is_whitespace)
}
fn hex(s: &str, n: usize) -> bool {
    s.len() == n
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const A: &str = "@owner:test";
    const R: &str = "!approval:test";
    const P: &str = "@bridge:test";
    const Q: &str = "approval_0123456789abcdef0123456789abcdef";
    const D: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    fn detail() -> Value {
        json!({"version":1,"request_id":Q,"agent":"worker","project":"p","project_room_id":"!project:test","input_digest":D,"owner_mxid":A,"publisher_mxid":P})
    }
    fn request() -> Value {
        let mut d = detail();
        d["kind"] = json!("request");
        d["revision"] = json!(1);
        d["expires_at"] = json!(5000);
        d["actions"] = json!([{"id":"approve_once","label":"Approve once","style":"primary"},{"id":"deny","label":"Deny","style":"danger"}]);
        json!({"msgtype":REQUEST_TYPE,KEY:d})
    }
    fn status(rev: u64, state: &str, decision: Option<&str>) -> Value {
        let mut d = detail();
        d["kind"] = json!("status");
        d["revision"] = json!(rev);
        d["state"] = json!(state);
        d["decision"] = decision.map_or(Value::Null, |v| json!(v));
        json!({"msgtype":STATUS_TYPE,KEY:d})
    }
    fn put(s: &mut ApprovalSession, id: &str, v: &Value) -> IngestResult {
        s.ingest_message(A, R, id, P, v, 1000)
    }
    fn ck() -> ApprovalClaimKey {
        ApprovalClaimKey {
            account_mxid: A.into(),
            approval_room_id: R.into(),
            request_id: Q.into(),
            input_digest: D.into(),
        }
    }
    fn marker() -> Value {
        json!({"type":MARKER_TYPE,KEY:{"version":1,"binding_generation":1,"publisher_mxid":P,"owner_mxid":A,"agent":"worker","project_room_associations":[{"project_room_id":"!project:test","active":true}]}})
    }

    fn marker_v2() -> Value {
        json!({
            "version": 2,
            "binding_generation": 7,
            "publisher_mxid": P,
            "owner_mxid": A,
            "project_room_associations": [
                {"agent":"probe","project_room_id":"!project1:test","active":true},
                {"agent":"claude","project_room_id":"!project1:test","active":true},
                {"agent":"claude","project_room_id":"!project2:test","active":true},
                {"agent":"codex","project_room_id":"!project2:test","active":true}
            ]
        })
    }

    #[test]
    fn approval_state_accepts_exact_original_bindings_and_rejects_sender_owner_digest_and_action_mismatches()
     {
        let mut s = ApprovalSession::default();
        assert!(matches!(
            put(&mut s, "$r", &request()),
            IngestResult::Changed(_)
        ));
        assert!(s.is_actionable(A, R, Q, 1000));
        let mut bad = request();
        bad[KEY]["owner_mxid"] = json!("owner");
        assert!(matches!(put(&mut s, "$b", &bad), IngestResult::Rejected(_)));
        let mut bad = request();
        bad[KEY]["input_digest"] = json!("abc");
        assert!(matches!(put(&mut s, "$b", &bad), IngestResult::Rejected(_)));
        assert!(matches!(
            s.ingest_message(A, R, "$b", "@other:test", &request(), 1000),
            IngestResult::Rejected(_)
        ));
        assert!(!s.is_actionable("@viewer:test", R, Q, 1000));
    }
    #[test]
    fn approval_status_before_request_folds_to_the_highest_terminal_revision() {
        let mut s = ApprovalSession::default();
        put(&mut s, "$s", &status(3, "consumed", Some("allow")));
        put(&mut s, "$r", &request());
        put(&mut s, "$old", &status(2, "approved", Some("allow")));
        let v = s.approval(A, R, Q).unwrap();
        assert_eq!(v.state, CanonicalApprovalState::Consumed);
        assert_eq!(v.latest_revision, 3);
        assert!(!s.is_actionable(A, R, Q, 1000));
    }
    #[test]
    fn equal_revision_conflict_fails_closed() {
        let mut s = ApprovalSession::default();
        put(&mut s, "$r", &request());
        put(&mut s, "$a", &status(2, "approved", Some("allow")));
        assert!(matches!(
            put(&mut s, "$d", &status(2, "denied", Some("deny"))),
            IngestResult::Rejected(ApprovalModelError::RevisionConflict)
        ));
        assert!(s.approval(A, R, Q).unwrap().conflicted);
    }
    #[test]
    fn legacy_status_is_retained_read_only() {
        let mut v = status(1, "pending", None);
        v[KEY]["migration_kind"] = json!("legacy_v1");
        let mut s = ApprovalSession::default();
        put(&mut s, "$l", &v);
        assert!(s.approval(A, R, Q).unwrap().legacy_read_only);
        assert!(!s.is_actionable(A, R, Q, 1000));
    }
    #[test]
    fn legacy_approval_relation_conflict_is_not_a_duplicate() {
        let mut first = status(1, "consumed", Some("allow"));
        first[KEY]["migration_kind"] = json!("legacy_v1");
        first["m.relates_to"] = json!({"m.in_reply_to":{"event_id":"$original"}});
        let mut other = first.clone();
        other["m.relates_to"]["m.in_reply_to"]["event_id"] = json!("$different");
        let mut session = ApprovalSession::default();
        assert!(matches!(
            put(&mut session, "$first-status", &first),
            IngestResult::Changed(_)
        ));
        assert!(matches!(
            put(&mut session, "$other-status", &other),
            IngestResult::Rejected(ApprovalModelError::RevisionConflict)
        ));
        assert!(session.approval(A, R, Q).unwrap().conflicted);
    }
    #[test]
    fn legacy_approval_states_never_create_actionable_requests() {
        for (state, decision) in [
            ("pending", None),
            ("approved", Some("allow")),
            ("denied", Some("deny")),
            ("expired", Some("deny")),
            ("consumed", Some("allow")),
            ("consumed", Some("deny")),
        ] {
            let mut content = status(1, state, decision);
            content[KEY]["migration_kind"] = json!("legacy_v1");
            content["m.relates_to"] = json!({"m.in_reply_to":{"event_id":"$original"}});
            let mut session = ApprovalSession::default();
            assert!(matches!(
                put(&mut session, "$status", &content),
                IngestResult::Changed(_)
            ));
            let view = session.approval(A, R, Q).unwrap();
            assert!(view.legacy_read_only);
            assert_eq!(view.legacy_original_event_id.as_deref(), Some("$original"));
            assert!(!session.is_actionable(A, R, Q, 1000));
            assert!(
                session
                    .try_claim(ck(), ApprovalAction::ApproveOnce, 1000)
                    .is_err()
            );
            assert!(session.try_claim(ck(), ApprovalAction::Deny, 1000).is_err());
            assert!(matches!(
                put(&mut session, "$late-native", &request()),
                IngestResult::Rejected(_)
            ));
            assert!(session.records.values().all(|record| !record.has_request));
        }
    }
    #[test]
    fn legacy_approval_epoch_is_shared_by_two_pane_observers() {
        let mut session = ApprovalSession::default();
        let pane_one_epoch = session.room_epoch(A, R);
        let pane_two_epoch = session.room_epoch(A, R);
        let mut content = status(1, "consumed", Some("allow"));
        content[KEY]["migration_kind"] = json!("legacy_v1");
        content["m.relates_to"] = json!({"m.in_reply_to":{"event_id":"$original"}});
        put(&mut session, "$status", &content);
        let pane_one_view = session.approval(A, R, Q).unwrap();
        assert!(session.room_epoch(A, R) > pane_one_epoch);
        let pane_two_view = session.approval(A, R, Q).unwrap();
        assert!(session.room_epoch(A, R) > pane_two_epoch);
        assert_eq!(pane_one_view, pane_two_view);
        let after = session.room_epoch(A, R);
        assert_eq!(
            put(&mut session, "$duplicate", &content),
            IngestResult::Duplicate
        );
        assert_eq!(session.room_epoch(A, R), after);
    }
    #[test]
    fn approval_navigation_requires_unique_joined_loaded_room() {
        let mut i = ApprovalMarkerIndex::default();
        i.ingest_marker(A, R, "$m", MARKER_TYPE, P, "", &marker(), 1000)
            .unwrap();
        let joined = HashSet::from([R.into()]);
        assert_eq!(
            i.resolve(A, "worker", "!project:test", &joined, 1000),
            MarkerResolution::Unique(R.into())
        );
        assert_eq!(
            i.resolve(
                A,
                "worker",
                "!project:test",
                &joined,
                1000 + MARKER_FRESHNESS_MS + 1
            ),
            MarkerResolution::Stale
        );
        assert_eq!(
            i.resolve(A, "worker", "!project:test", &HashSet::new(), 1000),
            MarkerResolution::NotJoined
        );
    }
    #[test]
    fn persisted_marker_requires_revalidation() {
        let mut i = ApprovalMarkerIndex::default();
        i.ingest_marker(A, R, "$m", MARKER_TYPE, P, "", &marker(), 1000)
            .unwrap();
        let bytes = serde_json::to_vec(&i).unwrap();
        let restored: ApprovalMarkerIndex = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            restored.resolve(
                A,
                "worker",
                "!project:test",
                &HashSet::from([R.into()]),
                1000
            ),
            MarkerResolution::Stale
        );
    }

    #[test]
    fn approval_marker_v2_shared_room_resolves_all_four_bindings() {
        // Membership observation is deliberately absent from the marker wire
        // model. `None` cannot remove a canonical binding-derived tuple.
        let agent_joined_observation: Option<bool> = None;
        assert_eq!(agent_joined_observation, None);
        let mut index = ApprovalMarkerIndex::default();
        index
            .ingest_marker(A, R, "$v2", MARKER_TYPE_V2, P, "", &marker_v2(), 1000)
            .unwrap();
        let joined = HashSet::from([R.into()]);

        assert_eq!(
            index.resolve(A, "claude", "!project2:test", &joined, 1000),
            MarkerResolution::Unique(R.into()),
        );
        assert_eq!(
            index.resolve(A, "codex", "!project2:test", &joined, 1000),
            MarkerResolution::Unique(R.into()),
        );
        assert_eq!(
            index.resolve(A, "probe", "!project1:test", &joined, 1000),
            MarkerResolution::Unique(R.into()),
        );
    }

    #[test]
    fn approval_marker_v2_floor_blocks_v1_rollback_and_malformed_fallback() {
        let mut index = ApprovalMarkerIndex::default();
        index.observe_v2(A, R).unwrap();
        assert!(
            index
                .ingest_marker(A, R, "$v1", MARKER_TYPE, P, "", &marker(), 1000)
                .is_err()
        );
        assert_eq!(
            index.resolve(
                A,
                "worker",
                "!project:test",
                &HashSet::from([R.into()]),
                1000,
            ),
            MarkerResolution::Missing,
        );

        index
            .ingest_marker(A, R, "$v2", MARKER_TYPE_V2, P, "", &marker_v2(), 2000)
            .unwrap();
        index.begin_revalidation();
        assert_eq!(
            index.resolve(
                A,
                "claude",
                "!project2:test",
                &HashSet::from([R.into()]),
                2001
            ),
            MarkerResolution::Stale
        );
        let mut index: ApprovalMarkerIndex =
            serde_json::from_value(serde_json::to_value(index).unwrap()).unwrap();
        let mut malformed = marker_v2();
        malformed["binding_generation"] = json!(6);
        assert!(
            index
                .ingest_marker(A, R, "$old", MARKER_TYPE_V2, P, "", &malformed, 3000)
                .is_err()
        );
        assert_eq!(
            index.resolve(
                A,
                "claude",
                "!project2:test",
                &HashSet::from([R.into()]),
                3000,
            ),
            MarkerResolution::Stale,
        );
    }

    #[test]
    fn persisted_v1_marker_cache_migrates_to_agent_project_tuple_unvalidated() {
        let legacy = json!({
            "version": 1,
            "markers": [{
                "account": A,
                "room": R,
                "event": "$legacy",
                "publisher": P,
                "owner": A,
                "agent": "worker",
                "generation": 3,
                "associations": [{"project_room_id":"!project:test","active":true}],
                "validated_at": 1000,
                "conflicted": false
            }]
        });
        let index: ApprovalMarkerIndex = serde_json::from_value(legacy).unwrap();
        assert_eq!(
            index.resolve(
                A,
                "worker",
                "!project:test",
                &HashSet::from([R.into()]),
                1000,
            ),
            MarkerResolution::Stale,
        );
        let persisted = serde_json::to_value(index).unwrap();
        assert_eq!(persisted["version"], json!(2));
        assert_eq!(
            persisted["markers"][0]["associations"][0]["agent"],
            json!("worker")
        );
    }

    #[test]
    fn approval_marker_v2_rejects_duplicate_and_overflow_without_partial_routes() {
        let mut index = ApprovalMarkerIndex::default();
        let mut duplicate = marker_v2();
        duplicate["project_room_associations"] = json!([
            {"agent":"claude","project_room_id":"!project:test","active":true},
            {"agent":"claude","project_room_id":"!project:test","active":false}
        ]);
        assert!(
            index
                .ingest_marker(A, R, "$duplicate", MARKER_TYPE_V2, P, "", &duplicate, 1000)
                .is_err()
        );
        assert_eq!(
            index.resolve(
                A,
                "claude",
                "!project:test",
                &HashSet::from([R.into()]),
                1000
            ),
            MarkerResolution::Missing,
        );

        let mut overflow = marker_v2();
        overflow["project_room_associations"] = Value::Array(
            (0..=MAX_V2_ASSOCIATIONS)
                .map(|item| {
                    json!({
                        "agent": format!("agent{item}"),
                        "project_room_id": format!("!project{item}:test"),
                        "active": true
                    })
                })
                .collect(),
        );
        assert_eq!(
            index.ingest_marker(A, R, "$overflow", MARKER_TYPE_V2, P, "", &overflow, 2000),
            Err(ApprovalModelError::CapacityExceeded),
        );
        assert_eq!(
            index.resolve(
                A,
                "agent0",
                "!project0:test",
                &HashSet::from([R.into()]),
                2000
            ),
            MarkerResolution::Missing,
        );
    }

    #[test]
    fn unknown_persisted_marker_cache_version_fails_closed() {
        let restored: ApprovalMarkerIndex = serde_json::from_value(json!({
            "version": 99,
            "markers": [{
                "account": A,
                "room": R,
                "owner": A,
                "publisher": P,
                "generation": 1,
                "associations": [{
                    "agent": "worker",
                    "project_room_id": "!project:test",
                    "active": true
                }]
            }]
        }))
        .unwrap();
        assert_eq!(
            restored.resolve(
                A,
                "worker",
                "!project:test",
                &HashSet::from([R.into()]),
                1000
            ),
            MarkerResolution::Missing,
        );
    }

    #[test]
    fn lower_marker_generation_invalidates_cached_high_water_mapping() {
        let mut index = ApprovalMarkerIndex::default();
        let mut generation_two = marker();
        generation_two[KEY]["binding_generation"] = json!(2);
        index
            .ingest_marker(A, R, "$m2", MARKER_TYPE, P, "", &generation_two, 1000)
            .unwrap();

        let mut stale_generation = marker();
        stale_generation[KEY]["project_room_associations"][0]["active"] = json!(false);
        assert_eq!(
            index.ingest_marker(A, R, "$m1", MARKER_TYPE, P, "", &stale_generation, 2000),
            Err(ApprovalModelError::RevisionConflict),
        );
        assert_eq!(
            index.resolve(
                A,
                "worker",
                "!project:test",
                &HashSet::from([R.into()]),
                2000,
            ),
            MarkerResolution::Stale,
        );

        index
            .ingest_marker(
                A,
                R,
                "$m2-refresh",
                MARKER_TYPE,
                P,
                "",
                &generation_two,
                3000,
            )
            .unwrap();
        assert_eq!(
            index.resolve(
                A,
                "worker",
                "!project:test",
                &HashSet::from([R.into()]),
                3000,
            ),
            MarkerResolution::Unique(R.into()),
        );
    }
    #[test]
    fn approval_verdict_outcome_keeps_unknown_claim_locked() {
        let mut s = ApprovalSession::default();
        put(&mut s, "$r", &request());
        let k = ck();
        let before = s.room_epoch(A, R);
        let g = s
            .try_claim(k.clone(), ApprovalAction::ApproveOnce, 1000)
            .unwrap();
        assert!(g.transaction_id.len() <= 128);
        assert!(s.room_epoch(A, R) > before);
        assert_eq!(
            s.record_send_result(&k, SendResult::ConfirmedPreSendFailure, 1000),
            ClaimDisposition::Released
        );
        s.try_claim(k.clone(), ApprovalAction::ApproveOnce, 1000)
            .unwrap();
        assert_eq!(
            s.record_send_result(&k, SendResult::OutcomeUnknown, 1000),
            ClaimDisposition::Retained
        );
        assert_eq!(s.claim_state(&k), Some(ClaimState::OutcomeUnknown));
        assert_eq!(
            s.record_send_result(&k, SendResult::ConfirmedPreSendFailure, 1000),
            ClaimDisposition::Retained
        );
        assert_eq!(
            s.try_claim(k, ApprovalAction::ApproveOnce, 1000),
            Err(ClaimError::AlreadyClaimed)
        );
    }
    #[test]
    fn terminal_status_retires_claim_and_advances_room_epoch() {
        let mut s = ApprovalSession::default();
        put(&mut s, "$r", &request());
        let before = s.room_epoch(A, R);
        let k = ck();
        s.try_claim(k.clone(), ApprovalAction::Deny, 1000).unwrap();
        let changed = put(&mut s, "$s", &status(2, "denied", Some("deny")));
        assert!(matches!(changed, IngestResult::Changed(_)));
        assert!(s.room_epoch(A, R) > before);
        assert_eq!(s.claim_state(&k), None);
    }
    #[test]
    fn same_revision_duplicate_ignores_body_and_timestamp() {
        let mut s = ApprovalSession::default();
        put(&mut s, "$r", &request());
        let a = status(2, "approved", Some("allow"));
        put(&mut s, "$a", &a);
        let mut b = a.clone();
        b["body"] = json!("translated");
        b[KEY]["decided_at"] = json!(9999);
        assert_eq!(put(&mut s, "$b", &b), IngestResult::Duplicate);
    }
    #[test]
    fn lifecycle_helpers_are_account_scoped() {
        let mut s = ApprovalSession::default();
        put(&mut s, "$r", &request());
        assert!(s.approval(A, R, Q).is_some());
        let key = ck();
        s.try_claim(key.clone(), ApprovalAction::Deny, 1000)
            .unwrap();
        assert_eq!(
            s.record_send_result(&key, SendResult::Sent, 1000),
            ClaimDisposition::Retained
        );
        s.remove_room(A, R);
        assert!(s.approval(A, R, Q).is_none());
        put(&mut s, "$r2", &request());
        s.clear_account(A);
        assert!(s.approval(A, R, Q).is_none());
        let mut index = ApprovalMarkerIndex::default();
        index
            .ingest_marker(A, R, "$m", MARKER_TYPE, P, "", &marker(), 1000)
            .unwrap();
        index.mark_room_unavailable(A, R);
        assert_eq!(
            index.resolve(
                A,
                "worker",
                "!project:test",
                &HashSet::from([R.into()]),
                1000
            ),
            MarkerResolution::Stale
        );
        index.remove_room(A, R);
        assert_eq!(
            index.resolve(A, "worker", "!project:test", &HashSet::new(), 1000),
            MarkerResolution::Missing
        );
        index
            .ingest_marker(A, R, "$m2", MARKER_TYPE, P, "", &marker(), 1000)
            .unwrap();
        index.clear_account(A);
        assert_eq!(
            index.resolve(A, "worker", "!project:test", &HashSet::new(), 1000),
            MarkerResolution::Missing
        );
    }

    #[test]
    fn repeated_claims_and_unknown_delivery_remain_single_use() {
        let mut session = ApprovalSession::default();
        put(&mut session, "$r", &request());
        session
            .try_claim(ck(), ApprovalAction::ApproveOnce, 1000)
            .unwrap();
        session.record_send_result(&ck(), SendResult::OutcomeUnknown, 1000);
        for _ in 0..40 {
            assert_eq!(
                session.record_send_result(&ck(), SendResult::ConfirmedPreSendFailure, 1000),
                ClaimDisposition::Retained
            );
            assert_eq!(
                session.try_claim(ck(), ApprovalAction::ApproveOnce, 1000),
                Err(ClaimError::AlreadyClaimed)
            );
        }
    }
}
