// SPDX-License-Identifier: AGPL-3.0-only
//! The picks a person makes for a computer, as a recipe file (`settings/add/choices.ts`): everything the computer
//! running the host has, nothing, or one saved recipe; and one tick or one choice moved on them. Picks and options are
//! the host's JSON, `RecipeFile` and `RecipeOptions`.

use serde_json::{json, Map, Value};

use super::model::{github_pick, str_of};

/// The name a computer's picks are filed under.
fn file_name(name: &str) -> &str {
    if name.trim().is_empty() {
        "computer"
    } else {
        name
    }
}

fn list<'a>(options: &'a Value, key: &str) -> &'a [Value] {
    options.get(key).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

/// `noPicks`: nothing ticked, and GitHub signing in on the computer.
pub fn no_picks(name: &str) -> Value {
    json!({ "name": file_name(name), "agents": {}, "mcp": {}, "clis": {}, "skills": {}, "plugins": {}, "folders": {}, "configs": { "github": { "signin": "machine" } } })
}

/// `folderKey`: the project's own name, lower case, every other run a dash.
pub fn folder_key(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            if dash && !out.is_empty() {
                out.push('-');
            }
            out.push(c);
            dash = false;
        } else {
            dash = true;
        }
    }
    if out.is_empty() {
        "folder".into()
    } else {
        out
    }
}

/// A project of this computer as a recipe's folder row, as `folderOf` writes it.
pub fn folder_of(project: &Value, look: Option<&Value>) -> Value {
    let mut row =
        json!({ "from": str_of(project, "path").unwrap_or_default(), "name": str_of(project, "name").unwrap_or_default(), "keep": [] });
    for key in ["icon", "hue", "image"] {
        if let Some(v) = look.and_then(|l| l.get(key)).filter(|v| !v.is_null()) {
            row[key] = v.clone();
        }
    }
    row
}

/// GitHub's row where the host offers it: signing in on the computer where that is one of its ways, else its first.
fn github_of(signins: Option<&Value>) -> Value {
    let ways: Vec<&str> = signins.and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    match ways.iter().find(|w| **w == "machine").or(ways.first()) {
        Some(way) => json!({ "signin": way }),
        None => json!({}),
    }
}

/// `everything`: every row the options offer, each agent signing in the first way it can, GitHub on the computer, and
/// every project here. No CLI, and no plugin switched off here.
pub fn everything(name: &str, options: &Value, projects: &[Value], looks: &Value) -> Value {
    let mut picks = no_picks(name);
    picks["configs"] = json!({});
    for a in list(options, "agents") {
        let way = a.get("signins").and_then(|s| s.get(0)).cloned();
        picks["agents"][str_of(a, "id").unwrap_or_default()] = way.map_or(json!({}), |w| json!({ "signin": w }));
    }
    for s in list(options, "mcp") {
        picks["mcp"][str_of(s, "name").unwrap_or_default()] = json!({ "agents": s.get("agents").cloned().unwrap_or(json!([])) });
    }
    for s in list(options, "skills") {
        picks["skills"][str_of(s, "name").unwrap_or_default()] = json!({ "from": s.get("from").cloned().unwrap_or(json!("")) });
    }
    for p in list(options, "plugins").iter().filter(|p| p.get("on") == Some(&json!(true))) {
        picks["plugins"][str_of(p, "name").unwrap_or_default()] = json!({});
    }
    for p in projects {
        let look = str_of(p, "id").and_then(|id| looks.get(id));
        picks["folders"][folder_key(str_of(p, "name").unwrap_or_default())] = folder_of(p, look);
    }
    for c in list(options, "configs") {
        let id = str_of(c, "id").unwrap_or_default();
        picks["configs"][id] = if id == "github" { github_of(c.get("signins")) } else { json!({}) };
    }
    picks
}

/// `fromRecipe`: a saved recipe as the picks for this computer, under its name.
pub fn from_recipe(file: &Value, name: &str) -> Value {
    let mut picks = file.clone();
    picks["name"] = json!(name);
    picks
}

/// `keyedServers`: the ticked servers that carry a key, with what their keys go by.
pub fn keyed_servers(picks: &Value, options: &Value) -> Vec<(String, Vec<String>)> {
    list(options, "mcp")
        .iter()
        .filter_map(|s| {
            let name = str_of(s, "name")?;
            let keys: Vec<String> = s.get("keys")?.as_array()?.iter().filter_map(Value::as_str).map(str::to_owned).collect();
            (picks.pointer(&format!("/mcp/{}", escape(name))).is_some() && !keys.is_empty()).then(|| (name.to_owned(), keys))
        })
        .collect()
}

/// A key as one step of a JSON pointer.
fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// Whether the picks hold a row of `kind` by `key`.
pub fn has(picks: &Value, kind: &str, key: &str) -> bool {
    picks.get(kind).and_then(Value::as_object).is_some_and(|t| t.contains_key(key))
}

pub fn copies_keys(picks: &Value) -> bool {
    picks.get("copyKeys") == Some(&json!(true))
}

pub fn set_copy_keys(picks: &Value, on: bool) -> Value {
    let mut next = picks.clone();
    if let Some(o) = next.as_object_mut() {
        o.remove("copyKeys");
        if on {
            o.insert("copyKeys".into(), json!(true));
        }
    }
    next
}

fn table<'a>(picks: &'a mut Value, kind: &str) -> &'a mut Map<String, Value> {
    if !picks.get(kind).is_some_and(Value::is_object) {
        picks[kind] = json!({});
    }
    picks[kind].as_object_mut().expect("a table")
}

/// `tick`: one row ticked takes the row the options give for it; unticked takes it out.
pub fn tick(picks: &Value, kind: &str, name: &str, on: bool, options: &Value) -> Value {
    let mut next = picks.clone();
    let find = |key: &str, field: &str| list(options, key).iter().find(|r| str_of(r, field) == Some(name)).cloned();
    let row = match kind {
        "agents" => find("agents", "id").and_then(|a| a.get("signins")?.get(0).cloned()).map_or(json!({}), |w| json!({ "signin": w })),
        "mcp" => json!({ "agents": find("mcp", "name").and_then(|s| s.get("agents").cloned()).unwrap_or(json!([])) }),
        "clis" => {
            let cli = find("clis", "name");
            let mut row = json!({ "via": cli.as_ref().and_then(|c| str_of(c, "via")).unwrap_or("apt") });
            if let Some(needs) = cli.as_ref().and_then(|c| c.get("needs")).filter(|n| !n.is_null()) {
                row["needs"] = needs.clone();
            }
            row
        }
        "skills" => json!({ "from": find("skills", "name").and_then(|s| s.get("from").cloned()).unwrap_or(json!("")) }),
        _ => json!({}),
    };
    let t = table(&mut next, kind);
    if on {
        t.insert(name.to_owned(), row);
    } else {
        t.remove(name);
    }
    next
}

/// `tickMany`: several rows at once, each as one tick moves it.
pub fn tick_many(picks: &Value, kind: &str, changes: &[(impl AsRef<str>, bool)], options: &Value) -> Value {
    changes.iter().fold(picks.clone(), |next, (name, on)| tick(&next, kind, name.as_ref(), *on, options))
}

/// `tickUsedClis`: every CLI the agents ran at least once ticked, on top of the picks.
pub fn tick_used_clis(picks: &Value, options: &Value) -> Value {
    let used: Vec<(String, bool)> = list(options, "clis")
        .iter()
        .filter(|c| c.get("calls").and_then(Value::as_u64).unwrap_or(0) > 0)
        .filter_map(|c| Some((str_of(c, "name")?.to_owned(), true)))
        .collect();
    tick_many(picks, "clis", &used, options)
}

/// `tickFolder`: a folder ticked with its row, or taken out.
pub fn tick_folder(picks: &Value, key: &str, row: Option<Value>) -> Value {
    let mut next = picks.clone();
    let t = table(&mut next, "folders");
    match row {
        Some(row) => {
            t.insert(key.to_owned(), row);
        }
        None => {
            t.remove(key);
        }
    }
    next
}

/// `signIn`: how one agent signs in.
pub fn sign_in(picks: &Value, agent: &str, way: &str) -> Value {
    let mut next = picks.clone();
    table(&mut next, "agents").insert(agent.to_owned(), json!({ "signin": way }));
    next
}

/// `setGitHub`.
pub fn set_github(picks: &Value, pick: &str) -> Value {
    let mut next = picks.clone();
    table(&mut next, "configs").insert("github".into(), json!({ "signin": pick }));
    next
}

/// `tickConfig`: git or the shell ticked or not.
pub fn tick_config(picks: &Value, id: &str, on: bool) -> Value {
    let mut next = picks.clone();
    let t = table(&mut next, "configs");
    if on {
        t.insert(id.to_owned(), json!({}));
    } else {
        t.remove(id);
    }
    next
}

/// `servable`: every sign-in moved onto one the options offer; picks that need no move come back as they were.
pub fn servable(picks: &Value, options: &Value) -> Value {
    let mut next = picks.clone();
    if let Some(agents) = picks.get("agents").and_then(Value::as_object) {
        for (agent, row) in agents {
            let offered = list(options, "agents")
                .iter()
                .find(|a| str_of(a, "id") == Some(agent))
                .and_then(|a| a.get("signins"))
                .and_then(Value::as_array);
            let (Some(offered), Some(way)) = (offered, row.get("signin")) else { continue };
            if offered.contains(way) {
                continue;
            }
            table(&mut next, "agents").insert(agent.clone(), offered.first().map_or(json!({}), |w| json!({ "signin": w })));
        }
    }
    let github = list(options, "configs")
        .iter()
        .find(|c| str_of(c, "id") == Some("github"))
        .and_then(|c| c.get("signins"))
        .and_then(Value::as_array);
    if let Some(ways) = github {
        let held = picks.pointer("/configs/github").is_some_and(|v| !v.is_null());
        if held && !ways.iter().any(|w| w.as_str() == Some(github_pick(picks))) {
            if let Some(first) = ways.first().and_then(Value::as_str) {
                next = set_github(&next, first);
            }
        }
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Value {
        json!({
            "agents": [{ "id": "claude", "name": "Claude Code", "signins": ["vault", "machine"] }],
            "mcp": [{ "name": "linear", "agents": ["claude"], "keys": ["LINEAR_KEY"] }],
            "clis": [{ "name": "rg", "via": "apt", "calls": 40 }, { "name": "jq", "via": "apt" }],
            "skills": [{ "name": "review", "from": "~/.claude/skills/review", "linked": false }],
            "plugins": [{ "name": "a@m", "on": true }, { "name": "b@m" }],
            "configs": [{ "id": "git", "label": "~/.gitconfig" }, { "id": "github", "label": "gh", "signins": ["vault", "machine", "skip"] }]
        })
    }

    #[test]
    fn everything_takes_every_row_but_the_clis_and_the_plugins_left_off() {
        let p = everything("box", &options(), &[json!({ "id": "p1", "name": "My App", "path": "/src/app" })], &json!({}));
        assert_eq!(p["agents"]["claude"]["signin"], "vault");
        assert_eq!(p["clis"], json!({}));
        assert!(has(&p, "plugins", "a@m") && !has(&p, "plugins", "b@m"));
        assert_eq!(p["folders"]["my-app"]["from"], "/src/app");
        assert_eq!(github_pick(&p), "machine");
        assert_eq!(keyed_servers(&p, &options()), vec![("linear".to_owned(), vec!["LINEAR_KEY".to_owned()])]);
    }

    #[test]
    fn ticks_move_one_row_and_servable_moves_a_way_nobody_offers() {
        let p = tick_used_clis(&no_picks("box"), &options());
        assert!(has(&p, "clis", "rg") && !has(&p, "clis", "jq"));
        let p = tick(&p, "clis", "rg", false, &options());
        assert!(!has(&p, "clis", "rg"));
        let p = sign_in(&p, "claude", "token");
        assert_eq!(servable(&p, &options())["agents"]["claude"]["signin"], "vault");
        assert_eq!(folder_key("  My--App 2 "), "my-app-2");
    }
}
