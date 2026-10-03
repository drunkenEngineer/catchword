//! Local logs (OBS-1): one JSON object per line in the `logs` folder,
//! rotated at 1 MB, with three files kept, so they never take more than
//! about 3 MB. Nothing is ever sent anywhere.
//!
//! Private by construction (PRIV-3). An event's name is fixed text written
//! in the code, and the values beside it are numbers or fixed codes, so no
//! document text, query or file name can reach a log by accident. Paths and
//! error details are `Private` values: they are written only while the user
//! has turned on detailed logs, and a line holding one is marked private, so
//! a diagnostics export can leave it out.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value as Json};

/// A file is rotated once it would pass this size.
const MAX_FILE_BYTES: u64 = 1 << 20;
/// The current file, then the older ones, newest first.
pub const FILES: [&str; 3] = ["catchword.log", "catchword.1.log", "catchword.2.log"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Error,
    Warn,
    Info,
    /// Written only in detailed logs.
    Debug,
}

impl Level {
    fn name(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
            Level::Debug => "debug",
        }
    }
}

/// A value written beside an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Number(u64),
    /// A fixed code from the code itself, such as a reason or a stage.
    Code(&'static str),
    Flag(bool),
    /// A path or an error's details: written only in detailed logs.
    Private(String),
}

pub struct Logger {
    folder: PathBuf,
    detailed: AtomicBool,
    /// The open file and its size, opened on first use.
    file: Mutex<Option<(File, u64)>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Logger {
    pub fn new(folder: PathBuf, detailed: bool) -> Self {
        Self {
            folder,
            detailed: AtomicBool::new(detailed),
            file: Mutex::new(None),
        }
    }

    pub fn folder(&self) -> &Path {
        &self.folder
    }

    pub fn set_detailed(&self, on: bool) {
        self.detailed.store(on, Ordering::SeqCst);
    }

    pub fn detailed(&self) -> bool {
        self.detailed.load(Ordering::SeqCst)
    }

    pub fn error(&self, event: &'static str, values: &[(&'static str, Value)]) {
        self.write(Level::Error, event, values);
    }

    pub fn warn(&self, event: &'static str, values: &[(&'static str, Value)]) {
        self.write(Level::Warn, event, values);
    }

    pub fn info(&self, event: &'static str, values: &[(&'static str, Value)]) {
        self.write(Level::Info, event, values);
    }

    pub fn debug(&self, event: &'static str, values: &[(&'static str, Value)]) {
        self.write(Level::Debug, event, values);
    }

    /// Write one line. A log that cannot be written is not worth stopping
    /// the app for, so failures are ignored.
    pub fn write(&self, level: Level, event: &'static str, values: &[(&'static str, Value)]) {
        let detailed = self.detailed();
        if level == Level::Debug && !detailed {
            return;
        }
        let mut line = Map::new();
        line.insert("time".into(), Json::from(timestamp(SystemTime::now())));
        line.insert("level".into(), Json::from(level.name()));
        line.insert("event".into(), Json::from(event));
        let mut private = false;
        for (key, value) in values {
            let json = match value {
                Value::Number(n) => Json::from(*n),
                Value::Code(code) => Json::from(*code),
                Value::Flag(flag) => Json::from(*flag),
                Value::Private(text) if detailed => {
                    private = true;
                    Json::from(text.as_str())
                }
                Value::Private(_) => continue,
            };
            line.insert((*key).into(), json);
        }
        if private {
            line.insert("private".into(), Json::from(true));
        }
        let mut text = Json::Object(line).to_string();
        text.push('\n');
        let _ = self.append(text.as_bytes());
    }

    fn append(&self, bytes: &[u8]) -> std::io::Result<()> {
        let mut file = lock(&self.file);
        let size = file.as_ref().map_or(0, |(_, size)| *size);
        if file.is_none() || size + bytes.len() as u64 > MAX_FILE_BYTES {
            *file = None;
            fs::create_dir_all(&self.folder)?;
            let current = self.folder.join(FILES[0]);
            let size = fs::metadata(&current).map_or(0, |meta| meta.len());
            if size + bytes.len() as u64 > MAX_FILE_BYTES {
                self.rotate()?;
            }
            let opened = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&current)?;
            let size = opened.metadata()?.len();
            *file = Some((opened, size));
        }
        let (open, size) = file.as_mut().expect("opened above");
        open.write_all(bytes)?;
        *size += bytes.len() as u64;
        Ok(())
    }

    /// The oldest file goes; each other moves one place down.
    fn rotate(&self) -> std::io::Result<()> {
        for at in (1..FILES.len()).rev() {
            let from = self.folder.join(FILES[at - 1]);
            if from.exists() {
                fs::rename(from, self.folder.join(FILES[at]))?;
            }
        }
        Ok(())
    }

    /// The last `count` lines, oldest first, from all files.
    pub fn last_lines(&self, count: usize) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        for name in FILES.iter().rev() {
            if let Ok(text) = fs::read_to_string(self.folder.join(name)) {
                lines.extend(text.lines().map(String::from));
            }
        }
        let skip = lines.len().saturating_sub(count);
        lines.split_off(skip)
    }

    /// Close the current file, so the folder can be deleted.
    pub fn close(&self) {
        *lock(&self.file) = None;
    }
}

/// Write panics to the log, then let them go on as before (OBS-3, in
/// part). Where in the code is written; the message only in detailed logs,
/// as it may hold a path.
pub fn record_panics(log: std::sync::Arc<Logger>) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let (file, line) = info
            .location()
            .map_or(("unknown", 0), |at| (at.file(), at.line()));
        // An event value must be fixed text. A source file name is, but
        // only for the program's lifetime, so it is kept for good.
        let file: &'static str = Box::leak(file.to_string().into_boxed_str());
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|text| text.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        log.error(
            "panic",
            &[
                ("file", Value::Code(file)),
                ("line", Value::Number(u64::from(line))),
                ("message", Value::Private(message)),
            ],
        );
        previous(info);
    }));
}

/// True if a log line holds private values.
pub fn is_private(line: &str) -> bool {
    serde_json::from_str::<Map<String, Json>>(line)
        .map_or(true, |line| line.get("private") == Some(&Json::from(true)))
}

/// UTC time as `2026-10-03T18:46:02Z`, without a date library.
pub fn timestamp(time: SystemTime) -> String {
    let secs = time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rest) = (secs / 86_400, secs % 86_400);
    // Days to a civil date: Howard Hinnant's algorithm.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn folder(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("catchword-log-test-{name}"));
        let _ = fs::remove_dir_all(&folder);
        folder
    }

    fn lines(logger: &Logger) -> Vec<Map<String, Json>> {
        logger
            .last_lines(usize::MAX)
            .iter()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn each_line_is_json_with_time_level_event_and_values() {
        let logger = Logger::new(folder("json"), false);
        logger.info(
            "index.finished",
            &[
                ("files", Value::Number(12)),
                ("stage", Value::Code("words")),
            ],
        );
        let line = &lines(&logger)[0];
        assert_eq!(line["level"], "info");
        assert_eq!(line["event"], "index.finished");
        assert_eq!(line["files"], 12);
        assert_eq!(line["stage"], "words");
        assert!(line["time"].as_str().unwrap().ends_with('Z'));
        assert!(line.get("private").is_none());
    }

    #[test]
    fn private_values_and_debug_lines_are_written_only_in_detailed_logs() {
        let logger = Logger::new(folder("private"), false);
        let path = || Value::Private(r"C:\Users\ana\Medical\results.pdf".to_string());
        logger.warn("file.failed", &[("path", path())]);
        logger.debug("file.read", &[("path", path())]);
        let text = logger.last_lines(10).join("\n");
        assert!(!text.contains("results.pdf"), "{text}");
        assert_eq!(lines(&logger).len(), 1);
        assert!(!is_private(&logger.last_lines(1)[0]));

        logger.set_detailed(true);
        logger.debug("file.read", &[("path", path())]);
        let last = logger.last_lines(1).remove(0);
        assert!(last.contains("results.pdf"));
        assert!(is_private(&last));
    }

    #[test]
    fn logs_are_rotated_and_capped() {
        let logger = Logger::new(folder("rotate"), false);
        let padding = Value::Code(
            "this fixed text only makes the line longer, to fill the files sooner, \
             and it is long enough to need no more than a few thousand lines",
        );
        for n in 0..40_000u64 {
            logger.info(
                "filler",
                &[("n", Value::Number(n)), ("text", padding.clone())],
            );
        }
        let mut total = 0;
        for name in FILES {
            let size = fs::metadata(logger.folder().join(name)).unwrap().len();
            assert!(size <= MAX_FILE_BYTES, "{name}: {size}");
            total += size;
        }
        assert!(total > 2 * MAX_FILE_BYTES);
        assert!(!logger.folder().join("catchword.3.log").exists());
        // The newest line is last.
        let last = lines(&logger).pop().unwrap();
        assert_eq!(last["n"], 39_999);
    }

    #[test]
    fn times_are_utc_dates() {
        assert_eq!(timestamp(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        let leap_day = UNIX_EPOCH + Duration::from_secs(951_782_400 + 3_723);
        assert_eq!(timestamp(leap_day), "2000-02-29T01:02:03Z");
        let later = UNIX_EPOCH + Duration::from_secs(1_791_052_199);
        assert_eq!(timestamp(later), "2026-10-03T18:29:59Z");
    }
}
