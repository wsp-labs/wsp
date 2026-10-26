// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/daemon-token.ts at 65f6b89c7: the daemon token's file on a machine, its derivation
//! from a runtime's seed, and the scripts and byte road that write it. Spec: spec/host_files.md.
use sha2::{Digest, Sha256};
use wsp_engine::errors::EngineError;
use wsp_engine::land_bytes::{has_byte_road, land_bytes, LandBytesOptions};
use wsp_engine::machine::{ExecOptions, Machine};
use wsp_wire::types::daemon_contract::DAEMON_TOKEN_PATH;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::exit::HostError;

fn host_of(e: EngineError) -> HostError {
    HostError::new(e.to_string())
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> Vec<u8> {
    const BLOCK: usize = 64;
    let mut block = [0u8; BLOCK];
    if key.len() > BLOCK {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let inner_pad: Vec<u8> = block.iter().map(|b| b ^ 0x36).chain(message.iter().copied()).collect();
    let inner = Sha256::digest(&inner_pad);
    let outer_pad: Vec<u8> = block.iter().map(|b| b ^ 0x5c).chain(inner.iter().copied()).collect();
    Sha256::digest(&outer_pad).to_vec()
}

/// The rotation script's answer on a machine that has a daemon. daemon-token.ts:14.
pub const DAEMON_TOKEN_SET: &str = "WSP_DAEMON_TOKEN_SET";
/// Its answer on a machine with no daemon: nothing is written. daemon-token.ts:16.
pub const DAEMON_TOKEN_NONE: &str = "WSP_DAEMON_TOKEN_NONE";
/// What a token or a seed that is not lowercase hex is refused with. daemon-token.ts:20.
pub const TOKEN_NOT_HEX: &str = "a daemon token is lowercase hex";

/// `Err(TOKEN_NOT_HEX)` unless `token` is one or more of `[0-9a-f]`. daemon-token.ts:19.
pub fn assert_token_shape(token: &str) -> Result<(), HostError> {
    if !token.is_empty() && token.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        Ok(())
    } else {
        Err(HostError::new(TOKEN_NOT_HEX))
    }
}

/// HMAC-SHA256 of `machine_id`'s UTF-8 bytes keyed by `seed`'s UTF-8 bytes (the text, not the bytes it spells), as 64
/// lowercase hex digits. The seed's shape is checked first. daemon-token.ts:26.
pub fn daemon_token_for(seed: &str, machine_id: &str) -> Result<String, HostError> {
    assert_token_shape(seed)?;
    Ok(hex::encode(hmac_sha256(seed.as_bytes(), machine_id.as_bytes())))
}

/// The four-line write: the assignment, `umask 077`, the printf to `<path>.next`, the rename. `path` is
/// DAEMON_TOKEN_PATH where `None`, and is not quoted. daemon-token.ts:33.
pub fn write_daemon_token_script(token: &str, path: Option<&str>) -> Result<String, HostError> {
    assert_token_shape(token)?;
    let path = path.unwrap_or(DAEMON_TOKEN_PATH);
    Ok([
        format!("WSP_DAEMON_TOKEN='{token}'"),
        "umask 077".to_string(),
        format!("printf '%s' \"$WSP_DAEMON_TOKEN\" > {path}.next"),
        format!("mv -f {path}.next {path}"),
    ]
    .join("\n"))
}

/// The existence test that answers DAEMON_TOKEN_NONE, the write, and DAEMON_TOKEN_SET. daemon-token.ts:44.
pub fn rotate_daemon_token_script(token: &str, path: Option<&str>) -> Result<String, HostError> {
    let path = path.unwrap_or(DAEMON_TOKEN_PATH);
    Ok([
        format!("test -f {path} || {{ echo {DAEMON_TOKEN_NONE}; exit 0; }}"),
        write_daemon_token_script(token, Some(path))?,
        format!("echo {DAEMON_TOKEN_SET}"),
    ]
    .join("\n"))
}

/// The machine's own token file where it names one, else the road's path. daemon-token.ts:53.
pub fn daemon_token_path_of(machine: &dyn Machine, road: Option<&str>) -> Option<String> {
    machine.daemon_token_path().or(road).map(str::to_string)
}

/// Whether the machine had a token file to replace, having replaced it: the one-command rotation on a machine with no
/// byte road, else an existence test naming only the path and the token over the machine's own byte road.
/// daemon-token.ts:59.
pub async fn rotate_daemon_token(machine: &dyn Machine, token: &str, path: Option<&str>) -> Result<bool, HostError> {
    assert_token_shape(token)?;
    let path = path.unwrap_or(DAEMON_TOKEN_PATH);
    if !has_byte_road(machine) {
        let rotated = machine.exec(&rotate_daemon_token_script(token, Some(path))?, ExecOptions::default()).await.map_err(host_of)?;
        return Ok(rotated.exit_code.get() == 0 && rotated.stdout.contains(DAEMON_TOKEN_SET));
    }
    let there = machine
        .exec(&format!("test -f {} && echo {DAEMON_TOKEN_SET} || echo {DAEMON_TOKEN_NONE}", shell_quote(path)), ExecOptions::default())
        .await
        .map_err(host_of)?;
    if there.exit_code.get() != 0 || !there.stdout.contains(DAEMON_TOKEN_SET) {
        return Ok(false);
    }
    land_bytes(machine, path, token.as_bytes(), LandBytesOptions::default()).await.map_err(host_of)?;
    Ok(true)
}
