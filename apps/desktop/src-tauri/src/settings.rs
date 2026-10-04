//! The user's settings: the folders to index and what to leave out of them.
//! Precious, unlike the index, so they are saved so that a crash or power
//! cut cannot lose them, and the previous copy is kept (section 12).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use catchword_engine::exclude::{check_pattern, MAX_PATTERNS};
use catchword_engine::Exclusions;
use serde::{Deserialize, Serialize};

use crate::contract::{ResourceMode, TextSize, Theme};

const FILE: &str = "settings.json";
const PREVIOUS: &str = "settings.previous.json";
const PARTIAL: &str = "settings.json.partial";
/// Every file the settings may leave behind, for deleting all data.
pub const FILES: [&str; 3] = [FILE, PREVIOUS, PARTIAL];

/// Version 2 added exclusions and the first-launch flag.
const VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Bumped when the format changes.
    pub version: u32,
    pub folders: Vec<FolderEntry>,
    /// Folders inside the chosen ones that are left out (SRC-2).
    #[serde(default)]
    pub excluded_folders: Vec<FolderEntry>,
    /// Names of files and folders that are left out.
    #[serde(default)]
    pub patterns: Vec<String>,
    /// The first-launch steps are done (APP-1).
    #[serde(default)]
    pub welcomed: bool,
    /// Logs also record file paths and error details (PRIV-3).
    #[serde(default)]
    pub detailed_logs: bool,
    #[serde(default)]
    pub resource_mode: ResourceMode,
    /// The user paused indexing; it stays paused after a restart.
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub text_size: TextSize,
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
            version: VERSION,
            folders: Vec::new(),
            excluded_folders: Vec::new(),
            patterns: Exclusions::with_default_patterns().patterns,
            welcomed: false,
            detailed_logs: false,
            resource_mode: ResourceMode::Balanced,
            paused: false,
            theme: Theme::System,
            text_size: TextSize::Normal,
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
                .and_then(Settings::upgraded)
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

    /// The settings in the current format, or None if they are from a newer
    /// version. Patterns that do not pass the checks are dropped.
    fn upgraded(mut self) -> Option<Settings> {
        match self.version {
            // Version 1 had no exclusions, and its user had started already.
            1 => {
                self.patterns = Exclusions::with_default_patterns().patterns;
                self.welcomed = true;
            }
            VERSION => {}
            _ => return None,
        }
        self.version = VERSION;
        self.patterns
            .retain(|pattern| check_pattern(pattern).is_ok());
        self.patterns.truncate(MAX_PATTERNS);
        Some(self)
    }

    /// What indexing leaves out.
    pub fn exclusions(&self) -> Exclusions {
        Exclusions {
            folders: self
                .excluded_folders
                .iter()
                .map(|entry| entry.path.clone())
                .collect(),
            patterns: self.patterns.clone(),
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

    fn new_entry(&mut self, path: PathBuf) -> FolderEntry {
        let entry = FolderEntry {
            id: self.next_id,
            path,
        };
        self.next_id += 1;
        entry
    }

    /// Add a folder, unless it is already there. Returns the new entry.
    pub fn add_folder(&mut self, path: PathBuf) -> Option<FolderEntry> {
        if self.folders.iter().any(|folder| folder.path == path) {
            return None;
        }
        let entry = self.new_entry(path);
        self.folders.push(entry.clone());
        Some(entry)
    }

    /// Remove a folder, and the exclusions inside it, which mean nothing
    /// without it.
    pub fn remove_folder(&mut self, id: u32) -> Option<FolderEntry> {
        let at = self.folders.iter().position(|folder| folder.id == id)?;
        let removed = self.folders.remove(at);
        self.excluded_folders
            .retain(|excluded| !excluded.path.starts_with(&removed.path));
        Some(removed)
    }

    /// Leave out a folder inside one of the chosen ones. Returns the new
    /// entry, or None if it is left out already.
    pub fn exclude_folder(&mut self, path: PathBuf) -> Result<Option<FolderEntry>, &'static str> {
        let inside = self
            .folders
            .iter()
            .any(|folder| path.starts_with(&folder.path) && path != folder.path);
        if !inside {
            return Err("Choose a folder inside one of your folders. To stop searching a whole folder, remove it in Library.");
        }
        if self
            .excluded_folders
            .iter()
            .any(|excluded| excluded.path == path)
        {
            return Ok(None);
        }
        let entry = self.new_entry(path);
        self.excluded_folders.push(entry.clone());
        Ok(Some(entry))
    }

    /// Stop leaving out an excluded folder.
    pub fn include_folder(&mut self, id: u32) -> Option<FolderEntry> {
        let at = self
            .excluded_folders
            .iter()
            .position(|folder| folder.id == id)?;
        Some(self.excluded_folders.remove(at))
    }

    /// Replace the patterns. Blank lines and repeats are dropped; any other
    /// pattern that does not pass the checks is refused, with why.
    pub fn set_patterns(&mut self, patterns: &[String]) -> Result<(), String> {
        let mut kept: Vec<String> = Vec::new();
        for pattern in patterns.iter().map(|p| p.trim()) {
            if pattern.is_empty() || kept.iter().any(|k| k == pattern) {
                continue;
            }
            check_pattern(pattern).map_err(|why| format!("“{pattern}”: {why}"))?;
            kept.push(pattern.to_string());
        }
        if kept.len() > MAX_PATTERNS {
            return Err(format!("At most {MAX_PATTERNS} patterns."));
        }
        self.patterns = kept;
        Ok(())
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
    fn new_settings_leave_out_the_default_patterns_and_await_the_first_launch() {
        let settings = Settings::default();
        assert!(!settings.welcomed);
        assert!(settings.patterns.iter().any(|p| p == "*.kdbx"));
        assert!(settings.exclusions().folders.is_empty());
    }

    #[test]
    fn only_folders_inside_chosen_ones_can_be_excluded() {
        let mut settings = Settings::default();
        let docs = settings.add_folder("C:/docs".into()).unwrap();
        let private = settings
            .exclude_folder("C:/docs/private".into())
            .unwrap()
            .unwrap();
        assert_eq!(settings.exclude_folder("C:/docs/private".into()), Ok(None));
        assert!(settings.exclude_folder("C:/docs".into()).is_err());
        assert!(settings.exclude_folder("D:/elsewhere".into()).is_err());
        assert_eq!(
            settings.exclusions().folders,
            vec![PathBuf::from("C:/docs/private")]
        );
        assert_ne!(private.id, docs.id);

        assert_eq!(settings.include_folder(private.id), Some(private.clone()));
        assert!(settings.excluded_folders.is_empty());

        // Removing a folder forgets what was excluded inside it.
        settings.exclude_folder("C:/docs/private".into()).unwrap();
        settings.remove_folder(docs.id);
        assert!(settings.excluded_folders.is_empty());
    }

    #[test]
    fn patterns_are_cleaned_and_checked() {
        let mut settings = Settings::default();
        let lines = |text: &str| text.lines().map(String::from).collect::<Vec<_>>();
        settings
            .set_patterns(&lines("  *.bak \n\n*.bak\nnode_modules"))
            .unwrap();
        assert_eq!(settings.patterns, vec!["*.bak", "node_modules"]);
        let refused = settings.set_patterns(&lines("*.tmp\ndocs/private"));
        assert!(refused.unwrap_err().contains("docs/private"));
        assert_eq!(settings.patterns, vec!["*.bak", "node_modules"]);
        settings.set_patterns(&[]).unwrap();
        assert!(settings.patterns.is_empty());
    }

    #[test]
    fn version_1_settings_are_upgraded_with_the_default_patterns() {
        let dir = folder("upgrade");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(FILE),
            r#"{ "version": 1, "folders": [{ "id": 1, "path": "C:/docs" }], "next_id": 2 }"#,
        )
        .unwrap();
        let (settings, notice) = Settings::load(&dir);
        assert_eq!(notice, None);
        assert_eq!(settings.version, VERSION);
        assert_eq!(settings.folders.len(), 1);
        assert!(settings.welcomed);
        assert_eq!(settings.patterns, Settings::default().patterns);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn settings_from_a_newer_version_are_not_misread() {
        let dir = folder("newer");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(FILE),
            r#"{ "version": 99, "folders": [], "next_id": 1 }"#,
        )
        .unwrap();
        let (settings, notice) = Settings::load(&dir);
        assert_eq!(settings, Settings::default());
        assert!(notice.is_some());
        fs::remove_dir_all(&dir).unwrap();
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
