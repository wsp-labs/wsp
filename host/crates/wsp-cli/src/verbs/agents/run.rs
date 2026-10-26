// SPDX-License-Identifier: AGPL-3.0-only
//! The agents verbs' command line runs (`verbs/agents.ts`); the entries are in `agents.rs`.

use indexmap::IndexMap;
use serde_json::json;

use wsp_catalog::catalog::{agent_name, catalog_entry};
use wsp_catalog::signin::mints_token;
use wsp_wire::types::agents_report::{AgentsTarget, AgentsTargetV0, AgentsTargetV1};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::thread_defaults::ENV_REFUSED_FIX;
use wsp_words::exit::{usage_refusal, HostError};
use wsp_words::js::{js_trim, to_number};
use wsp_words::thread_defaults::agent_env_refusal;
use wsp_words::words::computer::THIS_COMPUTER;

use super::super::agents_help::{
    add_tools, added_line, agent_defaults_line, agent_defaults_set, agent_row_lines, agent_setup_lines, agent_setup_set, agents_target,
    default_agent_line, default_agent_set, env_name_of, gone_from_line, is_in_line, plugin_changed, plugin_row_lines, plugin_turned_line,
    project_asked, removed_line, search_skills_sh, server_changed, server_command, server_row_lines, server_scope, server_tools_of,
    server_values, shown_text, sign_in_here, signed_in_line, skill_added, skill_changed, skill_hit_lines, skill_row_lines, skill_shown,
    tool_lines, tools_added_line, tools_project, turned_in_line, turned_line, AgentSetAsk, AgentSetupAsk, PluginChange, ProjectAsked,
    ServerOp, SignInAsk, SkillChange,
};
use super::super::client::*;
use super::super::io::confirmed;
use super::{add_body, agents_read, facts_and, flag_project, json_of, server_body, sign_in_workspace, with_scope, ReadAim};
use crate::cloud::cloud_on;

/// The one positional and the optional thread after it, or None for any other count.
fn one_and_thread(args: &[String]) -> Option<(&str, Option<&str>)> {
    match args {
        [first] => Some((first.as_str(), None)),
        [first, thread] => Some((first.as_str(), Some(thread.as_str()))),
        _ => None,
    }
}

/// `wsp agents` (`verbs/agents.ts:52-57`).
pub(super) fn run_agents(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if ctx.args.len() > 1 {
            return Err(usage_refusal("wsp agents takes one thread at most.", &usage_is(ctx)));
        }
        let client = ctx.client(None).await?;
        let report = agents_read(
            &*client,
            ReadAim {
                thread: ctx.args.first().map(String::as_str),
                on: flag(&ctx.flags, "on"),
                usage: &usage_is(ctx),
                line: "wsp agents",
                schema_usage: "wsp agents [<thread>] [--on <computer>]",
            },
            cloud_on(),
        )
        .await?;
        let line = agent_row_lines(&report).join("\n");
        ctx.out.emit(&facts_and(&report, "agents", json_of(&report.agents)), Some(line.as_str()));
        Ok(0)
    })
}

/// `wsp skills` (`verbs/agents.ts:74-79`).
pub(super) fn run_skills(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if ctx.args.len() > 1 {
            return Err(usage_refusal("wsp skills takes one thread at most.", &usage_is(ctx)));
        }
        let client = ctx.client(None).await?;
        let report = agents_read(
            &*client,
            ReadAim {
                thread: ctx.args.first().map(String::as_str),
                on: flag(&ctx.flags, "on"),
                usage: &usage_is(ctx),
                line: "wsp skills",
                schema_usage: "wsp skills [<thread>] [--on <computer>]",
            },
            cloud_on(),
        )
        .await?;
        let line = skill_row_lines(&report).join("\n");
        ctx.out.emit(&facts_and(&report, "skills", json_of(&report.skills)), Some(line.as_str()));
        Ok(0)
    })
}

/// `wsp skills search` (`verbs/agents.ts:96-104`).
pub(super) fn run_skills_search(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let query = ctx.args.join(" ");
        let limit = flag(&ctx.flags, "limit").map(to_number);
        if let Some(limit) = limit {
            if !(limit.is_finite() && limit.fract() == 0.0 && (1.0..=50.0).contains(&limit)) {
                return Err(usage_refusal("--limit takes a whole number from 1 to 50.", &usage_is(ctx)));
            }
        }
        let client = ctx.client(None).await?;
        let hits = search_skills_sh(&*client, &query, limit.map(|n| n as u64)).await?;
        let line = skill_hit_lines(&hits).join("\n");
        ctx.out.emit(&json!({ "skills": json_of(&hits) }), Some(line.as_str()));
        Ok(0)
    })
}

/// `wsp skills show` (`verbs/agents.ts:121-127`).
pub(super) fn run_skills_show(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let usage = usage_is(ctx);
        let Some((skill, thread)) = one_and_thread(&ctx.args) else {
            return Err(usage_refusal("wsp skills show takes one skill and one thread at most.", &usage));
        };
        let on = flag(&ctx.flags, "on");
        let client = ctx.client(None).await?;
        let project = project_asked(flag_project(&ctx.flags, "project").as_ref(), thread, on, &usage)?;
        let shown = skill_shown(&*client, skill, thread, on, &project, &usage, "wsp skills show", cloud_on()).await?;
        let text = shown_text(&shown);
        ctx.out.emit(&json_of(&shown), Some(text.as_str()));
        Ok(0)
    })
}

/// `wsp skills add` (`verbs/agents.ts:144-151`).
pub(super) fn run_skills_add(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let usage = usage_is(ctx);
        let Some((skill, thread)) = one_and_thread(&ctx.args) else {
            return Err(usage_refusal("wsp skills add takes one skill and one thread at most.", &usage));
        };
        let on = flag(&ctx.flags, "on");
        let agents = flag_list(&ctx.flags, "agent");
        let client = ctx.client(None).await?;
        let project = project_asked(flag_project(&ctx.flags, "project").as_ref(), thread, on, &usage)?;
        let agents = (!agents.is_empty()).then_some(agents.as_slice());
        let added = skill_added(&*client, skill, thread, on, agents, &project, &usage, "wsp skills add", cloud_on()).await?;
        let line = added_line(skill, &added);
        ctx.out.emit(&json_of(&added), Some(line.as_str()));
        Ok(0)
    })
}

/// `wsp skills remove` (`verbs/agents.ts:175-183`).
pub(super) fn run_skills_remove(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let usage = usage_is(ctx);
        let Some((name, thread)) = one_and_thread(&ctx.args) else {
            return Err(usage_refusal("wsp skills remove takes one skill's name and one thread at most.", &usage));
        };
        let on = flag(&ctx.flags, "on");
        let project = project_asked(flag_project(&ctx.flags, "project").as_ref(), thread, on, &usage)?;
        let question = format!("Remove the skill {name}?\nEvery folder it lives in and every link to it go.");
        if !confirmed(ctx, &question, name).await? {
            return Ok(1);
        }
        let client = ctx.client(None).await?;
        let removed =
            skill_changed(&*client, SkillChange::Remove, name, thread, on, &project, None, &usage, "wsp skills remove", cloud_on()).await?;
        let line = removed_line(name, &removed);
        ctx.out.emit(&json!({ "removed": removed }), Some(line.as_str()));
        Ok(0)
    })
}

/// `wsp skills disable|enable` (`verbs/agents.ts:201-207`): the one template, `enable` the word that differs.
async fn skills_toggle(ctx: &VerbContext, enable: bool) -> Result<i32, HostError> {
    let word = if enable { "enable" } else { "disable" };
    let usage = usage_is(ctx);
    let Some((name, thread)) = one_and_thread(&ctx.args) else {
        return Err(usage_refusal(&format!("wsp skills {word} takes one skill's name and one thread at most."), &usage));
    };
    let on = flag(&ctx.flags, "on");
    let client = ctx.client(None).await?;
    let no_project = ProjectAsked { project: false, name: None };
    let paths = skill_changed(
        &*client,
        SkillChange::Toggle,
        name,
        thread,
        on,
        &no_project,
        Some(enable),
        &usage,
        &format!("wsp skills {word}"),
        cloud_on(),
    )
    .await?;
    let line = turned_line(name, enable);
    ctx.out.emit(&json!({ "paths": paths }), Some(line.as_str()));
    Ok(0)
}

pub(super) fn run_skills_disable(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move { skills_toggle(ctx, false).await })
}

pub(super) fn run_skills_enable(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move { skills_toggle(ctx, true).await })
}

/// `wsp servers` (`verbs/agents.ts:225-230`).
pub(super) fn run_servers(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if ctx.args.len() > 1 {
            return Err(usage_refusal("wsp servers takes one thread at most.", &usage_is(ctx)));
        }
        let client = ctx.client(None).await?;
        let report = agents_read(
            &*client,
            ReadAim {
                thread: ctx.args.first().map(String::as_str),
                on: flag(&ctx.flags, "on"),
                usage: &usage_is(ctx),
                line: "wsp servers",
                schema_usage: "wsp servers [<thread>] [--on <computer>]",
            },
            cloud_on(),
        )
        .await?;
        let line = server_row_lines(&report).join("\n");
        ctx.out.emit(&facts_and(&report, "servers", json_of(&report.servers)), Some(line.as_str()));
        Ok(0)
    })
}

/// `wsp agents signin` (`verbs/agents.ts:248-257`).
pub(super) fn run_agents_signin(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let usage = usage_is(ctx);
        let Some((agent, thread)) = one_and_thread(&ctx.args) else {
            return Err(usage_refusal("wsp agents signin takes one agent and one thread at most.", &usage));
        };
        let client = ctx.client(None).await?;
        let target = match thread {
            None => agents_target(&*client, None, None, &usage, "wsp agents signin", None, cloud_on()).await?,
            Some(thread) => AgentsTarget::V1(AgentsTargetV1 { workspace_id: sign_in_workspace(&*client, agent, thread).await? }),
        };
        let place = match thread {
            None => THIS_COMPUTER.to_owned(),
            Some(thread) => format!("thread {thread}"),
        };
        let answer = sign_in_here(ctx, &target, &SignInAsk { agent: agent.to_owned(), name: None }, &place).await?;
        ctx.io.log(&signed_in_line(&agent_name(agent), &place, &answer));
        Ok(if answer.signed_in { 0 } else { 1 })
    })
}

/// `wsp agents key` (`verbs/agents.ts:267-283`).
pub(super) fn run_agents_key(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let usage = usage_is(ctx);
        let agent = match ctx.args.as_slice() {
            [agent] => agent.as_str(),
            _ => return Err(usage_refusal("wsp agents key takes one agent.", &usage)),
        };
        if !ctx.io.is_tty() {
            return Err(usage_refusal(
                "nobody is at this terminal to paste a token.",
                "Run it in a terminal on the computer the host runs on.",
            ));
        }
        let api_key = flag_set(&ctx.flags, "api-key");
        if agent == "codex" && !api_key {
            let target = AgentsTarget::V0(AgentsTargetV0 { place_id: HERE_PLACE_ID.to_owned(), project: None });
            let answer = sign_in_here(ctx, &target, &SignInAsk { agent: agent.to_owned(), name: None }, THIS_COMPUTER).await?;
            ctx.io.log(&signed_in_line(&agent_name(agent), THIS_COMPUTER, &answer));
            return Ok(if answer.signed_in { 0 } else { 1 });
        }
        let mint = catalog_entry(agent).and_then(|entry| mints_token(entry.sign_in())).map(|token| token.mint.clone());
        let question = match mint {
            Some(mint) if !api_key => format!("Run {mint} in another terminal, then paste the token it prints"),
            _ => format!("Paste {}'s API key", agent_name(agent)),
        };
        let key = ctx.io.ask_secret(&question, None).await?;
        let mut params = Frame::new();
        params.insert("agent".to_owned(), json!(agent));
        params.insert("key".to_owned(), json!(key));
        if api_key {
            params.insert("way".to_owned(), json!("key"));
        }
        let client = ctx.client(None).await?;
        client.request("agents.key", params).await?;
        ctx.io.log(&format!("{}'s key is in this host's vault; every turn reads it from there.", agent_name(agent)));
        Ok(0)
    })
}

/// `wsp agents addtools` (`verbs/agents.ts:291-297`).
pub(super) fn run_agents_addtools(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let usage = usage_is(ctx);
        let agent = match ctx.args.as_slice() {
            [agent] => agent.as_str(),
            _ => return Err(usage_refusal("wsp agents addtools takes one agent.", &usage)),
        };
        let client = ctx.client(None).await?;
        let added = add_tools(&*client, agent).await?;
        let line = tools_added_line(&agent_name(agent), &added.file);
        ctx.out.emit(&json_of(&added), Some(line.as_str()));
        Ok(0)
    })
}

/// `wsp agents default` (`verbs/agents.ts:314-320`).
pub(super) fn run_agents_default(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let usage = usage_is(ctx);
        let agent = match ctx.args.as_slice() {
            [agent] => agent.as_str(),
            _ => return Err(usage_refusal("wsp agents default takes one agent.", &usage)),
        };
        let client = ctx.client(None).await?;
        let set = default_agent_set(&*client, agent).await?;
        let line = default_agent_line(agent);
        ctx.out.emit(&json_of(&set), Some(line.as_str()));
        Ok(0)
    })
}

/// `wsp agents set` (`verbs/agents.ts:334-351`).
pub(super) fn run_agents_set(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let usage = usage_is(ctx);
        let agent = match ctx.args.as_slice() {
            [agent] => agent.as_str(),
            _ => return Err(usage_refusal("wsp agents set takes one agent.", &usage)),
        };
        let order = flag(&ctx.flags, "order")
            .map(|order| order.split(',').map(js_trim).filter(|m| !m.is_empty()).map(str::to_owned).collect::<Vec<String>>());
        let ask = AgentSetAsk {
            model: flag(&ctx.flags, "model").map(str::to_owned),
            effort: flag(&ctx.flags, "effort").map(str::to_owned),
            access: flag(&ctx.flags, "access").map(str::to_owned),
            hide: Some(flag_list(&ctx.flags, "hide")),
            show: Some(flag_list(&ctx.flags, "show")),
            order,
            add_model: Some(flag_list(&ctx.flags, "add-model")),
            drop_model: Some(flag_list(&ctx.flags, "drop-model")),
            reset: Some(flag_list(&ctx.flags, "reset")),
        };
        let client = ctx.client(None).await?;
        let set = agent_defaults_set(&*client, agent, &ask).await?;
        let line = agent_defaults_line(agent, &set);
        ctx.out.emit(&json!({ "agent": agent, "defaults": json_of(&set) }), Some(line.as_str()));
        Ok(0)
    })
}

/// `wsp agents setup` (`verbs/agents.ts:389-419`).
pub(super) fn run_agents_setup(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let usage = usage_is(ctx);
        let agent = match ctx.args.as_slice() {
            [agent] => agent.as_str(),
            _ => return Err(usage_refusal("wsp agents setup takes one agent.", &usage)),
        };
        let (enable, disable) = (flag_set(&ctx.flags, "enable"), flag_set(&ctx.flags, "disable"));
        if enable && disable {
            return Err(usage_refusal("--enable and --disable say two things.", "Name one."));
        }
        let names =
            flag_list(&ctx.flags, "env").iter().map(|name| env_name_of(name, "--env")).collect::<Result<Vec<String>, HostError>>()?;
        let display = agent_name(agent);
        for name in &names {
            if let Some(refused) = agent_env_refusal(name, &display, &[]) {
                return Err(usage_refusal(&format!("{refused}."), ENV_REFUSED_FIX));
            }
        }
        if !names.is_empty() && !ctx.io.is_tty() {
            return Err(usage_refusal(
                "nobody is at this terminal to type a variable's value.",
                "Run it in a terminal on the computer the host runs on.",
            ));
        }
        let mut values: IndexMap<String, String> = IndexMap::new();
        for name in &names {
            let value = ctx.io.ask_secret(&format!("{name} for {display}"), None).await?;
            values.insert(name.clone(), value);
        }
        let config = flag(&ctx.flags, "config");
        let ask = AgentSetupAsk {
            on: flag(&ctx.flags, "on").map(str::to_owned),
            enabled: if enable {
                Some(true)
            } else if disable {
                Some(false)
            } else {
                None
            },
            program: flag(&ctx.flags, "program").map(str::to_owned),
            config: config.map(str::to_owned),
            args: Some(flag_list(&ctx.flags, "arg")),
            unset_env: Some(flag_list(&ctx.flags, "unset-env")),
            reset: Some(flag_list(&ctx.flags, "reset")),
        };
        let client = ctx.client(None).await?;
        let row = agent_setup_set(&*client, agent, &ask, &values, &usage).await?;
        let text = agent_setup_lines(&row, config.is_some()).join("\n");
        ctx.out.emit(&json!({ "agent": json_of(&row) }), Some(text.as_str()));
        Ok(0)
    })
}

/// `wsp servers signin` (`verbs/agents.ts:452-463`).
pub(super) fn run_servers_signin(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let usage = usage_is(ctx);
        let Some((name, thread)) = one_and_thread(&ctx.args) else {
            return Err(usage_refusal("wsp servers signin takes one server's name and one thread at most.", &usage));
        };
        let Some(agent) = flag(&ctx.flags, "agent") else {
            return Err(usage_refusal(
                "wsp servers signin needs --agent, the agent whose config names the server, as wsp servers shows it.",
                &usage,
            ));
        };
        let on = flag(&ctx.flags, "on");
        let client = ctx.client(None).await?;
        let target = agents_target(&*client, thread, on, &usage, "wsp servers signin", None, cloud_on()).await?;
        let place = match (thread, on) {
            (Some(thread), _) => format!("thread {thread}"),
            (None, Some(on)) => on.to_owned(),
            (None, None) => THIS_COMPUTER.to_owned(),
        };
        let ask = SignInAsk { agent: agent.to_owned(), name: Some(name.to_owned()) };
        let answer = sign_in_here(ctx, &target, &ask, &place).await?;
        ctx.io.log(&signed_in_line(name, &place, &answer));
        Ok(if answer.signed_in { 0 } else { 1 })
    })
}

/// `wsp servers tools` (`verbs/agents.ts:471-480`).
pub(super) fn run_servers_tools(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let usage = usage_is(ctx);
        let Some((name, thread)) = one_and_thread(&ctx.args) else {
            return Err(usage_refusal("wsp servers tools takes one server's name and one thread at most.", &usage));
        };
        let Some(agent) = flag(&ctx.flags, "agent") else {
            return Err(usage_refusal(
                "wsp servers tools needs --agent, the agent whose config names the server, as wsp servers shows it.",
                &usage,
            ));
        };
        let on = flag(&ctx.flags, "on");
        let project = tools_project(flag(&ctx.flags, "project"), thread, on, &usage)?;
        let client = ctx.client(None).await?;
        let refresh = flag_set(&ctx.flags, "refresh");
        let answer =
            server_tools_of(&*client, name, agent, thread, on, refresh, &usage, "wsp servers tools", project.as_deref(), cloud_on())
                .await?;
        let text = tool_lines(name, &answer).join("\n");
        ctx.out.emit(&json_of(&answer), Some(text.as_str()));
        Ok(0)
    })
}

/// `wsp servers add` (`verbs/agents.ts:505-518`).
pub(super) fn run_servers_add(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let usage = usage_is(ctx);
        let Some((name, thread)) = one_and_thread(&ctx.args) else {
            return Err(usage_refusal("wsp servers add takes one server's name and one thread at most.", &usage));
        };
        let Some(agent) = flag(&ctx.flags, "agent") else {
            return Err(usage_refusal("wsp servers add needs --agent, the agent whose config takes the server.", &usage));
        };
        let on = flag(&ctx.flags, "on");
        let values = server_values(&ctx.env, &flag_list(&ctx.flags, "env"), &flag_list(&ctx.flags, "header"), &usage)?;
        let project = project_asked(flag_project(&ctx.flags, "project").as_ref(), thread, on, &usage)?;
        let command = server_command(flag(&ctx.flags, "command"), &usage)?;
        let body = add_body(agent, name, command, flag(&ctx.flags, "url"), values, &project);
        let client = ctx.client(None).await?;
        let added =
            server_changed(&*client, ServerOp::Add, &body, thread, on, &usage, "wsp servers add", project.name.as_deref(), cloud_on())
                .await?;
        let line = is_in_line(name, &added.file);
        ctx.out.emit(&json_of(&added), Some(line.as_str()));
        Ok(0)
    })
}

/// `wsp servers remove` (`verbs/agents.ts:549-560`).
pub(super) fn run_servers_remove(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let usage = usage_is(ctx);
        let Some((name, thread)) = one_and_thread(&ctx.args) else {
            return Err(usage_refusal("wsp servers remove takes one server's name and one thread at most.", &usage));
        };
        let Some(agent) = flag(&ctx.flags, "agent") else {
            return Err(usage_refusal(
                "wsp servers remove needs --agent, the agent whose config names the server, as wsp servers shows it.",
                &usage,
            ));
        };
        let on = flag(&ctx.flags, "on");
        let project = project_asked(flag_project(&ctx.flags, "project").as_ref(), thread, on, &usage)?;
        let scope = server_scope(flag(&ctx.flags, "scope"), &usage, &project)?;
        let question = format!("Remove {name} from the {agent} config?\nEvery other line of the file stays.");
        if !confirmed(ctx, &question, name).await? {
            return Ok(1);
        }
        let body = with_scope(server_body(agent, name), &scope);
        let client = ctx.client(None).await?;
        let removed = server_changed(
            &*client,
            ServerOp::Remove,
            &body,
            thread,
            on,
            &usage,
            "wsp servers remove",
            project.name.as_deref(),
            cloud_on(),
        )
        .await?;
        let line = gone_from_line(name, &removed.file);
        ctx.out.emit(&json_of(&removed), Some(line.as_str()));
        Ok(0)
    })
}

/// `wsp servers disable|enable` (`verbs/agents.ts:580-590`): the one template, `enable` the word that differs.
async fn servers_toggle(ctx: &VerbContext, enable: bool) -> Result<i32, HostError> {
    let word = if enable { "enable" } else { "disable" };
    let usage = usage_is(ctx);
    let Some((name, thread)) = one_and_thread(&ctx.args) else {
        return Err(usage_refusal(&format!("wsp servers {word} takes one server's name and one thread at most."), &usage));
    };
    let Some(agent) = flag(&ctx.flags, "agent") else {
        return Err(usage_refusal(
            &format!("wsp servers {word} needs --agent, the agent whose config names the server, as wsp servers shows it."),
            &usage,
        ));
    };
    let on = flag(&ctx.flags, "on");
    let project = project_asked(flag_project(&ctx.flags, "project").as_ref(), thread, on, &usage)?;
    let scope = server_scope(flag(&ctx.flags, "scope"), &usage, &project)?;
    let mut body = with_scope(server_body(agent, name), &scope);
    body.insert("on".to_owned(), json!(enable));
    let client = ctx.client(None).await?;
    let changed = server_changed(
        &*client,
        ServerOp::Toggle,
        &body,
        thread,
        on,
        &usage,
        &format!("wsp servers {word}"),
        project.name.as_deref(),
        cloud_on(),
    )
    .await?;
    let line = turned_in_line(name, enable, &changed.file);
    ctx.out.emit(&json_of(&changed), Some(line.as_str()));
    Ok(0)
}

pub(super) fn run_servers_disable(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move { servers_toggle(ctx, false).await })
}

pub(super) fn run_servers_enable(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move { servers_toggle(ctx, true).await })
}

/// `wsp plugins` (`verbs/agents.ts:610-615`).
pub(super) fn run_plugins(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if ctx.args.len() > 1 {
            return Err(usage_refusal("wsp plugins takes one thread at most.", &usage_is(ctx)));
        }
        let client = ctx.client(None).await?;
        let report = agents_read(
            &*client,
            ReadAim {
                thread: ctx.args.first().map(String::as_str),
                on: flag(&ctx.flags, "on"),
                usage: &usage_is(ctx),
                line: "wsp plugins",
                schema_usage: "wsp plugins [<thread>] [--on <computer>]",
            },
            cloud_on(),
        )
        .await?;
        let line = plugin_row_lines(&report).join("\n");
        ctx.out.emit(&facts_and(&report, "plugins", json_of(&report.plugins.clone().unwrap_or_default())), Some(line.as_str()));
        Ok(0)
    })
}

/// `wsp plugins disable|enable` (`verbs/agents.ts:633-641`): the one template, `enable` the word that differs.
async fn plugins_toggle(ctx: &VerbContext, enable: bool) -> Result<i32, HostError> {
    let word = if enable { "enable" } else { "disable" };
    let usage = usage_is(ctx);
    let Some((plugin, thread)) = one_and_thread(&ctx.args) else {
        return Err(usage_refusal(&format!("wsp plugins {word} takes one plugin's id and one thread at most."), &usage));
    };
    let Some(agent) = flag(&ctx.flags, "agent") else {
        return Err(usage_refusal(
            &format!("wsp plugins {word} needs --agent, the agent whose plugin it is, as wsp plugins shows it."),
            &usage,
        ));
    };
    let on = flag(&ctx.flags, "on");
    let client = ctx.client(None).await?;
    let ask = PluginChange { agent: agent.to_owned(), plugin: plugin.to_owned(), on: enable };
    let row = plugin_changed(&*client, &ask, thread, on, &usage, &format!("wsp plugins {word}"), cloud_on()).await?;
    let line = plugin_turned_line(&row);
    ctx.out.emit(&json!({ "plugin": json_of(&row) }), Some(line.as_str()));
    Ok(0)
}

pub(super) fn run_plugins_disable(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move { plugins_toggle(ctx, false).await })
}

pub(super) fn run_plugins_enable(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move { plugins_toggle(ctx, true).await })
}
