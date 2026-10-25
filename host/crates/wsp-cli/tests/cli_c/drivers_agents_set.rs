// SPDX-License-Identifier: AGPL-3.0-only
//! The drivers of `agents_set`: one per helper the probe `record/agents_set.probe.ts` called. Each reads the case's
//! arguments, calls the Rust helper of `src/verbs/agents_help/set.rs` and answers the value as JSON.
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use wsp_cli::verbs::agents_help::read::ProjectAsked;
use wsp_cli::verbs::agents_help::set as s;
use wsp_cli::verbs::client::BoxFuture;
use wsp_wire::types::agents_report::{AgentRow, SkillAdded, SkillHit, SkillPreview};
use wsp_wire::types::thread_defaults::{AgentDefaults, DefaultFrom, ThreadDefaults};
use wsp_words::exit::HostError;

use super::common::Env;

type Driven<'a> = BoxFuture<'a, Result<Value, HostError>>;

fn typed<T: DeserializeOwned>(env: &Env, name: &str) -> T {
    serde_json::from_value(env.arg(name).unwrap_or_else(|| panic!("case has no argument {name}")).clone()).unwrap_or_else(|e| panic!("argument {name} does not read: {e}"))
}

fn text(env: &Env, name: &str) -> String {
    env.str_arg(name).unwrap_or_else(|| panic!("case has no text argument {name}"))
}

fn project_asked(env: &Env) -> ProjectAsked {
    match env.arg("project") {
        None => ProjectAsked { project: false, name: None },
        Some(v) => ProjectAsked { project: v["project"].as_bool().unwrap_or(false), name: v["name"].as_str().map(String::from) },
    }
}

fn optional_strs(env: &Env, name: &str) -> Option<Vec<String>> {
    env.arg(name).map(|_| env.strs(name))
}

fn json_of<T: serde::Serialize>(v: T) -> Result<Value, HostError> {
    Ok(serde_json::to_value(v).expect("the value serializes"))
}

pub fn search_skills_sh(env: &Env) -> Driven<'_> {
    Box::pin(async move {
        let limit = env.arg("limit").and_then(|v| v.as_u64());
        json_of(s::search_skills_sh(env.client.as_ref(), &text(env, "q"), limit).await?)
    })
}

pub fn skill_hit_lines(env: &Env) -> Driven<'_> {
    Box::pin(async move {
        let hits: Vec<SkillHit> = typed(env, "hits");
        json_of(s::skill_hit_lines(&hits))
    })
}

pub fn skill_shown(env: &Env) -> Driven<'_> {
    Box::pin(async move {
        let project = project_asked(env);
        let thread = env.str_arg("thread");
        let on = env.str_arg("on");
        json_of(s::skill_shown(env.client.as_ref(), &text(env, "skill"), thread.as_deref(), on.as_deref(), &project, &text(env, "usage"), &text(env, "line"), env.cloud()).await?)
    })
}

pub fn shown_text(env: &Env) -> Driven<'_> {
    Box::pin(async move {
        let p: SkillPreview = typed(env, "p");
        json_of(s::shown_text(&p))
    })
}

pub fn skill_added(env: &Env) -> Driven<'_> {
    Box::pin(async move {
        let project = project_asked(env);
        let thread = env.str_arg("thread");
        let on = env.str_arg("on");
        let agents = optional_strs(env, "agents");
        json_of(s::skill_added(env.client.as_ref(), &text(env, "skill"), thread.as_deref(), on.as_deref(), agents.as_deref(), &project, &text(env, "usage"), &text(env, "line"), env.cloud()).await?)
    })
}

pub fn added_line(env: &Env) -> Driven<'_> {
    Box::pin(async move {
        let a: SkillAdded = typed(env, "a");
        json_of(s::added_line(&text(env, "skill"), &a))
    })
}

pub fn skill_changed(env: &Env) -> Driven<'_> {
    Box::pin(async move {
        let op = match text(env, "op").as_str() {
            "skills.remove" => s::SkillChange::Remove,
            "skills.toggle" => s::SkillChange::Toggle,
            other => panic!("no such op {other}"),
        };
        let project = project_asked(env);
        let thread = env.str_arg("thread");
        let on = env.str_arg("on");
        let turn = env.arg("turn").and_then(|v| v.as_bool());
        json_of(s::skill_changed(env.client.as_ref(), op, &text(env, "name"), thread.as_deref(), on.as_deref(), &project, turn, &text(env, "usage"), &text(env, "line"), env.cloud()).await?)
    })
}

pub fn removed_line(env: &Env) -> Driven<'_> {
    Box::pin(async move { json_of(s::removed_line(&text(env, "name"), &env.strs("removed"))) })
}

pub fn default_agent_set(env: &Env) -> Driven<'_> {
    Box::pin(async move { json_of(s::default_agent_set(env.client.as_ref(), &text(env, "agent")).await?) })
}

pub fn agent_defaults_set(env: &Env) -> Driven<'_> {
    Box::pin(async move {
        let ask: s::AgentSetAsk = typed(env, "ask");
        json_of(s::agent_defaults_set(env.client.as_ref(), &text(env, "agent"), &ask).await?)
    })
}

pub fn agent_defaults_line(env: &Env) -> Driven<'_> {
    Box::pin(async move {
        let d: AgentDefaults = typed(env, "d");
        json_of(s::agent_defaults_line(&text(env, "agent"), &d))
    })
}

pub fn env_name_of(env: &Env) -> Driven<'_> {
    Box::pin(async move { json_of(s::env_name_of(&text(env, "name"), &text(env, "named"))?) })
}

pub fn agent_setup_set(env: &Env) -> Driven<'_> {
    Box::pin(async move {
        let ask: s::AgentSetupAsk = typed(env, "ask");
        let values: IndexMap<String, String> = env.arg("values").map(|_| typed(env, "values")).unwrap_or_default();
        json_of(s::agent_setup_set(env.client.as_ref(), &text(env, "agent"), &ask, &values, &text(env, "usage")).await?)
    })
}

pub fn agent_setup_lines(env: &Env) -> Driven<'_> {
    Box::pin(async move {
        let row: AgentRow = typed(env, "row");
        json_of(s::agent_setup_lines(&row, env.flag("configMoved")))
    })
}

pub fn project_defaults_set(env: &Env) -> Driven<'_> {
    Box::pin(async move {
        let ask: s::ProjectSetAsk = typed(env, "ask");
        json_of(s::project_defaults_set(env.client.as_ref(), &text(env, "ref"), &ask).await?)
    })
}

pub fn defaults_cell(env: &Env) -> Driven<'_> {
    Box::pin(async move {
        let d: Option<ThreadDefaults> = env.arg("d").map(|_| typed(env, "d"));
        json_of(s::defaults_cell(d.as_ref()))
    })
}

pub fn thread_defaults_lines(env: &Env) -> Driven<'_> {
    Box::pin(async move {
        let d: ThreadDefaults = typed(env, "d");
        json_of(s::thread_defaults_lines(&d))
    })
}

/// The plain text and word tables, by the TS name of the const; an object keeps the TS key order.
pub fn const_text(env: &Env) -> Driven<'_> {
    Box::pin(async move {
        let name = text(env, "name");
        let v = match name.as_str() {
            "noSkillHitsLine" => json!(s::NO_SKILL_HITS_LINE),
            "SKILL_HIT_COLUMNS" => json!(s::SKILL_HIT_COLUMNS),
            "AGENT_SET_RESETS" => json!(s::AGENT_SET_RESETS),
            "PROJECT_SET_RESETS" => json!(s::PROJECT_SET_RESETS),
            "AGENT_SETUP_RESETS" => json!(s::AGENT_SETUP_RESETS),
            "agentSetNothingLine" => json!(s::AGENT_SET_NOTHING_LINE),
            "NOTHING_TO_SET_FIX" => json!(s::NOTHING_TO_SET_FIX),
            "ENV_NAME_FIX" => json!(s::ENV_NAME_FIX),
            "agentSetupNothingLine" => json!(s::AGENT_SETUP_NOTHING_LINE),
            "SETUP_WORDS" => json!({ "program": s::SETUP_WORDS.program, "configDir": s::SETUP_WORDS.config_dir, "args": s::SETUP_WORDS.args, "envNames": s::SETUP_WORDS.env_names }),
            "projectSetNothingLine" => json!(s::PROJECT_SET_NOTHING_LINE),
            "TRACKER_SHAPE_FIX" => json!(s::TRACKER_SHAPE_FIX),
            "afterWorktreeBlankLine" => json!(s::AFTER_WORKTREE_BLANK_LINE),
            "AFTER_WORKTREE_BLANK_FIX" => json!(s::AFTER_WORKTREE_BLANK_FIX),
            "FROM_WORDS" => json!({ "named": s::from_word(DefaultFrom::Named), "project": s::from_word(DefaultFrom::Project), "default": s::from_word(DefaultFrom::Default), "catalog": s::from_word(DefaultFrom::Catalog) }),
            "DEFAULTS_LABELS" => json!({ "agent": s::DEFAULTS_LABELS.agent, "model": s::DEFAULTS_LABELS.model, "effort": s::DEFAULTS_LABELS.effort, "access": s::DEFAULTS_LABELS.access }),
            "SKILL_CHANGE_WORDS" => json!(s::SKILL_CHANGE_WORDS),
            "AGENTS_ON_WORDS" => json!(s::AGENTS_ON_WORDS),
            "AGENTS_READ_WORDS" => json!(s::AGENTS_READ_WORDS),
            other => panic!("no const {other}"),
        };
        Ok(v)
    })
}

/// The sentence functions, by the TS name (`PICKER_WORDS.hide` for a member), with the arguments in `a`.
pub fn sentence(env: &Env) -> Driven<'_> {
    Box::pin(async move {
        let name = text(env, "name");
        let a = env.arg("a").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let st = |i: usize| a[i].as_str().unwrap_or_else(|| panic!("argument {i} of {name} is not text")).to_string();
        let on = |i: usize| a[i].as_bool().unwrap_or_else(|| panic!("argument {i} of {name} is not a flag"));
        let line = match name.as_str() {
            "previewCutLine" => s::preview_cut_line(a[0].as_u64().expect("a size")),
            "skillsShPlacelessLine" => s::skills_sh_placeless_line(&st(0)),
            "isInLine" => s::is_in_line(&st(0), &st(1)),
            "isInAlsoLine" => s::is_in_also_line(&st(0), &st(1), &st(2)),
            "agentCopyWords" => s::agent_copy_words(&st(0), &st(1)),
            "goneFromLine" => s::gone_from_line(&st(0), &st(1)),
            "turnedInLine" => s::turned_in_line(&st(0), on(1), &st(2)),
            "turnedLine" => s::turned_line(&st(0), on(1)),
            "PICKER_WORDS.hide" => s::picker_hide(&st(0)),
            "PICKER_WORDS.order" => s::picker_order(&st(0)),
            "PICKER_WORDS.custom" => s::picker_custom(&st(0)),
            "startsOnOwnLine" => s::starts_on_own_line(&st(0)),
            "startsOnLine" => s::starts_on_line(&st(0), &st(1)),
            "defaultAgentLine" => s::default_agent_line(&st(0)),
            "envNameLine" => s::env_name_line(&st(0), &st(1)),
            "setupOnLine" => s::setup_on_line(&st(0), on(1)),
            "trackerShapeLine" => s::tracker_shape_line(&st(0)),
            "trackersLine" => s::trackers_line(&a[0].as_array().expect("a list").iter().map(|v| v.as_str().expect("a repo").to_string()).collect::<Vec<_>>()),
            "afterWorktreeLine" => s::after_worktree_line(&st(0)),
            "noDefaultsAnsweredLine" => s::no_defaults_answered_line(&st(0)),
            "newThreadsHeadLine" => s::new_threads_head_line(&st(0)),
            "defaultsValueLine" => s::defaults_value_line(&st(0), &st(1), &st(2)),
            other => panic!("no sentence {other}"),
        };
        Ok(json!(line))
    })
}
