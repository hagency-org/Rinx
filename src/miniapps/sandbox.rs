//! Rinx's mini-app sandbox: the Splash isolate settings Rinx builds from the
//! app contract's [`AppPolicy`] (OctoSense ADR 0005).
//!
//! The contract fixes what an app may do; how a host sandboxes it is the
//! host's own, under one rule: **a host may restrict more than `AppPolicy`
//! says, never less.** This is the logic App Hub's `IsolateSettings` and
//! `splash_adapter::apply` gave Rinx 1.0.x (App Hub `0f332112`), ported
//! unchanged, so a mini app gets exactly the sandbox it had:
//!
//! - capabilities, hosts, storage quota, instruction budget and heap are the
//!   policy's, never more;
//! - the isolate's own network module is granted only with `net` and at
//!   least one host;
//! - the prompt right is the policy's `may_prompt` (the `prompt`
//!   capability), not "always" as a foreground App Hub surface now grants;
//! - the jail is `<root>/<app id>`, under a root Rinx chooses.
//!
//! The one addition is Rinx's own, as in 1.0.x: [`IsolateSettings::serve_bundle`]
//! lets the app reach the loopback server that serves its own frozen bundle
//! (images, kit files), and nothing else.
use makepad_widgets::{Cx, SplashRef};
use octosense_app_contract::AppPolicy;
use std::path::{Path, PathBuf};

/// What Rinx applies to a mini app's Splash isolate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IsolateSettings {
    /// Reported by `host.capabilities()` inside the isolate and checked by
    /// every host service before it acts.
    pub capabilities: Vec<String>,
    /// Whether the isolate's own network module is handed over.
    pub allow_net: bool,
    /// Whole-jail ceiling in bytes.
    pub storage_quota: u64,
    /// May this isolate raise a prompt.
    pub host_prompts: bool,
    /// Cumulative script instructions for the app's session.
    pub instruction_budget: u64,
    /// Heap ceiling for the isolate.
    pub memory_bytes: u64,
    /// The jail's directory.
    pub jail_root: PathBuf,
    /// Exactly the hosts the isolate may reach, on every network path.
    pub hosts: Vec<String>,
}

impl IsolateSettings {
    /// Exactly what `policy` grants, in the isolate's knobs, with the jail
    /// at `<app_data_root>/<app id>`.
    pub fn for_app(policy: &AppPolicy, app_data_root: &Path) -> Self {
        IsolateSettings {
            capabilities: policy.capabilities.iter().cloned().collect(),
            // A granted `net` with no hosts reaches nothing, so the module is
            // not handed over at all: less surface, same behaviour.
            allow_net: policy.allows("net") && !policy.hosts.is_empty(),
            storage_quota: policy.storage_bytes,
            host_prompts: policy.may_prompt,
            instruction_budget: policy.instruction_budget,
            memory_bytes: policy.memory_bytes,
            jail_root: app_data_root.join(&policy.app_id),
            hosts: policy.hosts.iter().cloned().collect(),
        }
    }

    /// Let the app load its own bundle's files from Rinx's loopback asset
    /// server (`allowlist_entry`, from `AssetServer::allowlist_entry`), the
    /// only way a bundle's images and kit reach the isolate. An L0 card also
    /// gets `rinx.event`, the channel its bindings call back through. As in
    /// Rinx 1.0.x; the server serves only the app's frozen snapshot.
    pub fn serve_bundle(&mut self, allowlist_entry: String, l0_card: bool) {
        self.hosts.push(allowlist_entry);
        self.allow_net = true;
        if l0_card {
            self.capabilities.push("rinx.event".into());
        }
        if !self.capabilities.iter().any(|s| s == "net") {
            self.capabilities.push("net".into());
        }
    }
}

/// Seat `settings` on `splash` before its body is evaluated.
///
/// Order matters: the jail and the quota are set before anything the app
/// runs can write; the policy (capabilities, hosts, budget) and the heap
/// ceiling come next; the isolate's own network module is granted last, so
/// a failure earlier leaves an isolate with less reach rather than more.
pub fn apply(splash: &SplashRef, cx: &mut Cx, settings: &IsolateSettings) {
    splash.set_sandbox_dir(cx, Some(settings.jail_root.clone()));
    splash.set_storage_quota(cx, Some(settings.storage_quota));
    splash.set_host_caps(cx, settings.capabilities.clone());
    splash.set_host_prompts(cx, settings.host_prompts);
    // `Some(hosts)` turns enforcement on for this isolate; an empty list
    // under a granted `net` reaches nothing.
    splash.set_policy(cx, Some(settings.hosts.clone()), Some(settings.instruction_budget));
    splash.set_memory_bytes(cx, Some(settings.memory_bytes as usize));
    if let Some(mut inner) = splash.borrow_mut() {
        inner.set_allow_net(settings.allow_net);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use octosense_app_contract::{HostLimits, parse, policy};

    fn policy(capabilities: &[&str], hosts: &[&str], compute: serde_json::Value) -> AppPolicy {
        let manifest = parse(
            &serde_json::json!({
                "schema": 1, "id": "dev.example.counter", "version": "1", "name": "Counter",
                "integrity": {"bundle_blake3": ""}, "capabilities": capabilities,
                "network": {"hosts": hosts}, "storage": {"max_bytes": 1u64 << 40},
                "compute": compute,
            })
            .to_string(),
        )
        .unwrap();
        policy::resolve(&manifest, &HostLimits::default().with_require_signature(false)).unwrap()
    }

    /// Never wider than the policy, field by field.
    fn assert_within(settings: &IsolateSettings, policy: &AppPolicy) {
        assert!(settings.capabilities.iter().all(|c| policy.allows(c)), "{settings:?}");
        assert!(settings.hosts.iter().all(|h| policy.allows_host(h)), "{settings:?}");
        assert!(!settings.allow_net || (policy.allows("net") && !policy.hosts.is_empty()));
        assert!(!settings.host_prompts || policy.may_prompt);
        assert!(settings.storage_quota <= policy.storage_bytes);
        assert!(settings.instruction_budget <= policy.instruction_budget);
        assert!(settings.memory_bytes <= policy.memory_bytes);
    }

    #[test]
    fn settings_are_exactly_the_policy() {
        let root = Path::new("/rinx/miniapps/acct");
        let cases = [
            policy(&[], &[], serde_json::json!({})),
            policy(&["storage"], &[], serde_json::json!({})),
            policy(&["net"], &[], serde_json::json!({})),
            policy(&["net", "storage", "prompt"], &["api.example.com"], serde_json::json!({})),
            policy(&["matrix.send_message"], &[], serde_json::json!({"instruction_budget": 10, "memory_bytes": 4096})),
        ];
        for policy in &cases {
            let settings = IsolateSettings::for_app(policy, root);
            assert_within(&settings, policy);
            assert_eq!(settings.capabilities, policy.capabilities.iter().cloned().collect::<Vec<_>>());
            assert_eq!(settings.hosts, policy.hosts.iter().cloned().collect::<Vec<_>>());
            assert_eq!(settings.storage_quota, policy.storage_bytes);
            assert_eq!(settings.instruction_budget, policy.instruction_budget);
            assert_eq!(settings.memory_bytes, policy.memory_bytes);
            assert_eq!(settings.host_prompts, policy.may_prompt);
            assert_eq!(settings.jail_root, root.join("dev.example.counter"));
        }
    }

    #[test]
    fn host_ceilings_clamp_what_the_app_asks() {
        let limits = HostLimits::default();
        let policy = policy(&[], &[], serde_json::json!({"instruction_budget": u64::MAX, "memory_bytes": u64::MAX}));
        let settings = IsolateSettings::for_app(&policy, Path::new("/r"));
        assert_eq!(settings.storage_quota, limits.max_storage_bytes);
        assert_eq!(settings.instruction_budget, limits.max_instruction_budget);
        assert_eq!(settings.memory_bytes, limits.max_memory_bytes);
    }

    #[test]
    fn net_without_hosts_and_no_prompt_capability_grant_nothing() {
        let settings = IsolateSettings::for_app(&policy(&["net"], &[], serde_json::json!({})), Path::new("/r"));
        assert!(!settings.allow_net);
        assert!(settings.hosts.is_empty());
        assert!(!settings.host_prompts);
    }

    #[test]
    fn serving_the_bundle_adds_only_its_loopback_origin() {
        let policy = policy(&["storage"], &[], serde_json::json!({}));
        let mut settings = IsolateSettings::for_app(&policy, Path::new("/r"));
        let before = settings.clone();
        settings.serve_bundle("127.0.0.1:4321".into(), false);
        assert_eq!(settings.hosts, ["127.0.0.1:4321"]);
        assert_eq!(settings.capabilities, ["storage", "net"]);
        assert_eq!(
            (settings.storage_quota, settings.instruction_budget, settings.memory_bytes, settings.host_prompts, settings.jail_root.clone()),
            (before.storage_quota, before.instruction_budget, before.memory_bytes, before.host_prompts, before.jail_root.clone()),
        );
        let mut card = before.clone();
        card.serve_bundle("127.0.0.1:4321".into(), true);
        assert_eq!(card.capabilities, ["storage", "rinx.event", "net"]);
    }
}
