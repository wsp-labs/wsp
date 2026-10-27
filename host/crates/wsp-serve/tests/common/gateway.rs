// SPDX-License-Identifier: AGPL-3.0-only
//! packages/engine/test/split-gateway.ts: the Solari gateway as it answered on 2026-09-26, two copies behind one
//! address, one holding every running sandbox and one that never heard of them, each call after a create landing on
//! whichever copy `route` names. The empty copy answers a delete with the same 200 and every read with a 404; only a
//! delete that lands on the holder ends it. It is the `fetch` a `SolariBackend` is handed.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use wsp_engine::errors::RoadError;
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Holder,
    Empty,
}

pub struct SplitGateway {
    route: Box<dyn Fn(u32, &str) -> Side>,
    /// The sandboxes the holder runs, with their state.
    pub holder: RefCell<IndexMap<String, String>>,
    call: Cell<u32>,
    made: Cell<u32>,
    /// Every request the gateway took, as `(method, url)`: `f.mock.calls`.
    pub calls: RefCell<Vec<(String, String)>>,
}

impl SplitGateway {
    pub fn new(route: impl Fn(u32, &str) -> Side + 'static) -> Rc<SplitGateway> {
        Rc::new(SplitGateway {
            route: Box::new(route),
            holder: RefCell::default(),
            call: Cell::new(0),
            made: Cell::new(0),
            calls: RefCell::default(),
        })
    }

    pub fn deletes(&self) -> usize {
        self.calls.borrow().iter().filter(|(m, _)| m == "DELETE").count()
    }

    pub fn gets(&self) -> usize {
        self.calls.borrow().iter().filter(|(m, _)| m == "GET").count()
    }

    /// The gateway as the `Fetch` a backend takes.
    pub fn fetch(self: &Rc<Self>) -> std::sync::Arc<dyn Fetch> {
        std::sync::Arc::new(Handle(self.clone()))
    }
}

struct Handle(Rc<SplitGateway>);

fn json(status: u16, body: serde_json::Value) -> FetchResponse {
    FetchResponse { status, headers: vec![("content-type".into(), "application/json".into())], body: body.to_string().into_bytes() }
}

#[async_trait(?Send)]
impl Fetch for Handle {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        let g = &self.0;
        g.calls.borrow_mut().push((req.method.clone(), req.url.clone()));
        if req.method == "POST" {
            g.made.set(g.made.get() + 1);
            let id = format!("sb{}", g.made.get());
            g.holder.borrow_mut().insert(id.clone(), "running".into());
            return Ok(json(201, serde_json::json!({ "sandboxId": id, "kind": "sandbox" })));
        }
        let path = req.url.split_once("://").map_or(req.url.as_str(), |(_, rest)| rest.split_once('/').map_or("", |(_, p)| p));
        let id = path.split('/').nth(1).unwrap_or_default().to_string();
        let n = g.call.get();
        g.call.set(n + 1);
        let known = (g.route)(n, &req.method) == Side::Holder && g.holder.borrow().contains_key(&id);
        if req.method == "DELETE" {
            if known {
                g.holder.borrow_mut().shift_remove(&id);
            }
            return Ok(json(200, serde_json::json!({ "ok": true })));
        }
        Ok(if known {
            let state = g.holder.borrow().get(&id).cloned().unwrap_or_default();
            json(200, serde_json::json!({ "sandboxId": id, "kind": "sandbox", "state": state }))
        } else {
            json(404, serde_json::json!({ "error": "Not found" }))
        })
    }
}
