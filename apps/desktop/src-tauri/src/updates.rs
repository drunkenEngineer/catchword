//! When to check for updates, and what may be downloaded (APP-2, PRIV-2).
//!
//! These rules are in every build and tested there. The network code that
//! follows them is in `update_net.rs`, compiled only into the GitHub build
//! (the `updater` feature); the Store build has none (ADR-24).

// The address and the download rule are used only by the network module,
// which only the GitHub build has; elsewhere they are kept for their tests.

/// This build checks for updates itself: the GitHub download. The Store
/// build is updated by Windows.
pub const IN_BUILD: bool = cfg!(feature = "updater");

/// The one address ever asked: the update manifest of the newest release
/// of Catchword on GitHub. No identifier and no version is sent with it.
#[cfg_attr(not(feature = "updater"), allow(dead_code))]
pub const MANIFEST: &str =
    "https://github.com/drunkenEngineer/catchword/releases/latest/download/latest.json";

/// Updates are downloaded only from this repository's releases. GitHub
/// sends the file itself from its own servers.
#[cfg_attr(not(feature = "updater"), allow(dead_code))]
pub const DOWNLOADS: &str = "https://github.com/drunkenEngineer/catchword/releases/download/";

/// A successful check is followed by the next one a day later.
pub const CHECK_EVERY_SECS: i64 = 24 * 60 * 60;

/// A failed check, offline for example, is tried again an hour later.
pub const RETRY_AFTER_SECS: i64 = 60 * 60;

/// True if a check should run now: the user agreed, the last successful
/// check was a day ago or more, and the last attempt an hour ago or more.
pub fn due(
    check: Option<bool>,
    last_success: Option<i64>,
    last_attempt: Option<i64>,
    now: i64,
) -> bool {
    check == Some(true)
        && last_success.is_none_or(|last| now - last >= CHECK_EVERY_SECS)
        && last_attempt.is_none_or(|last| now - last >= RETRY_AFTER_SECS)
}

/// True if an update announced at `url` may be downloaded: a file in one
/// of this repository's releases, and nothing else.
#[cfg_attr(not(feature = "updater"), allow(dead_code))]
pub fn allowed_download(url: &str) -> bool {
    let Some(rest) = url.strip_prefix(DOWNLOADS) else {
        return false;
    };
    let mut parts = rest.split('/');
    let (Some(tag), Some(file), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let plain = |part: &str| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    };
    plain(tag) && plain(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = CHECK_EVERY_SECS;
    const HOUR: i64 = RETRY_AFTER_SECS;

    #[test]
    fn checks_only_when_agreed_and_at_most_once_a_day() {
        let now = 1_000 * DAY;
        // Not asked yet, or said no: never.
        assert!(!due(None, None, None, now));
        assert!(!due(Some(false), None, None, now));
        // Agreed: at once, then a day later.
        assert!(due(Some(true), None, None, now));
        assert!(!due(Some(true), Some(now - DAY + 1), None, now));
        assert!(due(Some(true), Some(now - DAY), None, now));
        // A failed attempt waits an hour before the next.
        assert!(!due(
            Some(true),
            Some(now - 2 * DAY),
            Some(now - HOUR + 1),
            now
        ));
        assert!(due(Some(true), Some(now - 2 * DAY), Some(now - HOUR), now));
    }

    #[test]
    fn updates_come_only_from_this_repositorys_releases() {
        let good = format!("{DOWNLOADS}v0.2.0/Catchword_0.2.0_x64-setup.exe");
        assert!(allowed_download(&good));
        for bad in [
            "https://example.com/Catchword_0.2.0_x64-setup.exe",
            "http://github.com/drunkenEngineer/catchword/releases/download/v0.2.0/a.exe",
            "https://github.com/someone-else/catchword/releases/download/v0.2.0/a.exe",
            "https://github.com/drunkenEngineer/catchword/releases/download/v0.2.0/../../x.exe",
            "https://github.com/drunkenEngineer/catchword/releases/download/v0.2.0/a.exe?b=c",
            "https://github.com/drunkenEngineer/catchword/releases/download/v0.2.0/",
            "https://github.com/drunkenEngineer/catchword/releases/download/a/b/c.exe",
            "https://github.com/drunkenEngineer/catchword/releases/download/v0.2.0/a.exe@evil.com",
        ] {
            assert!(!allowed_download(bad), "{bad}");
        }
    }

    #[test]
    fn the_manifest_is_in_the_same_repository_as_the_downloads() {
        let repository = "https://github.com/drunkenEngineer/catchword/releases/";
        assert!(MANIFEST.starts_with(repository));
        assert!(DOWNLOADS.starts_with(repository));
        assert!(MANIFEST.starts_with("https://") && DOWNLOADS.starts_with("https://"));
    }
}
