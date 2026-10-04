//! Whether earlier runs ended cleanly, so that after two runs in a row that
//! did not, the app opens in a safe mode with indexing paused (spec section
//! 19, crash handling).
//!
//! A run marks itself as running when it starts and clears the mark when it
//! ends normally. A crash, a hang that had to be killed, or a power cut
//! leaves the mark, and the next start counts it.

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

const FILE: &str = "session.json";

/// Runs in a row that ended uncleanly, after which the app opens in safe mode.
pub const UNCLEAN_FOR_SAFE_MODE: u32 = 2;

#[derive(Debug, Default, Serialize, Deserialize)]
struct Session {
    running: bool,
    unclean_ends: u32,
}

/// Note a start in `folder`. Returns how many runs in a row before this one
/// ended uncleanly. A missing or damaged record counts as none.
pub fn start(folder: &Path) -> u32 {
    let earlier: Session = fs::read_to_string(folder.join(FILE))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let unclean_ends = if earlier.running {
        earlier.unclean_ends + 1
    } else {
        0
    };
    let _ = write(
        folder,
        &Session {
            running: true,
            unclean_ends,
        },
    );
    unclean_ends
}

/// Note a clean end: the run is over, and so is any streak.
pub fn clean_end(folder: &Path) {
    let _ = write(folder, &Session::default());
}

/// A record that cannot be written only costs the safe mode its warning.
fn write(folder: &Path, session: &Session) -> io::Result<()> {
    fs::create_dir_all(folder)?;
    let text = serde_json::to_string(session).map_err(io::Error::other)?;
    fs::write(folder.join(FILE), text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> std::path::PathBuf {
        let folder = std::env::temp_dir().join(format!("catchword-session-test-{name}"));
        let _ = fs::remove_dir_all(&folder);
        folder
    }

    #[test]
    fn unclean_ends_are_counted_until_a_clean_one() {
        let dir = folder("count");
        assert_eq!(start(&dir), 0);
        // Ended without clean_end: a crash.
        assert_eq!(start(&dir), 1);
        assert_eq!(start(&dir), UNCLEAN_FOR_SAFE_MODE);
        clean_end(&dir);
        assert_eq!(start(&dir), 0);
        clean_end(&dir);
        assert_eq!(start(&dir), 0);
    }

    #[test]
    fn a_damaged_record_counts_as_no_crash() {
        let dir = folder("damaged");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(FILE), "{ not json").unwrap();
        assert_eq!(start(&dir), 0);
    }
}
