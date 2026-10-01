use rinx_system_apps::{pack, ARTICLE_ID};
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "rinx-catalog-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("apps/test/bundle")).unwrap();
        let f = Self(root);
        f.catalog(json!([{"directory":"test"}]));
        f.manifest("test.script");
        fs::write(
            f.0.join("apps/test/bundle/main.splash"),
            "Label {text: \"Hello\"}",
        )
        .unwrap();
        f
    }
    fn catalog(&self, apps: Value) {
        fs::write(self.0.join("system-apps.json"),json!({"schema":1,"source":"apps","repository":"https://github.com/hagency-org/Rinx","apps":apps}).to_string()).unwrap();
    }
    fn manifest(&self, id: &str) {
        fs::write(self.0.join("apps/test/bundle/manifest.json"),json!({"schema":1,"id":id,"version":"1.0.0","name":"Test","integrity":{"bundle_blake3":""},"capabilities":["matrix.profile"]}).to_string()).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn script_roundtrip_uses_canonical_digest_and_detects_modified_content_and_manifest() {
    let f = Fixture::new();
    let source = fs::read(f.0.join("apps/test/bundle/manifest.json")).unwrap();
    let app = pack(&f.0).unwrap().apps.remove(0);
    assert!(!app.manifest.integrity.bundle_blake3.is_empty());
    assert_eq!(
        fs::read(f.0.join("apps/test/bundle/manifest.json")).unwrap(),
        source
    );
    let target = f.0.join("installed");
    app.materialize(&target).unwrap();
    assert_eq!(
        app.manifest.integrity.bundle_blake3,
        octosense_app_contract::digest_dir(&target).unwrap()
    );
    fs::write(target.join("main.splash"), "changed").unwrap();
    assert!(app.verify(&target).is_err());
    let target = f.0.join("second");
    app.materialize(&target).unwrap();
    fs::write(target.join("manifest.json"), source).unwrap();
    assert!(app.verify(&target).is_err());
}
#[test]
fn native_entries_require_the_registered_identity_and_no_executable_script() {
    let f = Fixture::new();
    f.catalog(json!([{"directory":"test","native":"article-editor"}]));
    let error = pack(&f.0).unwrap_err();
    assert!(error.contains("stable app id"), "{error}");
    f.manifest(ARTICLE_ID);
    assert!(pack(&f.0).unwrap_err().contains("executable script"));
    fs::remove_file(f.0.join("apps/test/bundle/main.splash")).unwrap();
    assert!(pack(&f.0).unwrap().apps[0].native.is_some());
    f.catalog(json!([{"directory":"test","native":"unregistered"}]));
    assert!(pack(&f.0).is_err());
}
#[test]
fn script_cannot_claim_native_identity_or_omit_entrypoint() {
    let f = Fixture::new();
    f.manifest(ARTICLE_ID);
    assert!(pack(&f.0).unwrap_err().contains("Reserved"));
    f.manifest("test.script");
    fs::remove_file(f.0.join("apps/test/bundle/main.splash")).unwrap();
    assert!(pack(&f.0).unwrap_err().contains("needs main.splash"));
}
#[test]
fn rejects_duplicate_ids_and_traversal() {
    let f = Fixture::new();
    fs::create_dir_all(f.0.join("apps/second/bundle")).unwrap();
    for name in ["manifest.json", "main.splash"] {
        fs::copy(
            f.0.join("apps/test/bundle").join(name),
            f.0.join("apps/second/bundle").join(name),
        )
        .unwrap();
    }
    f.catalog(json!([{"directory":"test"},{"directory":"second"}]));
    assert!(pack(&f.0).unwrap_err().contains("Duplicate system app id"));
    f.catalog(json!([{"directory":"../test"}]));
    assert!(pack(&f.0).is_err());
}
#[cfg(unix)]
#[test]
fn rejects_symlinked_app_directory_and_bundle_files() {
    let f = Fixture::new();
    std::os::unix::fs::symlink(f.0.join("apps/test"), f.0.join("apps/alias")).unwrap();
    f.catalog(json!([{"directory":"alias"}]));
    assert!(pack(&f.0).is_err());
    f.catalog(json!([{"directory":"test"}]));
    std::os::unix::fs::symlink("main.splash", f.0.join("apps/test/bundle/copy")).unwrap();
    assert!(pack(&f.0).is_err());
}
#[test]
fn refuses_stale_digests_and_unknown_capabilities() {
    let f = Fixture::new();
    let p = f.0.join("apps/test/bundle/manifest.json");
    let mut m: Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
    m["integrity"]["bundle_blake3"] = "0".repeat(64).into();
    fs::write(&p, m.to_string()).unwrap();
    assert!(pack(&f.0).unwrap_err().contains("Stale"));
    m["integrity"]["bundle_blake3"] = "".into();
    m["capabilities"] = json!(["octos.kernel.rpc"]);
    fs::write(&p, m.to_string()).unwrap();
    assert!(pack(&f.0).is_err());
}
#[test]
fn materialize_never_overwrites_an_existing_directory() {
    let f = Fixture::new();
    let app = pack(&f.0).unwrap().apps.remove(0);
    let target = f.0.join("occupied");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("keep"), "existing").unwrap();
    assert!(app.materialize(&target).is_err());
    assert_eq!(fs::read_to_string(target.join("keep")).unwrap(), "existing");
}
