// SPDX-License-Identifier: AGPL-3.0-only
//! A kept server's commands still going, by item id, across its turns. TS adapter-codex/src/background.ts. Spec:
//! spec/codex_turn.md rule 22.

use indexmap::IndexSet;

use super::fields::{rec, str};
use super::rpc::{read_message, thread_unsubscribe_line, RpcMessage};
use crate::port::ExecStream;

/// TS background.ts:8.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Background {
    pub commands: IndexSet<String>,
    /// The thread a turn's end kept subscribed for the commands, let go once the last one ends.
    pub held: Option<String>,
}

/// A line the server printed between turns, where a command a turn left going may end. TS background.ts:15.
pub fn ended_between(run: &dyn ExecStream, background: &mut Background, raw: &str) {
    let Some(RpcMessage::Notification { method, params, .. }) = read_message(raw) else { return };
    if method != "item/completed" {
        return;
    }
    let Some(item) = rec(params.get("item")) else { return };
    if str(item.get("type")) != Some("commandExecution") {
        return;
    }
    let Some(id) = str(item.get("id")) else { return };
    background.commands.shift_remove(id);
    if !background.commands.is_empty() {
        return;
    }
    let Some(thread) = background.held.take() else { return };
    let _ = tokio::task::spawn_local(run.write(thread_unsubscribe_line(&thread)));
}
