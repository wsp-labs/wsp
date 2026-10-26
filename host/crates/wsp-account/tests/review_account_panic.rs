// SPDX-License-Identifier: AGPL-3.0-only
//! Review for panics: random JSON of every type (null, huge and negative numbers, floats, deep arrays and objects, odd
//! strings) as the frames a page sends and as the replies a daemon gives, through the pty wrappers, the box panels, the
//! kill guard and the process scopes. A panic anywhere fails the test where it happens. Over the worlds of
//! tests/channels_ptys.rs and tests/box_panels.rs.
#![allow(dead_code, unused_imports)]
mod common;
include!("channels_ptys/world.rs");

use wsp_account::account::box_panels::{BoxPanels, BoxPanelsOpener, Placements, Watchers};
use wsp_account::account::core_types::Served;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len() as u64) as usize]
    }
}

const KEYS: [&str; 22] = [
    "id", "ok", "ptyId", "pid", "ptys", "thread", "roots", "cgroups", "env", "cwd", "watch", "port", "ports", "type", "exited", "op", "error",
    "code", "protect", "apps", "agents", "agentBins",
];

fn rnd(r: &mut Rng, depth: u8) -> Value {
    match r.below(if depth > 2 { 7 } else { 9 }) {
        0 => Value::Null,
        1 => Value::Bool(r.below(2) == 0),
        2 => json!(*r.pick(&[0_i64, 1, -1, 5, 4242, i64::MAX, i64::MIN, 2_147_483_648])),
        3 => json!(u64::MAX),
        4 => json!(*r.pick(&[0.5_f64, 1e21, 1e308, -0.0, 1.5e-7, 9007199254740993.0, 4242.0])),
        5 => Value::String(r.pick(&["", "x", "pty_1", "\u{1F4A5}", "a-b", "-1", "NaN", "/wsp-threads/ta", "pty_2"]).to_string()),
        6 => Value::String((0..r.below(6)).map(|_| char::from_u32(0x20 + r.below(0x2f0) as u32).unwrap_or('?')).collect()),
        7 => Value::Array((0..r.below(4)).map(|_| rnd(r, depth + 1)).collect()),
        _ => Value::Object((0..r.below(5)).map(|_| (r.pick(&KEYS).to_string(), rnd(r, depth + 1))).collect()),
    }
}

fn rnd_object(r: &mut Rng) -> Frame {
    (0..r.below(7)).map(|_| (r.pick(&KEYS).to_string(), rnd(r, 0))).collect()
}

fn rnd_frame(r: &mut Rng, ops: &[&str]) -> Frame {
    let mut f = rnd_object(r);
    if r.below(10) != 0 {
        f.insert("op".into(), json!(*r.pick(ops)));
    }
    if r.below(2) == 0 {
        f.insert("ptyId".into(), json!(*r.pick(&["pty_1", "pty_2", "pty_3"])));
    }
    f
}

#[test]
fn the_pty_wrappers_and_the_kill_guard_never_panic_on_what_a_page_or_a_daemon_sends() {
    run(async {
        for seed in 1..=300_u64 {
            let mut r = Rng(0x9E3779B97F4A7C15 ^ seed.wrapping_mul(0x2545F4914F6CDD1D));
            let w = World::new();
            let area = w.area();
            let rng = Rc::new(RefCell::new(Rng(seed.wrapping_mul(7919) | 1)));
            let d = {
                let rng = rng.clone();
                Daemon::new(move |_fr| {
                    let mut r = rng.borrow_mut();
                    let mut reply = rnd_object(&mut r);
                    if r.below(3) != 0 {
                        reply.insert("ok".into(), json!(true));
                    }
                    if r.below(3) != 0 {
                        reply.insert("ptyId".into(), json!(*r.pick(&["pty_1", "pty_2", "pty_3"])));
                    }
                    if r.below(3) != 0 {
                        let rows: Vec<Value> = (0..r.below(4)).map(|_| Value::Object(rnd_object(&mut r))).collect();
                        reply.insert("ptys".into(), Value::Array(rows));
                    }
                    reply
                })
            };
            let ptys = ["pty.create", "pty.list", "pty.kill", "pty.thread", "pty.attach", "ports.watch", "proc.kill", "ping", "pty.write"];
            let pane = area.rooted_ports("ws", "/work/ws", area.held_to_owner("ws", area.thread_tagged("ws", area.kill_guarded(d.clone(), seed % 2 == 0))));
            let lock = format!("/state/{}/{}", wsp_wire::types::daemon_contract::APP_DATA_FOLDER, wsp_wire::types::daemon_contract::APP_LOCK_FILE);
            for step in 0..25 {
                match r.below(10) {
                    0 => {
                        let link = r.pick(&["", "-", "a-", "-1", "host-", "host--1", "host-1-2", "host-99999999999999999999", "h\u{e9}-5", "host-0", "host-+5", "host-٣"]).to_string();
                        w.fs_links.borrow_mut().insert(lock.clone(), link);
                        w.alive.borrow_mut().insert(5);
                        w.alive.borrow_mut().insert(0);
                    }
                    1 => {
                        w.groups.borrow_mut().insert(*r.pick(&[5000_i64, 5001, -1, 0, i64::MAX]));
                        area.turn_groups.borrow_mut().entry("ws".into()).or_default().insert(*r.pick(&[5000_i64, 5001, -1, 0, i64::MAX]), "t".into());
                        area.port_roots_moved("ws");
                    }
                    2 => {
                        w.groups.borrow_mut().clear();
                        area.port_roots_moved("ws");
                    }
                    3 => area.shared_daemon_replaced(),
                    4 if step == 20 => w.delete_workspace_event("ws"),
                    _ => {
                        let f = rnd_frame(&mut r, &ptys);
                        let _ = pane.send(f).await;
                    }
                }
                settle().await;
            }
            let _ = area.thread_terminals();
            let _ = area.port_roots_of("ws");
            let _ = area.app_main();
        }
    });
}

// ---- the box panels ----

struct Link2(Rc<()>, RefCell<Rng>);

#[async_trait(?Send)]
impl BoxPanelsCtx for Link2 {
    fn on_workspace_deleted(&self, _f: Rc<dyn Fn(&str)>) {}
    fn session_rows(&self) -> Vec<StartedRow> {
        let mut r = self.1.borrow_mut();
        (0..r.below(4))
            .map(|_| StartedRow {
                workspace_id: "w1".into(),
                thread_id: if r.below(5) == 0 { None } else { Some(format!("t{}", r.below(25))) },
                started_at: *r.pick(&[None, Some(0.0), Some(5.0), Some(-1.0), Some(f64::INFINITY), Some(f64::NEG_INFINITY), Some(1e308)]),
            })
            .collect()
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        let mut r = self.1.borrow_mut();
        (0..r.below(40)).map(|i| (format!("t{i}"), "w1".to_string())).collect()
    }
    fn link(&self, _place_id: &str) -> Option<Rc<()>> {
        Some(self.0.clone())
    }
    fn name_of(&self, place_id: &str) -> String {
        place_id.to_string()
    }
    async fn daemon_version(&self, _place_id: &str) -> Option<i64> {
        let mut r = self.1.borrow_mut();
        *r.pick(&[None, Some(0), Some(135), Some(136), Some(i64::MAX), Some(i64::MIN), Some(-1)])
    }
    async fn watch_sys(&self, _place_id: &str) -> Result<Release, String> {
        if self.1.borrow_mut().below(3) == 0 {
            Err("no".into())
        } else {
            Ok(Rc::new(|| {}))
        }
    }
    fn scopes_on(&self, _place_id: &str) -> Vec<ProcScope> {
        vec![]
    }
    fn thread_env(&self, _entry: &Entry) -> IndexMap<String, String> {
        IndexMap::new()
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        tokio::task::spawn_local(f);
    }
}

#[test]
fn the_box_panels_never_panic_on_what_a_page_or_a_computer_sends() {
    run(async {
        use futures::FutureExt;
        for seed in 1..=300_u64 {
            let mut r = Rng(0x9E3779B97F4A7C15 ^ seed.wrapping_mul(0x2545F4914F6CDD1D));
            let rng = Rc::new(RefCell::new(Rng(seed.wrapping_mul(104729) | 1)));
            let link = Rc::new(());
            let ctx = Rc::new(Link2(link.clone(), RefCell::new(Rng(seed.wrapping_mul(31337) | 1))));
            let watchers = Watchers::default();
            let opener = BoxPanelsOpener::new(ctx, watchers.clone(), Placements::default());
            let served: Served = {
                let rng = rng.clone();
                Rc::new(move |_f: Frame| {
                    let mut r = rng.borrow_mut();
                    let mut reply = rnd_object(&mut r);
                    if r.below(3) != 0 {
                        reply.insert("ok".into(), json!(true));
                    }
                    if r.below(2) == 0 {
                        let rows: Vec<Value> = (0..r.below(4)).map(|_| rnd(&mut r, 1)).collect();
                        reply.insert("ports".into(), Value::Array(rows));
                    }
                    async move { AccountResult::Ok(reply) }.boxed_local()
                })
            };
            let entry = common::live(common::record(
                &json!({ "id": "w1", "name": "alpha-studio", "kind": "place", "project": "pA2", "machineId": "", "phase": "running", "place": "box1" }),
            ));
            let panes: Vec<Rc<dyn BoxPanels>> = (0..2).map(|_| opener.open(&entry, "box1", served.clone(), "/srv/alpha")).collect();
            let ops = ["ports.watch", "sys.watch", "sys.unwatch", "proc.watch", "proc.unwatch", "proc.inspect", "proc.kill", "container.stop", "pty.create", "other"];
            for _ in 0..25 {
                let pane = &panes[r.below(2) as usize];
                match r.below(8) {
                    0 => {
                        let mut e = rnd_object(&mut r);
                        e.insert("type".into(), json!(*r.pick(&["port.open", "port.close", "sys.sample", "proc.snapshot", "proc.changes", "x"])));
                        let _ = pane.heard(&Value::Object(e));
                    }
                    1 => pane.close(),
                    2 => {
                        let pushes: Vec<Rc<dyn Fn()>> = watchers.borrow().get("w1").cloned().unwrap_or_default();
                        for p in pushes {
                            p();
                        }
                    }
                    _ => {
                        let f = rnd_frame(&mut r, &ops);
                        let _ = pane.send(f).await;
                    }
                }
                settle().await;
            }
        }
    });
}
