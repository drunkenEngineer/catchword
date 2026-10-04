//! The diagnostics report (APP-5): plain text the user reads before saving
//! it, and sends only if they choose to. It holds no document text and no
//! searches. Paths, and the names to leave out that the user typed, are in
//! it only if the user asks for them (section 12).

use std::fmt::Write;
use std::time::SystemTime;

use catchword_service::is_parked;
use catchword_store::{Counts, Problem};

use crate::log::{is_private, timestamp};
use crate::settings::Settings;

/// The log lines a report shows, at most.
pub const LOG_LINES: usize = 300;

/// What the report is made from.
pub struct Facts {
    pub made: SystemTime,
    pub version: &'static str,
    pub windows: Option<String>,
    /// "ready", "starting" or "off".
    pub meaning: &'static str,
    /// Why meaning search is off. Shown only with paths, as it may hold one.
    pub meaning_detail: Option<String>,
    pub layout_version: i64,
    pub counts: Counts,
    pub model: Option<String>,
    pub index_bytes: u64,
    pub problems: Vec<Problem>,
    pub settings: Settings,
    pub default_patterns: Vec<String>,
    pub detailed_logs: bool,
    /// Oldest first.
    pub log_lines: Vec<String>,
    /// The crash reports kept, oldest first.
    pub crash_reports: Vec<String>,
}

pub fn report(facts: &Facts, include_paths: bool) -> String {
    let mut out = String::new();
    // Writing to a String cannot fail.
    let mut line = |text: String| {
        let _ = writeln!(out, "{text}");
    };
    let yes_no = |on: bool| if on { "on" } else { "off" };

    line("Catchword diagnostics report".into());
    line(format!("Made {}.", timestamp(facts.made)));
    line(String::new());
    line("Read this before you share it. It holds no document text and no searches.".into());
    line(if include_paths {
        "File and folder names: included, because you asked for them.".into()
    } else {
        "File and folder names: left out.".into()
    });

    line(String::new());
    line("App".into());
    line(format!("  Version: {}", facts.version));
    line(format!(
        "  Windows: {}",
        facts.windows.as_deref().unwrap_or("unknown")
    ));
    match (&facts.meaning_detail, include_paths) {
        (Some(detail), true) => line(format!("  Meaning search: {} ({detail})", facts.meaning)),
        _ => line(format!("  Meaning search: {}", facts.meaning)),
    }
    line(format!("  Detailed logs: {}", yes_no(facts.detailed_logs)));

    line(String::new());
    line("Index".into());
    line(format!("  Layout version: {}", facts.layout_version));
    line(format!("  Files: {}", facts.counts.files));
    line(format!("  Different contents: {}", facts.counts.contents));
    line(format!("  Passages: {}", facts.counts.passages));
    line(format!("  Searchable by meaning: {}", facts.counts.vectors));
    line(format!(
        "  Embedding model: {}",
        facts.model.as_deref().unwrap_or("none yet")
    ));
    line(format!("  Size on disk: {} bytes", facts.index_bytes));

    line(String::new());
    line(format!("Files not indexed: {}", facts.problems.len()));
    let mut by_reason: Vec<(&str, usize, usize)> = Vec::new();
    for problem in &facts.problems {
        let code = problem.reason.code();
        let parked = usize::from(is_parked(problem));
        match by_reason.iter_mut().find(|(c, _, _)| *c == code) {
            Some(entry) => {
                entry.1 += 1;
                entry.2 += parked;
            }
            None => by_reason.push((code, 1, parked)),
        }
    }
    for (code, count, parked) in by_reason {
        if parked > 0 {
            line(format!("  {code}: {count} ({parked} parked)"));
        } else {
            line(format!("  {code}: {count}"));
        }
    }
    if include_paths {
        for problem in &facts.problems {
            line(format!("    {}: {}", problem.path, problem.reason.code()));
        }
    }

    line(String::new());
    line("Settings".into());
    let settings = &facts.settings;
    line(format!("  Folders: {}", settings.folders.len()));
    if include_paths {
        for folder in &settings.folders {
            line(format!("    {}", folder.path.display()));
        }
    }
    line(format!(
        "  Folders left out: {}",
        settings.excluded_folders.len()
    ));
    if include_paths {
        for folder in &settings.excluded_folders {
            line(format!("    {}", folder.path.display()));
        }
    }
    let defaults = if settings.patterns == facts.default_patterns {
        "the default list"
    } else {
        "changed from the default list"
    };
    line(format!(
        "  Names left out: {}, {defaults}",
        settings.patterns.len()
    ));
    if include_paths {
        for pattern in &settings.patterns {
            line(format!("    {pattern}"));
        }
    }

    line(String::new());
    line(format!("Crash reports: {}", facts.crash_reports.len()));
    if let Some(newest) = facts.crash_reports.last() {
        line("  The newest:".into());
        for report_line in newest.lines() {
            // A message is in a report only if detailed logs were on.
            if !include_paths
                && report_line.starts_with("Message: ")
                && !report_line.starts_with("Message: left out")
            {
                line("  Message: left out".into());
                continue;
            }
            line(format!("    {report_line}"));
        }
    }

    line(String::new());
    let shown: Vec<&String> = facts
        .log_lines
        .iter()
        .filter(|log_line| include_paths || !is_private(log_line))
        .collect();
    let hidden = facts.log_lines.len() - shown.len();
    if hidden > 0 {
        line(format!(
            "Log, last {} lines ({hidden} with file names left out)",
            shown.len()
        ));
    } else {
        line(format!("Log, last {} lines", shown.len()));
    }
    for log_line in shown {
        line(format!("  {log_line}"));
    }
    out
}

/// The Windows version, such as "25H2, build 26200", from the registry.
#[cfg(windows)]
pub fn windows_version() -> Option<String> {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};

    let key: Vec<u16> = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\0"
        .encode_utf16()
        .collect();
    let read = |name: &str| -> Option<String> {
        let name: Vec<u16> = name.encode_utf16().chain([0]).collect();
        let mut buffer = [0u16; 64];
        let mut bytes = std::mem::size_of_val(&buffer) as u32;
        // SAFETY: both names end in NUL, and `bytes` is the buffer's size in
        // bytes; the call writes no more than that and reports how much.
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        if status != 0 {
            return None;
        }
        // The size counts the closing NUL.
        let chars = (bytes as usize / 2).saturating_sub(1).min(buffer.len());
        Some(String::from_utf16_lossy(&buffer[..chars]))
    };
    let build = read("CurrentBuild")?;
    Some(match read("DisplayVersion") {
        Some(version) => format!("{version}, build {build}"),
        None => format!("build {build}"),
    })
}

#[cfg(not(windows))]
pub fn windows_version() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use catchword_engine::exclude::DEFAULT_PATTERNS;
    use catchword_engine::extract::Reason;

    fn facts() -> Facts {
        let mut settings = Settings::default();
        settings.add_folder(r"C:\Users\ana\Documents\Medical".into());
        settings
            .exclude_folder(r"C:\Users\ana\Documents\Medical\Therapy".into())
            .unwrap();
        settings.set_patterns(&["*divorce*".to_string()]).unwrap();
        Facts {
            made: SystemTime::UNIX_EPOCH,
            version: "0.0.1",
            windows: Some("25H2, build 26200".into()),
            meaning: "off",
            meaning_detail: Some(r"cannot read C:\Users\ana\model.onnx".into()),
            layout_version: 4,
            counts: Counts {
                files: 12,
                contents: 11,
                passages: 340,
                vectors: 300,
            },
            model: Some("granite-embedding-97m-multilingual-r2".into()),
            index_bytes: 18_350_080,
            problems: vec![
                Problem {
                    path: r"C:\Users\ana\Documents\Medical\scan.pdf".into(),
                    reason: Reason::NeedsOcr,
                    attempts: 1,
                },
                Problem {
                    path: r"C:\Users\ana\Documents\Medical\broken.pdf".into(),
                    reason: Reason::Crashed,
                    attempts: 2,
                },
            ],
            settings,
            default_patterns: DEFAULT_PATTERNS.iter().map(|p| p.to_string()).collect(),
            detailed_logs: true,
            log_lines: vec![
                r#"{"event":"index.folder","files":12}"#.into(),
                r#"{"event":"file.failed","path":"C:\\Users\\ana\\Documents\\Medical\\broken.pdf","private":true}"#.into(),
            ],
            crash_reports: vec![
                "Catchword crash report\nWhere: src/x.rs:1\nMessage: cannot read C:\\Users\\ana\\x.txt\n".into(),
            ],
        }
    }

    #[test]
    fn without_consent_no_path_or_typed_name_is_in_the_report() {
        let text = report(&facts(), false);
        for secret in [
            "ana",
            "Medical",
            "Therapy",
            "divorce",
            "scan.pdf",
            "model.onnx",
        ] {
            assert!(!text.contains(secret), "{secret} in:\n{text}");
        }
        assert!(text.contains("File and folder names: left out."));
        assert!(text.contains("Files: 12"));
        assert!(text.contains("needs-ocr: 1"));
        assert!(text.contains("crashed: 1 (1 parked)"));
        assert!(text.contains("Folders: 1"));
        assert!(text.contains("Names left out: 1, changed from the default list"));
        assert!(text.contains("Log, last 1 lines (1 with file names left out)"));
        assert!(text.contains("Crash reports: 1"));
        assert!(text.contains("Where: src/x.rs:1"));
        assert!(text.contains(r#""event":"index.folder""#));
    }

    #[test]
    fn with_consent_paths_names_and_private_log_lines_are_included() {
        let text = report(&facts(), true);
        for wanted in [
            r"C:\Users\ana\Documents\Medical",
            r"C:\Users\ana\Documents\Medical\Therapy",
            "*divorce*",
            r"scan.pdf: needs-ocr",
            "cannot read",
            "Log, last 2 lines",
        ] {
            assert!(text.contains(wanted), "{wanted} not in:\n{text}");
        }
    }

    #[test]
    fn the_windows_version_is_read() {
        if cfg!(windows) {
            assert!(windows_version().unwrap().contains("build "));
        }
    }
}
