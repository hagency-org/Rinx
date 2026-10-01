//! Frozen canonical OctoSense bundles. Catalog and unsigned developer imports
//! have separate admission paths; both verify the bytes actually executed.
use octosense_app_contract::{AppManifest, AppPolicy};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Call {
    pub service: String,
    #[serde(default = "object")]
    pub args: Value,
    pub target: String,
}
fn object() -> Value {
    serde_json::json!({})
}
#[derive(Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bindings {
    #[serde(default)]
    pub on_open: Vec<Call>,
    #[serde(default)]
    pub events: BTreeMap<String, Call>,
}
pub struct Package {
    pub root: PathBuf,
    pub manifest: AppManifest,
    pub policy: AppPolicy,
    pub source: String,
    pub script: bool,
    pub data: Value,
    pub bindings: Bindings,
    original: PathBuf,
    verified: Option<rinx_miniapp_catalog::VerifiedBundle>,
    _snapshot: Snapshot,
    builtin: Option<&'static crate::system_apps::PackedApp>,
}
struct Snapshot(PathBuf);
impl Drop for Snapshot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
impl Package {
    #[cfg(test)]
    pub fn load(root: &Path) -> Result<Self, String> {
        Self::load_in(root, &std::env::temp_dir())
    }
    pub fn load_in(root: &Path, snapshots: &Path) -> Result<Self, String> {
        Self::load_inner(root, snapshots, None, None)
    }
    pub fn load_verified(bundle: rinx_miniapp_catalog::VerifiedBundle, snapshots: &Path) -> Result<Self, String> {
        Self::load_inner(&bundle.root.clone(), snapshots, None, Some(bundle))
    }
    pub fn load_builtin(id: &str, snapshots: &Path) -> Result<Self, String> {
        let app = crate::system_apps::get(id).ok_or("Unknown built-in app")?;
        if app.native.is_some() { return Err("Native apps use the host registry".into()); }
        std::fs::create_dir_all(snapshots).map_err(|e| e.to_string())?;
        let source = snapshots.join(format!("builtin-{}", uuid::Uuid::new_v4()));
        app.materialize(&source)?;
        let _cleanup = Snapshot(source.clone());
        Self::load_inner(&source, snapshots, Some(app), None)
    }
    fn load_inner(root: &Path, snapshots: &Path, builtin: Option<&'static crate::system_apps::PackedApp>, verified: Option<rinx_miniapp_catalog::VerifiedBundle>) -> Result<Self, String> {
        std::fs::create_dir_all(snapshots).map_err(|e| e.to_string())?;
        // Freeze the admitted bytes. Source edits cannot change a running app's
        // kit, images or bindings after the user reviews its grants.
        let original = root.to_owned();
        let path = snapshots.join(format!("rinx-miniapp-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).map_err(|e| e.to_string())?;
        // Own cleanup only after creating the directory: a failed admission
        // must never remove another import's snapshot on a name collision.
        let snapshot = Snapshot(path);
        copy_bundle(root, &snapshot.0, &mut 0, &mut 0, 0)?;
        let root = &snapshot.0;
        let manifest = AppManifest::parse(
            &std::fs::read_to_string(root.join("manifest.json")).map_err(|e| e.to_string())?,
        )?;
        if let Some(app) = builtin {
            app.verify(root)?;
        } else if crate::system_apps::is_reserved(&manifest.id) {
            return Err("This app id belongs to Rinx's built-in catalog".into());
        }
        if manifest.agent.is_some() {
            return Err("Bundle agent profiles are not supported here; declare explicit octos.* services instead".into());
        }
        if let Some(bundle) = &verified {
            bundle.verify(root)?;
        } else {
            octosense_app_contract::admit_digest(
                &manifest,
                &octosense_app_contract::digest_dir(root)?,
                &octosense_app_contract::RefuseAllSignatures,
            )?;
        }
        let policy = octosense_app_contract::policy::resolve(
            &manifest,
            &octosense_app_contract::HostLimits::default().with_require_signature(false),
        )?;
        let script = root.join(octosense_app_contract::SCRIPT_ENTRY).is_file();
        let entry = if script {
            octosense_app_contract::SCRIPT_ENTRY
        } else {
            "page.card"
        };
        let source =
            std::fs::read_to_string(root.join(entry)).map_err(|e| format!("{entry}: {e}"))?;
        let data = read_json(root, "page.data.json")?.unwrap_or_else(object);
        if !data.is_object() {
            return Err("page.data.json must be an object".into());
        }
        let bindings: Bindings =
            serde_json::from_value(read_json(root, "bindings.json")?.unwrap_or_else(object))
                .map_err(|e| e.to_string())?;
        if script && (!bindings.on_open.is_empty() || !bindings.events.is_empty()) {
            return Err(
                "main.splash apps call host.request directly; bindings.json is for L0 cards".into(),
            );
        }
        if bindings.on_open.len() > 8 || bindings.events.len() > 64 {
            return Err("Too many service bindings".into());
        }
        for call in bindings.on_open.iter().chain(bindings.events.values()) {
            if !manifest.capabilities.contains(&call.service) {
                return Err(format!("{} is not declared in capabilities", call.service));
            }
            if call.target.is_empty()
                || !call
                    .target
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            {
                return Err("Binding target must be a top-level data name".into());
            }
            if !call.args.is_object() {
                return Err("Binding arguments must be an object".into());
            }
        }
        Ok(Self {
            root: root.to_owned(),
            manifest,
            policy,
            source,
            script,
            data,
            bindings,
            original,
            verified,
            _snapshot: snapshot,
            builtin,
        })
    }
    pub fn unchanged(&self) -> Result<(), String> {
        if let Some(app) = self.builtin { return app.verify(&self.root); }
        let current = Self::load_inner(
            &self.original,
            self.root.parent().ok_or("Missing bundle cache")?,
            None,
            self.verified.clone(),
        )?;
        if current.manifest.signing_bytes()? != self.manifest.signing_bytes()? {
            return Err("Package changed; review it again".into());
        }
        Ok(())
    }
    pub fn builtin_id(&self) -> Option<&'static str> {
        self.builtin.map(|app| app.manifest.id.as_str())
    }
}
fn read_json(root: &Path, name: &str) -> Result<Option<Value>, String> {
    match std::fs::read_to_string(root.join(name)) {
        Ok(s) => serde_json::from_str(&s)
            .map(Some)
            .map_err(|e| format!("{name}: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
fn copy_bundle(
    source: &Path,
    destination: &Path,
    total: &mut u64,
    count: &mut usize,
    depth: usize,
) -> Result<(), String> {
    if depth > 32 {
        return Err("Bundle exceeds 32 directory levels".into());
    }
    *count += 1;
    if *count > 4096 {
        return Err("Bundle exceeds 4096 entries".into());
    }
    let metadata = std::fs::symlink_metadata(source).map_err(|e| e.to_string())?;
    if metadata.file_type().is_symlink() {
        return Err("Bundle symlinks are not allowed".into());
    }
    if metadata.is_dir() {
        if depth != 0 {
            std::fs::create_dir(destination).map_err(|e| e.to_string())?;
        }
        for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            copy_bundle(
                &entry.path(),
                &destination.join(entry.file_name()),
                total,
                count,
                depth + 1,
            )?;
        }
    } else if metadata.is_file() {
        use std::io::Read;
        // Bound the read itself, even if the source grows while importing.
        let mut bytes = Vec::new();
        std::fs::File::open(source)
            .map_err(|e| e.to_string())?
            .take(32 * 1024 * 1024 + 1 - *total)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        *total += bytes.len() as u64;
        if *total > 32 * 1024 * 1024 {
            return Err("Bundle exceeds 32 MiB".into());
        }
        std::fs::write(destination, bytes).map_err(|e| e.to_string())?;
    } else {
        return Err("Bundle contains a special file".into());
    }
    Ok(())
}
pub fn arguments(
    template: &Value,
    data: &Value,
    state: &octoscript_ui_l0::InstanceStore,
    key: &str,
    payload: &Value,
) -> Result<Value, String> {
    Ok(match template {
        Value::Object(o) if o.len() == 1 && o.contains_key("$data") => data
            .pointer(o["$data"].as_str().ok_or("Invalid data pointer")?)
            .cloned()
            .ok_or("Missing binding data")?,
        Value::Object(o) if o.len() == 1 && o.contains_key("$state") => state
            .get(key, o["$state"].as_str().ok_or("Invalid state field")?)
            .or_else(|| {
                state.get(
                    octoscript_ui_l0::CARD_STATE_KEY,
                    o["$state"].as_str().unwrap(),
                )
            })
            .cloned()
            .ok_or("Missing binding state")?,
        Value::Object(o) if o.len() == 1 && o.get("$value") == Some(&Value::Bool(true)) => {
            payload.clone()
        }
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, v)| Ok((k.clone(), arguments(v, data, state, key, payload)?)))
                .collect::<Result<_, String>>()?,
        ),
        Value::Array(a) => Value::Array(
            a.iter()
                .map(|v| arguments(v, data, state, key, payload))
                .collect::<Result<_, _>>()?,
        ),
        value => value.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_hub_bundle_launches_from_a_verified_snapshot_only() {
        use rinx_miniapp_catalog::{VerifiedBundle, hub};
        let original =
            std::env::temp_dir().join(format!("rinx-signed-runtime-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&original).unwrap();
        let _cleanup = Snapshot(original.clone());
        std::fs::write(original.join("main.splash"), "Label{text: \"Signed app\"}").unwrap();
        let mut manifest = AppManifest::parse(r#"{"schema":1,"id":"signed-demo","name":"Signed demo","version":"1","integrity":{"bundle_blake3":""}}"#).unwrap();
        std::fs::write(
            original.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        manifest.integrity.bundle_blake3 = octosense_app_contract::digest_dir(&original).unwrap();
        let publisher = hub::HubKey::generate();
        hub::sign_manifest(&publisher, &mut manifest, "demo-publisher").unwrap();
        std::fs::write(
            original.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert!(
            Package::load(&original).is_err(),
            "Developer import must not confer catalog trust"
        );
        let verified = VerifiedBundle {
            root: original.clone(),
            manifest,
            publisher: "demo-publisher".into(),
            publisher_key: publisher.public_hex(),
        };
        let package = Package::load_verified(verified, &std::env::temp_dir()).unwrap();
        assert!(package.script);
        assert_ne!(package.root, original);
        package.unchanged().unwrap();
        std::fs::write(original.join("main.splash"), "modified after admission").unwrap();
        assert!(package.unchanged().is_err());
        assert!(
            std::fs::read_to_string(package.root.join("main.splash"))
                .unwrap()
                .contains("Signed app")
        );
    }

    #[test]
    fn built_in_script_owns_its_snapshot_and_rechecks_content_and_manifest() {
        let root = std::env::temp_dir().join(format!("rinx-builtin-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let _cleanup = Snapshot(root.clone());
        let bundle = root.join("apps/demo/bundle");
        std::fs::create_dir_all(&bundle).unwrap();
        std::fs::write(root.join("system-apps.json"), serde_json::json!({
            "schema": 1, "source": "apps", "repository": "https://github.com/hagency-org/Rinx",
            "apps": [{"directory": "demo"}]
        }).to_string()).unwrap();
        std::fs::write(bundle.join("manifest.json"), serde_json::json!({
            "schema": 1, "id": "test.embedded", "name": "Embedded", "version": "1.0.0",
            "integrity": {"bundle_blake3": ""}, "capabilities": ["matrix.profile"]
        }).to_string()).unwrap();
        let script = "Label {text: \"Embedded\"}";
        std::fs::write(bundle.join("main.splash"), script).unwrap();
        let app = Box::leak(Box::new(rinx_system_apps::pack(&root).unwrap().apps.remove(0)));
        let extracted = root.join("extracted");
        app.materialize(&extracted).unwrap();
        let package = Package::load_inner(&extracted, &root.join("snapshots"), Some(app), None).unwrap();
        // The extraction is disposable; review/run owns the frozen snapshot.
        std::fs::remove_dir_all(&extracted).unwrap();
        assert_eq!(package.builtin_id(), Some("test.embedded"));
        assert!(package.script);
        package.unchanged().unwrap();
        std::fs::write(package.root.join("main.splash"), "modified").unwrap();
        assert!(package.unchanged().is_err());
        std::fs::write(package.root.join("main.splash"), script).unwrap();
        package.unchanged().unwrap();
        let manifest = std::fs::read(package.root.join("manifest.json")).unwrap();
        let mut changed: Value = serde_json::from_slice(&manifest).unwrap();
        changed["capabilities"] = serde_json::json!(["matrix.send_message"]);
        std::fs::write(package.root.join("manifest.json"), changed.to_string()).unwrap();
        assert!(package.unchanged().is_err());
    }

    #[test]
    fn local_import_cannot_impersonate_the_built_in_native_editor() {
        let root = std::env::temp_dir().join(format!("rinx-reserved-id-{}", uuid::Uuid::new_v4()));
        crate::system_apps::get(crate::system_apps::ARTICLE_ID).unwrap().materialize(&root).unwrap();
        let result = Package::load(&root);
        assert!(matches!(result, Err(e) if e.contains("built-in catalog")));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn script_bundle_uses_the_shared_entry_and_frozen_assets() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/miniapps/matrix-octos-script");
        let package = Package::load(&path).unwrap();
        assert!(package.script);
        assert!(package.source.contains("host.request(\"matrix.profile\""));
        assert!(package.bindings.events.is_empty());
        std::fs::write(
            package.root.join("main.splash"),
            "Image{src: http_resource(\"{{assets}}/icon.png\")}",
        )
        .unwrap();
        let rendered = octosense_app_contract::script_source(&package.root, "http://127.0.0.1:1234/")
            .unwrap()
            .unwrap();
        assert_eq!(
            rendered,
            "Image{src: http_resource(\"http://127.0.0.1:1234/icon.png\")}"
        );
        assert!(Package::load(&package.root).is_err());
    }
    #[test]
    fn service_results_realize_as_declared_record_state() {
        let source = r#"state result {shape: record}
view root Surface { TextBody(text: result.data.display_name) }
"#;
        let report = octoscript_ui_l0::realize(
            source,
            &serde_json::json!({"result":{"data":{"display_name":"Alice"}}}),
            Default::default(),
        );
        let root = report.complete_root().unwrap();
        assert!(format!("{root:?}").contains("Alice"));
    }
    #[test]
    fn sample_bundle_uses_the_shared_renderer_and_updates_state() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/miniapps/matrix-octos");
        let package = Package::load(&path).unwrap();
        let mut state = octoscript_ui_l0::InstanceStore::default();
        let prepared = octoscript_makepad::l0::prepare_with_state(
            &package.source,
            &package.data,
            &state,
            &path.join("kit"),
        )
        .unwrap();
        let ui = octoscript_makepad::to_makepad_l0_ui(&prepared.tree);
        assert!(ui.contains("Matrix + Octos"));
        assert!(ui.contains("on_change"));
        assert!(octoscript_ui_l0::dispatch_with_data(
            &package.source,
            &mut state,
            octoscript_ui_l0::CARD_STATE_KEY,
            "typing",
            Some(&Value::String("hello".into())),
            &package.data
        ));
        let call = &package.bindings.events["ask"];
        assert_eq!(
            arguments(
                &call.args,
                &package.data,
                &state,
                octoscript_ui_l0::CARD_STATE_KEY,
                &Value::Null
            )
            .unwrap()["text"],
            "hello"
        );
    }
    #[test]
    fn imported_bytes_are_frozen_and_admission_rejects_tampering() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/miniapps/matrix-octos");
        let imported = Package::load(&source).unwrap();
        // An independently admitted copy owns its bytes even if its source
        // folder is edited after review.
        let frozen = Package::load(&imported.root).unwrap();
        std::fs::write(imported.root.join("page.card"), "changed after review").unwrap();
        assert!(frozen.source.contains("Matrix + Octos"));
        assert!(frozen.unchanged().is_err());
        assert!(Package::load(&imported.root).is_err());
    }

    #[test]
    fn binding_arguments_are_data_not_identity() {
        let template = serde_json::json!({"body":{"$value":true},"room_id":{"$data":"/room"}});
        let data = serde_json::json!({"room":"!allowed:example.org"});
        let args = arguments(
            &template,
            &data,
            &Default::default(),
            "root",
            &Value::String("hello".into()),
        )
        .unwrap();
        assert_eq!(
            args,
            serde_json::json!({"body":"hello","room_id":"!allowed:example.org"})
        );
        assert!(
            arguments(
                &serde_json::json!({"$data":"/missing"}),
                &data,
                &Default::default(),
                "root",
                &Value::Null
            )
            .is_err()
        );
    }
}
