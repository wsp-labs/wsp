// SPDX-License-Identifier: AGPL-3.0-only
//! The host process and the command dispatcher, thin: what `up`, `mcp` and the command line need of `cli/wiring.ts`,
//! `server.ts` and `cli.ts` (chapter 01's `wsp-host` row, ruling 2 on #2476). The 27 host ports and the serve doors
//! that are `wsp-setup`'s are not here; the runtime answers each as the TS does for a road that is missing.
pub mod broker;
pub mod broker_sign_ins;
pub mod cli;
pub mod commands;
pub mod envfile;
pub mod file_doors;
pub mod git_roads;
pub mod guest;
pub mod io;
pub mod mcp;
pub mod paths;
pub mod serve;
pub mod server;
pub mod start;
pub mod state;
pub mod up;
pub mod wiring;

use std::sync::Arc;

use cli::{read_line, run_verb_line, up_line, Line, Running};
use io::TerminalIo;

/// Runs one command line and answers the exit code it ends the process with.
pub fn run(argv: Vec<String>) -> i32 {
    let io = Arc::new(TerminalIo::new());
    let run = Running::here();
    let line = match read_line(&argv, io.as_ref(), &run) {
        Line::Done(code) => return code,
        Line::Mcp(rest) => return mcp::mcp(io.as_ref(), &rest, &run),
        other => other,
    };
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async move {
        match line {
            Line::Verb(words) => run_verb_line(words, io, &run).await,
            Line::Up(words) => up_line(words, io, &run).await,
            Line::Done(code) => code,
            Line::Mcp(_) => unreachable!("the tool server runs before a runtime is built"),
        }
    })
}
