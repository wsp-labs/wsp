// SPDX-License-Identifier: AGPL-3.0-only
//! What every `wire.rs` is handed: the host's own wiring and a command line's run, the two things `cli/wiring.ts` and
//! `cli.ts` compose each road from.
use indexmap::IndexMap;
use wsp_cli::verbs::client::{failed, CliIo};
use wsp_words::exit::HostError;

pub type Env = IndexMap<String, String>;

/// What `makeRuntime(keys, statePath, ...)` of `cli/wiring.ts` composes each port from: the state file, this
/// computer's home, the environment, the loopback address a turn here dials back once the host has bound it, and the
/// words that start this same wsp.
#[derive(Clone, Debug)]
pub struct HostWiring {
    pub state_path: String,
    pub home: String,
    pub env: Env,
    pub here_url: Option<String>,
    pub wsp: Vec<String>,
}

/// What a command line is run with: the environment it was made in, the words that start this same wsp, and the folder
/// it started in.
#[derive(Clone, Debug)]
pub struct CommandRun {
    pub env: Env,
    pub wsp: Vec<String>,
    pub cwd: String,
}

/// The refusal every command line of `wsp-setup` answers with until its packet lands: `word` is the line's first word.
pub fn not_in_build(io: &dyn CliIo, json: bool, word: &str) -> i32 {
    failed(io, json, &HostError::new(format!("wsp {word} is not in this build of the command line yet")), "")
}
