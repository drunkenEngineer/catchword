//! Free disk space, so indexing stops before the disk fills up (RSC-3),
//! and whether a folder is copied to a cloud service (PRIV-4).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Below this much free space on the index's drive, indexing pauses.
pub const MIN_FREE_BYTES: u64 = 1 << 30;

/// Free bytes on the drive that holds `path`, for this user, or None if
/// that cannot be told.
#[cfg(windows)]
pub fn free_bytes(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    // Windows asks for a folder: the nearest existing one holding `path`,
    // which may be a file or not made yet, is on the same drive.
    let existing = path.ancestors().find(|place| place.is_dir())?;
    let wide: Vec<u16> = existing.as_os_str().encode_wide().chain([0]).collect();
    let mut free = 0u64;
    // SAFETY: the path ends in NUL, and the out-pointer is a valid u64; the
    // two values not asked for may be null.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(free)
}

#[cfg(not(windows))]
pub fn free_bytes(_path: &Path) -> Option<u64> {
    None
}

/// The cloud service that copies `path` to the internet, if one does: the
/// index holds the text of the user's documents and must stay here.
pub fn synced_by(path: &Path) -> Option<&'static str> {
    // OneDrive says where its folders are; the others go by their names.
    let onedrive: Vec<PathBuf> = ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"]
        .into_iter()
        .filter_map(std::env::var_os)
        .filter(|root: &OsString| !root.is_empty())
        .map(PathBuf::from)
        .collect();
    synced_by_in(path, &onedrive)
}

fn synced_by_in(path: &Path, onedrive: &[PathBuf]) -> Option<&'static str> {
    if onedrive.iter().any(|root| path.starts_with(root)) {
        return Some("OneDrive");
    }
    const BY_NAME: [(&str, &str); 4] = [
        ("Dropbox", "Dropbox"),
        ("Google Drive", "Google Drive"),
        ("My Drive", "Google Drive"),
        ("iCloudDrive", "iCloud Drive"),
    ];
    path.components().find_map(|part| {
        let part = part.as_os_str().to_string_lossy();
        BY_NAME
            .iter()
            .find(|(folder, _)| part.eq_ignore_ascii_case(folder))
            .map(|(_, service)| *service)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_in_a_synced_place_is_recognised() {
        // Paths as this system writes them, from its root.
        let root = if cfg!(windows) { r"C:\" } else { "/" };
        let place = |parts: &[&str]| {
            parts
                .iter()
                .fold(PathBuf::from(root), |path, part| path.join(part))
        };
        let onedrive = [place(&["Users", "ana", "OneDrive"])];
        let synced = |parts: &[&str]| synced_by_in(&place(parts), &onedrive);
        assert_eq!(
            synced(&["Users", "ana", "OneDrive", "Catchword", "data"]),
            Some("OneDrive")
        );
        assert_eq!(synced(&["dropbox", "apps", "catchword"]), Some("Dropbox"));
        assert_eq!(synced(&["My Drive", "catchword"]), Some("Google Drive"));
        let local = ["Users", "ana", "AppData", "Local", "org.catchword.desktop"];
        assert_eq!(synced(&local), None);
        // A folder that merely starts the same is not synced.
        assert_eq!(synced(&["Users", "ana", "OneDrive-old", "data"]), None);
    }

    #[test]
    fn free_space_is_read_for_a_file_or_a_folder_not_made_yet() {
        let folder = std::env::temp_dir().join("catchword-disk-test");
        std::fs::create_dir_all(&folder).unwrap();
        let file = folder.join("index.db");
        std::fs::write(&file, "x").unwrap();
        if cfg!(windows) {
            assert!(free_bytes(&file).unwrap() > 0);
            assert!(free_bytes(&folder.join("not/made/yet")).unwrap() > 0);
        }
    }
}
