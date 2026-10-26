// SPDX-License-Identifier: AGPL-3.0-only
//! The hand-written tests of `agents_set` (spec/agents-set.md, "Rule -> test"): the rules that are about what is
//! sent and what is not, in what order, held whole; the host invariants this unit holds; the TS tests of
//! `skills-sh.test.ts`, `agents-cloud.test.ts` and `contract.test.ts` that reach these helpers. The recorded cases
//! (`agents_set.rs`) pin the same helpers value by value; these name the rules. Each one runs over the scripted host
//! of `common.rs`, with the replies of a recorded case as the host's answers.
use std::future::Future;
use std::sync::{Arc, Mutex};

use indexmap::IndexMap;
use wsp_cli::verbs::agents_help::read::ProjectAsked;
use wsp_cli::verbs::agents_help::set as s;
use wsp_wire::types::agents_report::{SkillAdded, SkillHit, SkillPreview};
use wsp_words::exit::{HostError, Kind};

use super::common::{case, Script, ScriptedHost, Socket};

const UNIT: &str = "agents_set";

/// The replies a recorded case scripted, by op.
fn replies(func: &str, name: &str) -> IndexMap<String, String> {
    case(UNIT, func, name, "/cwd").script.replies
}

/// A client over those replies, and the log of the frames it is sent.
fn client(replies: IndexMap<String, String>) -> (Socket, Arc<Mutex<Vec<String>>>) {
    let log = Arc::new(Mutex::new(Vec::new()));
    (Socket(ScriptedHost::new(Script { replies, ..Script::default() }, log.clone(), Arc::new(Mutex::new(Vec::new())))), log)
}

fn block<T>(f: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime").block_on(f)
}

fn sent(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    log.lock().unwrap().clone()
}

fn strs(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

fn no_project() -> ProjectAsked {
    ProjectAsked { project: false, name: None }
}

fn usage_refusal(e: &HostError) {
    assert_eq!(e.kind, Some(Kind::Usage), "a refusal of the line is kind usage: {}", e.message);
    assert!(e.fix.is_some(), "a refusal of the line carries its fix: {}", e.message);
}

fn has_control(text: &str) -> bool {
    text.chars().any(|c| matches!(c, '\u{0}'..='\u{8}' | '\u{b}'..='\u{1f}' | '\u{7f}'..='\u{9f}'))
}

fn hit(name: &str, id: &str) -> SkillHit {
    serde_json::from_value(serde_json::json!({ "id": id, "source": "a/b", "skillId": "c", "name": name, "installs": 1 })).expect("a hit")
}

fn preview(text: &str, size: u64) -> SkillPreview {
    serde_json::from_value(serde_json::json!({ "text": text, "size": size })).expect("a preview")
}

// ---------------------------------------------------------------------------------------------------------------
// Host invariant 7: with the cloud off no page, tool or skill line names a cloud. This unit reads no CLOUD_ON of
// its own; the words are the same under both settings (the recorded cloud-on twins of its cases hold that), and
// none of them names a cloud.
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn invariant_7_no_word_or_sentence_of_this_unit_names_a_cloud() {
    let mut said: Vec<String> = vec![
        s::NO_SKILL_HITS_LINE.into(),
        s::AGENT_SET_NOTHING_LINE.into(),
        s::NOTHING_TO_SET_FIX.into(),
        s::ENV_NAME_FIX.into(),
        s::AGENT_SETUP_NOTHING_LINE.into(),
        s::PROJECT_SET_NOTHING_LINE.into(),
        s::TRACKER_SHAPE_FIX.into(),
        s::AFTER_WORKTREE_BLANK_LINE.into(),
        s::AFTER_WORKTREE_BLANK_FIX.into(),
        s::SKILL_CHANGE_WORDS.into(),
        s::AGENTS_ON_WORDS.into(),
        s::AGENTS_READ_WORDS.into(),
        s::SETUP_WORDS.program.into(),
        s::SETUP_WORDS.config_dir.into(),
        s::SETUP_WORDS.args.into(),
        s::SETUP_WORDS.env_names.into(),
        s::DEFAULTS_LABELS.agent.into(),
        s::DEFAULTS_LABELS.model.into(),
        s::DEFAULTS_LABELS.effort.into(),
        s::DEFAULTS_LABELS.access.into(),
        s::skills_sh_placeless_line("o/r/s"),
        s::preview_cut_line(65),
        s::is_in_line("a", "b"),
        s::is_in_also_line("a", "b", "c"),
        s::agent_copy_words("a", "b"),
        s::gone_from_line("a", "b"),
        s::turned_in_line("a", true, "b"),
        s::turned_in_line("a", false, "b"),
        s::turned_line("a", true),
        s::turned_line("a", false),
        s::picker_hide("m"),
        s::picker_order("m"),
        s::picker_custom("m"),
        s::starts_on_own_line("Claude Code"),
        s::starts_on_line("Claude Code", "opus"),
        s::default_agent_line("claude"),
        s::env_name_line("--env", "\"1\""),
        s::setup_on_line("Claude Code", true),
        s::setup_on_line("Claude Code", false),
        s::tracker_shape_line("lab"),
        s::trackers_line(&strs(&["a/b", "c/d"])),
        s::after_worktree_line("make"),
        s::no_defaults_answered_line("wsp"),
        s::new_threads_head_line("wsp"),
        s::defaults_value_line("agent", "Codex", "this project's"),
    ];
    said.extend(s::SKILL_HIT_COLUMNS.iter().map(|c| c.to_string()));
    for from in [
        wsp_wire::types::thread_defaults::DefaultFrom::Named,
        wsp_wire::types::thread_defaults::DefaultFrom::Project,
        wsp_wire::types::thread_defaults::DefaultFrom::Default,
        wsp_wire::types::thread_defaults::DefaultFrom::Catalog,
    ] {
        said.push(s::from_word(from).to_string());
    }
    for text in said {
        assert!(!text.to_lowercase().contains("cloud"), "names a cloud: {text}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The guest rule: a caller on a machine, `elsewhere`, never has this computer's disk read or its folders resolved,
// and is refused before any path is touched. These helpers take no folder of their own to read: the one path they
// handle, a config folder, is the caller's word for a folder on the target, checked for its first character and sent
// as it was given.
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn guest_rule_a_relative_config_folder_is_refused_before_any_frame_or_path() {
    for config in ["claude", "~/claude", "./claude", "../claude", ""] {
        let (c, log) = client(replies("agentSetupSet", "a config folder that is not absolute"));
        let ask = s::AgentSetupAsk { config: Some(config.into()), ..Default::default() };
        let e = block(s::agent_setup_set(&c, "claude", &ask, &IndexMap::new(), "usage: wsp agents setup <agent>")).unwrap_err();
        usage_refusal(&e);
        assert_eq!(e.message, format!("--config is a folder on that computer, absolute, and got {}. Give a path that opens with /, since whoever reads it works in a folder this line cannot see.", serde_json::to_string(config).unwrap()));
        assert!(sent(&log).is_empty(), "frames sent for {config:?}");
    }
}

#[test]
#[cfg(unix)]
fn guest_rule_an_absolute_config_folder_is_sent_as_given_never_resolved() {
    let dir = tempfile::tempdir().expect("temp folder");
    let real = dir.path().join("real");
    std::fs::create_dir(&real).unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    for config in
        [format!("{}/x/../y", link.display()), "/nonexistent/../place//x/".to_string(), format!("{}/missing", dir.path().display())]
    {
        let (c, log) = client(replies("agentSetupSet", "set here"));
        let ask = s::AgentSetupAsk { config: Some(config.clone()), ..Default::default() };
        block(s::agent_setup_set(&c, "claude", &ask, &IndexMap::new(), "usage: wsp agents setup <agent>")).expect("sent");
        let frame = serde_json::to_string(
            &serde_json::json!({ "placeId": "here", "agent": "claude", "configDir": config, "id": 2, "op": "agents.setup" }),
        )
        .unwrap();
        assert_eq!(sent(&log), vec![frame], "the folder rides as it was given");
    }
}

#[test]
fn guest_rule_a_skills_sh_id_names_no_computer_and_is_refused_before_any_frame() {
    let usage = "usage: wsp skills show <skill>";
    for (thread, on, project) in [
        (Some("t-landing"), None, no_project()),
        (None, Some("attic"), no_project()),
        (None, None, ProjectAsked { project: true, name: None }),
        (None, Some("attic"), ProjectAsked { project: true, name: Some("api".into()) }),
        (Some("t-landing"), Some("attic"), ProjectAsked { project: true, name: None }),
    ] {
        let (c, log) = client(replies("skillShown", "an id off skills.sh asks skills.get"));
        let e = block(s::skill_shown(&c, "o/r/s", thread, on, &project, usage, "wsp skills show", false)).unwrap_err();
        usage_refusal(&e);
        assert_eq!(e.message, format!("{} {}", s::skills_sh_placeless_line("o/r/s"), usage));
        assert!(sent(&log).is_empty(), "frames sent for {thread:?} {on:?}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// skills.sh and skills
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn skills_sh_id_has_exactly_three_parts() {
    // agents-help.ts:313: three parts, an empty middle included, is skills.sh; two, four and none ask the target.
    for (skill, op) in [
        ("o/r/s", "skills.get"),
        ("a//c", "skills.get"),
        ("///", "skills.preview"),
        ("o/r", "skills.preview"),
        ("a/b/c/d", "skills.preview"),
        ("review", "skills.preview"),
        ("", "skills.preview"),
    ] {
        let mut r = replies("skillShown", "here: skills.preview of the name, no project key");
        r.insert("skills.get".into(), replies("skillShown", "an id off skills.sh asks skills.get")["skills.get"].clone());
        let (c, log) = client(r);
        block(s::skill_shown(&c, skill, None, None, &no_project(), "usage: wsp skills show <skill>", "wsp skills show", false))
            .expect("read");
        let first = sent(&log).remove(0);
        assert!(first.ends_with(&format!("\"op\":\"{op}\"}}")), "{skill:?} asked {first}");
    }
}

#[test]
fn shown_text_cuts_the_line_over_the_cap_only_and_rounds_the_size_up() {
    // agents-help.ts:329-332 and the cap SKILL_PREVIEW_BYTES (65536): under and at it the text alone, over it the cut line.
    assert_eq!(wsp_wire::types::agents_report::SKILL_PREVIEW_BYTES, 65_536);
    assert_eq!(s::shown_text(&preview("x", 65_535)), "x");
    assert_eq!(s::shown_text(&preview("x", 65_536)), "x");
    assert_eq!(s::shown_text(&preview("x", 65_537)), "x\n\n(the first 64 KB of 65 KB)");
    assert_eq!(s::shown_text(&preview("x", 66_560)), "x\n\n(the first 64 KB of 65 KB)");
    assert_eq!(s::shown_text(&preview("x", 66_561)), "x\n\n(the first 64 KB of 66 KB)");
}

#[test]
fn hostile_text_from_skills_sh_is_printed_without_control_characters() {
    // skills-sh.test.ts "what a skills verb prints of skills.sh's text: drops control characters and escape sequences, keeping newlines and tabs".
    let hostile =
        "# pdf\n\tUse it.\x1b]52;c;cm0gLXJmIH4=\x07\x1b]0;title\x07\x1b]8;;https://evil.test\x1b\\click\x1b]8;;\x1b\\\r\u{9b}2J\0end";
    let text = s::shown_text(&preview(hostile, hostile.len() as u64));
    assert!(text.chars().all(|c| c == '\n' || c == '\t' || !has_control(&c.to_string())), "{text:?}");
    assert!(text.starts_with("# pdf\n\tUse it."));
    assert!(text.ends_with("end"));
    let lines = s::skill_hit_lines(&[hit("c\x1b]0;x\x07", "a/b/c\x1b[2J")]).join("\n");
    assert!(!lines.chars().any(|c| c != '\n' && has_control(&c.to_string())), "{lines:?}");
}

#[test]
fn folders_printed_by_added_and_removed_have_no_escapes_or_line_breaks() {
    // skills-sh.test.ts "prints the folders skills add and skills remove answer without their escapes or line breaks".
    let a: SkillAdded = serde_json::from_value(serde_json::json!({ "path": "~/.agents/skills/pdf\u{1b}]0;x\u{7}", "agents": [{ "agent": "claude", "path": "~/.claude/skills/pdf\u{9b}2J\nfake" }] })).unwrap();
    let added = s::added_line("a/b/pdf", &a);
    let removed = s::removed_line("pdf\x1b[2J", &strs(&["~/.claude/skills/pdf\x1b]52;c;eA==\x07", "~/.agents/skills/pdf\r\n"]));
    for line in [&added, &removed] {
        assert!(!has_control(line), "{line:?}");
    }
    assert!(added.contains("~/.claude/skills/pdf2J fake"), "{added:?}");
}

#[test]
fn search_skills_sh_sends_the_limit_only_when_given() {
    for (limit, frame) in [
        (None, r#"{"q":"review code","id":2,"op":"skills.search"}"#),
        (Some(2), r#"{"q":"review code","limit":2,"id":2,"op":"skills.search"}"#),
        (Some(0), r#"{"q":"review code","limit":0,"id":2,"op":"skills.search"}"#),
    ] {
        let (c, log) = client(replies("searchSkillsSh", "hits with a limit, an extra key dropped"));
        let hits = block(s::search_skills_sh(&c, "review code", limit)).expect("hits");
        assert_eq!(hits.len(), 2);
        assert_eq!(sent(&log), vec![frame.to_string()]);
    }
}

#[test]
fn skill_added_sends_agents_only_when_there_is_one_and_project_only_when_asked() {
    let go = |agents: Option<Vec<String>>, project: ProjectAsked| {
        let (c, log) = client(replies("skillAdded", "alone, no agents key"));
        block(s::skill_added(
            &c,
            "plain",
            None,
            None,
            agents.as_deref(),
            &project,
            "usage: wsp skills add <skill>",
            "wsp skills add",
            false,
        ))
        .expect("added");
        sent(&log)
    };
    assert_eq!(go(None, no_project()), vec![r#"{"target":{"placeId":"here"},"skill":"plain","id":2,"op":"skills.add"}"#]);
    assert_eq!(go(Some(vec![]), no_project()), vec![r#"{"target":{"placeId":"here"},"skill":"plain","id":2,"op":"skills.add"}"#]);
    assert_eq!(
        go(Some(strs(&["codex"])), no_project()),
        vec![r#"{"target":{"placeId":"here"},"skill":"plain","agents":["codex"],"id":2,"op":"skills.add"}"#]
    );
    assert_eq!(
        go(None, ProjectAsked { project: true, name: None }),
        vec![r#"{"target":{"placeId":"here"},"skill":"plain","project":true,"id":2,"op":"skills.add"}"#]
    );
}

#[test]
fn skill_changed_remove_reads_removed_and_toggle_reads_paths() {
    let both = r#"{"id":1,"ok":true,"removed":["~/a"],"paths":["~/b","~/c"]}"#;
    let one = |op: s::SkillChange| {
        let mut r = IndexMap::new();
        r.insert(op.op().to_string(), both.to_string());
        let (c, log) = client(r);
        let got = block(s::skill_changed(
            &c,
            op,
            "review",
            None,
            None,
            &no_project(),
            Some(false),
            "usage: wsp skills disable <skill>",
            "wsp skills disable",
            false,
        ))
        .expect("changed");
        (got, sent(&log))
    };
    assert_eq!(
        one(s::SkillChange::Remove),
        (strs(&["~/a"]), vec![r#"{"target":{"placeId":"here"},"name":"review","on":false,"id":2,"op":"skills.remove"}"#.to_string()])
    );
    assert_eq!(
        one(s::SkillChange::Toggle),
        (
            strs(&["~/b", "~/c"]),
            vec![r#"{"target":{"placeId":"here"},"name":"review","on":false,"id":2,"op":"skills.toggle"}"#.to_string()]
        )
    );
}

// ---------------------------------------------------------------------------------------------------------------
// agentDefaultsSet
// ---------------------------------------------------------------------------------------------------------------

fn agent_ask(f: impl FnOnce(&mut s::AgentSetAsk)) -> s::AgentSetAsk {
    let mut ask = s::AgentSetAsk::default();
    f(&mut ask);
    ask
}

#[test]
fn agent_defaults_set_sends_nothing_before_the_patch_is_whole() {
    let asks = [
        agent_ask(|_| {}),
        agent_ask(|a| {
            a.hide = Some(vec![]);
            a.show = Some(vec![]);
            a.order = Some(vec![]);
            a.add_model = Some(vec![]);
            a.drop_model = Some(vec![]);
            a.reset = Some(vec![]);
        }),
        agent_ask(|a| a.reset = Some(strs(&["bogus"]))),
        agent_ask(|a| a.reset = Some(strs(&["model", "after-worktree"]))),
        agent_ask(|a| a.access = Some("bypassPermissions".into())),
        agent_ask(|a| a.access = Some("".into())),
    ];
    for ask in asks {
        let (c, log) = client(replies("agentDefaultsSet", "the picker's lists merged over what stands"));
        let e = block(s::agent_defaults_set(&c, "claude", &ask)).unwrap_err();
        usage_refusal(&e);
        assert!(sent(&log).is_empty(), "frames sent for {ask:?}");
    }
    let ask = agent_ask(|a| {
        a.hide = Some(strs(&["h"]));
        a.access = Some("Full".into());
    });
    let (c, log) = client(replies("agentDefaultsSet", "the picker's lists merged over what stands"));
    let e = block(s::agent_defaults_set(&c, "claude", &ask)).unwrap_err();
    usage_refusal(&e);
    assert_eq!(sent(&log).len(), 1, "a list to merge reads the preferences before the access word is checked (agents-help.ts:410-419)");
    let (c, _) = client(IndexMap::new());
    let e = block(s::agent_defaults_set(&c, "claude", &agent_ask(|_| {}))).unwrap_err();
    assert_eq!(e.message, format!("{} {}", s::AGENT_SET_NOTHING_LINE, s::NOTHING_TO_SET_FIX));
    let e = block(s::agent_defaults_set(&c, "claude", &agent_ask(|a| a.reset = Some(strs(&["effort", "x"]))))).unwrap_err();
    assert_eq!(e.message, "--reset takes model, effort, access, models, and got \"x\". Name one of those for each --reset.");
}

#[test]
fn agent_defaults_set_merges_the_picker_over_what_stands() {
    // The kept lists are hide [sonnet, "x 🧪"], custom [c0, c9]; hide haiku, show sonnet, order [opus, haiku],
    // addModel [custom-1, c0], dropModel [c9] leave hide ["x 🧪", haiku], that order, custom [c0, custom-1].
    let (c, log) = client(replies("agentDefaultsSet", "the picker's lists merged over what stands"));
    let ask = agent_ask(|a| {
        a.hide = Some(strs(&["haiku"]));
        a.show = Some(strs(&["sonnet"]));
        a.order = Some(strs(&["opus", "haiku"]));
        a.add_model = Some(strs(&["custom-1", "c0"]));
        a.drop_model = Some(strs(&["c9"]));
    });
    let got = block(s::agent_defaults_set(&c, "claude", &ask)).expect("set");
    assert_eq!(
        sent(&log),
        vec![
            r#"{"id":2,"op":"preferences.get"}"#.to_string(),
            r#"{"patch":{"agentDefaults":{"claude":{"models":{"hide":["x 🧪","haiku"],"order":["opus","haiku"],"custom":["c0","custom-1"]}}}},"id":3,"op":"preferences.set"}"#.to_string()
        ]
    );
    assert_eq!(
        serde_json::to_string(&got).unwrap(),
        r#"{"models":{"hide":["x 🧪","haiku"],"order":["opus","haiku"],"custom":["c0","custom-1"]}}"#
    );
}

#[test]
fn agent_defaults_set_with_nothing_standing_reads_once_and_sends_only_what_is_named() {
    let (c, log) = client(replies("agentDefaultsSet", "the picker's lists with none standing"));
    block(s::agent_defaults_set(&c, "codex", &agent_ask(|a| a.hide = Some(strs(&["gpt"]))))).expect("set");
    assert_eq!(
        sent(&log),
        vec![
            r#"{"id":2,"op":"preferences.get"}"#.to_string(),
            r#"{"patch":{"agentDefaults":{"codex":{"models":{"hide":["gpt"]}}}},"id":3,"op":"preferences.set"}"#.to_string()
        ]
    );
}

#[test]
fn agent_defaults_set_reset_models_is_null_and_reads_nothing() {
    let (c, log) = client(replies("agentDefaultsSet", "put back"));
    let ask = agent_ask(|a| {
        a.reset = Some(strs(&["model", "models"]));
        a.hide = Some(strs(&["ignored"]));
    });
    block(s::agent_defaults_set(&c, "claude", &ask)).expect("set");
    assert_eq!(
        sent(&log),
        vec![r#"{"patch":{"agentDefaults":{"claude":{"model":null,"models":null}}},"id":2,"op":"preferences.set"}"#.to_string()]
    );
}

#[test]
fn agent_defaults_set_each_reset_word_puts_its_field_back() {
    for word in s::AGENT_SET_RESETS {
        let (c, log) = client(replies("agentDefaultsSet", "put back"));
        block(s::agent_defaults_set(&c, "claude", &agent_ask(|a| a.reset = Some(strs(&[word]))))).expect("set");
        assert_eq!(
            sent(&log),
            vec![format!(r#"{{"patch":{{"agentDefaults":{{"claude":{{"{word}":null}}}}}},"id":2,"op":"preferences.set"}}"#)],
            "{word}"
        );
    }
}

#[test]
fn agent_defaults_set_access_words_go_to_the_host_as_wsp_words() {
    for word in ["ask", "auto-edit", "full", "plan"] {
        let (c, log) = client(replies("agentDefaultsSet", "model, effort and access"));
        block(s::agent_defaults_set(&c, "claude", &agent_ask(|a| a.access = Some(word.into())))).expect("set");
        assert_eq!(
            sent(&log),
            vec![format!(r#"{{"patch":{{"agentDefaults":{{"claude":{{"access":"{word}"}}}}}},"id":2,"op":"preferences.set"}}"#)]
        );
    }
    let (c, log) = client(replies("agentDefaultsSet", "a word the agent has no mode for"));
    let e = block(s::agent_defaults_set(&c, "claude", &agent_ask(|a| a.access = Some("plan".into())))).unwrap_err();
    assert_eq!(e.message, "Claude Code takes no plan access; it takes ask, auto-edit, full");
    assert_eq!(e.kind, Some(Kind::Usage));
    assert_eq!(sent(&log).len(), 1);
}

#[test]
fn agent_defaults_set_the_patch_keeps_model_effort_access_models_in_that_order() {
    let mut r = replies("agentDefaultsSet", "model, effort and access");
    r.insert("preferences.get".into(), replies("agentDefaultsSet", "the picker's lists with none standing")["preferences.get"].clone());
    let (c, log) = client(r);
    let ask = agent_ask(|a| {
        a.hide = Some(strs(&["h"]));
        a.access = Some("full".into());
        a.effort = Some("low".into());
        a.model = Some("m".into());
    });
    block(s::agent_defaults_set(&c, "claude", &ask)).expect("set");
    assert_eq!(
        sent(&log)[1],
        r#"{"patch":{"agentDefaults":{"claude":{"model":"m","effort":"low","access":"full","models":{"hide":["h"]}}}},"id":3,"op":"preferences.set"}"#
    );
}

// ---------------------------------------------------------------------------------------------------------------
// projectDefaultsSet
// ---------------------------------------------------------------------------------------------------------------

fn project_ask(f: impl FnOnce(&mut s::ProjectSetAsk)) -> s::ProjectSetAsk {
    let mut ask = s::ProjectSetAsk::default();
    f(&mut ask);
    ask
}

#[test]
fn project_defaults_set_asks_resolve_then_set_then_defaults() {
    let (c, log) = client(replies("projectDefaultsSet", "set"));
    let ask = project_ask(|a| {
        a.access = Some("full".into());
        a.agent = Some("codex".into());
    });
    let got = block(s::project_defaults_set(&c, "wsp", &ask)).expect("set");
    assert_eq!(
        sent(&log),
        vec![
            r#"{"ref":"wsp","id":2,"op":"projects.resolve"}"#.to_string(),
            r#"{"patch":{"projectDefaults":{"proj-1":{"agent":"codex","access":"full"}}},"id":3,"op":"preferences.set"}"#.to_string(),
            r#"{"id":4,"op":"projects.defaults"}"#.to_string(),
        ]
    );
    assert_eq!(got.project.id, "proj-1");
    assert!(got.after_worktree.is_none() && got.trackers.is_none());
}

#[test]
fn project_defaults_set_refuses_in_a_fixed_order_before_any_frame() {
    // Blank after-worktree, then a word that is no repo, then a reset word outside the list, then the access word, then nothing to set.
    let asks = [
        (
            project_ask(|a| {
                a.after_worktree = Some(" ".into());
                a.trackers = Some(strs(&["lab"]));
                a.reset = Some(strs(&["nope"]));
                a.access = Some("bad".into());
            }),
            s::AFTER_WORKTREE_BLANK_LINE,
        ),
        (
            project_ask(|a| {
                a.trackers = Some(strs(&["ok/ok", "lab"]));
                a.reset = Some(strs(&["nope"]));
                a.access = Some("bad".into());
            }),
            "--tracker \"lab\" names no repo.",
        ),
        (
            project_ask(|a| {
                a.reset = Some(strs(&["nope"]));
                a.access = Some("bad".into());
            }),
            "--reset takes agent, model, effort, access, after-worktree, trackers, and got \"nope\".",
        ),
        (project_ask(|a| a.access = Some("bad".into())), "--access takes ask, auto-edit, full or plan, and got bad."),
        (project_ask(|_| {}), s::PROJECT_SET_NOTHING_LINE),
    ];
    for (ask, words) in asks {
        let (c, log) = client(replies("projectDefaultsSet", "set"));
        let e = block(s::project_defaults_set(&c, "wsp", &ask)).unwrap_err();
        usage_refusal(&e);
        assert!(e.message.starts_with(words), "{} does not open with {words}", e.message);
        assert!(sent(&log).is_empty(), "frames sent for {ask:?}");
    }
}

#[test]
fn project_defaults_set_a_blank_command_is_blank_after_trim_and_a_padded_one_is_sent_untrimmed() {
    // contract.test.ts "a blank after-worktree command is the line's own refusal, never the host's parse of it".
    for blank in ["", " ", "  ", "\t\n "] {
        let (c, log) = client(replies("projectDefaultsSet", "set"));
        let e = block(s::project_defaults_set(&c, "alpha", &project_ask(|a| a.after_worktree = Some(blank.into())))).unwrap_err();
        assert_eq!(e.message, format!("{} {}", s::AFTER_WORKTREE_BLANK_LINE, s::AFTER_WORKTREE_BLANK_FIX));
        assert_eq!(e.kind, Some(Kind::Usage));
        assert!(sent(&log).is_empty());
    }
    let (c, log) = client(replies("projectDefaultsSet", "an after-worktree command is sent untrimmed"));
    let got = block(s::project_defaults_set(&c, "wsp", &project_ask(|a| a.after_worktree = Some("  make setup  ".into())))).expect("set");
    assert_eq!(
        sent(&log)[1],
        r#"{"patch":{"projectDefaults":{"proj-1":{"afterWorktree":"  make setup  "}}},"id":3,"op":"preferences.set"}"#
    );
    assert_eq!(got.after_worktree.as_deref(), Some("make setup"));
}

#[test]
fn project_defaults_set_each_reset_word_puts_its_field_back() {
    for word in s::PROJECT_SET_RESETS {
        let key = if word == "after-worktree" { "afterWorktree" } else { word };
        let (c, log) = client(replies("projectDefaultsSet", "reset after-worktree alone"));
        block(s::project_defaults_set(&c, "wsp", &project_ask(|a| a.reset = Some(strs(&[word]))))).expect("set");
        assert_eq!(
            sent(&log)[1],
            format!(r#"{{"patch":{{"projectDefaults":{{"proj-1":{{"{key}":null}}}}}},"id":3,"op":"preferences.set"}}"#),
            "{word}"
        );
    }
}

#[test]
fn project_defaults_set_the_patch_keeps_agent_model_effort_access_afterworktree_trackers_in_that_order() {
    let (c, log) = client(replies("projectDefaultsSet", "every field set"));
    let ask = project_ask(|a| {
        a.trackers = Some(strs(&["a/b", "a/b"]));
        a.after_worktree = Some("make".into());
        a.access = Some("ask".into());
        a.effort = Some("e".into());
        a.model = Some("m".into());
        a.agent = Some("claude".into());
    });
    block(s::project_defaults_set(&c, "wsp", &ask)).expect("set");
    assert_eq!(
        sent(&log)[1],
        r#"{"patch":{"projectDefaults":{"proj-1":{"agent":"claude","model":"m","effort":"e","access":"ask","afterWorktree":"make","trackers":["a/b"]}}},"id":3,"op":"preferences.set"}"#
    );
}

#[test]
fn project_defaults_set_trackers_keep_the_whole_list_and_a_word_naming_no_repo_is_refused() {
    // agents-cloud.test.ts "wsp projects set --tracker keeps the whole list in the project's defaults, --reset trackers takes it off, and a word that names no repo is refused".
    let (c, log) = client(replies("projectDefaultsSet", "trackers echo with the whole list"));
    let got = block(s::project_defaults_set(&c, "h-1", &project_ask(|a| a.trackers = Some(strs(&["acme/lab", "acme/map"]))))).expect("set");
    assert_eq!(got.trackers, Some(strs(&["acme/lab", "acme/map"])));
    assert_eq!(
        sent(&log)[1],
        r#"{"patch":{"projectDefaults":{"proj-1":{"trackers":["acme/lab","acme/map"]}}},"id":3,"op":"preferences.set"}"#
    );
    assert_eq!(s::trackers_line(&strs(&["acme/lab"])), "gh on a sub-thread's machine may also comment on: acme/lab");
    let (c, log) = client(replies("projectDefaultsSet", "reset trackers and after-worktree"));
    let got = block(s::project_defaults_set(&c, "h-1", &project_ask(|a| a.reset = Some(strs(&["trackers"]))))).expect("set");
    assert_eq!(got.trackers, None);
    assert_eq!(sent(&log)[1], r#"{"patch":{"projectDefaults":{"proj-1":{"trackers":null}}},"id":3,"op":"preferences.set"}"#);
    let (c, log) = client(replies("projectDefaultsSet", "set"));
    let e = block(s::project_defaults_set(&c, "h-1", &project_ask(|a| a.trackers = Some(strs(&["lab"]))))).unwrap_err();
    assert_eq!(e.message, "--tracker \"lab\" names no repo. Name it as owner/name, the way gh repo view does.");
    assert!(sent(&log).is_empty());
}

#[test]
fn project_defaults_set_answers_a_plain_error_when_the_host_has_no_defaults_for_the_project() {
    let (c, log) = client(replies("projectDefaultsSet", "no defaults answered"));
    let e = block(s::project_defaults_set(&c, "wsp", &project_ask(|a| a.effort = Some("low".into())))).unwrap_err();
    assert_eq!(e.message, s::no_defaults_answered_line("zingzy's \u{85}box\u{7f} \"one\" \\ two\nthree \u{1f9ea}"));
    assert_eq!(e.kind, None, "no kind: a provider failure, not a refusal");
    assert_eq!(sent(&log).len(), 3);
}

#[test]
fn project_defaults_set_stops_at_the_first_frame_the_host_refuses() {
    let mut r = replies("projectDefaultsSet", "set");
    r.insert("preferences.set".into(), r#"{"id":1,"ok":false,"error":"no harness named x","kind":"usage"}"#.into());
    let (c, log) = client(r);
    let e = block(s::project_defaults_set(&c, "wsp", &project_ask(|a| a.agent = Some("x".into())))).unwrap_err();
    assert_eq!(e.message, "no harness named x");
    assert_eq!(sent(&log).len(), 2, "projects.defaults is not asked after a refused set");
    let (c, log) = client(replies("projectDefaultsSet", "no such project"));
    let e = block(s::project_defaults_set(&c, "nope", &project_ask(|a| a.model = Some("m".into())))).unwrap_err();
    assert_eq!(e.message, "no project \"nope\"; you have wsp");
    assert_eq!(sent(&log), vec![r#"{"ref":"nope","id":2,"op":"projects.resolve"}"#.to_string()]);
}

// ---------------------------------------------------------------------------------------------------------------
// agentSetupSet
// ---------------------------------------------------------------------------------------------------------------

fn setup_ask(f: impl FnOnce(&mut s::AgentSetupAsk)) -> s::AgentSetupAsk {
    let mut ask = s::AgentSetupAsk::default();
    f(&mut ask);
    ask
}

const SETUP_USAGE: &str = "usage: wsp agents setup <agent>";

#[test]
fn agent_setup_set_sends_every_field_in_the_changes_own_order() {
    let (c, log) = client(replies("agentSetupSet", "set here"));
    let ask = setup_ask(|a| {
        a.unset_env = Some(strs(&["OLD"]));
        a.args = Some(strs(&["--debug", "a \u{85} b"]));
        a.config = Some("/Users/x/claude wsp".into());
        a.program = Some("/opt/\u{1f9ea}/claude".into());
        a.enabled = Some(false);
    });
    block(s::agent_setup_set(&c, "claude", &ask, &IndexMap::new(), SETUP_USAGE)).expect("set");
    assert_eq!(sent(&log), vec!["{\"placeId\":\"here\",\"agent\":\"claude\",\"on\":false,\"program\":\"/opt/\u{1f9ea}/claude\",\"configDir\":\"/Users/x/claude wsp\",\"args\":[\"--debug\",\"a \u{85} b\"],\"env\":{\"OLD\":null},\"id\":2,\"op\":\"agents.setup\"}".to_string()]);
}

#[test]
fn agent_setup_set_env_is_the_values_then_the_unset_names_as_null() {
    let mut values = IndexMap::new();
    values.insert("FOO".to_string(), "bar".to_string());
    values.insert("BAZ".to_string(), "q".to_string());
    let (c, log) = client(replies("agentSetupSet", "values and unset names, values first"));
    block(s::agent_setup_set(&c, "claude", &setup_ask(|a| a.unset_env = Some(strs(&["OLD", "OLD2"]))), &values, SETUP_USAGE)).expect("set");
    assert_eq!(
        sent(&log),
        vec![r#"{"placeId":"here","agent":"claude","env":{"FOO":"bar","BAZ":"q","OLD":null,"OLD2":null},"id":2,"op":"agents.setup"}"#
            .to_string()]
    );
    // A name given as a value and as an unset name keeps the value's place and is null.
    values.clear();
    values.insert("FOO".to_string(), "bar".to_string());
    values.insert("BAR".to_string(), "b".to_string());
    let (c, log) = client(replies("agentSetupSet", "values and unset names, values first"));
    block(s::agent_setup_set(&c, "claude", &setup_ask(|a| a.unset_env = Some(strs(&["FOO"]))), &values, SETUP_USAGE)).expect("set");
    assert_eq!(
        sent(&log),
        vec![r#"{"placeId":"here","agent":"claude","env":{"FOO":null,"BAR":"b"},"id":2,"op":"agents.setup"}"#.to_string()]
    );
}

#[test]
fn agent_setup_set_looks_up_the_computer_before_its_own_refusals() {
    for ask in [
        setup_ask(|a| a.on = Some("attic".into())),
        setup_ask(|a| {
            a.on = Some("attic".into());
            a.config = Some("rel".into());
        }),
        setup_ask(|a| {
            a.on = Some("attic".into());
            a.unset_env = Some(strs(&["1x"]));
        }),
    ] {
        let (c, log) = client(replies("agentSetupSet", "nothing to change still looks the computer up first"));
        let e = block(s::agent_setup_set(&c, "claude", &ask, &IndexMap::new(), SETUP_USAGE)).unwrap_err();
        usage_refusal(&e);
        assert_eq!(sent(&log), vec![r#"{"id":2,"op":"places.list"}"#.to_string()], "{ask:?}");
    }
    // A reset word outside the list comes before the lookup, so nothing is sent.
    let (c, log) = client(replies("agentSetupSet", "nothing to change still looks the computer up first"));
    let e = block(s::agent_setup_set(
        &c,
        "claude",
        &setup_ask(|a| {
            a.on = Some("attic".into());
            a.reset = Some(strs(&["nope"]));
        }),
        &IndexMap::new(),
        SETUP_USAGE,
    ))
    .unwrap_err();
    assert_eq!(e.message, "--reset takes program, config, args, and got \"nope\". Name one of those for each --reset.");
    assert!(sent(&log).is_empty());
}

#[test]
fn agent_setup_set_without_a_computer_sends_the_here_place_and_with_one_its_id() {
    let (c, log) = client(replies("agentSetupSet", "reset program"));
    block(s::agent_setup_set(&c, "claude", &setup_ask(|a| a.enabled = Some(true)), &IndexMap::new(), SETUP_USAGE)).expect("set");
    assert_eq!(sent(&log), vec![r#"{"placeId":"here","agent":"claude","on":true,"id":2,"op":"agents.setup"}"#.to_string()]);
    let (c, log) = client(replies("agentSetupSet", "on a computer"));
    block(s::agent_setup_set(
        &c,
        "claude",
        &setup_ask(|a| {
            a.on = Some("attic".into());
            a.enabled = Some(true);
            a.reset = Some(strs(&["program", "args"]));
        }),
        &IndexMap::new(),
        SETUP_USAGE,
    ))
    .expect("set");
    assert_eq!(
        sent(&log),
        vec![
            r#"{"id":2,"op":"places.list"}"#.to_string(),
            r#"{"placeId":"place-9","agent":"claude","on":true,"program":null,"args":null,"id":3,"op":"agents.setup"}"#.to_string()
        ]
    );
}

#[test]
fn agent_setup_set_each_reset_word_puts_its_field_back() {
    for word in s::AGENT_SETUP_RESETS {
        let key = if word == "config" { "configDir" } else { word };
        let (c, log) = client(replies("agentSetupSet", "reset program"));
        block(s::agent_setup_set(&c, "claude", &setup_ask(|a| a.reset = Some(strs(&[word]))), &IndexMap::new(), SETUP_USAGE)).expect("set");
        assert_eq!(sent(&log), vec![format!(r#"{{"placeId":"here","agent":"claude","{key}":null,"id":2,"op":"agents.setup"}}"#)], "{word}");
    }
}

#[test]
fn agent_setup_set_refuses_a_name_a_shell_would_not_read_before_any_frame() {
    for bad in ["1 BAD", "", "A-B", "a b"] {
        let (c, log) = client(IndexMap::new());
        let e =
            block(s::agent_setup_set(&c, "claude", &setup_ask(|a| a.unset_env = Some(strs(&["OK", bad]))), &IndexMap::new(), SETUP_USAGE))
                .unwrap_err();
        assert_eq!(
            e.message,
            format!("--unset-env takes a variable's name, and got {}. {}", serde_json::to_string(bad).unwrap(), s::ENV_NAME_FIX)
        );
        assert!(sent(&log).is_empty());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The lines of the setup and of the defaults
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn agent_setup_lines_ask_for_a_sign_in_only_where_the_config_folder_moved() {
    let row = |setup: serde_json::Value| {
        serde_json::from_value(serde_json::json!({ "id": "claude", "name": "my claude", "installed": true, "road": "wsp", "signIn": "signed-in", "signInRoad": "device", "wspTools": true, "setup": setup })).expect("a row")
    };
    let with = row(serde_json::json!({ "on": true, "configDir": "/c", "envNames": [] }));
    let without = row(serde_json::json!({ "on": true, "envNames": [] }));
    assert_eq!(s::agent_setup_lines(&with, false), vec!["my claude is on there.", "config folder: /c"]);
    assert_eq!(s::agent_setup_lines(&without, true), vec!["my claude is on there."]);
    let moved = s::agent_setup_lines(&with, true);
    assert_eq!(moved.len(), 3);
    assert!(
        moved[2].starts_with("my claude keeps its sign-in under its config folder") && moved[2].ends_with("before its next thread."),
        "{}",
        moved[2]
    );
}

#[test]
fn every_reset_list_and_nothing_line_names_the_flags_the_verb_takes() {
    for (nothing, flags) in [
        (
            s::AGENT_SET_NOTHING_LINE,
            vec!["--model", "--effort", "--access", "--hide", "--show", "--add-model", "--drop-model", "--order", "--reset"],
        ),
        (s::AGENT_SETUP_NOTHING_LINE, vec!["--enable", "--disable", "--program", "--config", "--arg", "--env", "--unset-env", "--reset"]),
        (s::PROJECT_SET_NOTHING_LINE, vec!["--agent", "--model", "--effort", "--access", "--after-worktree", "--tracker", "--reset"]),
    ] {
        for flag in flags {
            assert!(nothing.contains(flag), "{nothing} lacks {flag}");
        }
    }
    assert_eq!(s::AGENT_SET_RESETS.len() + s::AGENT_SETUP_RESETS.len() + s::PROJECT_SET_RESETS.len(), 13);
}
