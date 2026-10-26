// SPDX-License-Identifier: AGPL-3.0-only
//! The agents verbs' tool calls (`verbs/agents.ts`); the entries are in `agents.rs`.

use indexmap::IndexMap;
use serde_json::json;

use wsp_catalog::catalog::agent_name;
use wsp_words::exit::HostError;

use super::super::agents_help::{
    add_tools, added_line, agent_defaults_line, agent_defaults_set, agent_row_lines, agent_setup_lines, agent_setup_set, aimed_usage,
    default_agent_line, default_agent_set, gone_from_line, is_in_line, plugin_changed, plugin_row_lines, plugin_turned_line, project_asked,
    read_plugins, removed_line, search_skills_sh, server_changed, server_command, server_row_lines, server_scope, server_tools_of,
    server_values, shown_text, skill_added, skill_changed, skill_hit_lines, skill_row_lines, skill_shown, tool_lines, tools_added_line,
    tools_project, turned_in_line, turned_line, AgentSetAsk, AgentSetupAsk, PluginChange, ProjectAsked, ServerOp, SkillChange,
};
use super::super::client::*;
use super::super::io::as_text;
use super::{add_body, agents_read, arg_bool, arg_list, arg_project, arg_str, facts_and, json_of, server_body, with_scope, ReadAim};
use crate::cloud::cloud_on;

fn arg_u64(args: &Frame, key: &str) -> Option<u64> {
    args.get(key).and_then(serde_json::Value::as_u64)
}

/// The `agents` tool's call (`verbs/agents.ts:62-65`).
pub(super) fn call_agents(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let report = agents_read(
            &*client,
            ReadAim {
                thread: arg_str(&args, "thread"),
                on: arg_str(&args, "on"),
                usage: &aimed_usage("agents"),
                line: "wsp agents",
                schema_usage: "wsp agents [<thread>] [--on <computer>]",
            },
            cloud_on(),
        )
        .await?;
        let text = agent_row_lines(&report).join("\n");
        Ok(as_text(&text, facts_and(&report, "agents", json_of(&report.agents))))
    })
}

/// The `skills` tool's call (`verbs/agents.ts:84-87`).
pub(super) fn call_skills(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let report = agents_read(
            &*client,
            ReadAim {
                thread: arg_str(&args, "thread"),
                on: arg_str(&args, "on"),
                usage: &aimed_usage("skills"),
                line: "wsp skills",
                schema_usage: "wsp skills [<thread>] [--on <computer>]",
            },
            cloud_on(),
        )
        .await?;
        let text = skill_row_lines(&report).join("\n");
        Ok(as_text(&text, facts_and(&report, "skills", json_of(&report.skills))))
    })
}

/// The `skills_search` tool's call (`verbs/agents.ts:109-112`).
pub(super) fn call_skills_search(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let hits = search_skills_sh(&*client, arg_str(&args, "query").unwrap_or_default(), arg_u64(&args, "limit")).await?;
        let text = skill_hit_lines(&hits).join("\n");
        Ok(as_text(&text, json!({ "skills": json_of(&hits) })))
    })
}

/// The `skills_show` tool's call (`verbs/agents.ts:132-135`).
pub(super) fn call_skills_show(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let (thread, on) = (arg_str(&args, "thread"), arg_str(&args, "on"));
        let client = deps.client(None).await?;
        let project = project_asked(arg_project(&args, "project").as_ref(), thread, on, "skills_show")?;
        let shown = skill_shown(
            &*client,
            arg_str(&args, "skill").unwrap_or_default(),
            thread,
            on,
            &project,
            &aimed_usage("skills_show"),
            "wsp skills show",
            cloud_on(),
        )
        .await?;
        Ok(as_text(&shown_text(&shown), json_of(&shown)))
    })
}

/// The `skills_add` tool's call (`verbs/agents.ts:163-166`).
pub(super) fn call_skills_add(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let (skill, thread, on) = (arg_str(&args, "skill").unwrap_or_default(), arg_str(&args, "thread"), arg_str(&args, "on"));
        let agents = arg_list(&args, "agent");
        let client = deps.client(None).await?;
        let project = project_asked(arg_project(&args, "project").as_ref(), thread, on, "skills_add")?;
        let added =
            skill_added(&*client, skill, thread, on, agents.as_deref(), &project, &aimed_usage("skills_add"), "wsp skills add", cloud_on())
                .await?;
        Ok(as_text(&added_line(skill, &added), json_of(&added)))
    })
}

/// The `skills_remove` tool's call (`verbs/agents.ts:188-191`).
pub(super) fn call_skills_remove(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let (name, thread, on) = (arg_str(&args, "name").unwrap_or_default(), arg_str(&args, "thread"), arg_str(&args, "on"));
        let client = deps.client(None).await?;
        let project = project_asked(arg_project(&args, "project").as_ref(), thread, on, "skills_remove")?;
        let removed = skill_changed(
            &*client,
            SkillChange::Remove,
            name,
            thread,
            on,
            &project,
            None,
            &aimed_usage("skills_remove"),
            "wsp skills remove",
            cloud_on(),
        )
        .await?;
        Ok(as_text(&removed_line(name, &removed), json!({ "removed": removed })))
    })
}

/// The `skills_disable` and `skills_enable` tools' call (`verbs/agents.ts:212-215`).
async fn skills_toggle_call(args: &Frame, deps: &VerbDeps, enable: bool) -> Result<CallToolResult, HostError> {
    let word = if enable { "enable" } else { "disable" };
    let (name, thread, on) = (arg_str(args, "name").unwrap_or_default(), arg_str(args, "thread"), arg_str(args, "on"));
    let client = deps.client(None).await?;
    let no_project = ProjectAsked { project: false, name: None };
    let paths = skill_changed(
        &*client,
        SkillChange::Toggle,
        name,
        thread,
        on,
        &no_project,
        Some(enable),
        &aimed_usage(&format!("skills_{word}")),
        &format!("wsp skills {word}"),
        cloud_on(),
    )
    .await?;
    Ok(as_text(&turned_line(name, enable), json!({ "paths": paths })))
}

pub(super) fn call_skills_disable(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move { skills_toggle_call(&args, deps, false).await })
}

pub(super) fn call_skills_enable(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move { skills_toggle_call(&args, deps, true).await })
}

/// The `servers` tool's call (`verbs/agents.ts:235-238`).
pub(super) fn call_servers(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let report = agents_read(
            &*client,
            ReadAim {
                thread: arg_str(&args, "thread"),
                on: arg_str(&args, "on"),
                usage: &aimed_usage("servers"),
                line: "wsp servers",
                schema_usage: "wsp servers [<thread>] [--on <computer>]",
            },
            cloud_on(),
        )
        .await?;
        let text = server_row_lines(&report).join("\n");
        Ok(as_text(&text, facts_and(&report, "servers", json_of(&report.servers))))
    })
}

/// The `agents_addtools` tool's call (`verbs/agents.ts:302-305`).
pub(super) fn call_agents_addtools(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let agent = arg_str(&args, "agent").unwrap_or_default();
        let client = deps.client(None).await?;
        let added = add_tools(&*client, agent).await?;
        Ok(as_text(&tools_added_line(&agent_name(agent), &added.file), json_of(&added)))
    })
}

/// The `agents_default` tool's call (`verbs/agents.ts:325-325`).
pub(super) fn call_agents_default(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let agent = arg_str(&args, "agent").unwrap_or_default();
        let client = deps.client(None).await?;
        let set = default_agent_set(&*client, agent).await?;
        Ok(as_text(&default_agent_line(agent), json_of(&set)))
    })
}

/// The `agents_set` tool's call (`verbs/agents.ts:367-380`).
pub(super) fn call_agents_set(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let agent = arg_str(&args, "agent").unwrap_or_default();
        let ask = AgentSetAsk {
            model: arg_str(&args, "model").map(str::to_owned),
            effort: arg_str(&args, "effort").map(str::to_owned),
            access: arg_str(&args, "access").map(str::to_owned),
            hide: arg_list(&args, "hide"),
            show: arg_list(&args, "show"),
            order: arg_list(&args, "order"),
            add_model: arg_list(&args, "add_model"),
            drop_model: arg_list(&args, "drop_model"),
            reset: arg_list(&args, "reset"),
        };
        let client = deps.client(None).await?;
        let set = agent_defaults_set(&*client, agent, &ask).await?;
        Ok(as_text(&agent_defaults_line(agent, &set), json!({ "agent": agent, "defaults": json_of(&set) })))
    })
}

/// The `agents_setup` tool's call (`verbs/agents.ts:433-442`): no variable values cross this door.
pub(super) fn call_agents_setup(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let agent = arg_str(&args, "agent").unwrap_or_default();
        let config = arg_str(&args, "config");
        let ask = AgentSetupAsk {
            on: arg_str(&args, "on").map(str::to_owned),
            enabled: arg_bool(&args, "enabled"),
            program: arg_str(&args, "program").map(str::to_owned),
            config: config.map(str::to_owned),
            args: arg_list(&args, "args"),
            unset_env: arg_list(&args, "unset_env"),
            reset: arg_list(&args, "reset"),
        };
        let client = deps.client(None).await?;
        let row = agent_setup_set(&*client, agent, &ask, &IndexMap::new(), "agents_setup takes on, a computer by name").await?;
        let text = agent_setup_lines(&row, config.is_some()).join("\n");
        Ok(as_text(&text, json!({ "agent": json_of(&row) })))
    })
}

/// The `servers_tools` tool's call (`verbs/agents.ts:492-496`).
pub(super) fn call_servers_tools(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let usage = aimed_usage("servers_tools");
        let (name, agent, thread, on) = (
            arg_str(&args, "name").unwrap_or_default(),
            arg_str(&args, "agent").unwrap_or_default(),
            arg_str(&args, "thread"),
            arg_str(&args, "on"),
        );
        let client = deps.client(None).await?;
        let project = tools_project(arg_str(&args, "project"), thread, on, &usage)?;
        let refresh = arg_bool(&args, "refresh") == Some(true);
        let answer =
            server_tools_of(&*client, name, agent, thread, on, refresh, &usage, "wsp servers tools", project.as_deref(), cloud_on())
                .await?;
        Ok(as_text(&tool_lines(name, &answer).join("\n"), json_of(&answer)))
    })
}

/// The `servers_add` tool's call (`verbs/agents.ts:533-540`).
pub(super) fn call_servers_add(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let usage = aimed_usage("servers_add");
        let (name, agent, thread, on) = (
            arg_str(&args, "name").unwrap_or_default(),
            arg_str(&args, "agent").unwrap_or_default(),
            arg_str(&args, "thread"),
            arg_str(&args, "on"),
        );
        let values =
            server_values(&deps.env, &arg_list(&args, "env").unwrap_or_default(), &arg_list(&args, "header").unwrap_or_default(), &usage)?;
        let asked = project_asked(arg_project(&args, "project").as_ref(), thread, on, &usage)?;
        let command = server_command(arg_str(&args, "command"), &usage)?;
        let body = add_body(agent, name, command, arg_str(&args, "url"), values, &asked);
        let client = deps.client(None).await?;
        let added =
            server_changed(&*client, ServerOp::Add, &body, thread, on, &usage, "wsp servers add", asked.name.as_deref(), cloud_on())
                .await?;
        Ok(as_text(&is_in_line(name, &added.file), json_of(&added)))
    })
}

/// The `servers_remove` tool's call (`verbs/agents.ts:565-570`).
pub(super) fn call_servers_remove(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let usage = aimed_usage("servers_remove");
        let (name, agent, thread, on) = (
            arg_str(&args, "name").unwrap_or_default(),
            arg_str(&args, "agent").unwrap_or_default(),
            arg_str(&args, "thread"),
            arg_str(&args, "on"),
        );
        let asked = project_asked(arg_project(&args, "project").as_ref(), thread, on, &usage)?;
        let client = deps.client(None).await?;
        let scope = server_scope(arg_str(&args, "scope"), &usage, &asked)?;
        let body = with_scope(server_body(agent, name), &scope);
        let removed =
            server_changed(&*client, ServerOp::Remove, &body, thread, on, &usage, "wsp servers remove", asked.name.as_deref(), cloud_on())
                .await?;
        Ok(as_text(&gone_from_line(name, &removed.file), json_of(&removed)))
    })
}

/// The `servers_disable` and `servers_enable` tools' call (`verbs/agents.ts:595-600`).
async fn servers_toggle_call(args: &Frame, deps: &VerbDeps, enable: bool) -> Result<CallToolResult, HostError> {
    let word = if enable { "enable" } else { "disable" };
    let usage = aimed_usage(&format!("servers_{word}"));
    let (name, agent, thread, on) = (
        arg_str(args, "name").unwrap_or_default(),
        arg_str(args, "agent").unwrap_or_default(),
        arg_str(args, "thread"),
        arg_str(args, "on"),
    );
    let asked = project_asked(arg_project(args, "project").as_ref(), thread, on, &usage)?;
    let client = deps.client(None).await?;
    let scope = server_scope(arg_str(args, "scope"), &usage, &asked)?;
    let mut body = with_scope(server_body(agent, name), &scope);
    body.insert("on".to_owned(), json!(enable));
    let changed = server_changed(
        &*client,
        ServerOp::Toggle,
        &body,
        thread,
        on,
        &usage,
        &format!("wsp servers {word}"),
        asked.name.as_deref(),
        cloud_on(),
    )
    .await?;
    Ok(as_text(&turned_in_line(name, enable, &changed.file), json_of(&changed)))
}

pub(super) fn call_servers_disable(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move { servers_toggle_call(&args, deps, false).await })
}

pub(super) fn call_servers_enable(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move { servers_toggle_call(&args, deps, true).await })
}

/// The `plugins` tool's call (`verbs/agents.ts:620-623`).
pub(super) fn call_plugins(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let report = agents_read(
            &*client,
            ReadAim {
                thread: arg_str(&args, "thread"),
                on: arg_str(&args, "on"),
                usage: &aimed_usage("plugins"),
                line: "wsp plugins",
                schema_usage: "wsp plugins [<thread>] [--on <computer>]",
            },
            cloud_on(),
        )
        .await?;
        let text = plugin_row_lines(&report).join("\n");
        Ok(as_text(&text, facts_and(&report, "plugins", json_of(&read_plugins(&report)))))
    })
}

/// The `plugins_disable` and `plugins_enable` tools' call (`verbs/agents.ts:646-649`).
async fn plugins_toggle_call(args: &Frame, deps: &VerbDeps, enable: bool) -> Result<CallToolResult, HostError> {
    let word = if enable { "enable" } else { "disable" };
    let (plugin, agent, thread, on) = (
        arg_str(args, "plugin").unwrap_or_default(),
        arg_str(args, "agent").unwrap_or_default(),
        arg_str(args, "thread"),
        arg_str(args, "on"),
    );
    let client = deps.client(None).await?;
    let ask = PluginChange { agent: agent.to_owned(), plugin: plugin.to_owned(), on: enable };
    let row =
        plugin_changed(&*client, &ask, thread, on, &aimed_usage(&format!("plugins_{word}")), &format!("wsp plugins {word}"), cloud_on())
            .await?;
    Ok(as_text(&plugin_turned_line(&row), json!({ "plugin": json_of(&row) })))
}

pub(super) fn call_plugins_disable(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move { plugins_toggle_call(&args, deps, false).await })
}

pub(super) fn call_plugins_enable(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move { plugins_toggle_call(&args, deps, true).await })
}
