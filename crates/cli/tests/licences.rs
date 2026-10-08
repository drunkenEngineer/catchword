//! One list of allowed licences (CLAUDE.md, rule 8): cargo-deny checks every
//! dependency against deny.toml, and the licence notices against about.toml.
//! The two must allow exactly the same licences.

use std::path::Path;

/// The quoted entries of the list that starts with `key = [` in `file`.
fn list(file: &str, key: &str) -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = std::fs::read_to_string(root.join(file)).unwrap();
    let start = text
        .find(&format!("{key} = ["))
        .unwrap_or_else(|| panic!("no {key} list in {file}"));
    let body = &text[start..start + text[start..].find(']').unwrap()];
    let mut entries: Vec<String> = body
        .lines()
        .skip(1)
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix('"')
                .and_then(|rest| rest.split('"').next())
                .map(str::to_string)
        })
        .collect();
    entries.sort();
    entries
}

#[test]
fn deny_and_about_allow_the_same_licences() {
    let deny = list("deny.toml", "allow");
    let about = list("about.toml", "accepted");
    assert!(deny.contains(&"Apache-2.0".to_string()), "{deny:?}");
    assert_eq!(deny, about);
    for copyleft in ["GPL", "AGPL", "LGPL"] {
        assert!(
            deny.iter().all(|licence| !licence.contains(copyleft)),
            "{deny:?}"
        );
    }
}
