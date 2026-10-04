//! Free disk space, so indexing stops before the disk fills up (RSC-3).

use std::path::Path;

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

#[cfg(test)]
mod tests {
    use super::*;

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
