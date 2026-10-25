// SPDX-License-Identifier: AGPL-3.0-only
//! A sign-in run on a computer's own terminal and shown in this one, over the daemon channel the host holds. Port of
//! `packages/host/src/place-signin.ts` at 65f6b89c7; the spec is `spec/place_roads.md`.
use std::sync::Arc;

use futures::future::BoxFuture;
use wsp_wire::types::agents_report::{AgentsTarget, SignInLine};
use wsp_words::exit::HostError;

use crate::signin_relay::{PtyLink, RelayTerminal};
use crate::verbs::client::HostClient;

/// place-signin.ts:14: `SIGN_IN_WAIT_MS`, ten minutes.
pub const BOX_SIGN_IN_MS: u64 = 600_000;
/// place-signin.ts:16.
pub const STATUS_MS: u64 = 20_000;

/// place-signin.ts:20-23: the link and its close, which is the caller's.
pub struct PlaceLink {
    pub link: Arc<dyn PtyLink>,
    close: Box<dyn FnOnce() -> BoxFuture<'static, ()> + Send>,
}

impl PlaceLink {
    /// place-signin.ts:57-61: detach from the client, settle `closed`, send `daemon.close`, its refusal swallowed.
    pub async fn close(self) {
        todo!()
    }
}

/// place-signin.ts:27-29: `target_link(client, {placeId})`.
pub async fn place_link(client: Arc<dyn HostClient>, place_id: &str) -> Result<PlaceLink, HostError> {
    todo!()
}

/// place-signin.ts:32-63.
pub async fn target_link(client: Arc<dyn HostClient>, target: &AgentsTarget) -> Result<PlaceLink, HostError> {
    todo!()
}

/// place-signin.ts:67-74: nothing where the step ran clean, else what stopped it.
pub async fn prepare_failed(link: &dyn PtyLink, cmd: &str) -> Option<String> {
    todo!()
}

/// place-signin.ts:77.
pub fn sign_in_prepare_line(r#where: &str, said: &str) -> String {
    todo!()
}

/// place-signin.ts:79-91.
pub struct BoxSignIn {
    pub r#where: String,
    pub link: Arc<dyn PtyLink>,
    pub agent: Option<String>,
    pub line: SignInLine,
    pub open: Arc<dyn Fn(String) -> BoxFuture<'static, bool> + Send + Sync>,
    /// `None` is `BOX_SIGN_IN_MS`.
    pub timeout_ms: Option<u64>,
}

/// place-signin.ts:93-99. Serialized `signedIn`, then `detail` or `said` where present.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxSignedIn {
    pub signed_in: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub said: Option<String>,
}

/// place-signin.ts:104-127.
pub async fn relay_sign_in(o: BoxSignIn, terminal: &mut RelayTerminal) -> Result<BoxSignedIn, HostError> {
    todo!()
}
