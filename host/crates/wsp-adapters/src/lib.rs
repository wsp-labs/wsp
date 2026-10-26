// SPDX-License-Identifier: AGPL-3.0-only
//! The harness adapters: packages/adapter-{claude,codex,cursor,opencode}, against the port in
//! packages/protocol/src/adapter-port.ts and the runtime's HarnessAdapter (packages/runtime/src/types/harness.ts).

mod kept_run;
mod lost_session;
pub mod pending;
pub mod port;

pub mod claude;
pub mod codex;
pub mod cursor;
pub mod opencode;
