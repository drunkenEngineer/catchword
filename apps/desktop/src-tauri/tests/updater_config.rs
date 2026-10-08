//! The updater's settings in tauri.conf.json (APP-2, ADR-24): updates are
//! checked against the owner's public key, must be signed for the version
//! they claim, and are never older than the version installed.

use std::path::Path;

fn updater() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    config["plugins"]["updater"].clone()
}

#[test]
fn updates_are_checked_against_the_owners_key() {
    // A minisign public key, base64-encoded: "untrusted comment: minisign
    // public key: ..." followed by the key itself.
    let key = updater()["pubkey"].as_str().unwrap_or_default().to_string();
    assert!(
        key.starts_with("dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6"),
        "{key}"
    );
    assert!(key.len() > 100, "{key}");
}

#[test]
fn updates_must_be_signed_for_their_version_and_never_go_back() {
    let updater = updater();
    assert_eq!(updater["requireSignedVersion"], true);
    // Off unless set: an older, validly signed release must not install.
    assert_ne!(updater["allowDowngrades"], true);
    // The address is fixed in the code (src/updates.rs), not here.
    assert!(updater.get("endpoints").is_none());
}
