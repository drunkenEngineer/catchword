//! What a scan leaves out (SRC-2): folders the user excluded, and files and
//! folders whose name matches a pattern. Pure logic: nothing is read here.

use std::path::{Path, PathBuf};

/// Left out until the user says otherwise: system files, development
/// folders, and files that often hold passwords or keys (threat T13).
pub const DEFAULT_PATTERNS: &[&str] = &[
    // System
    "$RECYCLE.BIN",
    "System Volume Information",
    "desktop.ini",
    "Thumbs.db",
    "~$*",
    // Development
    "node_modules",
    "__pycache__",
    "site-packages",
    "bower_components",
    "venv",
    // Passwords and keys
    "*.kdbx",
    "*.kdb",
    "*.key",
    "*.pem",
    "*.pfx",
    "*.p12",
    "*.ppk",
    "id_rsa*",
    "id_dsa*",
    "id_ecdsa*",
    "id_ed25519*",
    "*passwords*",
    "*recovery*codes*",
    "*backup*codes*",
];

/// The longest pattern, in characters.
pub const MAX_PATTERN_CHARS: usize = 200;
/// The most patterns one list may hold.
pub const MAX_PATTERNS: usize = 500;

/// The folders and name patterns a scan leaves out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Exclusions {
    /// Folders left out with everything in them, as resolved paths.
    pub folders: Vec<PathBuf>,
    /// Names of files and folders to leave out; see [`matches`].
    pub patterns: Vec<String>,
}

impl Exclusions {
    /// The default patterns, and no folders.
    pub fn with_default_patterns() -> Self {
        Self {
            folders: Vec::new(),
            patterns: DEFAULT_PATTERNS.iter().map(|p| p.to_string()).collect(),
        }
    }

    /// True if the file or folder at `path` is left out: it is in an
    /// excluded folder, or its own name matches a pattern. A folder's
    /// pattern leaves out everything in it, since the scan never enters it.
    pub fn excludes(&self, path: &Path) -> bool {
        if self.folders.iter().any(|folder| path.starts_with(folder)) {
            return true;
        }
        let Some(name) = path.file_name() else {
            return false;
        };
        let name = name.to_string_lossy();
        self.patterns.iter().any(|pattern| matches(pattern, &name))
    }
}

/// Why `pattern` cannot be used, or Ok if it can. Patterns match names, so
/// they hold no folder separators; a folder is excluded by choosing it.
pub fn check_pattern(pattern: &str) -> Result<(), &'static str> {
    if pattern.trim().is_empty() {
        return Err("A pattern cannot be empty.");
    }
    if pattern.chars().count() > MAX_PATTERN_CHARS {
        return Err("A pattern can be at most 200 characters long.");
    }
    if pattern.contains(['/', '\\']) {
        return Err("A pattern matches names, so it cannot contain / or \\. To leave out a folder, choose it instead.");
    }
    if pattern.chars().any(char::is_control) {
        return Err("A pattern cannot contain control characters.");
    }
    Ok(())
}

/// Whether `name` matches `pattern`: `*` stands for any run of characters,
/// `?` for exactly one, and case is ignored, as Windows does with names.
pub fn matches(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().flat_map(char::to_lowercase).collect();
    let name: Vec<char> = name.chars().flat_map(char::to_lowercase).collect();
    let (mut p, mut n) = (0, 0);
    // The last `*` seen, and where in the name its run began.
    let mut star: Option<(usize, usize)> = None;
    while n < name.len() {
        if pattern.get(p) == Some(&'*') {
            star = Some((p, n));
            p += 1;
        } else if pattern.get(p) == Some(&'?') || pattern.get(p) == Some(&name[n]) {
            p += 1;
            n += 1;
        } else if let Some((star_at, run_from)) = star {
            // Let the last `*` take one more character, and try again.
            star = Some((star_at, run_from + 1));
            p = star_at + 1;
            n = run_from + 1;
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stars_and_question_marks_match_like_windows_names() {
        assert!(matches("*.pem", "server.pem"));
        assert!(matches("*.PEM", "Server.pem"));
        assert!(!matches("*.pem", "server.pem.txt"));
        assert!(matches("id_rsa*", "id_rsa"));
        assert!(matches("id_rsa*", "id_rsa.pub"));
        assert!(matches("*recovery*codes*", "GitHub recovery codes.txt"));
        assert!(matches("report-????.pdf", "report-2025.pdf"));
        assert!(!matches("report-????.pdf", "report-25.pdf"));
        assert!(matches("node_modules", "node_modules"));
        assert!(!matches("node_modules", "node_modules_notes.txt"));
        assert!(matches("*", ""));
        assert!(matches("a*b*c", "a-b-b-c"));
        assert!(!matches("a*b*c", "a-b-b-d"));
        assert!(matches("~$*", "~$Budget.docx"));
    }

    #[test]
    fn letters_beyond_ascii_match_without_case() {
        assert!(matches("ÄRZTE*", "ärztebrief.pdf"));
        assert!(matches("عقد*", "عقد الإيجار.pdf"));
    }

    #[test]
    fn excluded_folders_cover_everything_inside_them() {
        let exclusions = Exclusions {
            folders: vec![PathBuf::from("/docs/private")],
            patterns: Vec::new(),
        };
        assert!(exclusions.excludes(Path::new("/docs/private")));
        assert!(exclusions.excludes(Path::new("/docs/private/tax/2025.pdf")));
        // Whole names only: a folder that merely starts the same is kept.
        assert!(!exclusions.excludes(Path::new("/docs/private-notes/a.txt")));
        assert!(!exclusions.excludes(Path::new("/docs/a.txt")));
    }

    #[test]
    fn the_default_list_leaves_out_keys_password_lists_and_system_files() {
        let defaults = Exclusions::with_default_patterns();
        for name in [
            "Passwords.txt",
            "my-passwords-export.csv",
            "id_ed25519",
            "vault.kdbx",
            "Thumbs.db",
            "node_modules",
            "GitHub recovery codes.txt",
        ] {
            assert!(defaults.excludes(&Path::new("/docs").join(name)), "{name}");
        }
        for name in ["lease.pdf", "password reset steps.md", "notes.txt"] {
            assert!(!defaults.excludes(&Path::new("/docs").join(name)), "{name}");
        }
        for pattern in DEFAULT_PATTERNS {
            assert_eq!(check_pattern(pattern), Ok(()), "{pattern}");
        }
    }

    #[test]
    fn unusable_patterns_say_why() {
        assert!(check_pattern("  ").is_err());
        assert!(check_pattern("docs/private")
            .unwrap_err()
            .contains("choose it"));
        assert!(check_pattern(r"C:\private").is_err());
        assert!(check_pattern(&"x".repeat(201)).is_err());
        assert!(check_pattern("a\u{1b}b").is_err());
        assert_eq!(check_pattern("*.bak"), Ok(()));
    }
}
