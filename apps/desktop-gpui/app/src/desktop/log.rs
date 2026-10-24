// SPDX-License-Identifier: AGPL-3.0-only
//! The app's own log (`desktop:app-log.ts`): `<wsp home>/logs/app.log`, which today's app writes too, each entry with
//! its time and level. Rotated at 2 MiB with four files kept and every entry cut at 16 Ki characters, so one line
//! cannot rotate the log away. No secret enters it: the host's redaction rule (`redact` in
//! `packages/host/src/init-log.ts`) runs on every entry, and so does every value the launch's environment holds under
//! a credential's name. A write that fails is dropped: the app never stops for its log.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, OnceLock};

use chrono::{DateTime, Utc};
use regex::Regex;
use wsp_host_client::Env;

pub const APP_LOG: &str = "app.log";
/// The size app.log is rotated at.
pub const LOG_BYTES: u64 = 2 * 1024 * 1024;
/// Files kept, app.log among them: app.1.log is the one before it.
pub const LOGS_KEPT: usize = 4;
/// One entry's ceiling, in the UTF-16 units a JavaScript string counts.
pub const ENTRY_CHARS: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    fn word(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }
}

/// `SECRET_NAME`: a name that says credential, anywhere in it.
static SECRET_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new("(?i)KEY|TOKEN|SECRET|PASSWORD").unwrap());
/// `SECRET_ASSIGN`: a NAME=value whose name says credential, quoted or bare.
static SECRET_ASSIGN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)(?-u:\b)([A-Za-z0-9_]*(?:KEY|TOKEN|SECRET|PASSWORD)[A-Za-z0-9_]*)=("[^"]*"|'[^']*'|\S+)"#).unwrap());
/// `SECRET_FIELD`: the same name as a quoted JSON key.
static SECRET_FIELD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)("[A-Za-z0-9_-]*(?:KEY|TOKEN|SECRET|PASSWORD)[A-Za-z0-9_-]*"\s*:\s*)"[^"]*""#).unwrap());
/// `AUTH_VALUE`: a bearer or basic header's value.
static AUTH_VALUE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"((?-u:\b)Bearer |[Aa]uthorization['"]?:\s*['"]?Basic )[^\s"',;]+"#).unwrap());
/// `URL_LOGIN`: an address's login.
static URL_LOGIN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)((?-u:\b)[a-z][a-z0-9+.-]*://)[^\s/@]+@").unwrap());

/// One of `TOKEN_SHAPES` (`packages/host/src/token-shapes.ts`): the regex crate has no lookaround, so the character a
/// shape may not follow, and what its JavaScript lookahead asks of a match, are checked beside it.
struct Shape {
    run: Regex,
    after: fn(char) -> bool,
    holds: fn(&str, Option<char>) -> bool,
}

fn alnum(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

fn alnum_dash(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn nothing(_: char) -> bool {
    false
}

fn any(_: &str, _: Option<char>) -> bool {
    true
}

static TOKEN_SHAPES: LazyLock<Vec<Shape>> = LazyLock::new(|| {
    let shape =
        |run: &str, after: fn(char) -> bool, holds: fn(&str, Option<char>) -> bool| Shape { run: Regex::new(run).unwrap(), after, holds };
    vec![
        // `sk-(?:proj-|ant-)?(?=[A-Za-z_-]*[0-9])[A-Za-z0-9_-]{20,}` matches what this does: a run of 20 or more after
        // `sk-` that holds a digit.
        shape("sk-[A-Za-z0-9_-]{20,}", alnum, |m, _| m[3..].bytes().any(|b| b.is_ascii_digit())),
        shape("AIza[A-Za-z0-9_-]{30,}", alnum, any),
        shape("github_pat_[A-Za-z0-9_]{20,}", alnum, any),
        shape("gh[opsur]_[A-Za-z0-9]{20,}", alnum, any),
        shape("glpat-[A-Za-z0-9_-]{20,}", alnum, any),
        shape("xox[abpr]-[A-Za-z0-9-]{10,}", alnum, any),
        shape("AKIA[0-9A-Z]{16}", alnum, |_, next| !next.is_some_and(alnum)),
        shape(r"eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}", alnum_dash, any),
        shape("-----BEGIN [A-Z ]*PRIVATE KEY-----", nothing, any),
    ]
});

const MARK: &str = "<redacted>";

/// `blankTokens` for one shape: every match not preceded by a character the shape may not follow, a refused match
/// tried again one character on, as a backtracking engine would.
fn blank(text: &str, shape: &Shape) -> String {
    let mut out = String::with_capacity(text.len());
    let (mut from, mut kept) = (0, 0);
    while let Some(found) = shape.run.find_at(text, from) {
        let before = text[..found.start()].chars().next_back();
        let next = text[found.end()..].chars().next();
        if !before.is_some_and(shape.after) && (shape.holds)(found.as_str(), next) {
            out.push_str(&text[kept..found.start()]);
            out.push_str(MARK);
            (from, kept) = (found.end(), found.end());
        } else {
            from = found.start() + text[found.start()..].chars().next().map_or(1, char::len_utf8);
        }
        if from > text.len() {
            break;
        }
    }
    out.push_str(&text[kept..]);
    out
}

/// `redacted` (`packages/protocol/src/words/thread.ts`): each value of four or more characters, and its trimmed
/// form, blanked wherever it turns up, the longest first.
fn redacted(text: &str, hidden: &[String]) -> String {
    let mut values: Vec<&str> = hidden.iter().filter(|h| h.trim().chars().count() >= 4).flat_map(|h| [h.as_str(), h.trim()]).collect();
    values.sort_by_key(|v| std::cmp::Reverse(v.len()));
    values.dedup();
    values.into_iter().fold(text.to_owned(), |out, v| out.replace(v, MARK))
}

/// The host's `redact`: credential-named assignments, JSON fields, auth headers and address logins lose their value,
/// every `hidden` value is blanked, then every run shaped like a token.
pub fn redact(text: &str, hidden: &[String]) -> String {
    let named = SECRET_ASSIGN.replace_all(text, "${1}=<redacted>");
    let named = SECRET_FIELD.replace_all(&named, "${1}\"<redacted>\"");
    let named = AUTH_VALUE.replace_all(&named, "${1}<redacted>");
    let named = URL_LOGIN.replace_all(&named, "${1}<redacted>@");
    TOKEN_SHAPES.iter().fold(redacted(&named, hidden), |out, shape| blank(&out, shape))
}

/// `credentialValues`: each value this environment carries under a name that says credential.
pub fn credential_values(env: &Env) -> Vec<String> {
    env.iter().filter(|(name, _)| SECRET_NAME.is_match(name)).map(|(_, value)| value.clone()).collect()
}

/// One entry as it lands: its time as `toISOString` writes it, its level, the text blanked, then cut to the ceiling,
/// so the cut never leaves half a secret that no longer matches the whole, and later lines indented two.
pub fn entry(level: Level, text: &str, at: DateTime<Utc>, hidden: &[String]) -> String {
    let clean = redact(text, hidden);
    let units = clean.encode_utf16().count();
    let body = if units > ENTRY_CHARS {
        let mut counted = 0;
        let cut = clean.char_indices().find(|(_, c)| {
            counted += c.len_utf16();
            counted > ENTRY_CHARS
        });
        let end = cut.map_or(clean.len(), |(at, _)| at);
        format!("{} ({} characters cut)", &clean[..end], units - ENTRY_CHARS)
    } else {
        clean
    };
    format!("{} {} {}\n", at.format("%Y-%m-%dT%H:%M:%S%.3fZ"), level.word(), body.replace('\n', "\n  "))
}

/// The log at `dir/app.log`.
pub struct AppLog {
    dir: PathBuf,
    path: PathBuf,
    hidden: Vec<String>,
    max_bytes: u64,
    kept: usize,
    /// The file's size as this process last wrote it; writes are one at a time under it.
    size: Mutex<u64>,
}

impl AppLog {
    /// The log in `dir`, the folder made, blanking every credential-named value of `env`.
    pub fn open(dir: &Path, env: &Env) -> AppLog {
        AppLog::open_with(dir, env, LOG_BYTES, LOGS_KEPT)
    }

    pub fn open_with(dir: &Path, env: &Env, max_bytes: u64, kept: usize) -> AppLog {
        let path = dir.join(APP_LOG);
        // A first launch has no log yet; a folder that cannot be made fails every write below the same quiet way.
        _ = fs::create_dir_all(dir);
        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        AppLog { dir: dir.to_owned(), path, hidden: credential_values(env), max_bytes, kept, size: Mutex::new(size) }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write(&self, level: Level, text: &str) {
        let entry = entry(level, text, Utc::now(), &self.hidden);
        let bytes = entry.len() as u64;
        let mut size = self.size.lock().unwrap_or_else(|held| held.into_inner());
        if *size > 0 && *size + bytes > self.max_bytes {
            // A file another launch rotated first is no reason to drop this entry.
            _ = self.rotate();
            *size = 0;
        }
        let written = OpenOptions::new().create(true).append(true).open(&self.path).and_then(|mut file| file.write_all(entry.as_bytes()));
        if written.is_ok() {
            *size += bytes;
        }
    }

    fn rotate(&self) -> std::io::Result<()> {
        let numbered = |n: usize| self.dir.join(format!("app.{n}.log"));
        _ = fs::remove_file(numbered(self.kept - 1));
        for n in (1..self.kept - 1).rev() {
            if numbered(n).exists() {
                fs::rename(numbered(n), numbered(n + 1))?;
            }
        }
        fs::rename(&self.path, numbered(1))
    }
}

static LOG: OnceLock<AppLog> = OnceLock::new();

/// Opens this process's log under `dir`, once.
pub fn init(dir: &Path, env: &Env) {
    _ = LOG.set(AppLog::open(dir, env));
}

/// Says the launch in the log, once the app holds its home, and logs every panic after it.
pub fn started(release: &str) {
    let (os, arch) = (node_word(std::env::consts::OS), node_word(std::env::consts::ARCH));
    write(Level::Info, &format!("wsp app {release} started (pid {}, {os} {arch}, gpui)", std::process::id()));
    let earlier = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        write(Level::Error, &format!("main: {info}\n{}", std::backtrace::Backtrace::force_capture()));
        earlier(info);
    }));
}

/// The platform and arch as Node names them, as today's app writes them.
fn node_word(rust: &str) -> &str {
    match rust {
        "macos" => "darwin",
        "windows" => "win32",
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    }
}

/// The file this process logs to, once opened.
pub fn path() -> Option<PathBuf> {
    LOG.get().map(|log| log.path.clone())
}

/// An entry in this process's log; nothing before it is opened.
pub fn write(level: Level, text: &str) {
    if let Some(log) = LOG.get() {
        log.write(level, text);
    }
}

/// A warning on stderr and in the log, as main.ts's `io.error` says one.
pub fn warn(text: &str) {
    eprintln!("desktop-gpui: {text}");
    write(Level::Warn, text);
}

pub fn error(text: &str) {
    eprintln!("desktop-gpui: {text}");
    write(Level::Error, text);
}

pub fn info(text: &str) {
    eprintln!("desktop-gpui: {text}");
    write(Level::Info, text);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-10T11:23:55.178Z").unwrap().with_timezone(&Utc)
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        names.sort();
        names
    }

    fn env(pairs: &[(&str, &str)]) -> Env {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn makes_its_folder_and_writes_each_entry_with_its_time_and_level_a_stack_indented() {
        assert_eq!(
            entry(Level::Error, "main: panic: forced\n    at main.rs:1:1", at(), &[]),
            "2026-10-10T11:23:55.178Z error main: panic: forced\n      at main.rs:1:1\n"
        );
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join(".wsp/logs");
        let log = AppLog::open(&dir, &Env::default());
        log.write(Level::Info, "attached http://127.0.0.1:4400");
        log.write(Level::Warn, "menu bar: no host answered");
        assert_eq!(log.path(), dir.join(APP_LOG));
        let text = fs::read_to_string(log.path()).unwrap();
        let time = Regex::new(r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z ").unwrap();
        let lines: Vec<String> = text.lines().map(|l| time.replace(l, "").into_owned()).collect();
        assert_eq!(lines, ["info attached http://127.0.0.1:4400", "warn menu bar: no host answered"]);
        assert!(text.lines().all(|l| time.is_match(l)), "{text}");
        AppLog::open(&dir, &Env::default()).write(Level::Info, "second launch");
        assert_eq!(fs::read_to_string(log.path()).unwrap().lines().count(), 3, "a later launch appends");
    }

    #[test]
    fn stays_bounded_rotated_at_its_size_four_files_kept_the_oldest_dropped() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join("logs");
        let log = AppLog::open_with(&dir, &Env::default(), 1000, 4);
        for n in 0..200 {
            log.write(Level::Error, &format!("entry {n:03} {}", "x".repeat(60)));
        }
        assert_eq!(files(&dir), ["app.1.log", "app.2.log", "app.3.log", "app.log"]);
        for file in files(&dir) {
            assert!(fs::metadata(dir.join(&file)).unwrap().len() <= 1000, "{file}");
        }
        assert!(fs::read_to_string(dir.join(APP_LOG)).unwrap().contains("entry 199"));
        let all: String = files(&dir).iter().map(|f| fs::read_to_string(dir.join(f)).unwrap()).collect();
        assert!(!all.contains("entry 000"));
        assert!(all.contains("entry 190"), "the files kept hold the newest entries");
    }

    #[test]
    fn cuts_one_huge_entry_to_its_ceiling() {
        let home = tempfile::tempdir().unwrap();
        let log = AppLog::open(home.path(), &Env::default());
        log.write(Level::Error, &"y".repeat(ENTRY_CHARS * 10));
        let text = fs::read_to_string(log.path()).unwrap();
        assert!(text.len() < ENTRY_CHARS + 200, "{}", text.len());
        assert!(text.contains(&format!("({} characters cut)", ENTRY_CHARS * 9)));
    }

    #[test]
    fn never_holds_a_secret() {
        let home = tempfile::tempdir().unwrap();
        let key = "sk-ant-api03-forced-secret-value-1234";
        let token = "ghp_forcedtoken5678";
        let named = [("ANTHROPIC_API_KEY", key), ("GH_TOKEN", token), ("DB_PASSWORD", "hunter22"), ("HOME", "/home/dev")];
        let log = AppLog::open(home.path(), &env(&named));
        log.write(Level::Error, "quit: the database said hunter22 is wrong for /home/dev");
        log.write(Level::Error, &format!("update: get failed: fetch https://api.example/release?token={token} refused"));
        log.write(Level::Error, &format!("menu bar: bad key {key}"));
        log.write(Level::Error, r#"main: spawn failed with SOLARI_API_KEY=slr_live_abcdef and OPENAI_API_KEY="sk-proj-xyz""#);
        log.write(Level::Error, &format!("{}{key}", "z".repeat(ENTRY_CHARS - 5)));
        let text = fs::read_to_string(log.path()).unwrap();
        for secret in [key, token, "slr_live_abcdef", "sk-proj-xyz", "sk-ant", "hunter22"] {
            assert!(!text.contains(secret), "{secret} was logged");
        }
        assert!(text.contains("token=<redacted>"));
        assert!(text.contains("bad key <redacted>"));
        assert!(text.contains("SOLARI_API_KEY=<redacted>"));
        assert!(text.contains("said <redacted> is wrong for /home/dev"), "a value of no token's shape, and HOME is no credential");
        let mut held =
            credential_values(&env(&[("ANTHROPIC_API_KEY", key), ("GH_TOKEN", token), ("HOME", "/home/dev"), ("DB_PASSWORD", "hunter22")]));
        held.sort();
        assert_eq!(held, ["ghp_forcedtoken5678", "hunter22", key]);
    }

    #[test]
    fn blanks_the_hosts_patterns() {
        let said = redact(
            concat!(
                r#"{"apiKey": "abc"} Authorization: Basic dXNlcjpw Bearer tok.en https://me:pw@git.example/x "#,
                "AKIAABCDEFGHIJKLMNOP AKIAABCDEFGHIJKLMNOPQ xoxb-1234567890ab glpat-abcdefghij0123456789 ",
                "AIzaabcdefghijabcdefghijabcdefghij0 github_pat_abcdefghij0123456789 ",
                "eyJhbGciOiJ.eyJzdWIiOiJ.c2lnbmF0dXJl -----BEGIN RSA PRIVATE KEY----- ",
                "xsk-abcdefghij0123456789x_sk-abcdefghij0123456789 sk-abcdefghijabcdefghijabc"
            ),
            &[],
        );
        assert_eq!(
            said,
            concat!(
                r#"{"apiKey": "<redacted>"} Authorization: Basic <redacted> Bearer <redacted> https://<redacted>@git.example/x "#,
                "<redacted> AKIAABCDEFGHIJKLMNOPQ <redacted> <redacted> ",
                "<redacted> <redacted> ",
                "<redacted> <redacted> ",
                "xsk-abcdefghij0123456789x_<redacted> sk-abcdefghijabcdefghijabc"
            )
        );
    }

    /// Catches: a launch whose start goes unsaid, or a panic after it that leaves nothing in the log.
    #[test]
    fn the_launch_and_a_panic_after_it_are_logged() {
        let home = tempfile::tempdir().unwrap();
        init(&home.path().join("logs"), &Env::default());
        started("0.0.0-test");
        let path = path().expect("the log is open");
        _ = std::panic::catch_unwind(|| panic!("forced for the log"));
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(&format!(" info wsp app 0.0.0-test started (pid {}, ", std::process::id())), "{text}");
        assert!(text.contains(" error main: panicked at ") && text.contains("forced for the log"), "{text}");
    }

    #[test]
    fn drops_a_write_it_cannot_make() {
        let home = tempfile::tempdir().unwrap();
        // A file where the folder should be: nothing can be made under it.
        fs::write(home.path().join("logs"), "").unwrap();
        let log = AppLog::open(&home.path().join("logs"), &Env::default());
        log.write(Level::Error, "nowhere to go");
        assert_eq!(fs::read_to_string(home.path().join("logs")).unwrap(), "");
    }
}
