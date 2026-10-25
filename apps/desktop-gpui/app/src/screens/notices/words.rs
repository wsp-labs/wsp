// SPDX-License-Identifier: AGPL-3.0-only
//! The sentences a notice says, in the protocol's words (`packages/protocol/src`): each function names the one it
//! ports, so the two can be read side by side.

use serde_json::Value;

use crate::ui::fmt_bytes;

pub const OPEN: &str = "Open";
pub const UNDO: &str = "Undo";
pub const A_THREAD: &str = "A thread";
/// `NOTIFY_ME`: a line the runtime sends the person rather than another thread.
pub const NOTIFY_ME: &str = "me";
/// `NEEDS_YOU_MARK`: what a window title leads with while a need stands.
pub const NEEDS_YOU_MARK: &str = "\u{2022} ";

fn field<'a>(v: &'a Value, name: &str) -> Option<&'a str> {
    v.get(name).and_then(Value::as_str)
}

/// `toolField`: a field holding text that is not blank.
fn tool_field<'a>(v: &'a Value, name: &str) -> Option<&'a str> {
    field(v, name).filter(|s| !s.trim().is_empty())
}

pub fn not_added(said: &str) -> String {
    format!("Computer not added: {said}")
}

pub fn joined(name: &str) -> String {
    format!("{name} joined")
}

pub fn not_set_up(name: &str, said: &str) -> String {
    format!("{name} not set up: {said}")
}

pub fn thread_stopped(title: &str, said: Option<&str>) -> String {
    match said {
        Some(said) => format!("{title} stopped before it replied: {said}"),
        None => format!("{title} stopped before it replied"),
    }
}

pub fn gone(name: &str, reason: &str) -> String {
    format!("{name} is gone: {reason}")
}

pub fn image_not_built(said: Option<&str>) -> String {
    match said {
        Some(said) => format!("The image was not built: {said}"),
        None => "The image was not built".to_owned(),
    }
}

pub fn image_sealed(version: Option<u64>) -> String {
    match version {
        Some(v) => format!("Image v{v} sealed"),
        None => "Image sealed".to_owned(),
    }
}

/// `exitLine`.
pub fn exit_line(code: Option<i64>) -> Option<String> {
    code.map(|c| format!("exit {c}"))
}

/// `initNeedsYouLine`.
pub fn init_needs_you(what: &str) -> String {
    format!("wsp needs you: {what}")
}

/// `setupNeedsYouLine`.
pub fn setup_needs_you(computer: &str, what: &str) -> String {
    format!("{computer} needs you: {what}")
}

/// `setupReadyLine`.
pub fn setup_ready(computer: &str) -> String {
    format!("{computer} is ready")
}

/// `ADD_COMPUTER_WORDS.ready`, how a setup that ended whole is said.
pub fn computer_ready(computer: &str) -> String {
    format!("{computer} is ready: threads you start on it run there")
}

/// `waitLine`: a sign-in a setup waits on.
pub fn wait_line(wait: &Value) -> String {
    let label = field(wait, "label").unwrap_or_default();
    if field(wait, "state") == Some("expired") {
        return format!("{label}'s sign-in ran out; a retry asks for a fresh code");
    }
    let at = field(wait, "url").map(|u| format!(" at {u}")).unwrap_or_default();
    let code = field(wait, "code").map(|c| format!(" with code {c}")).unwrap_or_default();
    format!("{label} waits on you to sign in{at}{code}")
}

/// `titleWithNeed`: the title with the mark while a need stands, never doubled.
pub fn title_with_need(title: &str, needed: bool) -> String {
    let plain = title.strip_prefix(NEEDS_YOU_MARK).unwrap_or(title);
    if needed {
        format!("{NEEDS_YOU_MARK}{plain}")
    } else {
        plain.to_owned()
    }
}

/// `planAlertLine`.
pub fn plan_alert(label: &str, alert: &Value) -> String {
    match field(alert, "kind") {
        Some("low") => {
            let window = match field(alert, "window").unwrap_or_default() {
                "session" => "session",
                "week" => "week",
                "week_opus" => "Opus week",
                "week_sonnet" => "Sonnet week",
                "month" => "month",
                "overage" => "extra usage",
                other => other,
            };
            let step = alert.get("step").and_then(Value::as_f64).unwrap_or_default();
            format!("{label} has used {step}% of its {window}")
        }
        Some("blocked") => format!("{label} reached its plan limit"),
        _ => format!("{label} can run again: its plan limit reset"),
    }
}

/// `folderName`.
fn folder_name(path: &str) -> &str {
    let path = path.trim_end_matches('/');
    path.rsplit('/').next().unwrap_or(path)
}

/// `parentFolderName`.
fn parent_folder_name(path: &str) -> &str {
    let parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    if parts.len() < 2 {
        ""
    } else {
        parts[parts.len() - 2]
    }
}

/// `fileInFolder`.
fn file_in_folder(path: &str) -> String {
    match parent_folder_name(path) {
        "" => folder_name(path).to_owned(),
        folder => format!("{} in {folder}", folder_name(path)),
    }
}

/// `serverToolWords`: a tool a server lends, `mcp__<server>__<tool>`.
fn server_tool_words(tool: &str) -> Option<String> {
    let parts: Vec<&str> = tool.split("__").collect();
    (parts.len() >= 3 && parts[0] == "mcp" && !parts[1].is_empty())
        .then(|| format!("Use {}'s {}", parts[1], parts[2..].join("__").replace('_', " ")))
}

/// `askingLine`: the lead a permission prompt is said by, one line.
pub fn asking_line(tool: &str, input: &str, detail: Option<&str>) -> String {
    let fields: Option<Value> = serde_json::from_str::<Value>(input).ok().filter(Value::is_object);
    let lead = fields.as_ref().and_then(|f| ask_lead(tool, f));
    lead.unwrap_or_else(|| match detail.filter(|d| !d.is_empty()) {
        Some(detail) => format!("Permission for {tool}: {detail}"),
        None => format!("Permission for {tool}"),
    })
}

fn ask_lead(tool: &str, input: &Value) -> Option<String> {
    let command = || tool_field(input, "command").map(|c| format!("Run a command: {c}"));
    match tool {
        "Write" => {
            let path = tool_field(input, "file_path")?;
            let size = field(input, "content").map(|c| format!(" ({})", fmt_bytes(c.len() as u64))).unwrap_or_default();
            Some(format!("Write {}{size}", file_in_folder(path)))
        }
        "Edit" | "MultiEdit" => {
            let path = tool_field(input, "file_path")?;
            let edits = input.get("edits").and_then(Value::as_array).map_or(0, Vec::len);
            let count = if edits > 1 { format!(" ({edits} places)") } else { String::new() };
            Some(format!("Edit {}{count}", file_in_folder(path)))
        }
        "Bash" | "command_execution" => command(),
        "WebFetch" => tool_field(input, "url").map(|u| format!("Fetch a page: {u}")),
        "Skill" => tool_field(input, "skill").map(|s| format!("Run the skill {s}")),
        "AskUserQuestion" => input.get("questions")?.as_array()?.iter().find_map(|q| tool_field(q, "question")).map(str::to_owned),
        "file_change" => {
            let paths: Vec<&str> = input.get("changes")?.as_array()?.iter().filter_map(|c| tool_field(c, "path")).collect();
            match paths.as_slice() {
                [] => None,
                [one] => Some(format!("Change {}", file_in_folder(one))),
                many => {
                    Some(format!("Change {} files: {}", many.len(), many.iter().map(|p| folder_name(p)).collect::<Vec<_>>().join(", ")))
                }
            }
        }
        other => server_tool_words(other),
    }
}

/// `listWords`.
fn list_words(words: &[String]) -> String {
    match words {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// `settleSaying`'s sentence: what a settle took, what it left and why, and any process its stop left running; and
/// the threads it took, which Undo restores.
pub fn settled(reply: &Value) -> (String, Vec<String>, bool) {
    let rows = |name: &str| reply.get(name).and_then(Value::as_array).cloned().unwrap_or_default();
    let moved: Vec<String> = rows("settled").iter().filter_map(|t| field(t, "threadId")).map(str::to_owned).collect();
    let left = rows("left");
    let mut whys: Vec<&str> = Vec::new();
    for why in left.iter().filter_map(|t| field(t, "why")) {
        if !whys.contains(&why) {
            whys.push(why);
        }
    }
    let threads = |n: usize| if n == 1 { "thread" } else { "threads" };
    let mut parts: Vec<String> = Vec::new();
    if !moved.is_empty() {
        parts.push(format!("Settled {} {}", moved.len(), threads(moved.len())));
    }
    for why in whys {
        let n = left.iter().filter(|t| field(t, "why") == Some(why)).count();
        parts.push(format!("Left {n} {}: {why}", threads(n)));
    }
    // `standingAfterStop`, each named by its pid and its program.
    let standing: Vec<String> = rows("ended")
        .iter()
        .flat_map(|e| {
            let left: Vec<i64> = e.get("left").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_i64).collect();
            e.get("ended")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(move |p| p.get("pid").and_then(Value::as_i64).is_some_and(|pid| left.contains(&pid)))
        })
        .map(|p| format!("{} {}", p.get("pid").and_then(Value::as_i64).unwrap_or_default(), proc_name(&p)))
        .collect();
    let mut text = parts.join(". ");
    if !standing.is_empty() {
        text.push_str(&format!(". Still running after Stop: {}.", list_words(&standing)));
    }
    (text, moved, standing.is_empty())
}

/// A process by its program's name, the first word of `procName`.
fn proc_name(p: &Value) -> String {
    let comm = field(p, "name").unwrap_or_default();
    if !comm.is_empty() {
        return comm.to_owned();
    }
    let bin = field(p, "command").and_then(|c| c.split_whitespace().next()).unwrap_or_default();
    folder_name(bin).to_owned()
}

/// `GET_THE_APP_WORD`: the word on the version notice's one button, which opens the releases page.
pub const GET_THE_APP: &str = "Get";
/// `RELEASES` (`packages/protocol/src/bundles.mjs`).
pub const RELEASES: &str = "https://github.com/wsp-labs/wsp/releases";

/// `ShellVersionNotice`: the one line, and whether it points at a newer app to get.
#[derive(Debug, PartialEq)]
pub struct VersionNotice {
    pub line: String,
    pub update: bool,
}

/// `shellVersionNotice`: an app and the host it attached to of two releases, the older half named with what to do about
/// it, and nothing while the two agree. An app with no release is older than any host that reads this.
pub fn version_notice(app: Option<&str>, host: &str, label: Option<&str>) -> Option<VersionNotice> {
    let named = label.map_or_else(|| "the host".to_owned(), |label| format!("the host {label}"));
    let Some(app) = app else {
        return Some(VersionNotice { line: format!("this app is older than {named}, which is {host}: get the new app"), update: true });
    };
    let both = format!("this app is {app}, {named} is {host}");
    match wsp_host_client::release_order(app, host) {
        std::cmp::Ordering::Equal => None,
        std::cmp::Ordering::Less => Some(VersionNotice { line: format!("{both}: get the new app"), update: true }),
        std::cmp::Ordering::Greater => Some(VersionNotice { line: format!("{both}: run the app's own host"), update: false }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_prompt_reads_by_its_kind() {
        assert_eq!(asking_line("Bash", r#"{"command":"ls -la"}"#, None), "Run a command: ls -la");
        assert_eq!(asking_line("Edit", r#"{"file_path":"/a/src/health.ts","edits":[1,2]}"#, None), "Edit health.ts in src (2 places)");
        assert_eq!(asking_line("Write", r#"{"file_path":"x.md","content":"hello"}"#, None), "Write x.md (5 B)");
        assert_eq!(asking_line("mcp__wsp__run_thread", "{}", None), "Use wsp's run thread");
        assert_eq!(asking_line("Odd", "not json", Some("why")), "Permission for Odd: why");
    }

    #[test]
    fn the_mark_never_doubles() {
        let marked = title_with_need("wsp", true);
        assert_eq!(marked, "\u{2022} wsp");
        assert_eq!(title_with_need(&marked, true), marked);
        assert_eq!(title_with_need(&marked, false), "wsp");
    }

    #[test]
    fn a_settle_says_what_it_took_left_and_kept_running() {
        let reply = json!({
            "settled": [{ "threadId": "a", "title": "A" }],
            "left": [{ "threadId": "b", "why": "it runs" }, { "threadId": "c", "why": "it runs" }],
            "ended": [{ "pid": 1, "ended": [{ "pid": 7, "name": "node", "command": "node x" }], "forced": [], "left": [7] }],
        });
        let (text, moved, clean) = settled(&reply);
        assert_eq!(text, "Settled 1 thread. Left 2 threads: it runs. Still running after Stop: 7 node.");
        assert_eq!(moved, ["a"]);
        assert!(!clean);
    }

    #[test]
    fn plan_alerts_name_the_window() {
        assert_eq!(plan_alert("Max", &json!({ "kind": "low", "window": "week_opus", "step": 90 })), "Max has used 90% of its Opus week");
        assert_eq!(plan_alert("Max", &json!({ "kind": "blocked" })), "Max reached its plan limit");
    }

    fn notice(line: &str, update: bool) -> Option<VersionNotice> {
        Some(VersionNotice { line: line.to_owned(), update })
    }

    #[test]
    fn says_nothing_while_the_app_and_host_are_one_release() {
        assert_eq!(version_notice(Some("0.1.5"), "0.1.5", None), None);
    }

    #[test]
    fn names_both_numbers_and_points_at_the_new_app_when_the_app_is_the_older_half() {
        assert_eq!(version_notice(Some("0.1.3"), "0.1.5", None), notice("this app is 0.1.3, the host is 0.1.5: get the new app", true));
    }

    #[test]
    fn names_both_numbers_and_asks_for_the_apps_own_host_when_the_host_is_the_older_half() {
        assert_eq!(
            version_notice(Some("0.1.5"), "0.1.3", None),
            notice("this app is 0.1.5, the host is 0.1.3: run the app's own host", false)
        );
    }

    #[test]
    fn reads_an_app_with_no_release_as_older_than_the_host() {
        assert_eq!(version_notice(None, "0.1.5", None), notice("this app is older than the host, which is 0.1.5: get the new app", true));
    }

    #[test]
    fn names_a_host_elsewhere_by_its_alias() {
        assert_eq!(
            version_notice(Some("0.1.3"), "0.1.5", Some("studio")),
            notice("this app is 0.1.3, the host studio is 0.1.5: get the new app", true)
        );
        assert_eq!(
            version_notice(None, "0.1.5", Some("studio")),
            notice("this app is older than the host studio, which is 0.1.5: get the new app", true)
        );
    }

    #[test]
    fn reads_the_numbers_as_numbers() {
        assert_eq!(version_notice(Some("0.1.10"), "0.1.9", None).map(|n| n.update), Some(false));
        assert_eq!(version_notice(Some("0.1.9"), "0.1.10", None).map(|n| n.update), Some(true));
    }

    #[test]
    fn puts_a_prerelease_before_its_release_and_a_shorter_number_before_a_longer_one_that_grows() {
        assert_eq!(version_notice(Some("1.0.0-rc.1"), "1.0.0", None).map(|n| n.update), Some(true));
        assert_eq!(version_notice(Some("1.0.0"), "1.0.0-rc.1", None).map(|n| n.update), Some(false));
        assert_eq!(version_notice(Some("0.2"), "0.2.1", None).map(|n| n.update), Some(true));
        assert_eq!(version_notice(Some("0.2"), "0.2.0", None), None);
    }

    #[test]
    fn tells_two_prereleases_of_one_release_apart() {
        assert_eq!(
            version_notice(Some("0.1.6-alpha"), "0.1.6-beta", None),
            notice("this app is 0.1.6-alpha, the host is 0.1.6-beta: get the new app", true)
        );
        assert_eq!(version_notice(Some("0.1.6-beta"), "0.1.6-alpha", None).map(|n| n.update), Some(false));
        assert_eq!(version_notice(Some("0.1.6-rc1"), "0.1.6-rc2", None).map(|n| n.update), Some(true));
        assert_eq!(version_notice(Some("0.1.6-rc2"), "0.1.6-rc1", None).map(|n| n.update), Some(false));
        assert_eq!(version_notice(Some("0.1.6-rc1"), "0.1.6", None).map(|n| n.update), Some(true));
        assert_eq!(version_notice(Some("0.1.6-rc.2"), "0.1.6-rc.10", None).map(|n| n.update), Some(true));
        assert_eq!(version_notice(Some("0.1.6+a1b2c3d"), "0.1.6+9f8e7d6", None), None);
    }
}
