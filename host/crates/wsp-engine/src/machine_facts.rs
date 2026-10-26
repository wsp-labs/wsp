// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/machine-facts.ts. Spec: spec/machine_facts.md.
//!
//! What a machine that already existed says about itself, read through `key value` lines that one shell's printf
//! prints. The `*_READ` constants are the TS strings byte for byte (tests/fixtures/oracle/machine_facts.json).
use indexmap::IndexMap;
use wsp_words::js::{js_round, js_trim};

use crate::errors::Result;
use crate::machine::ExecResult;

/// machine-facts.ts:14. The last value of each key, keys in first-seen order. A key whose line printed nothing is
/// present with `""`.
pub fn read_values(stdout: &str) -> IndexMap<String, String> {
    read_lists(stdout).into_iter().filter_map(|(key, values)| values.last().map(|last| (key, last.clone()))).collect()
}

/// machine-facts.ts:19. Every value of each key, in order; keys in first-seen order.
pub fn read_lists(stdout: &str) -> IndexMap<String, Vec<String>> {
    let mut lists: IndexMap<String, Vec<String>> = IndexMap::new();
    for line in stdout.split('\n') {
        let Some(space) = line.find(' ').filter(|&i| i > 0) else { continue };
        lists.entry(line[..space].to_string()).or_default().push(js_trim(&line[space + 1..]).to_string());
    }
    lists
}

/// machine-facts.ts:34.
pub const OS_READ: [&str; 3] = [
    r#"printf "pretty %s\n" "$(sed -n 's/^PRETTY_NAME=//p' /etc/os-release 2>/dev/null | head -1 | tr -d '"')""#,
    r#"printf "mac %s\n" "$(sw_vers -productVersion 2>/dev/null)""#,
    r#"printf "kernel %s %s\n" "$(uname -s)" "$(uname -r)""#,
];

/// machine-facts.ts:42.
pub const UPTIME_READ: [&str; 2] = [
    r#"printf "uptime %s\n" "$(cut -d' ' -f1 /proc/uptime 2>/dev/null)""#,
    r#"printf "boot %s\n" "$(sysctl -n kern.boottime 2>/dev/null)""#,
];

/// machine-facts.ts:49.
pub const HOME_READ: &str = r#"printf "home %s\n" "$HOME""#;

/// machine-facts.ts:53.
pub const SYSTEM_READ: &str = r#"printf "system %s\n" "$(uname -s)""#;

/// machine-facts.ts:58.
pub const ARCH_READ: &str = r#"printf "arch %s\n" "$(uname -m)""#;

/// machine-facts.ts:61.
pub const MEM_READ: &str = r#"printf "memkb %s\n" "$(awk '/MemTotal/{print $2}' /proc/meminfo 2>/dev/null || echo $(( $(sysctl -n hw.memsize 2>/dev/null || echo 0) / 1024 )))""#;

/// machine-facts.ts:69.
pub const SHELL_READ: &str = r#"shell=$(getent passwd "$(id -un)" 2>/dev/null | cut -d: -f7); shell=${shell##*/}; shell=${shell:-${SHELL##*/}}; shell=${shell:-bash}"#;

/// machine-facts.ts:73.
pub fn arch_of(values: &IndexMap<String, String>) -> Option<String> {
    non_empty(values, "arch")
}

/// machine-facts.ts:79.
pub fn system_of(values: &IndexMap<String, String>) -> Option<String> {
    non_empty(values, "system")
}

/// machine-facts.ts:85. Whole MB, rounded half up as `Math.round`; `None` for a figure that is not a finite number
/// above 0 under JavaScript's `Number()`.
pub fn mem_mb_of(values: &IndexMap<String, String>) -> Option<u64> {
    let kb = js_number(values.get("memkb").map_or("", String::as_str));
    (kb.is_finite() && kb > 0.0).then(|| js_round(kb / 1024.0) as u64)
}

/// machine-facts.ts:93.
pub fn os_name_of(values: &IndexMap<String, String>) -> Option<String> {
    if let Some(mac) = non_empty(values, "mac") {
        return Some(format!("macOS {mac}"));
    }
    non_empty(values, "pretty").or_else(|| non_empty(values, "kernel"))
}

/// machine-facts.ts:106. `now` is ms since the epoch on this computer's clock. Negative where /proc says so: the TS
/// rounds whatever finite figure it reads.
pub fn uptime_ms_of(values: &IndexMap<String, String>, now: i64) -> Option<i64> {
    if let Some(uptime) = values.get("uptime").filter(|v| !v.is_empty()) {
        let secs = js_number(uptime);
        if secs.is_finite() {
            return Some(js_round(secs * 1000.0) as i64);
        }
    }
    let boot = values.get("boot")?;
    let sec = boot_seconds(boot)?;
    Some((now as f64 - sec * 1000.0).max(0.0) as i64)
}

/// `/sec = (\d+)/` read unanchored: the digits after the first `sec = ` that has one, ASCII digits only.
fn boot_seconds(boot: &str) -> Option<f64> {
    boot.match_indices("sec = ").find_map(|(at, _)| {
        let digits: String = boot[at + "sec = ".len()..].chars().take_while(char::is_ascii_digit).collect();
        (!digits.is_empty()).then(|| js_number(&digits))
    })
}

fn non_empty(values: &IndexMap<String, String>, key: &str) -> Option<String> {
    values.get(key).filter(|v| !v.is_empty()).cloned()
}

/// JavaScript's `Number(s)` on a string: whitespace trimmed, `""` is 0, the `0b`/`0o`/`0x` forms with no sign, a
/// decimal with an optional sign, a point or an exponent, `Infinity`, and NaN for anything else.
fn js_number(s: &str) -> f64 {
    let t = js_trim(s);
    if t.is_empty() {
        return 0.0;
    }
    let radix = match t.get(..2) {
        Some("0b" | "0B") => Some(2),
        Some("0o" | "0O") => Some(8),
        Some("0x" | "0X") => Some(16),
        _ => None,
    };
    if let Some(radix) = radix {
        let digits = &t[2..];
        return match digits.chars().try_fold(0.0f64, |acc, c| c.to_digit(radix).map(|d| acc * f64::from(radix) + f64::from(d))) {
            Some(v) if !digits.is_empty() => v,
            _ => f64::NAN,
        };
    }
    let (sign, body) = match t.as_bytes()[0] {
        b'+' => (1.0, &t[1..]),
        b'-' => (-1.0, &t[1..]),
        _ => (1.0, t),
    };
    if body == "Infinity" {
        return sign * f64::INFINITY;
    }
    if !is_decimal_literal(body) {
        return f64::NAN;
    }
    body.parse::<f64>().map_or(f64::NAN, |v| sign * v)
}

/// The decimal literal grammar: digits with an optional point and fraction, or a point and digits, then an optional
/// exponent. Rust's parser is only asked once this has matched, so `inf`, `nan` and underscores never reach it.
fn is_decimal_literal(s: &str) -> bool {
    let (mantissa, exponent) = match s.find(['e', 'E']) {
        Some(at) => (&s[..at], Some(&s[at + 1..])),
        None => (s, None),
    };
    let (int, frac) = match mantissa.split_once('.') {
        Some((int, frac)) => (int, Some(frac)),
        None => (mantissa, None),
    };
    let digits = |d: &str| d.bytes().all(|b| b.is_ascii_digit());
    let mantissa_ok = (!int.is_empty() || frac.is_some_and(|f| !f.is_empty())) && digits(int) && frac.is_none_or(digits);
    let exponent_ok = exponent.is_none_or(|e| {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        !e.is_empty() && digits(e)
    });
    mantissa_ok && exponent_ok
}

/// machine-facts.ts:115. `None` where the exec failed, exited non-zero, or the answer names no system.
pub async fn read_os_name<F, Fut>(exec: F) -> Option<String>
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = Result<ExecResult>>,
{
    let res = exec(OS_READ.join("\n")).await.ok()?;
    (res.exit_code.get() == 0).then(|| os_name_of(&read_values(&res.stdout))).flatten()
}
