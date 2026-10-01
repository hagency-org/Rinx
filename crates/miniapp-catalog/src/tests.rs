use super::*;
use hub::{Catalog, HubKey};
use serde_json::json;

struct Fixture {
    dir: tempfile::TempDir,
    anchor: HubKey,
    working: HubKey,
    publisher: HubKey,
}
impl Fixture {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
            anchor: HubKey::generate(),
            working: HubKey::generate(),
            publisher: HubKey::generate(),
        }
    }
    fn root(&self) -> PathBuf {
        self.dir.path().join("device")
    }
    fn client(&self) -> Client {
        Client::new(
            self.root(),
            Source::Directory(self.dir.path().into()),
            self.anchor.public_hex(),
            "macos",
        )
        .unwrap()
    }
    fn entry(&self, version: &str, capabilities: &[&str]) -> Entry {
        let artifact = format!("artifacts/counter/{version}.bundle");
        let root = self.dir.path().join(&artifact);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("main.splash"),
            "Label {text: \"A real bundled app\"}",
        )
        .unwrap();
        let mut manifest = AppManifest::parse(
            &json!({"schema":1,"id":"counter","name":"Counter","version":version,
            "integrity":{"bundle_blake3":""},"capabilities":capabilities})
            .to_string(),
        )
        .unwrap();
        fs::write(
            root.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        manifest.integrity.bundle_blake3 = octosense_app_contract::digest_dir(&root).unwrap();
        hub::sign_manifest(&self.publisher, &mut manifest, "test-publisher").unwrap();
        fs::write(
            root.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let entry = Entry {
            manifest,
            listing: None,
            tools: Default::default(),
            artifact: artifact.clone(),
            publisher: "test-publisher".into(),
            publisher_key: self.publisher.public_hex(),
            source: hub::Source {
                repository: "https://example.invalid/counter".into(),
                commit: "test-commit".into(),
            },
            status: hub::Status::Offered,
            admitted: hub::today(),
        };
        self.repack(&entry);
        entry
    }
    fn repack(&self, entry: &Entry) {
        let pack = hub::pack_dir(&self.dir.path().join(&entry.artifact)).unwrap();
        fs::write(
            self.dir
                .path()
                .join(format!("{}.pack.json", entry.artifact)),
            serde_json::to_vec(&pack).unwrap(),
        )
        .unwrap();
    }
    fn publish(&self, seq: u64, entries: Vec<Entry>) {
        self.publish_date(seq, entries, &hub::today());
    }
    fn publish_date(&self, seq: u64, entries: Vec<Entry>, date: &str) {
        let mut catalog = Catalog::new(seq, date, entries);
        self.working
            .sign_catalog(
                &mut catalog,
                &self.anchor.certify(&self.working.public_hex()).unwrap(),
            )
            .unwrap();
        fs::write(
            self.dir.path().join("catalog.json"),
            serde_json::to_vec(&catalog).unwrap(),
        )
        .unwrap();
    }
}
fn consent(snapshot: &Snapshot) -> Consent {
    let app = &snapshot.apps[0];
    Consent {
        id: app.id.clone(),
        entry: app.consent.clone(),
    }
}

#[test]
fn signed_app_installs_opens_and_survives_offline_restart() {
    let f = Fixture::new();
    let entry = f.entry("1", &["storage"]);
    f.publish(1, vec![entry]);
    let mut c = f.client();
    let approval = consent(&c.refresh());
    assert_eq!(
        c.install(&approval).unwrap().apps[0].status,
        Status::Installed
    );
    c.open(&approval).unwrap();
    c.record_open("counter").unwrap();
    drop(c);
    fs::remove_file(f.dir.path().join("catalog.json")).unwrap();
    let mut c = f.client();
    let snapshot = c.refresh();
    assert!(snapshot.warning.is_some());
    assert_eq!(snapshot.recent, ["counter"]);
    c.open(&approval).unwrap();
}
#[test]
fn damaged_download_preserves_previous_version() {
    let f = Fixture::new();
    let old = f.entry("1", &[]);
    f.publish(1, vec![old.clone()]);
    let mut c = f.client();
    let a = consent(&c.refresh());
    c.install(&a).unwrap();
    let next = f.entry("2", &[]);
    f.publish(2, vec![old, next.clone()]);
    fs::write(
        f.dir.path().join(&next.artifact).join("main.splash"),
        "tampered",
    )
    .unwrap();
    f.repack(&next);
    let a = consent(&c.refresh());
    assert!(c.install(&a).is_err());
    assert_eq!(c.store.installed_version("counter").as_deref(), Some("1"));
    assert!(
        fs::read_to_string(c.store.install_dir("counter").join("main.splash"))
            .unwrap()
            .contains("real bundled")
    );
}
#[test]
fn changed_permissions_require_new_consent() {
    let f = Fixture::new();
    let entry = f.entry("1", &[]);
    f.publish(1, vec![entry]);
    let mut c = f.client();
    let a = consent(&c.refresh());
    f.publish(2, vec![f.entry("2", &["matrix.send_message"])]);
    assert!(c.install(&a).unwrap_err().contains("Review"));
    assert!(c.store.installed_version("counter").is_none());
}
#[test]
fn withdrawn_app_cannot_run_or_be_restored_by_catalog_replay() {
    let f = Fixture::new();
    let entry = f.entry("1", &[]);
    f.publish(1, vec![entry.clone()]);
    let old_catalog = fs::read(f.dir.path().join("catalog.json")).unwrap();
    let mut c = f.client();
    let a = consent(&c.refresh());
    c.install(&a).unwrap();
    let mut withdrawn = entry;
    withdrawn.status = hub::Status::Withdrawn("Publisher recalled it".into());
    f.publish(2, vec![withdrawn]);
    c.refresh();
    assert!(c.open(&a).is_err());
    drop(c);
    fs::write(f.dir.path().join("catalog.json"), old_catalog).unwrap();
    let mut c = f.client();
    assert!(c.refresh().warning.is_some());
    assert!(c.open(&a).is_err());
}
#[test]
fn installed_bytes_and_complete_manifest_are_reverified() {
    let f = Fixture::new();
    f.publish(1, vec![f.entry("1", &[])]);
    let mut c = f.client();
    let a = consent(&c.refresh());
    c.install(&a).unwrap();
    let root = c.store.install_dir("counter");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    manifest["capabilities"] = json!(["matrix.send_message"]);
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    assert!(c.open(&a).err().unwrap().contains("manifest differs"));
}
#[test]
fn unsupported_services_remain_visible_and_cannot_install() {
    let f = Fixture::new();
    f.publish(1, vec![f.entry("1", &["mail"])]);
    let mut c = f.client();
    let s = c.refresh();
    assert!(matches!(&s.apps[0].status, Status::Unavailable(e) if e.contains("mail")));
    assert!(c.install(&consent(&s)).is_err());
}
#[test]
fn unknown_anchor_and_changed_signed_catalog_are_rejected() {
    let f = Fixture::new();
    f.publish(1, vec![f.entry("1", &[])]);
    let mut c = Client::new(
        f.root(),
        Source::Directory(f.dir.path().into()),
        HubKey::generate().public_hex(),
        "macos",
    )
    .unwrap();
    assert!(!c.refresh().verified);
    assert!(c.snapshot().apps.is_empty());
    drop(c);
    let mut c = f.client();
    c.refresh();
    let mut json: serde_json::Value =
        serde_json::from_slice(&fs::read(f.dir.path().join("catalog.json")).unwrap()).unwrap();
    json["entries"][0]["manifest"]["name"] = json!("Forged name");
    fs::write(f.dir.path().join("catalog.json"), json.to_string()).unwrap();
    assert_eq!(c.refresh().apps[0].name, "Counter");
}
#[test]
fn old_catalog_blocks_new_installs_but_allows_verified_installed_apps() {
    let f = Fixture::new();
    let entry = f.entry("1", &[]);
    f.publish(1, vec![entry.clone()]);
    let mut c = f.client();
    let a = consent(&c.refresh());
    c.install(&a).unwrap();
    f.publish_date(2, vec![entry], "2020-01-01");
    c.refresh();
    c.open(&a).unwrap();
    assert!(c.install(&a).is_err());
}
#[test]
fn crash_between_replacement_renames_restores_old_bundle() {
    let f = Fixture::new();
    f.publish(1, vec![f.entry("1", &[])]);
    let mut c = f.client();
    let a = consent(&c.refresh());
    c.install(&a).unwrap();
    drop(c);
    let app = f.root().join("counter");
    fs::rename(app.join("bundle"), app.join("bundle.previous")).unwrap();
    f.client().open(&a).unwrap();
    assert!(!app.join("bundle.previous").exists());
}
#[test]
fn pack_paths_and_size_are_bounded_before_unpacking() {
    let mut pack = hub::Pack {
        schema: 1,
        files: Default::default(),
    };
    for name in [
        "../escape",
        "/absolute",
        "a/../../b",
        "a\\b",
        "a/%2e%2e/b",
        "a//b",
    ] {
        pack.files.clear();
        pack.files.insert(name.into(), "".into());
        assert!(validate_pack(&pack).is_err(), "{name}");
    }
    pack.files.clear();
    pack.files
        .insert("main.splash".into(), "a".repeat(MAX_BYTES * 2));
    assert!(validate_pack(&pack).is_err());
}
#[test]
fn removal_clears_library_history_but_leaves_separate_documents() {
    let f = Fixture::new();
    f.publish(1, vec![f.entry("1", &[])]);
    let mut c = f.client();
    let a = consent(&c.refresh());
    c.install(&a).unwrap();
    c.record_open("counter").unwrap();
    let snapshot = c.remove("counter").unwrap();
    assert!(!snapshot.apps[0].installed);
    assert!(snapshot.recent.is_empty());
    assert!(c.remove("../other-account").is_err());
    assert!(c.remove(ARTICLE_ID).is_err());
}

#[test]
fn app_removed_from_catalog_remains_removable_in_the_library() {
    let f = Fixture::new();
    f.publish(1, vec![f.entry("1", &[])]);
    let mut c = f.client();
    let a = consent(&c.refresh());
    c.install(&a).unwrap();
    f.publish(2, vec![]);
    let s = c.refresh();
    assert!(s.apps[0].installed);
    assert!(matches!(s.apps[0].status, Status::Unavailable(_)));
    assert!(c.open(&a).is_err());
    assert!(c.remove("counter").unwrap().apps.is_empty());
}

#[test]
#[ignore = "contacts the production GitHub catalog; run explicitly"]
fn production_catalog_verifies_with_the_shared_anchor() {
    let root = tempfile::tempdir().unwrap();
    let mut client = Client::production(root.path().into(), "macos").unwrap();
    let snapshot = client.refresh();
    assert!(snapshot.verified, "{:?}", snapshot.warning);
    assert!(snapshot.warning.is_none(), "{:?}", snapshot.warning);
    println!(
        "Verified production App Hub: {} entries",
        snapshot.apps.len()
    );
}
