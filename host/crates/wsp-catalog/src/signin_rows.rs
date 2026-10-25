// SPDX-License-Identifier: AGPL-3.0-only
//! The sign-in rows and everything derived from them: the shared logins, the paths no image or brokered machine may
//! hold, and the vault's variables. Port of packages/catalog/src/signin.ts:343-594 at a3b870a6e; spec/signin.md.
use std::sync::LazyLock;

use indexmap::{IndexMap, IndexSet};
use serde::Serialize;

use crate::signin::{SharedLogin, SignIn};

/// The rows by tool, in signin.ts:344's order: gh, gcloud, aws, wrangler, vercel, netlify, fly, supabase, railway,
/// doppler, claude, codex, gemini, opencode, cursor, cloudflared, op, kubectl, pi, hermes.
pub static SIGN_IN_ROWS: LazyLock<IndexMap<String, SignIn>> = LazyLock::new(|| todo!());

/// One row by its tool; panics on a tool with no row, which the TS type system rules out.
pub fn sign_in_row(tool: &str) -> &'static SignIn {
    todo!()
}

/// signin.ts:539.
pub static SHARED_LOGINS: LazyLock<Vec<SharedLogin>> = LazyLock::new(|| todo!());

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Share {
    pub source: String,
    pub target: String,
}

/// signin.ts:545.
pub fn shares_in(logins: &str) -> Vec<Share> {
    todo!()
}

/// `<dir>/<file>` (signin.ts:550).
pub fn shared_file_in(shared: &SharedLogin) -> String {
    todo!()
}

/// signin.ts:557.
pub static NEVER_IN_IMAGE: LazyLock<Vec<String>> = LazyLock::new(|| todo!());

pub const SECRETS_FILE_SH: &str = "/etc/profile.d/wsp-secrets.sh";
pub const SECRETS_FILE_FISH: &str = "/etc/fish/conf.d/wsp-secrets.fish";

/// Deduplicated, first place kept (signin.ts:569).
pub static LOGIN_PATHS: LazyLock<Vec<String>> = LazyLock::new(|| todo!());

/// signin.ts:575.
pub static NEVER_ON_BROKERED: LazyLock<Vec<String>> = LazyLock::new(|| todo!());

/// signin.ts:578.
pub static GH_LOGIN_PATHS: LazyLock<Vec<String>> = LazyLock::new(|| todo!());

/// Each row's token variable then its key variable, in row order (signin.ts:587).
pub static VAULT_VARIABLES: LazyLock<IndexSet<String>> = LazyLock::new(|| todo!());

/// The record less every vault variable, with each header token's bearer name (signin.ts:591).
pub fn server_values_of(record: &IndexMap<String, String>) -> IndexMap<String, String> {
    todo!()
}

/// The first row keeping its token or key under `name` (signin.ts:594).
pub fn vault_variable_row(name: &str) -> Option<String> {
    todo!()
}
