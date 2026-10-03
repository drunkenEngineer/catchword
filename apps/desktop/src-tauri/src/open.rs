//! Opening a result's file and showing it in Explorer (RES-2).
//!
//! The path always comes from the index, never from the interface, and
//! only document types Catchword indexes are opened (threat T15).

use std::io;
use std::path::Path;

use catchword_engine::document_kind;

/// Refuse anything that is not an indexed document type, or is gone.
fn check(path: &Path) -> io::Result<()> {
    if document_kind(path).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "only document types Catchword indexes can be opened",
        ));
    }
    if !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the file has been moved or deleted; rescan the folder",
        ));
    }
    Ok(())
}

/// Open the file in its default application.
pub fn open_file(path: &Path) -> io::Result<()> {
    check(path)?;
    shell_open(path)
}

/// Show the file selected in its folder.
pub fn reveal_file(path: &Path) -> io::Result<()> {
    check(path)?;
    tauri_plugin_opener::reveal_item_in_dir(path).map_err(io::Error::other)
}

/// On Windows, through the shell API itself: no command line is involved,
/// so nothing in the path can be read as a command.
#[cfg(windows)]
fn shell_open(path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let file: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain([0]).collect();
    // SAFETY: both strings are NUL-terminated UTF-16 that live until the
    // call returns; a null window, parameter list and directory are allowed.
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW reports success with a value above 32.
    if result as isize > 32 {
        Ok(())
    } else {
        Err(io::Error::other("Windows could not open the file"))
    }
}

#[cfg(not(windows))]
fn shell_open(path: &Path) -> io::Result<()> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_indexed_document_types_are_opened() {
        let folder = std::env::temp_dir().join("catchword-open-test");
        std::fs::create_dir_all(&folder).unwrap();
        let program = folder.join("setup.exe");
        std::fs::write(&program, b"MZ").unwrap();
        let error = open_file(&program).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        let error = reveal_file(&folder.join("gone.pdf")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
