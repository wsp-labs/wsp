// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/serve-broker.ts at 65f6b89c7: the broker ops a socket asks, answered off the
//! host's broker. Spec: spec/serve_helpers.md, H24.
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use wsp_wire::types::agents_report::AgentsSignInEvent;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime::{BrokerStatus, ProviderKeyWorks};
use wsp_wire::wire::runtime_ops::RuntimeRequest;
use wsp_words::exit::HostError;

use crate::serve::served::Payload;

/// What a host with no broker answers the broker ops with. serve-broker.ts:7.
pub const BROKER_UNSERVED: &str = "the host that answered wired no broker for machines at a cloud provider";

/// A sign-in followed through the cloud road: its id and the leave that stops following. sign-in-socket.ts:20-24.
pub struct CloudSignInFollowing {
    pub sign_in_id: String,
    pub leave: Box<dyn FnOnce()>,
}

/// `CloudSignIn`: the host's own road to an agent's login on this computer, which machines at a provider borrow.
/// sign-in-socket.ts:20-24.
pub type CloudSignIn = Rc<dyn Fn(String, Rc<dyn Fn(AgentsSignInEvent)>, Option<Caller>) -> LocalBoxFuture<'static, Result<CloudSignInFollowing, HostError>>>;

/// The host's broker as its socket answers it; each road absent is refused. serve-broker.ts:9-16.
#[async_trait(?Send)]
pub trait BrokerDoor {
    /// None: the host wired no status road.
    async fn status(&self) -> Option<Result<BrokerStatus, HostError>>;
    /// None: the host wired no key check.
    async fn check_key(&self, provider: &str, key: &str) -> Option<Result<ProviderKeyWorks, HostError>>;
    fn cloud_sign_in(&self) -> Option<CloudSignIn>;
}

/// `providers.checkKey` or `broker.status`. serve-broker.ts:20.
pub fn is_broker_op(msg: &RuntimeRequest) -> bool {
    todo!()
}

/// `answerBroker(door, msg, ownRoad)`. serve-broker.ts:22-31.
pub async fn answer_broker(door: Option<&Rc<dyn BrokerDoor>>, msg: &RuntimeRequest, own_road: bool) -> Result<Payload, HostError> {
    todo!()
}
