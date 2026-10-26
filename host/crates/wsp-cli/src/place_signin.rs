// SPDX-License-Identifier: AGPL-3.0-only
//! A sign-in run on a computer's own terminal and shown in this one, over the daemon channel the host holds. Port of
//! `packages/host/src/place-signin.ts` at 65f6b89c7; the spec is `spec/place_roads.md`.
use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};
use tokio::sync::watch;
use wsp_catalog::catalog::login_sign_in;
use wsp_wire::types::agents_report::{AgentsTarget, SignInLine};
use wsp_words::agents_report::pty_bare_on;
use wsp_words::exit::HostError;
use wsp_words::js::number_string;
use wsp_words::words::base::last_line;

use crate::signin_relay::{relay_pty, run_quiet, Detach, EventSink, Frame, PtyLink, RelayOptions, RelayTerminal};
use crate::verbs::client::{FrameOff, HostClient};

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
        (self.close)().await
    }
}

/// place-signin.ts:27-29: `target_link(client, {placeId})`.
pub async fn place_link(client: Arc<dyn HostClient>, place_id: &str) -> Result<PlaceLink, HostError> {
    let target = serde_json::from_value::<AgentsTarget>(json!({"placeId": place_id})).map_err(|e| HostError::new(e.to_string()))?;
    target_link(client, &target).await
}

/// One channel the host opened: its readers, and the latch that settles `closed`.
struct Road {
    channel: String,
    readers: Mutex<Vec<(u64, EventSink)>>,
    gone: watch::Sender<bool>,
}

impl Road {
    fn settle(&self) {
        self.gone.send_replace(true);
    }

    fn closed(&self) -> Option<BoxFuture<'static, ()>> {
        let mut rx = self.gone.subscribe();
        Some(
            async move {
                let _ = rx.wait_for(|v| *v).await;
            }
            .boxed(),
        )
    }
}

/// The `PtyLink` of one channel: ops ride `daemon.send`, events come from the frames of that channel.
struct RoadLink {
    client: Arc<dyn HostClient>,
    road: Arc<Road>,
    bare: bool,
    next: Mutex<u64>,
}

#[async_trait::async_trait]
impl PtyLink for RoadLink {
    async fn op(&self, op: &str, extra: Frame) -> Result<Frame, HostError> {
        let mut frame = extra;
        frame.insert("op".into(), json!(op));
        let reply = self.client.request("daemon.send", channel_frame(&self.road.channel, frame)).await?;
        Ok(reply.get("reply").and_then(Value::as_object).cloned().unwrap_or_default())
    }

    fn on_event(&self, sink: EventSink) -> Detach {
        let id = {
            let mut next = self.next.lock().unwrap();
            *next += 1;
            *next
        };
        self.road.readers.lock().unwrap().push((id, sink));
        let road = self.road.clone();
        Detach(Box::new(move || road.readers.lock().unwrap().retain(|(i, _)| *i != id)))
    }

    fn closed(&self) -> Option<BoxFuture<'static, ()>> {
        self.road.closed()
    }

    fn bare(&self) -> Option<bool> {
        Some(self.bare)
    }
}

fn channel_frame(channel: &str, frame: Frame) -> Frame {
    let mut params = channel_only(channel);
    params.insert("frame".into(), Value::Object(frame));
    params
}

fn channel_only(channel: &str) -> Frame {
    let mut params = Frame::new();
    params.insert("channel".into(), json!(channel));
    params
}

/// place-signin.ts:32-63.
pub async fn target_link(client: Arc<dyn HostClient>, target: &AgentsTarget) -> Result<PlaceLink, HostError> {
    let target_params = match serde_json::to_value(target) {
        Ok(Value::Object(m)) => m,
        _ => Frame::new(),
    };
    let reply = client.request("daemon.open", target_params).await?;
    let channel = reply.get("channel").and_then(Value::as_str).unwrap_or("").to_string();
    let (gone, _) = watch::channel(false);
    let road = Arc::new(Road { channel: channel.clone(), readers: Mutex::new(Vec::new()), gone });
    let listener = road.clone();
    let off: FrameOff = client.on_frame(Arc::new(move |frame: &Frame| {
        if frame.get("channel").and_then(Value::as_str) != Some(listener.channel.as_str()) {
            return;
        }
        match frame.get("type").and_then(Value::as_str) {
            Some("daemon.event") => {
                if let Some(Value::Object(event)) = frame.get("event") {
                    let readers: Vec<EventSink> = listener.readers.lock().unwrap().iter().map(|(_, r)| r.clone()).collect();
                    for read in readers {
                        read(event);
                    }
                }
            }
            Some("daemon.closed") => listener.settle(),
            _ => {}
        }
    }));
    {
        let client = client.clone();
        let road = road.clone();
        tokio::spawn(async move {
            client.closed().await;
            road.settle();
        });
    }
    let link = RoadLink { client: client.clone(), road: road.clone(), bare: pty_bare_on(target), next: Mutex::new(0) };
    Ok(PlaceLink {
        link: Arc::new(link),
        close: Box::new(move || {
            Box::pin(async move {
                off.off();
                road.settle();
                let _ = client.request("daemon.close", channel_only(&channel)).await;
            })
        }),
    })
}

/// place-signin.ts:67-74: nothing where the step ran clean, else what stopped it.
pub async fn prepare_failed(link: &dyn PtyLink, cmd: &str) -> Option<String> {
    let mut exec = Frame::new();
    exec.insert("cmd".into(), json!(cmd));
    let reply = match link.op("exec", exec).await {
        Ok(reply) => reply,
        Err(e) => {
            let mut failed = Frame::new();
            failed.insert("ok".into(), json!(false));
            failed.insert("error".into(), json!(e.message));
            failed
        }
    };
    if reply.get("ok") == Some(&Value::Bool(false)) {
        return Some(match reply.get("error").and_then(Value::as_str) {
            Some(error) if !error.is_empty() => error.to_string(),
            _ => "the computer refused it".to_string(),
        });
    }
    let exit = reply.get("exitCode").and_then(Value::as_f64)?;
    if exit == 0.0 {
        return None;
    }
    let said = |key: &str| reply.get(key).and_then(Value::as_str).unwrap_or("").to_string();
    Some(last_line(&said("stderr")).or_else(|| last_line(&said("stdout"))).unwrap_or_else(|| format!("it exited {}", number_string(exit))))
}

/// place-signin.ts:77.
pub fn sign_in_prepare_line(r#where: &str, said: &str) -> String {
    format!("the sign-in on {} did not start, since the step before it failed: {said}", r#where)
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
    if let Some(prepare) = o.line.prepare.as_deref() {
        if let Some(said) = prepare_failed(o.link.as_ref(), prepare).await {
            return Err(HostError::new(sign_in_prepare_line(&o.r#where, &said)));
        }
    }
    let outcome = relay_pty(
        RelayOptions {
            link: o.link.clone(),
            command: Some(o.line.command.clone()),
            open: o.open.clone(),
            callback_url: None,
            on_consent: None,
            env: o.line.env.clone(),
            timeout_ms: o.timeout_ms.unwrap_or(BOX_SIGN_IN_MS),
            flush_ms: None,
            now: None,
        },
        terminal,
    )
    .await?;
    let check = o.agent.as_deref().and_then(login_sign_in).and_then(|row| row.status());
    let (check, status_cmd) = match (check, o.line.status.as_deref()) {
        (Some(check), Some(status_cmd)) => (check, status_cmd),
        _ => return Ok(BoxSignedIn { signed_in: outcome.exit_code == 0, detail: None, said: None }),
    };
    let env = o.line.env.clone().unwrap_or_default();
    let status = run_quiet(o.link.as_ref(), status_cmd, STATUS_MS, &env).await?;
    let signed_in = check.signed_in(&status.output, i32::try_from(status.exit_code).unwrap_or(-1));
    let detail = check.detail(&status.output, &IndexMap::new());
    let said = if signed_in { None } else { check.why(&status.output).or_else(|| last_line(&status.output)) };
    Ok(BoxSignedIn { signed_in, detail: if signed_in { detail } else { None }, said })
}
