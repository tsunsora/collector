//! A small log file, so there is something to look at, and to send, when the
//! app misbehaves. The release build has no console, so without this nothing
//! it reports about itself would ever be seen.
//!
//! What goes in: when things start and stop, and every failure. What never
//! goes in: the Twitch token, the account's name or id, and request addresses
//! (which can carry a playback token). Channel names do appear.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::SystemTime,
};

const FILE: &str = "log.txt";
const OLD_FILE: &str = "log.old.txt";
/// Past this size the file is set aside as `log.old.txt` and a new one begun,
/// so the two together never grow beyond about a megabyte.
const MAX_BYTES: u64 = 512 * 1024;

static DIR: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

fn dir() -> &'static Mutex<Option<PathBuf>> {
    DIR.get_or_init(|| Mutex::new(None))
}

/// Start writing to `log.txt` in `folder`. Until this is called, lines only
/// go to the console.
pub fn start(folder: Option<PathBuf>) {
    if let Some(folder) = &folder {
        let _ = fs::create_dir_all(folder);
    }
    *dir().lock().unwrap() = folder;
}

/// Where the log file is, once logging has started.
pub fn path() -> Option<PathBuf> {
    dir().lock().unwrap().as_ref().map(|d| d.join(FILE))
}

/// One line, with the time in front. Never fails: a log that cannot be
/// written must not take the app down with it.
pub fn write(line: &str) {
    eprintln!("{line}");
    let guard = dir().lock().unwrap();
    let Some(folder) = guard.as_ref() else { return };
    let file = folder.join(FILE);
    if fs::metadata(&file).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = fs::rename(&file, folder.join(OLD_FILE));
    }
    if let Ok(mut out) = OpenOptions::new().create(true).append(true).open(&file) {
        let _ = writeln!(out, "{}  {}", crate::twitch::hermes::timestamp(SystemTime::now()), one_line(line));
    }
}

/// Keep an entry on one line, whatever an error message contains.
fn one_line(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
}

/// `log!("poll {login}: {e}")`: to the console and the log file.
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::log::write(&format!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_stays_on_one_line() {
        assert_eq!(one_line("first\r\nsecond\nthird"), "first  second third");
    }
}
