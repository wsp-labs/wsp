// SPDX-License-Identifier: AGPL-3.0-only
//! `wsp mcp` (`cli/mcp.ts`): the tool server on this process's own stdio until the agent closes its stdin. The server is
//! `wsp-mcp`'s, which dials the host the way the command line does. `wsp mcp install` writes an agent's config and is
//! `wsp-setup`'s.
use std::collections::HashMap;

use wsp_cli::host_lock::STARTED_BY_ENV;
use wsp_cli::hosts::empty_host_refusal;
use wsp_cli::parse_args::parse_args;
use wsp_cli::verbs::client::{failed, flag, flag_set, json_asked, CliIo, FlagKind, FlagRow, FlagTable};
use wsp_wire::types::env::SCOPED_MCP_ARG;
use wsp_wire::views::session_events::host_from_env;
use wsp_words::exit::{auth_refusal, usage_refusal, HostError};
use wsp_words::words::computer::SCOPED_NO_PAIR_LINE;
use wsp_words::words::thread::{run_for_the_list, unknown_word_line};

use crate::cli::{Running, MCP_COMMAND};
use crate::state::state_path_from;
use crate::wiring::person_home;

/// `MCP_OPTIONS`: the flags `wsp mcp` and `wsp mcp install` parse.
fn mcp_options() -> FlagTable {
    FlagTable(vec![
        ("agent", FlagRow::STRINGS),
        ("host", FlagRow::STRING),
        ("json", FlagRow::BOOLEAN),
        ("remove", FlagRow::BOOLEAN),
        ("state", FlagRow::STRING),
        ("scoped", FlagRow::BOOLEAN),
        ("no-slate", FlagRow::BOOLEAN),
        ("help", FlagRow { kind: FlagKind::Boolean, multiple: false, short: Some('h'), value_with: None }),
    ])
}

const MCP_INSTALL_USAGE: &str = "wsp mcp install --agent <id> [--agent <id>] [--host <alias>] [--json] [--remove]";

fn mcp_usage() -> String {
    format!("usage: wsp {MCP_COMMAND} [--host <alias>] [{SCOPED_MCP_ARG} [--no-slate]]\n       {MCP_INSTALL_USAGE}")
}

/// The tool server, on this thread: it builds the runtime it dials the host with, so none runs around it.
pub fn mcp(io: &dyn CliIo, argv: &[String], run: &Running) -> i32 {
    let usage = mcp_usage();
    let json = json_asked(argv);
    let parsed = match parse_args(argv, &mcp_options()) {
        Ok(parsed) => parsed,
        Err(e) => return failed(io, json, &usage_refusal(&e.message, &usage), ""),
    };
    let (values, words) = (parsed.values, parsed.positionals);
    if flag_set(&values, "help") {
        io.log(&usage);
        return 0;
    }
    if flag(&values, "host").is_some_and(|host| host.trim().is_empty()) {
        return failed(io, json, &empty_host_refusal(), "");
    }
    let scoped = flag_set(&values, "scoped");
    if flag_set(&values, "no-slate") && !scoped {
        let refusal = usage_refusal(&format!("--no-slate goes with {SCOPED_MCP_ARG}: it is for a thread another thread started"), &usage);
        return failed(io, json, &refusal, "");
    }
    // Ahead of every reading of the state: a scoped server missing its pair would otherwise dial this computer's host on
    // the host's own token, which is acting as the person.
    let launched: HashMap<String, String> = run.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    if scoped && words.is_empty() && host_from_env(&launched).is_none() {
        return failed(io, json, &auth_refusal(SCOPED_NO_PAIR_LINE), "");
    }
    if !words.is_empty() {
        let refusal = if words[0] == "install" && words.len() == 1 {
            HostError::new("wsp mcp install is not in this build of the command line yet")
        } else {
            usage_refusal(
                &unknown_word_line(&format!("{MCP_COMMAND} {}", words.join(" "))),
                &run_for_the_list(&format!("wsp {MCP_COMMAND} --help")),
            )
        };
        return failed(io, json, &refusal, "");
    }
    let note = |line: &str| io.error(line);
    let state = state_path_from(flag(&values, "state"), &run.env, &run.cwd, &person_home(), &note);
    std::env::remove_var(STARTED_BY_ENV);
    wsp_mcp::run(&wsp_mcp::Args {
        state: state.into(),
        host: flag(&values, "host").map(str::to_string),
        scoped,
        no_slate: flag_set(&values, "no-slate"),
        no_look: false,
        json: flag_set(&values, "json"),
        guest: false,
        wsp: run.wsp.clone(),
    })
}
