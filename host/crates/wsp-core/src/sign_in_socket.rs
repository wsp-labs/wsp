// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/sign-in-socket.ts at 65f6b89c7: one socket's side of the sign-ins, the runs it
//! started, joined or followed, which are the only ones it may type a code into or stop. Spec: spec/sockets.md.
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use wsp_wire::types::agents_report::{AgentsSignInEvent, AgentsSignInRun, AgentsTarget, McpScope};
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::helpers::ReqId;
use wsp_wire::wire::runtime::AgentsCloudSignInAgent;
use wsp_wire::wire::runtime_ops::RuntimeOp;
use wsp_words::exit::HostError;

use crate::sys_socket::{Send, SocketSide};
use crate::types::api::{AgentsApi, SignInFollowing};
use crate::types::pending::SignInAsk;

/// The ops this handler takes, in the TS's order. sign-in-socket.ts:11.
pub const SIGN_IN_OPS: [&str; 6] = ["agents.signIn", "servers.signIn", "agents.cloudSignIn", "agents.signIns", "agents.signInCode", "agents.signInStop"];

/// What a host with no road to the logins machines at a provider borrow answers agents.cloudSignIn with.
/// sign-in-socket.ts:17.
pub const CLOUD_SIGN_IN_UNSERVED: &str = "the host that answered wired no sign-in for machines at a cloud provider";

/// The host's own road to an agent's login on this computer, which machines at a provider borrow.
/// sign-in-socket.ts:20.
pub type CloudSignIn = Rc<dyn Fn(AgentsCloudSignInAgent, Rc<dyn Fn(AgentsSignInEvent)>, Option<Caller>) -> LocalBoxFuture<'static, Result<SignInFollowing, HostError>>>;

/// One sign-in op, as the handler reads it off the request. sign-in-socket.ts:12.
#[derive(Clone, Debug, PartialEq)]
pub enum SignInOp {
    SignIn { id: ReqId, target: AgentsTarget, agent: String, terminal: Option<bool> },
    ServerSignIn { id: ReqId, target: AgentsTarget, agent: String, name: String, scope: Option<McpScope>, project: Option<String> },
    CloudSignIn { id: ReqId, agent: AgentsCloudSignInAgent },
    SignIns { id: ReqId },
    Code { id: ReqId, sign_in_id: String, code: String },
    Stop { id: ReqId, sign_in_id: String },
}

/// The op as a sign-in op, or `None` for every other op. sign-in-socket.ts:14 `isSignInOp`.
pub fn sign_in_op(op: &RuntimeOp) -> Option<SignInOp> {
    todo!()
}

/// The runs the handler asks of the runtime's agents (sign-in-socket.ts:27 `Runtime["agents"]`).
#[async_trait(?Send)]
pub trait SignInRuns {
    async fn sign_in(&self, target: AgentsTarget, ask: SignInAsk, emit: Rc<dyn Fn(AgentsSignInEvent)>, origin: Option<Caller>) -> Result<SignInFollowing, HostError>;
    fn sign_ins(&self) -> Vec<AgentsSignInRun>;
    fn sign_in_follow(&self, sign_in_id: &str, emit: Rc<dyn Fn(AgentsSignInEvent)>) -> Result<SignInFollowing, HostError>;
    async fn sign_in_code(&self, sign_in_id: &str, code: &str) -> Result<(), HostError>;
    fn sign_in_stop(&self, sign_in_id: &str);
}

#[async_trait(?Send)]
impl<A: AgentsApi + ?Sized> SignInRuns for A {
    async fn sign_in(&self, target: AgentsTarget, ask: SignInAsk, emit: Rc<dyn Fn(AgentsSignInEvent)>, origin: Option<Caller>) -> Result<SignInFollowing, HostError> {
        todo!()
    }

    fn sign_ins(&self) -> Vec<AgentsSignInRun> {
        todo!()
    }

    fn sign_in_follow(&self, sign_in_id: &str, emit: Rc<dyn Fn(AgentsSignInEvent)>) -> Result<SignInFollowing, HostError> {
        todo!()
    }

    async fn sign_in_code(&self, sign_in_id: &str, code: &str) -> Result<(), HostError> {
        todo!()
    }

    fn sign_in_stop(&self, sign_in_id: &str) {
        todo!()
    }
}

/// One socket's handler of every sign-in op. A refusal of a start (and an unserved cloud sign-in) is the handler's
/// `Err`, which the door answers as it answers any op's; a code or stop for a run this socket does not hold is answered
/// `{id, ok: false, error: noSignInRefusal, kind: "usage"}` by the handler itself. sign-in-socket.ts:26.
pub fn sign_in_socket(
    agents: Rc<dyn SignInRuns>,
    socket: Rc<dyn SocketSide>,
    cloud_sign_in: Option<CloudSignIn>,
) -> Rc<dyn Fn(SignInOp, Send, Option<Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>> {
    todo!()
}
