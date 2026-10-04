//! What a file's attributes say, read from the folder listing without
//! opening the file. The operating-system specific part of scanning
//! (PORT-4): on Windows, attributes; elsewhere, only the dot convention.

use std::fs::Metadata;

/// Hidden or system: on Windows by its attributes; everywhere, a name that
/// starts with a dot.
pub(crate) fn hidden(name: &str, meta: &Metadata) -> bool {
    name.starts_with('.') || platform::hidden(meta)
}

/// Its content is not on this computer: a cloud placeholder (OneDrive and
/// the like) or an archived file. Opening it would download or recall it,
/// so it is never opened (SRC-4).
pub(crate) fn cloud_only(meta: &Metadata) -> bool {
    platform::cloud_only(meta)
}

#[cfg(windows)]
mod platform {
    use std::fs::Metadata;
    use std::os::windows::fs::MetadataExt;

    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_OFFLINE, FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS,
        FILE_ATTRIBUTE_RECALL_ON_OPEN, FILE_ATTRIBUTE_SYSTEM,
    };

    pub fn hidden(meta: &Metadata) -> bool {
        meta.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
    }

    pub fn cloud_only(meta: &Metadata) -> bool {
        is_cloud_only(meta.file_attributes())
    }

    pub fn is_cloud_only(attributes: u32) -> bool {
        attributes
            & (FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS
                | FILE_ATTRIBUTE_RECALL_ON_OPEN
                | FILE_ATTRIBUTE_OFFLINE)
            != 0
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_ARCHIVE, FILE_ATTRIBUTE_PINNED, FILE_ATTRIBUTE_READONLY,
        };

        #[test]
        fn placeholders_and_archived_files_are_cloud_only() {
            // A OneDrive file kept only online.
            assert!(is_cloud_only(
                FILE_ATTRIBUTE_ARCHIVE | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS
            ));
            assert!(is_cloud_only(FILE_ATTRIBUTE_RECALL_ON_OPEN));
            assert!(is_cloud_only(FILE_ATTRIBUTE_OFFLINE));
            // Kept on this device: readable.
            assert!(!is_cloud_only(
                FILE_ATTRIBUTE_ARCHIVE | FILE_ATTRIBUTE_PINNED
            ));
            assert!(!is_cloud_only(FILE_ATTRIBUTE_READONLY));
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::fs::Metadata;

    pub fn hidden(_meta: &Metadata) -> bool {
        false
    }

    pub fn cloud_only(_meta: &Metadata) -> bool {
        false
    }
}
