//! One version number everywhere (spec section 18, release pipeline: "the
//! build fails if any artefact disagrees"). The workspace's `Cargo.toml` is
//! the source; the MSIX package takes its version from there too.

use std::path::Path;

fn version_in(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(file);
    let text = std::fs::read_to_string(&path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    json["version"]
        .as_str()
        .unwrap_or_else(|| panic!("no version in {}", path.display()))
        .to_string()
}

#[test]
fn the_app_and_its_interface_carry_the_workspace_version() {
    let workspace = env!("CARGO_PKG_VERSION");
    assert_eq!(version_in("tauri.conf.json"), workspace, "tauri.conf.json");
    assert_eq!(
        version_in("../ui/package.json"),
        workspace,
        "ui/package.json"
    );
    assert_eq!(
        version_in("../ui/package-lock.json"),
        workspace,
        "ui/package-lock.json (run npm install after changing package.json)"
    );
}
