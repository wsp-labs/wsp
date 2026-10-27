// SPDX-License-Identifier: AGPL-3.0-only
//! `target-road.ts`: where a write on one computer or workspace runs, and the reader's Host over the same road.
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use wsp_collect::host::Host;
use wsp_machines::agents_read::AgentsOn;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

/// `WRITE_MS` (`target-road.ts:14`).
pub const WRITE_MS: u64 = 60_000;

/// `Road` (`target-road.ts:17`), where a target's lines run: the reader's Host over it, and one line as its login with bytes on its stdin.
pub struct Road {
    pub host: Host,
    pub run: Rc<dyn Fn(&str, Option<&[u8]>) -> LocalBoxFuture<'static, Result<ExecResult, HostError>>>,
}

/// `runHere` (`target-road.ts:23`).
pub async fn run_here(_home: &str, _line: &str, _stdin: Option<&[u8]>, _ms: u64) -> ExecResult {
    todo!()
}

/// `roadOf` (`target-road.ts:50`).
pub async fn road_of(_on: &AgentsOn, _here: Rc<dyn Fn() -> Host>, _what: &str) -> Result<Road, HostError> {
    todo!()
}

/// `firstLine` (`target-road.ts:69`).
pub fn first_line(_r: &ExecResult) -> String {
    todo!()
}
