// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/sign-in-socket.ts at 65f6b89c7: one socket's side of the sign-ins, the runs it
//! started, joined or followed, which are the only ones it may type a code into or stop. Spec: spec/sockets.md.
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use serde_json::{Map, Value};
use wsp_wire::types::agents_report::{AgentsSignInEvent, AgentsSignInRun, AgentsTarget, McpScope, NO_SIGN_IN_REFUSAL};
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::helpers::ReqId;
use wsp_wire::wire::runtime::AgentsCloudSignInAgent;
use wsp_wire::wire::runtime_ops::RuntimeOp;
use wsp_words::exit::HostError;

use crate::sys_socket::{Send, SocketSide};
use crate::types::api::{Runtime, SignInFollowing};
use crate::types::pending::SignInAsk;
use crate::types::wiring::Detach;

type Leave = Rc<RefCell<Option<Detach>>>;

fn run_leave(leave: &Leave) {
    let taken = leave.borrow_mut().take();
    if let Some(leave) = taken {
        leave();
    }
}

fn reply(id: &ReqId, ok: bool) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("id".into(), serde_json::to_value(id).unwrap_or(Value::Null));
    out.insert("ok".into(), Value::Bool(ok));
    out
}

fn event_frame(event: &AgentsSignInEvent) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("type".into(), Value::from(AgentsSignInEvent::TYPE));
    if let Value::Object(fields) = serde_json::to_value(event).unwrap_or(Value::Null) {
        out.extend(fields);
    }
    out
}

/// The events a run pushes while it starts wait behind its answer; once answered they go out in order, and after that
/// each goes straight out while the socket is open.
fn held_behind_answer(send: Send, socket: Rc<dyn SocketSide>) -> (Rc<dyn Fn(AgentsSignInEvent)>, Rc<dyn Fn(Map<String, Value>)>) {
    let queued: Rc<RefCell<Option<Vec<AgentsSignInEvent>>>> = Rc::new(RefCell::new(Some(Vec::new())));
    let emit_queue = queued.clone();
    let emit_send = send.clone();
    let emit: Rc<dyn Fn(AgentsSignInEvent)> = Rc::new(move |event| {
        let event = {
            let mut held = emit_queue.borrow_mut();
            match held.as_mut() {
                Some(events) => {
                    events.push(event);
                    return;
                }
                None => event,
            }
        };
        if socket.open() {
            emit_send(event_frame(&event));
        }
    });
    let answer: Rc<dyn Fn(Map<String, Value>)> = Rc::new(move |payload| {
        send(payload);
        let held = queued.borrow_mut().take().unwrap_or_default();
        for event in held {
            send(event_frame(&event));
        }
    });
    (emit, answer)
}

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
    Some(match op {
        RuntimeOp::AgentsSignIn { id, target, agent, terminal } => {
            SignInOp::SignIn { id: id.clone(), target: target.clone(), agent: agent.clone(), terminal: *terminal }
        }
        RuntimeOp::ServersSignIn { id, target, agent, name, scope, project } => SignInOp::ServerSignIn {
            id: id.clone(),
            target: target.clone(),
            agent: agent.clone(),
            name: name.clone(),
            scope: scope.clone(),
            project: project.clone(),
        },
        RuntimeOp::AgentsCloudSignIn { id, agent } => SignInOp::CloudSignIn { id: id.clone(), agent: agent.clone() },
        RuntimeOp::AgentsSignIns { id } => SignInOp::SignIns { id: id.clone() },
        RuntimeOp::AgentsSignInCode { id, sign_in_id, code } => {
            SignInOp::Code { id: id.clone(), sign_in_id: sign_in_id.clone(), code: code.clone() }
        }
        RuntimeOp::AgentsSignInStop { id, sign_in_id } => SignInOp::Stop { id: id.clone(), sign_in_id: sign_in_id.clone() },
        _ => return None,
    })
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
impl<R: Runtime + ?Sized> SignInRuns for R {
    async fn sign_in(&self, target: AgentsTarget, ask: SignInAsk, emit: Rc<dyn Fn(AgentsSignInEvent)>, origin: Option<Caller>) -> Result<SignInFollowing, HostError> {
        self.agents().sign_in(target, ask, emit, origin).await
    }

    fn sign_ins(&self) -> Vec<AgentsSignInRun> {
        self.agents().sign_ins()
    }

    fn sign_in_follow(&self, sign_in_id: &str, emit: Rc<dyn Fn(AgentsSignInEvent)>) -> Result<SignInFollowing, HostError> {
        self.agents().sign_in_follow(sign_in_id, emit)
    }

    async fn sign_in_code(&self, sign_in_id: &str, code: &str) -> Result<(), HostError> {
        self.agents().sign_in_code(sign_in_id, code).await
    }

    fn sign_in_stop(&self, sign_in_id: &str) {
        self.agents().sign_in_stop(sign_in_id)
    }
}

/// One socket's handler of every sign-in op. A refusal of a start (and an unserved cloud sign-in) is the handler's
/// `Err`, which the door answers as it answers any op's; a code or stop for a run this socket does not hold is answered
/// `{id, ok: false, error: noSignInRefusal, kind: "usage"}` by the handler itself. sign-in-socket.ts:26.
pub fn sign_in_socket<A: SignInRuns + ?Sized + 'static>(
    agents: Rc<A>,
    socket: Rc<dyn SocketSide>,
    cloud_sign_in: Option<CloudSignIn>,
) -> Rc<dyn Fn(SignInOp, Send, Option<Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>> {
    let signed: Rc<RefCell<HashSet<String>>> = Rc::new(RefCell::new(HashSet::new()));
    Rc::new(move |msg: SignInOp, send: Send, origin: Option<Caller>| -> LocalBoxFuture<'static, Result<(), HostError>> {
        let agents = agents.clone();
        let socket = socket.clone();
        let cloud_sign_in = cloud_sign_in.clone();
        let signed = signed.clone();
        Box::pin(async move {
            match msg {
                SignInOp::SignIn { id, target, agent, terminal } => {
                    let ask = SignInAsk { agent, server: None, terminal: terminal.filter(|t| *t), scope: None, project: None };
                    let (emit, answer) = held_behind_answer(send.clone(), socket.clone());
                    let following = agents.sign_in(target, ask, emit, origin).await?;
                    hold_start(&signed, &socket, &send, &answer, &id, following);
                }
                SignInOp::ServerSignIn { id, target, agent, name, scope, project } => {
                    let ask = SignInAsk { agent, server: Some(name), terminal: None, scope, project };
                    let (emit, answer) = held_behind_answer(send.clone(), socket.clone());
                    let following = agents.sign_in(target, ask, emit, origin).await?;
                    hold_start(&signed, &socket, &send, &answer, &id, following);
                }
                SignInOp::CloudSignIn { id, agent } => {
                    let (emit, answer) = held_behind_answer(send.clone(), socket.clone());
                    let Some(road) = cloud_sign_in else {
                        return Err(HostError::new(CLOUD_SIGN_IN_UNSERVED));
                    };
                    let following = road(agent, emit, origin).await?;
                    hold_start(&signed, &socket, &send, &answer, &id, following);
                }
                SignInOp::SignIns { id } => {
                    let (emit, answer) = held_behind_answer(send.clone(), socket.clone());
                    let runs = agents.sign_ins();
                    for run in &runs {
                        if run.ended.is_some() || signed.borrow().contains(&run.sign_in_id) {
                            continue;
                        }
                        signed.borrow_mut().insert(run.sign_in_id.clone());
                        let following = agents.sign_in_follow(&run.sign_in_id, emit.clone())?;
                        socket.detach(following.leave);
                    }
                    let mut frame = reply(&id, true);
                    frame.insert("runs".into(), serde_json::to_value(&runs).unwrap_or(Value::Null));
                    answer(frame);
                }
                SignInOp::Code { id, sign_in_id, code } => {
                    if !signed.borrow().contains(&sign_in_id) {
                        send(refusal(&id));
                        return Ok(());
                    }
                    agents.sign_in_code(&sign_in_id, &code).await?;
                    send(reply(&id, true));
                }
                SignInOp::Stop { id, sign_in_id } => {
                    if !signed.borrow().contains(&sign_in_id) {
                        send(refusal(&id));
                        return Ok(());
                    }
                    agents.sign_in_stop(&sign_in_id);
                    send(reply(&id, true));
                }
            }
            Ok(())
        })
    })
}

/// A start that answers a run this socket already holds joins it and answers alone; any other run is held and its
/// leave registered on the socket, which a socket closed before the answer runs at once.
fn hold_start(
    signed: &RefCell<HashSet<String>>,
    socket: &Rc<dyn SocketSide>,
    send: &Send,
    answer: &Rc<dyn Fn(Map<String, Value>)>,
    id: &ReqId,
    following: SignInFollowing,
) {
    let SignInFollowing { sign_in_id, leave } = following;
    let mut frame = reply(id, true);
    frame.insert("signInId".into(), Value::from(sign_in_id.clone()));
    if signed.borrow().contains(&sign_in_id) {
        leave();
        send(frame);
        return;
    }
    signed.borrow_mut().insert(sign_in_id);
    let leave: Leave = Rc::new(RefCell::new(Some(leave)));
    let on_close = leave.clone();
    socket.detach(Box::new(move || run_leave(&on_close)));
    if !socket.open() {
        run_leave(&leave);
        return;
    }
    answer(frame);
}

fn refusal(id: &ReqId) -> Map<String, Value> {
    let mut frame = reply(id, false);
    frame.insert("error".into(), Value::from(NO_SIGN_IN_REFUSAL));
    frame.insert("kind".into(), Value::from("usage"));
    frame
}
