// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/daemon-token.ts at 65f6b89c7: the daemon token's file on a machine, its derivation
//! from a runtime's seed, and the scripts and byte road that write it. Spec: spec/host_files.md.
use wsp_engine::machine::Machine;
use wsp_words::exit::HostError;

/// The rotation script's answer on a machine that has a daemon. daemon-token.ts:14.
pub const DAEMON_TOKEN_SET: &str = "WSP_DAEMON_TOKEN_SET";
/// Its answer on a machine with no daemon: nothing is written. daemon-token.ts:16.
pub const DAEMON_TOKEN_NONE: &str = "WSP_DAEMON_TOKEN_NONE";
/// What a token or a seed that is not lowercase hex is refused with. daemon-token.ts:20.
pub const TOKEN_NOT_HEX: &str = "a daemon token is lowercase hex";

/// `Err(TOKEN_NOT_HEX)` unless `token` is one or more of `[0-9a-f]`. daemon-token.ts:19.
pub fn assert_token_shape(token: &str) -> Result<(), HostError> {
    todo!()
}

/// HMAC-SHA256 of `machine_id`'s UTF-8 bytes keyed by `seed`'s UTF-8 bytes (the text, not the bytes it spells), as 64
/// lowercase hex digits. The seed's shape is checked first. daemon-token.ts:26.
pub fn daemon_token_for(seed: &str, machine_id: &str) -> Result<String, HostError> {
    todo!()
}

/// The four-line write: the assignment, `umask 077`, the printf to `<path>.next`, the rename. `path` is
/// DAEMON_TOKEN_PATH where `None`, and is not quoted. daemon-token.ts:33.
pub fn write_daemon_token_script(token: &str, path: Option<&str>) -> Result<String, HostError> {
    todo!()
}

/// The existence test that answers DAEMON_TOKEN_NONE, the write, and DAEMON_TOKEN_SET. daemon-token.ts:44.
pub fn rotate_daemon_token_script(token: &str, path: Option<&str>) -> Result<String, HostError> {
    todo!()
}

/// The machine's own token file where it names one, else the road's path. daemon-token.ts:53.
pub fn daemon_token_path_of(machine: &dyn Machine, road: Option<&str>) -> Option<String> {
    todo!()
}

/// Whether the machine had a token file to replace, having replaced it: the one-command rotation on a machine with no
/// byte road, else an existence test naming only the path and the token over the machine's own byte road.
/// daemon-token.ts:59.
pub async fn rotate_daemon_token(machine: &dyn Machine, token: &str, path: Option<&str>) -> Result<bool, HostError> {
    todo!()
}
