// SPDX-License-Identifier: AGPL-3.0-only
//! The one write of an agent's config file on any computer, as a line for that computer's own sh. Port of
//! packages/catalog/src/config-write.ts at a3b870a6e; the spec is spec/mcp_launch.md.
use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::agents_report::{config_changed_refusal, config_hard_link_refusal, config_link_refusal};
use wsp_words::exit::HostError;
use wsp_words::project_path::under_project;

/// The home where the store sits under it, else the store (config-write.ts:341).
pub fn store_base(store: &str, home: &str) -> String {
    if under_project(store, home) {
        home.to_string()
    } else {
        store.to_string()
    }
}

pub const CONFIG_LINK_EXIT: i32 = 4;
pub const CONFIG_CHANGED_EXIT: i32 = 5;
pub const CONFIG_LINKS_EXIT: i32 = 6;

const TEMP: &str = ".wsp-config-tmp.";
/// A temp file older than this belongs to a write that was killed; a live one takes milliseconds.
const STALE_MINUTES: u32 = 10;

/// One write (config-write.ts:358).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigWrite {
    pub file: String,
    pub base: String,
    /// `config_sum` of the file as read; `None` where there was no file.
    pub sum: Option<String>,
    pub bytes: u64,
    /// A landing beside the file the bytes wait in; `None`, stdin.
    pub from: Option<String>,
}

/// `<file's folder>/.wsp-config-tmp.<id>`; errors "a landing's id is twelve hex digits" (config-write.ts:373).
pub fn config_landing(file: &str, id: &str) -> Result<String, HostError> {
    let hex = id.len() == 12 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if !hex {
        return Err(HostError::new("a landing's id is twelve hex digits"));
    }
    let folder = match file.rfind('/') {
        Some(i) => &file[..i],
        None => &file[..file.len().saturating_sub(1)],
    };
    Ok(format!("{folder}/{TEMP}{id}"))
}

/// config-write.ts:380.
pub fn under_base(inside: &str, outside: &str) -> String {
    format!("case $r/ in \"${{b%/}}\"/*) {inside} ;; *) {outside} ;; esac")
}

/// config-write.ts:383.
pub fn inside_base(said: &str) -> String {
    under_base(":", &format!("printf '%s\\000%s\\000' {said} \"$r\"; exit {CONFIG_LINK_EXIT}"))
}

/// A refusal's exit: the file named on stdout, each path ending in a NUL (config-write.ts:371).
fn refuse(code: i32) -> String {
    format!("{{ printf '%s\\000' \"$f\"; exit {code}; }}")
}

/// The sh line (config-write.ts:389).
pub fn config_write_line(w: &ConfigWrite) -> String {
    let q = shell_quote;
    let there = "{ [ -e \"$r\" ] || [ -L \"$r\" ]; }";
    let landing = if w.from.is_some() { " \"$l\"" } else { "" };
    let minted = [
        format!("{TEMP}{}", "[A-Za-z0-9]".repeat(6)),
        format!("{TEMP}{}", "[0-9a-f]".repeat(12)),
        format!("{TEMP}{}.part[0-9]*", "[0-9a-f]".repeat(12)),
    ];
    let names = minted.iter().map(|m| format!("-name '{m}'")).collect::<Vec<_>>().join(" -o ");
    let mut lines: Vec<String> = vec!["umask 077".to_string()];
    if let Some(from) = &w.from {
        lines.push(format!("l={}", q(from)));
        lines.push(format!("trap 'rm -f --{landing}' EXIT"));
    }
    lines.push(format!("b=$(realpath {}) || exit 1", q(&w.base)));
    lines.push(format!("f={}", q(&w.file)));
    lines.push("if [ -e \"$f\" ] || [ -L \"$f\" ]; then".to_string());
    lines.push(format!("  {}", match w.sum {
        None => refuse(CONFIG_CHANGED_EXIT),
        Some(_) => ":".to_string(),
    }));
    lines.push("  r=$(realpath \"$f\" 2>/dev/null) || { printf '%s\\000%s\\000' \"$f\" \"$(readlink \"$f\")\"; exit 4; }".to_string());
    lines.push(format!("  {}", inside_base("\"$f\"")));
    lines.push(format!("  [ -z \"$(find \"$r\" -links +1)\" ] || {}", refuse(CONFIG_LINKS_EXIT)));
    lines.push("else".to_string());
    lines.push(format!("  {}", match w.sum {
        None => ":".to_string(),
        Some(_) => refuse(CONFIG_CHANGED_EXIT),
    }));
    lines.push("  d=${f%/*}; a=$d".to_string());
    lines.push("  while [ -n \"$a\" ] && [ ! -e \"$a\" ]; do a=${a%/*}; done".to_string());
    lines.push("  r=$(realpath \"${a:-/}\") || exit 1".to_string());
    lines.push(format!("  {}", inside_base("\"$a\"")));
    lines.push("  mkdir -p \"$d\" || exit 1".to_string());
    lines.push("  r=$f".to_string());
    lines.push("fi".to_string());
    lines.push(format!(
        "for s in \"${{f%/*}}\" \"${{r%/*}}\"; do LC_ALL=C find \"$s\" -maxdepth 1 -type f \\( {names} \\) -mmin +{STALE_MINUTES} -exec rm -f {{}} + 2>/dev/null; done"
    ));
    lines.push(format!("n=$(mktemp \"${{r%/*}}/{TEMP}XXXXXX\") || exit 1"));
    lines.push(format!("trap 'rm -f -- \"$n\"{landing}' EXIT"));
    lines.push(format!("if {there}; then cp -p \"$r\" \"$n\" || exit 1; fi"));
    lines.push(format!("cat{landing} > \"$n\" || exit 1"));
    lines.push(format!("[ \"$(wc -c < \"$n\" | tr -d ' ')\" = {} ] || exit 1", w.bytes));
    lines.push(match &w.sum {
        None => format!("{there} && {}", refuse(CONFIG_CHANGED_EXIT)),
        Some(sum) => format!("[ \"$(cksum < \"$r\")\" = {} ] || {}", q(sum), refuse(CONFIG_CHANGED_EXIT)),
    });
    lines.push("mv -f \"$n\" \"$r\" || exit 1".to_string());
    lines.join("\n")
}

/// The sentence for a refused write, the path named on stdout up to its NUL first, else `file`; nothing for any other
/// exit (config-write.ts:425).
pub fn config_refusal(exit_code: i32, stdout: &str, file: &str, shown: &dyn Fn(&str) -> String) -> Option<String> {
    let mut parts = stdout.split('\0');
    let named = parts.next().unwrap_or("");
    let to = parts.next().unwrap_or("");
    let at = shown(if named.is_empty() { file } else { named });
    match exit_code {
        CONFIG_LINK_EXIT => Some(config_link_refusal(&at, &shown(to))),
        CONFIG_CHANGED_EXIT => Some(config_changed_refusal(&at)),
        CONFIG_LINKS_EXIT => Some(config_hard_link_refusal(&at)),
        _ => None,
    }
}

/// What POSIX cksum prints: `<crc> <length>` (config-write.ts:441).
pub fn config_sum(bytes: &[u8]) -> String {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, slot) in t.iter_mut().enumerate() {
            let mut c = (i as u32) << 24;
            for _ in 0..8 {
                c = if c & 0x8000_0000 != 0 { (c << 1) ^ 0x04c1_1db7 } else { c << 1 };
            }
            *slot = c;
        }
        t
    });
    let mut crc: u32 = 0;
    let step = |crc: &mut u32, b: u32| *crc = (*crc << 8) ^ table[(((*crc >> 24) ^ b) & 0xff) as usize];
    for &b in bytes {
        step(&mut crc, b as u32);
    }
    let mut n = bytes.len();
    while n > 0 {
        step(&mut crc, (n & 0xff) as u32);
        n /= 256;
    }
    format!("{} {}", !crc, bytes.len())
}
