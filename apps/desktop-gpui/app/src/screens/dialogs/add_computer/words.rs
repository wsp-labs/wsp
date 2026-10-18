// SPDX-License-Identifier: AGPL-3.0-only
//! Add a computer's words (`ADD_COMPUTER_WORDS` in `settings/format.ts`, `STEP_TITLES` and `stepLine` in
//! `addFlow.ts`) and the protocol's own rules they read: bytes, counts, a recipe's summary.

use serde_json::Value;

use super::model::AddStep;

pub const TITLE: &str = "Add a computer";
pub const WHERE: &str = "A Linux box you have root on.";
pub const REST_DONE: &str = "Everything else is done.";
pub const SKIP_FOR_NOW: &str = "Skip for now";
pub const SKIP: &str = "Skip";
pub const SUGGESTED: &str = "From your ssh config";
pub const INSTALLED: &str = "Installed and connected.";
pub const TAB_OPENED: &str = "A tab opened in your browser.";
pub const NOT_SIGNED_IN_YET: &str = "Not signed in yet.";
pub const SKIPPED: &str = "Skipped.";
pub const TOKEN_COPIED: &str = "Token copied.";
pub const SIGNED_IN_WITH_TOKEN: &str = "Signed in with a token.";
pub const KEYS_STAY: &str = "Skipped until you copy its keys.";
pub const NEEDS_GITHUB: &str = "Private; needs GitHub to clone. Go back to sign in, or skip it.";
pub const NO_REMOTE: &str = "No origin remote, so the add refuses it; push it somewhere first.";
pub const SUDO_FIX: &str = "Type it below; it goes to sudo there and is kept nowhere.";
pub const REFUSED_FIX: &str = "Check the user and the address, or name a key in your ssh config.";
pub const HOST_LOST: &str = "The host lost track of this add, so how it ended is not known; add it again if the computer is not listed.";
pub const COPY_KEYS: &str = "Copy the keys";
pub const PLACEHOLDER: &str = "user@host or an ssh alias";

/// `PLACE_HOST_KEY_KIND`, `PLACE_SUDO_KIND`, `PLACE_LOGIN_REFUSED_KIND`.
pub const HOST_KEY_KIND: &str = "host-key";
pub const SUDO_KIND: &str = "sudo";
pub const LOGIN_REFUSED_KIND: &str = "login";

/// `placeNoPicksLine`: a computer that carries no picks has nothing to go on it until it has some.
pub fn no_picks(name: &str) -> String {
    format!("{name} has nothing picked to go on it: choose in the app's Add a computer, or run wsp add {name} --resume --recipe <name>")
}

pub fn hosts_not_read(said: &str) -> String {
    format!("Hosts from your ssh config not read: {said}")
}

pub fn still_setting_up(name: &str) -> String {
    format!("{name} is still being set up; press again once it is done.")
}

pub fn sign_in_on(name: &str) -> String {
    format!("Sign in on {name}")
}

pub fn signed_in_on(name: &str) -> String {
    if name.is_empty() {
        "Signed in there.".into()
    } else {
        format!("Signed in on {name}.")
    }
}

pub fn at_its_terminal(name: &str) -> String {
    format!("Signs in at its own terminal on {name}.")
}

pub fn waits_for_install(name: &str) -> String {
    format!("Waits for {name} to install.")
}

pub fn paste_once_set_up(word: &str) -> String {
    format!("Signs in with a {word} you paste once it is set up.")
}

pub fn at_its_terminal_once_set_up(name: &str) -> String {
    format!("Signs in at its own terminal on {name} once it is set up.")
}

pub fn saved_recipe(summary: &str, day: Option<&str>) -> String {
    let on = day.map(|d| format!(" on {d}")).unwrap_or_default();
    format!("A recipe you saved{on}. {}.", capitalised(summary))
}

pub fn github_account(account: &str, scopes: &[String]) -> String {
    if scopes.is_empty() {
        format!("Signed in as {account}.")
    } else {
        format!("Signed in as {account} with {}.", list_words(scopes))
    }
}

pub fn copy_keys_to(name: &str) -> String {
    format!("{COPY_KEYS} to {}", if name.is_empty() { "its computers" } else { name })
}

pub fn keys_named(servers: &[(String, Vec<String>)]) -> String {
    format!("{}.", servers.iter().map(|(n, k)| format!("{n}: {}", k.join(", "))).collect::<Vec<_>>().join("; "))
}

pub fn leave_keys(here: &str) -> String {
    format!("Leave them on {here}")
}

pub fn keys_left(names: &[String], name: &str) -> String {
    let one = names.len() == 1;
    format!(
        "{} {} skipped on {} until you copy {}.",
        list_words(names),
        if one { "is" } else { "are" },
        if name.is_empty() { "its computers" } else { name },
        if one { "its keys" } else { "their keys" }
    )
}

/// `keysNotCopied` on a computer that follows no recipe; one that follows a recipe says where the yes is given.
pub fn keys_not_copied(keys: &[String], recipe: Option<&str>) -> String {
    let needs = if keys.is_empty() { "a key".to_owned() } else { list_words(keys) };
    match recipe {
        None => format!("Not copied: it needs {needs}."),
        Some(recipe) => format!(
            "Not copied: it needs {needs}; to send {}, pick {COPY_KEYS} under MCP servers on the recipe {recipe} in Settings > Recipes.",
            if keys.len() == 1 { "it" } else { "them" }
        ),
    }
}

pub fn server_sign_in(kind: &str, name: &str) -> Option<String> {
    Some(match kind {
        "none" => "No sign-in.".into(),
        "key" => "Key copied.".into(),
        "token" => "Token copied.".into(),
        "oauth" if name.is_empty() => "Signs in there.".into(),
        "oauth" => format!("Signs in on {name}."),
        _ => return None,
    })
}

pub fn remote_line(remote: &str, unpushed: Option<u64>) -> String {
    let tail = match unpushed {
        None => String::new(),
        Some(0) => ", clean".into(),
        Some(1) => ", 1 unpushed commit comes along".into(),
        Some(n) => format!(", {n} unpushed commits come along"),
    };
    format!("{remote}{tail}.")
}

pub fn disk_line(needed: &str, kept: &str, free: Option<&str>) -> String {
    format!("{needed} needed, {kept} kept free{}", free.map(|f| format!(", {f} free")).unwrap_or_default())
}

pub fn disk_short(name: &str, free: &str, needed: &str, kept: &str) -> (String, String) {
    (
        format!("{name} has {free} free; these picks need {needed}, and wsp keeps {kept} free there."),
        format!("Untick some rows, or free room on {name}."),
    )
}

pub fn unmeasured(n: u64) -> String {
    format!("{n} picked {} not measured", if n == 1 { "row was" } else { "rows were" })
}

/// `STEP_TITLES`.
pub fn step_title(step: AddStep) -> &'static str {
    match step {
        AddStep::Where | AddStep::Running | AddStep::Ready => TITLE,
        AddStep::Checks => "Checks",
        AddStep::StartFrom => "Start from",
        AddStep::Agents => "Agents",
        AddStep::Mcp => "MCP servers",
        AddStep::Clis => "CLIs",
        AddStep::Skills => "Skills",
        AddStep::Plugins => "Plugins",
        AddStep::Github => "GitHub",
        AddStep::Projects => "Import projects",
        AddStep::Other => "Other config",
        AddStep::Summary => "Summary",
    }
}

/// `stepLine`: what a step does and why, `name` the computer being added and `here` the one the host runs on.
pub fn step_line(step: AddStep, name: &str, here: &str) -> String {
    match step {
        AddStep::Where | AddStep::Running | AddStep::Ready => WHERE.into(),
        AddStep::Checks => format!("Making sure wsp can reach {name} and run there."),
        AddStep::StartFrom => format!("Begin with everything on {here}, a saved recipe, or nothing."),
        AddStep::Agents => format!("The coding agents to install on {name}, signed in so threads run there."),
        AddStep::Mcp => format!("The tool servers your agents use, set up for them on {name}."),
        AddStep::Clis => format!("The command-line tools your agents run, to install on {name}. Most used first."),
        AddStep::Skills => format!("Your agents' skills, copied so they work the same on {name}."),
        AddStep::Plugins => format!("Claude Code plugins to install on {name}."),
        AddStep::Github => format!("How {name} signs in to GitHub to clone and push your repos."),
        AddStep::Projects => format!("Projects on {here} to clone onto {name}."),
        AddStep::Other => format!("Your git and shell settings, so {name} behaves like {here}."),
        AddStep::Summary => format!("What goes on {name}, before it starts."),
    }
}

/// `SETUP_STEP_WORDS`: each setup step as its log line names it.
pub fn setup_step_words(step: &str) -> Option<&'static str> {
    Some(match step {
        "floor" => "the base tools",
        "agents" => "the agents",
        "signins" => "the agents' sign-ins",
        "clis" => "the CLIs",
        "skills" => "the skills",
        "mcp" => "the agents' own files and MCP servers",
        "plugins" => "the plugins",
        "configs" => "git and the shell",
        "folders" => "the folders, as projects",
        "folderServers" => "the projects' own MCP servers",
        "github" => "the GitHub sign-in",
        "context" => "what the agents read about this computer",
        _ => return None,
    })
}

pub fn capitalised(word: &str) -> String {
    let mut c = word.chars();
    c.next().map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
}

/// `listWords`: "A", "A and B", "A, B and C".
pub fn list_words(words: &[String]) -> String {
    match words {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// `plural`: a count with its noun, an s past one.
pub fn plural(n: usize, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
}

/// `fmtCalls`.
pub fn fmt_calls(n: u64) -> String {
    let digits = n.to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!("{grouped} {}", if n == 1 { "call" } else { "calls" })
}

/// The kinds of row a recipe holds, in `RECIPE_KINDS`' order, with the word each is counted by.
const KINDS: [(&str, &str); 7] = [
    ("agents", "agent"),
    ("mcp", "MCP server"),
    ("clis", "CLI"),
    ("skills", "skill"),
    ("plugins", "plugin"),
    ("folders", "folder"),
    ("configs", "config"),
];

/// `recipeCounts`: the rows of picks by kind, configs by the ones ticked.
pub fn recipe_counts(picks: &Value) -> Vec<(&'static str, usize)> {
    KINDS
        .iter()
        .map(|(kind, _)| {
            let n = picks.get(*kind).and_then(Value::as_object).map_or(0, |t| t.values().filter(|v| !v.is_null()).count());
            (*kind, n)
        })
        .collect()
}

/// `recipeSummary`: "2 agents, 3 CLIs, 20 skills", or "nothing picked".
pub fn recipe_summary(picks: &Value) -> String {
    let said: Vec<String> =
        recipe_counts(picks).into_iter().zip(KINDS).filter(|((_, n), _)| *n > 0).map(|((_, n), (_, word))| plural(n, word)).collect();
    if said.is_empty() {
        "nothing picked".into()
    } else {
        said.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_summary_counts_each_kind_once() {
        let picks = json!({ "name": "b", "agents": { "claude": {} }, "clis": { "rg": {}, "jq": {} }, "configs": { "git": {} } });
        assert_eq!(recipe_summary(&picks), "1 agent, 2 CLIs, 1 config");
        assert_eq!(recipe_summary(&json!({ "name": "b" })), "nothing picked");
        assert_eq!(fmt_calls(1234), "1,234 calls");
        assert_eq!(list_words(&["a".into(), "b".into(), "c".into()]), "a, b and c");
    }
}
