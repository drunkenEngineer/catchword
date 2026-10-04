//! The command contract: every value that crosses between the shell and the
//! interface (ARC-3). The TypeScript side is generated from these types into
//! `apps/desktop/ui/src/contract/`, and a test fails when the two drift.
//!
//! The interface speaks in ids: it never sends a file path to be read or
//! opened. Paths here are for display only (section 9, rule 7).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Everything the interface shows about the index and its work.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub folders: Vec<FolderStatus>,
    /// What indexing is doing right now, if anything.
    pub work: Option<Work>,
    #[ts(type = "number")]
    pub files: i64,
    #[ts(type = "number")]
    pub passages: i64,
    #[ts(type = "number")]
    pub searchable_by_meaning: i64,
    pub meaning: Meaning,
    /// Files that are not in the index, with why (COV-2).
    pub not_indexed: Vec<NotIndexed>,
    /// The last thing that went wrong, in plain words.
    pub problem: Option<String>,
    /// The first-launch steps are not done yet (APP-1).
    pub first_launch: bool,
    /// Why indexing is paused, if it is (IDX-5, RSC-3).
    pub paused: Option<PauseReason>,
    /// When a scan of every folder last finished, in seconds since 1970.
    #[ts(type = "number | null")]
    pub last_scan_secs: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum PauseReason {
    /// The user paused it.
    You,
    /// Free space on the index's drive ran low.
    LowDisk,
    /// The index was made by a newer version of Catchword. It is left
    /// untouched until the user rebuilds it (REL-5).
    NewerIndex,
}

/// Light or dark, or as Windows is set (APP-3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// How large the interface is drawn, on top of Windows' own scaling.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum TextSize {
    #[default]
    Normal,
    /// 115%.
    Large,
    /// 130%.
    Larger,
}

/// How much of the processor indexing may use (IDX-5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ResourceMode {
    /// One core: slowest, kindest to a laptop on battery.
    Light,
    /// Half the processor: the default.
    #[default]
    Balanced,
    /// All cores but one: soonest done.
    Fast,
}

/// The settings the interface shows and changes (APP-3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    /// Folders inside the chosen ones that are left out.
    pub excluded_folders: Vec<Folder>,
    /// Names of files and folders that are left out.
    pub patterns: Vec<String>,
    /// The patterns a new install starts with.
    pub default_patterns: Vec<String>,
    /// Where the index is stored (COV-3).
    pub data_folder: String,
    /// The size of the index on disk, in bytes.
    #[ts(type = "number")]
    pub index_bytes: u64,
    /// Logs also record file paths and error details.
    pub detailed_logs: bool,
    pub version: String,
    pub resource_mode: ResourceMode,
    /// The cloud service that copies the data folder, if one does (PRIV-4).
    pub data_synced_by: Option<String>,
    pub theme: Theme,
    pub text_size: TextSize,
    /// Larger files are skipped (SRC-7).
    pub max_file_mb: u32,
    /// Pages past this many are not read from a PDF.
    pub max_pages: u32,
}

/// A folder the user chose. The path is shown, never sent back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: u32,
    pub path: String,
}

/// A folder the user chose, with what indexing knows of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FolderStatus {
    pub id: u32,
    pub path: String,
    pub state: FolderState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum FolderState {
    Ready,
    Scanning,
    /// It could not be reached on the last scan: its files stay in the
    /// index, searchable, but cannot be opened (SRC-5).
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    /// Reading files and making them searchable by their words.
    Words,
    /// Making passages searchable by their meaning.
    Meaning,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Work {
    pub stage: Stage,
    #[ts(type = "number")]
    pub done: u64,
    #[ts(type = "number")]
    pub total: u64,
    /// Files or passages a second, over the last half minute, once known.
    pub per_second: Option<f32>,
    /// The time left at that pace, once known (COV-1).
    #[ts(type = "number | null")]
    pub seconds_left: Option<u64>,
}

/// Whether meaning search is available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Meaning {
    Loading,
    Ready,
    Off { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NotIndexed {
    pub name: String,
    pub folder: String,
    pub reason: String,
    /// True when reading failed; false when the file was skipped by a rule.
    pub failed: bool,
    /// True when reading failed too often: it is not tried again until the
    /// user asks for a retry.
    pub parked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    /// One entry per file, best first, its passages beneath it (SEA-3).
    pub files: Vec<FileHit>,
    /// What to know about these results, in plain words.
    pub notes: Vec<String>,
    pub elapsed_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FileHit {
    pub name: String,
    pub folder: String,
    /// How many files share this exact content (1 means no copies).
    #[ts(type = "number")]
    pub copies: i64,
    /// When the file was last changed, in seconds since 1970 (SEA-2).
    #[ts(type = "number")]
    pub modified_secs: i64,
    pub passages: Vec<PassageHit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PassageHit {
    /// The id to preview, open or reveal this passage's file with.
    #[ts(type = "number")]
    pub id: i64,
    pub location: String,
    /// The matching words, marked; shown as plain text only.
    pub snippet: Vec<Span>,
    pub found: FoundBy,
}

/// A piece of text; `marked` pieces are the words that matched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub text: String,
    pub marked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum FoundBy {
    Words,
    Meaning,
    Both,
    /// Only by its file or folder name (SEA-1).
    Name,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use ts_rs::Config;

    /// The generated TypeScript of each type, by file name.
    fn generated() -> Vec<(String, String)> {
        let config = Config::default();
        let mut files = Vec::new();
        macro_rules! each {
            ($($t:ty),*) => {$(
                files.push((
                    format!("{}.ts", <$t as TS>::name(&config)),
                    <$t as TS>::export_to_string(&config).unwrap(),
                ));
            )*};
        }
        each!(
            Status,
            SettingsView,
            Folder,
            FolderStatus,
            FolderState,
            PauseReason,
            ResourceMode,
            Theme,
            TextSize,
            Stage,
            Work,
            Meaning,
            NotIndexed,
            SearchResponse,
            FileHit,
            PassageHit,
            Span,
            FoundBy
        );
        files
    }

    fn folder() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ui/src/contract")
    }

    /// Fails when a Rust type changed and the TypeScript was not regenerated.
    /// To regenerate: `UPDATE_CONTRACT=1 cargo test -p catchword-desktop contract`.
    #[test]
    fn the_typescript_contract_matches_the_rust_types() {
        let update = std::env::var_os("UPDATE_CONTRACT").is_some();
        let mut stale = Vec::new();
        for (name, content) in generated() {
            let path = folder().join(&name);
            if update {
                fs::create_dir_all(folder()).unwrap();
                fs::write(&path, &content).unwrap();
            } else {
                // A Windows checkout may turn line endings into CRLF.
                let committed = fs::read_to_string(&path).map(|text| text.replace("\r\n", "\n"));
                if committed.ok().as_deref() != Some(content.as_str()) {
                    stale.push(name);
                }
            }
        }
        assert!(
            stale.is_empty(),
            "the TypeScript contract is out of date: {stale:?}. Regenerate it with \
             UPDATE_CONTRACT=1 cargo test -p catchword-desktop contract"
        );
    }
}
