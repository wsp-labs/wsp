// SPDX-License-Identifier: AGPL-3.0-only
//! What a machine's codex home holds for a thread whose resume Codex refused, as one log line. TS
//! adapter-codex/src/lost-session.ts:1-58. Spec: spec/codex_session_title.md rules 10-11.

use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::js::js_trim;

use super::command::slug;
use super::session_title::NEWEST_STATE_DB;

/// TS lost-session.ts:32.
pub fn session_held_command(home: &str, thread_id: &str) -> Result<String, String> {
    let id = slug("threadId", thread_id)?;
    let row = |columns: &str| shell_quote(&format!("select json_object({columns}) from threads where id = '{id}' limit 1;"));
    let lines = [
        "echo \"boot $(cat /proc/sys/kernel/random/boot_id 2>/dev/null)\"".to_string(),
        format!("cd {} 2>/dev/null || {{ echo nohome; exit 0; }}", shell_quote(home)),
        format!(
            "find sessions archived_sessions -type f -name {} 2>/dev/null | while IFS= read -r f; do echo \"file $(stat -c %s \"$f\" 2>/dev/null || stat -f %z \"$f\") $PWD/$f\"; done",
            shell_quote(&format!("*{id}*"))
        ),
        "echo listed".to_string(),
        "command -v sqlite3 > /dev/null 2>&1 || { echo nosqlite; exit 0; }".to_string(),
        format!("{NEWEST_STATE_DB}; [ -n \"$d\" ] || {{ echo nodb; exit 0; }}"),
        format!(
            "r=$(sqlite3 -readonly \"$d\" {} 2>/dev/null) || r=$(sqlite3 -readonly \"$d\" {} 2>&1) || {{ echo \"unread $d $(printf '%s' \"$r\" | head -n 1 | cut -c 1-160)\"; exit 0; }}",
            row("'path', rollout_path, 'mode', history_mode"),
            row("'path', rollout_path")
        ),
        "[ -n \"$r\" ] || { echo \"norow $d\"; exit 0; }".to_string(),
        "echo \"row $d $r\"".to_string(),
        "p=$(printf '%s' \"$r\" | sed -n 's/.*\"path\":\"\\([^\"]*\\)\".*/\\1/p'); if [ -e \"$p\" ] || [ -e \"$p.zst\" ]; then echo named there; else echo named missing; fi".to_string(),
    ];
    Ok(lines.join("\n"))
}

/// TS lost-session.ts:50.
pub fn parse_session_held(stdout: &str) -> String {
    let lines: Vec<&str> = stdout.split('\n').map(js_trim).collect();
    let boot = lines
        .iter()
        .find(|line| line.starts_with("boot "))
        .map(|line| js_trim(&line[5..]))
        .filter(|boot| !boot.is_empty())
        .unwrap_or("unread");
    let files: Vec<String> = lines
        .iter()
        .filter(|line| line.starts_with("file "))
        .map(|line| {
            let mut parts = line.split(' ').skip(1);
            let size = parts.next().unwrap_or("");
            let path: Vec<&str> = parts.collect();
            format!("{} ({size} bytes)", path.join(" "))
        })
        .collect();
    let said = |word: &str| lines.contains(&word);
    let row = lines.iter().find(|line| line.starts_with("row ") || line.starts_with("norow ") || line.starts_with("unread "));
    let index = if said("nohome") {
        "no Codex home".to_string()
    } else if said("nosqlite") {
        "not read, no sqlite3".to_string()
    } else if said("nodb") {
        "no state db".to_string()
    } else {
        match row {
            None => "not read".to_string(),
            Some(row) if row.starts_with("unread ") => format!("not read from {}", &row[7..]),
            Some(row) if row.starts_with("norow ") => format!("no row in {}", &row[6..]),
            Some(row) => format!("{}, the file it names {}", &row[4..], if said("named there") { "is there" } else { "is missing" }),
        }
    };
    let listed = said("listed") || said("nohome");
    let files = if files.is_empty() { "none".to_string() } else { files.join(", ") };
    format!("boot {boot}; rollout files: {files}{}; index row: {index}", if listed { "" } else { ", the listing cut short" })
}
