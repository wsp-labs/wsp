// SPDX-License-Identifier: AGPL-3.0-only
//! What the system package manager has that a person chose themselves: marked manual, of a priority the distro does
//! not need, shipping a command. Read-only: apt-mark, dpkg-query and dpkg's own records. Port of
//! packages/collect/src/detect/apt.ts at a3b870a6e; the unit is apt_tools and its spec is spec/apt_tools.md.
use std::sync::LazyLock;

use futures::StreamExt;
use indexmap::IndexMap;
use regex::Regex;
use wsp_words::exit::HostError;
use wsp_words::js::{js_trim, to_number, JS_WS};

use crate::detect::tools::{argv, Pkg};
use crate::host::{Host, RunOptions};

/// Where dpkg keeps what each package installed (apt.ts:15).
pub const DPKG_INFO: &str = "/var/lib/dpkg/info";

/// The priorities a person's own choice has (apt.ts:19).
pub const CHOSEN: [&str; 2] = ["optional", "extra"];

/// Packages marked manual that nobody chose (apt.ts:23).
pub const NOT_A_CHOICE: [&str; 4] = ["sudo", "unminimize", "lsb-release", "apt-transport-https"];

/// A file directly under /usr/bin or /bin, as a JavaScript regex source (apt.ts:27).
pub const COMMAND_PATH: &str = r"^\/(usr\/)?bin\/[^/]+$";

static DPKG_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"^([^{JS_WS}]+)[{JS_WS}]+([^{JS_WS}]+)[{JS_WS}]*$")).expect("DPKG_LINE compiles"));

static COMMAND: LazyLock<Regex> = LazyLock::new(|| Regex::new(COMMAND_PATH).expect("COMMAND_PATH compiles"));

/// `<package> <field>` lines by package, in first-seen order; a later line for the same package replaces the value
/// and keeps the position, as a JS Map's set does.
pub fn parse_dpkg_fields(text: &str) -> IndexMap<String, String> {
    let mut out = IndexMap::new();
    for line in text.split('\n') {
        let Some(m) = DPKG_LINE.captures(line) else { continue };
        out.insert(m[1].to_string(), m[2].to_string());
    }
    out
}

/// The names `apt-mark showmanual` answers with, in its own order.
pub fn parse_apt_manual(text: &str) -> Vec<String> {
    text.split('\n').map(js_trim).filter(|l| !l.is_empty()).map(str::to_string).collect()
}

async fn file_lists(host: &Host) -> IndexMap<String, String> {
    let mut out = IndexMap::new();
    for name in host.fs.list(DPKG_INFO).await {
        let Some(stem) = name.strip_suffix(".list") else { continue };
        let pkg = stem.split(':').next().unwrap_or("");
        if !pkg.is_empty() {
            out.insert(pkg.to_string(), format!("{DPKG_INFO}/{name}"));
        }
    }
    out
}

async fn ships_command(host: &Host, file: Option<&str>) -> Result<bool, HostError> {
    let Some(file) = file else { return Ok(false) };
    let mut lines = host.fs.lines(file, None);
    while let Some(line) = lines.next().await {
        let line = line.map_err(|e| HostError::new(e.to_string()))?;
        if COMMAND.is_match(js_trim(&line)) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The caller has already found apt-mark on PATH. A list file whose `lines` stream yields an `Err` part way fails the
/// whole read with `HostError::new(err.to_string())`, the TS iterator's throw (coordinator's ruling on #2446).
pub async fn apt_packages(host: &Host) -> Result<Vec<Pkg>, HostError> {
    let manual = parse_apt_manual(&host.exec.run("apt-mark", &argv(&["showmanual"]), RunOptions::default()).await.unwrap_or_default());
    let mine: Vec<String> = manual.into_iter().filter(|n| !NOT_A_CHOICE.contains(&n.as_str())).collect();
    if mine.is_empty() {
        return Ok(Vec::new());
    }
    let priority = parse_dpkg_fields(&host.exec.run("dpkg-query", &argv(&["-Wf", "${Package} ${Priority}\n"]), RunOptions::default()).await.unwrap_or_default());
    let lists = file_lists(host).await;
    let mut out = Vec::new();
    for name in mine {
        let p = priority.get(&name).map_or("", String::as_str);
        if !CHOSEN.contains(&p) {
            continue;
        }
        if ships_command(host, lists.get(&name).map(String::as_str)).await? {
            out.push(Pkg { name, version: None, path: None });
        }
    }
    Ok(out)
}

/// Bytes by package name, in the order of `names`; JavaScript's `Number()` reads the field, so the value is an f64.
pub async fn apt_sizes(host: &Host, names: &[String]) -> IndexMap<String, f64> {
    let mut out = IndexMap::new();
    if names.is_empty() {
        return out;
    }
    let fields = parse_dpkg_fields(&host.exec.run("dpkg-query", &argv(&["-Wf", "${Package} ${Installed-Size}\n"]), RunOptions::default()).await.unwrap_or_default());
    for name in names {
        let kib = fields.get(name).map_or(f64::NAN, |v| to_number(v));
        if kib.is_finite() && kib > 0.0 {
            out.insert(name.clone(), kib * 1024.0);
        }
    }
    out
}
