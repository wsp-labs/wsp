// SPDX-License-Identifier: AGPL-3.0-only
//! box-backend.test.ts:60 `FakeBox`: the Boat API as the 2026-09-11 spike saw it. Every route answers its scripted
//! replies in order and repeats the last one; a route with none answers 404 not_found. Nothing dials the real API.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_engine::errors::{RoadCause, RoadError};
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse};

/// One request the fake was sent.
#[derive(Debug, Clone)]
pub struct Seen {
    pub method: String,
    /// The path under https://boat.dev/api/v1, without the query; the whole URL for any other origin.
    pub path: String,
    pub query: Vec<(String, String)>,
    /// Header names lowercased.
    pub headers: IndexMap<String, String>,
    /// The body parsed as JSON; `Value::Null` where none was sent.
    pub body: Value,
    /// The body as the bytes it went out as.
    pub raw: Option<String>,
}

impl Seen {
    pub fn query_get(&self, key: &str) -> Option<&str> {
        self.query.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
    pub fn command(&self) -> String {
        self.body["command"].as_str().unwrap_or_default().to_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub body: Value,
}

/// A scripted answer: a reply, one made from the request, a road that fails, or a request nothing answers until the
/// request's own abort fires (which then fails as fetch does, TimeoutError for a cap).
#[derive(Clone)]
pub enum Answer {
    Reply(Reply),
    With(Arc<dyn Fn(&Seen) -> Answer>),
    Road(RoadError),
    Hang,
}

impl From<Reply> for Answer {
    fn from(r: Reply) -> Self {
        Answer::Reply(r)
    }
}

pub fn with(f: impl Fn(&Seen) -> Answer + 'static) -> Answer {
    Answer::With(Arc::new(f))
}

#[derive(Default)]
pub struct FakeBox {
    pub seen: Mutex<Vec<Seen>>,
    routes: Mutex<HashMap<String, Vec<Answer>>>,
    /// The requestId an unrouted request's 404 carries where it is not the unit tests' own.
    pub unrouted_request_id: Mutex<Option<String>>,
}

impl FakeBox {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Sets a route's replies, replacing any it had.
    pub fn on(self: &Arc<Self>, method: &str, path: &str, answers: Vec<Answer>) -> Arc<Self> {
        self.routes.lock().unwrap().insert(format!("{method} {path}"), answers);
        self.clone()
    }

    pub fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }

    /// `METHOD path` of every call, in order.
    pub fn calls(&self) -> Vec<String> {
        self.seen().iter().map(|s| format!("{} {}", s.method, s.path)).collect()
    }

    pub fn took(&self, method: &str, path: &str) -> Option<Seen> {
        self.seen().into_iter().find(|s| s.method == method && s.path == path)
    }

    /// Every command sent to a `/commands` route, in order.
    pub fn commands_sent(&self) -> Vec<String> {
        self.seen().iter().filter(|s| s.path.ends_with("/commands")).map(|s| s.command()).collect()
    }

    fn next(&self, key: &str) -> Option<Answer> {
        let mut routes = self.routes.lock().unwrap();
        let queue = routes.get_mut(key)?;
        if queue.is_empty() {
            return None;
        }
        Some(if queue.len() > 1 { queue.remove(0) } else { queue[0].clone() })
    }

    pub fn seen_of(req: &FetchRequest) -> Seen {
        let url = url::Url::parse(&req.url).expect("an absolute URL");
        let path = if url.origin().ascii_serialization() == "https://boat.dev" && url.path().starts_with("/api/v1/") {
            url.path()["/api/v1".len()..].to_string()
        } else {
            req.url.clone()
        };
        let raw = req.body.as_ref().map(|b| String::from_utf8(b.clone()).expect("a UTF-8 body"));
        Seen {
            method: req.method.clone(),
            path,
            query: url.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect(),
            headers: req.headers.iter().map(|(k, v)| (k.to_lowercase(), v.clone())).collect(),
            body: raw.as_deref().map(|r| serde_json::from_str(r).expect("a JSON body")).unwrap_or(Value::Null),
            raw,
        }
    }
}

#[async_trait(?Send)]
impl Fetch for FakeBox {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        let seen = Self::seen_of(&req);
        self.seen.lock().unwrap().push(seen.clone());
        let mut answer = self.next(&format!("{} {}", seen.method, seen.path)).unwrap_or_else(|| {
            let mut reply = error(404, "not_found", "not_found");
            if let Some(id) = self.unrouted_request_id.lock().unwrap().as_ref() {
                reply.body["requestId"] = Value::String(id.clone());
            }
            Answer::Reply(reply)
        });
        loop {
            match answer {
                Answer::With(f) => answer = f(&seen),
                Answer::Reply(r) => {
                    return Ok(FetchResponse { status: r.status, headers: Vec::new(), body: serde_json::to_vec(&r.body).unwrap() })
                }
                Answer::Road(e) => return Err(e),
                Answer::Hang => {
                    let Some(abort) = req.abort.clone() else { return std::future::pending().await };
                    abort.token.cancelled().await;
                    let timed_out = abort.timed_out.load(std::sync::atomic::Ordering::SeqCst);
                    return Err(if timed_out {
                        RoadError {
                            name: "TimeoutError".into(),
                            message: "The operation was aborted due to timeout".into(),
                            code: None,
                            cause: None,
                        }
                    } else {
                        RoadError { name: "AbortError".into(), message: "This operation was aborted".into(), code: None, cause: None }
                    });
                }
            }
        }
    }
}

/// box-backend.test.ts:94 `BOX`.
pub fn box_view(id: &str, state: &str, extra: Value) -> Value {
    let archived = state == "archived";
    let mut v = json!({
        "id": id,
        "name": "Box 2026-09-11 06:28",
        "state": state,
        "error": null,
        "type": "small",
        "vcpu": 2,
        "memoryGB": 4,
        "billingMultiplier": 0.5,
        "machineProvider": if archived { Value::Null } else { json!("hetzner") },
        "url": if archived { Value::Null } else { json!("https://box-node-f06a49ff29e4ed7a.on.boat.dev") },
        "ip": if archived { Value::Null } else { json!("2001:41d0:8:8d24:100::29") },
        "sshEndpoint": null,
        "createdAt": "2026-09-11T06:28:06.053Z",
        "updatedAt": "2026-09-11T06:28:07.139Z",
        "archiveAfter": if archived { Value::Null } else { json!("2026-09-11T08:28:06.059Z") },
        "desktopAvailable": true,
        "desktopUrl": null,
        "snapshotAvailable": true,
        "subdomain": "box-node-f06a49ff29e4ed7a",
        "environment": "base",
        "environmentVersion": 1,
    });
    if let Value::Object(extra) = extra {
        for (k, val) in extra {
            v[k] = val;
        }
    }
    v
}

/// box-backend.test.ts:118 `INFO`.
pub fn info(id: &str, state: &str, extra: Value) -> Answer {
    Answer::Reply(Reply { status: 200, body: json!({ "ok": true, "type": "sandbox.info", "sandbox": box_view(id, state, extra) }) })
}

/// box-backend.test.ts:119 `ERROR`, as a reply under its own status.
pub fn error(status: u16, code: &str, message: &str) -> Reply {
    Reply { status, body: error_body(status, code, message) }
}

pub fn error_body(status: u16, code: &str, message: &str) -> Value {
    json!({ "ok": false, "type": "sandbox.error", "status": status, "code": code, "message": message, "requestId": "req_b603629401be48d082e623575a1b9111" })
}

/// box-backend.test.ts:120 `LIMITS`.
pub fn limits(tier: &str) -> Answer {
    Answer::Reply(Reply {
        status: 200,
        body: json!({ "ok": true, "type": "limits.info", "accessTier": tier, "maxActiveSandboxes": 2, "startLimits": { "perMinute": 5, "perHour": 25, "perDay": 75 } }),
    })
}

/// box-backend.test.ts:121 `COMMAND`; `exit_code` `None` is a null exit code.
pub fn command(stdout: &str, exit_code: Option<i64>, extra: Value) -> Answer {
    let mut body = json!({ "ok": true, "type": "command.finished", "success": exit_code == Some(0), "exitCode": exit_code, "signal": null, "stdout": stdout, "stderr": "", "stdoutTruncated": false, "stderrTruncated": false, "timedOut": false });
    if let Value::Object(extra) = extra {
        for (k, v) in extra {
            body[k] = v;
        }
    }
    Answer::Reply(Reply { status: 200, body })
}

/// box-backend.test.ts:125 `NAMED`.
pub fn named(name: &str, status: &str, extra: Value) -> Value {
    let mut v = json!({ "name": name, "status": status, "sourceSandboxId": "bx_tumrjngm", "createdAt": "2026-09-11T06:47:23.959Z" });
    if status == "ready" {
        v["snapshotId"] = json!("dc8cd70e-d16b-472a-b5d7-070bc84db7a7");
        v["type"] = json!("small");
        v["sizeBytes"] = json!(1255755776u64);
    }
    if let Value::Object(extra) = extra {
        for (k, val) in extra {
            v[k] = val;
        }
    }
    v
}

/// box-backend.test.ts:135 `inner`: the script inside one sudo wrapper, its shell quoting undone.
pub fn inner(command: &str) -> String {
    let s = command.strip_prefix("sudo -n bash -c '").unwrap_or(command);
    let s = s.strip_suffix('\'').unwrap_or(s);
    s.replace("'\\''", "'")
}

/// box-backend.test.ts:150 `offline`: what fetch rejects with when the name does not resolve.
pub fn offline() -> RoadError {
    RoadError {
        name: "TypeError".into(),
        message: "fetch failed".into(),
        code: None,
        cause: Some(RoadCause { message: "getaddrinfo ENOTFOUND api.example".into(), code: Some("ENOTFOUND".into()), errors: None }),
    }
}

/// box-backend.test.ts:1763 `dropped`: the connection dropped after the request left.
pub fn dropped() -> RoadError {
    RoadError {
        name: "TypeError".into(),
        message: "fetch failed".into(),
        code: None,
        cause: Some(RoadCause { message: "read ECONNRESET".into(), code: Some("ECONNRESET".into()), errors: None }),
    }
}

/// box-backend.test.ts:167 `backendOn`'s budgets: poll 10, pause, resume, create, snapshot and restore 100 each.
pub fn test_budgets() -> wsp_engine::box_backend::BoxBudgetsAsk {
    wsp_engine::box_backend::BoxBudgetsAsk {
        poll_ms: Some(10),
        pause_ms: Some(100),
        resume_ms: Some(100),
        create_ms: Some(100),
        snapshot_ms: Some(100),
        restore_ms: Some(100),
        ..Default::default()
    }
}

/// What `backend_on` hands back.
pub struct On {
    pub backend: wsp_engine::box_backend::BoxBackend,
    pub clock: Arc<crate::common::fake_clock::FakeClock>,
    pub warned: crate::common::warn::Warned,
}

/// box-backend.test.ts:167 `backendOn`: `GET /limits` answers `limits(tier)`, the fake clock, the test budgets
/// overlaid by `budgets`' set fields, and the warn lines recorded.
pub fn backend_on(api: &Arc<FakeBox>, tier: &str, budgets: wsp_engine::box_backend::BoxBudgetsAsk) -> On {
    api.on("GET", "/limits", vec![limits(tier)]);
    backend_with(api.clone() as Arc<dyn Fetch>, budgets)
}

/// The same backend over any fetch (a wrapper that loses answers, one that hangs).
pub fn backend_with(fetch: Arc<dyn Fetch>, budgets: wsp_engine::box_backend::BoxBudgetsAsk) -> On {
    let clock = crate::common::fake_clock::FakeClock::new();
    let warned = crate::common::warn::Warned::new();
    let base = test_budgets();
    let pick = |a: Option<u64>, b: Option<u64>| a.or(b);
    let budgets = wsp_engine::box_backend::BoxBudgetsAsk {
        create_ms: pick(budgets.create_ms, base.create_ms),
        pause_ms: pick(budgets.pause_ms, base.pause_ms),
        resume_ms: pick(budgets.resume_ms, base.resume_ms),
        snapshot_ms: pick(budgets.snapshot_ms, base.snapshot_ms),
        restore_ms: pick(budgets.restore_ms, base.restore_ms),
        poll_ms: pick(budgets.poll_ms, base.poll_ms),
        read_ms: budgets.read_ms,
        backstop_ms: budgets.backstop_ms,
    };
    let backend = wsp_engine::box_backend::BoxBackend::new(wsp_engine::box_backend::BoxBackendOptions {
        api_key: "box_x".into(),
        base_url: None,
        fetch: Some(fetch),
        clock: Some(clock.clone()),
        budgets,
        warn: Some(warned.sink()),
    });
    On { backend, clock, warned }
}

/// box-backend.test.ts:174 `machineOn`: a BoxMachine handle on `id` (default bx_tumrjngm) over `backend_on(api, "trial")`.
pub fn machine_on(api: &Arc<FakeBox>, id: &str) -> (On, wsp_engine::box_backend::BoxMachine) {
    let on = backend_on(api, "trial", Default::default());
    let machine =
        wsp_engine::box_backend::BoxMachine::new(on.backend.clone(), id, wsp_engine::machine::MachineKind::Sandbox, None, None, false);
    (on, machine)
}
