//! The user's settings: the folders to index. Precious, unlike the index,
//! so they are saved so that a crash or power cut cannot lose them, and the
//! previous copy is kept (section 12).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const FILE: &str = "settings.json";
const PREVIOUS: &str = "settings.previous.json";
const PARTIAL: &str = "settings.json.partial";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Bumped when the format changes.
    pub version: u32,
    pub folders: Vec<FolderEntry>,
    next_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderEntry {
    pub id: u32,
    pub path: PathBuf,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            folders: Vec::new(),
            next_id: 1,
        }
    }
}

impl Settings {
    /// Read the settings in `folder`. A damaged file falls back to the
    /// previous copy, then to defaults; the second value says what happened.
    pub fn load(folder: &Path) -> (Settings, Option<String>) {
        let read = |name: &str| -> Option<Settings> {
            let text = fs::read_to_string(folder.join(name)).ok()?;
            serde_json::from_str::<Settings>(&text)
                .ok()
                .filter(|settings| settings.version == 1)
        };
        if let Some(settings) = read(FILE) {
            return (settings, None);
        }
        if !folder.join(FILE).exists() {
            return (Settings::default(), None);
        }
        match read(PREVIOUS) {
            Some(settings) => (
                settings,
                Some("The settings file was damaged; the previous copy was restored.".into()),
            ),
            None => (
                Settings::default(),
                Some("The settings file was damaged; please choose your folders again.".into()),
            ),
        }
    }

    /// Write to a new file first, keep the current one as the previous copy,
    /// then put the new one in place, so a crash at any point leaves a
    /// readable file behind.
    pub fn save(&self, folder: &Path) -> io::Result<()> {
        fs::create_dir_all(folder)?;
        let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(folder.join(PARTIAL), text)?;
        if folder.join(FILE).exists() {
            fs::rename(folder.join(FILE), folder.join(PREVIOUS))?;
        }
        fs::rename(folder.join(PARTIAL), folder.join(FILE))
    }

    /// Add a folder, unless it is already there. Returns the new entry.
    pub fn add_folder(&mut self, path: PathBuf) -> Option<FolderEntry> {
        if self.folders.iter().any(|folder| folder.path == path) {
            return None;
        }
        let entry = FolderEntry {
            id: self.next_id,
            path,
        };
        self.next_id += 1;
        self.folders.push(entry.clone());
        Some(entry)
    }

    pub fn remove_folder(&mut self, id: u32) -> Option<FolderEntry> {
        let at = self.folders.iter().position(|folder| folder.id == id)?;
        Some(self.folders.remove(at))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("catchword-settings-test-{name}"));
        let _ = fs::remove_dir_all(&folder);
        folder
    }

    #[test]
    fn folders_are_added_once_and_removed_by_id() {
        let mut settings = Settings::default();
        let first = settings.add_folder("C:/docs".into()).unwrap();
        assert!(settings.add_folder("C:/docs".into()).is_none());
        let second = settings.add_folder("D:/scans".into()).unwrap();
        assert_ne!(first.id, second.id);
        assert_eq!(settings.remove_folder(first.id), Some(first));
        assert_eq!(settings.folders, vec![second]);
        assert_eq!(settings.remove_folder(999), None);
    }

    #[test]
    fn settings_survive_a_save_and_load() {
        let dir = folder("round-trip");
        let mut settings = Settings::default();
        settings.add_folder("C:/docs".into());
        settings.save(&dir).unwrap();
        assert_eq!(Settings::load(&dir), (settings, None));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn no_file_means_defaults_without_a_notice() {
        let dir = folder("missing");
        assert_eq!(Settings::load(&dir), (Settings::default(), None));
    }

    #[test]
    fn a_damaged_file_falls_back_to_the_previous_copy_then_to_defaults() {
        let dir = folder("damaged");
        let mut settings = Settings::default();
        settings.add_folder("C:/old".into());
        settings.save(&dir).unwrap();
        settings.add_folder("C:/new".into());
        settings.save(&dir).unwrap();

        fs::write(dir.join(FILE), "{ not json").unwrap();
        let (restored, notice) = Settings::load(&dir);
        assert_eq!(restored.folders.len(), 1);
        assert!(notice.unwrap().contains("previous copy"));

        fs::write(dir.join(PREVIOUS), "also damaged").unwrap();
        let (fresh, notice) = Settings::load(&dir);
        assert_eq!(fresh, Settings::default());
        assert!(notice.unwrap().contains("choose your folders again"));
        fs::remove_dir_all(&dir).unwrap();
    }
}
