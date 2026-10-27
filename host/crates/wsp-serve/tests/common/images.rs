// SPDX-License-Identifier: AGPL-3.0-only
//! image-fixtures.ts:15-37: what the image tests seal from, and the exec answers a builder needs from a stub.
use wsp_adapters::pending::shell_quote;
use wsp_engine::machine::ExecResult;

use super::stub_backend::{exec_exit, exec_fn, exec_ok, ExecImpl};

/// The paths such a seal archives: a login's file and the secrets file the secrets step writes.
pub const VAULT_PATHS: [&str; 2] = ["/root/.codex/auth.json", "/etc/profile.d/wsp-secrets.sh"];

/// image-fixtures.ts:22 `dfOk`: a disk reading, the ready check, the context probe, and the vault paths it holds. The
/// seal's guard probe asks about the paths no image may hold, and a builder that answered it with one would be
/// refused, so only the vault's own probe is answered.
pub fn df_ok_answer(cmd: &str) -> ExecResult {
    if cmd.starts_with("df -Pk") {
        return exec_ok(&format!("{}\n", 3000 * 1024));
    }
    if cmd == "echo ok" {
        return exec_ok("ok\n");
    }
    if cmd.contains("echo WSP_CTX") {
        return exec_ok("WSP_CTX\nWSP_CTX_END\n");
    }
    let probe = format!("for p in {};", VAULT_PATHS.iter().map(|p| shell_quote(p)).collect::<Vec<_>>().join(" "));
    if cmd.starts_with(&probe) {
        return exec_ok(&format!("{}\n", VAULT_PATHS[1]));
    }
    exec_exit(0, "", "")
}

pub fn df_ok() -> ExecImpl {
    exec_fn(|_, cmd| Ok(df_ok_answer(cmd)))
}
