// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the channels' roads to a person's accounts: where a folder's origin is read, and the line a sign-in
//! terminal runs. Expected values are the TS's: node probes `oracle/zz-review-accounts-origin.oracle.ts` and
//! `zz-review-accounts-terminal.oracle.ts` (their text and output are in the review report). Over the same world as
//! tests/channels_copy.rs.
#![allow(dead_code, unused_imports)]
mod common;
include!("channels_copy/world.rs");

use wsp_account::account::channels::ChannelRow;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::workspace::ThreadLit;

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-4: channels.ts:906-917. `originNow` keeps the origin it read per caller object, so a listing or a stream asks
// git once. Node counted: one caller object asked three times -> 1 read; three equal objects -> 3; no caller -> 3.
// channel.rs `origin_now` reads on every call (copy.rs:719-731), on the thread that is meant to hold nothing for long.

// ---------------------------------------------------------------------------------------------------------------
// R-ACC-5: channels.ts:690 `terminalLine(harness, signIn.program)`. Claude's sign-in has no program (brokered.ts:94-98,
// node: keys ["env"], program undefined), so the TS line REMOVES any program of Claude's in its folder (`rm -f`). copy.rs:431
// passes `""`: `ChannelsCtx::terminal_line` takes `&str` and cannot say none. A context that reads that as
// `ownProgram !== undefined` (the one literal reading) writes an EMPTY EXECUTABLE named claude, which the shell's login
// puts first on its PATH: `claude` in a sign-in terminal on the machine then runs nothing and answers 0.
// Below, `Lit` is the sim with the one change the TS has: claude's sign-in carries no program, and `terminal_line` is the
// TS function (brokered.ts:47-58) read the literal way.

pub const TS_CLAUDE_UNDEFINED: &str = r####"mkdir -p /usr/local/libexec/wsp-terminal/claude && rm -f /usr/local/libexec/wsp-terminal/claude/claude && printf %s '#!/bin/sh
printf '\''%s\n'\'' '\''wsp: Codex cannot sign in on this machine. This terminal'\''\'\'''\''s thread runs Claude Code, and your computer signs in only that agent here. Start a thread on Codex to use it.'\'' >&2
exit 1
' > /usr/local/libexec/wsp-terminal/claude/codex.$$ && chmod 755 /usr/local/libexec/wsp-terminal/claude/codex.$$ && mv -f /usr/local/libexec/wsp-terminal/claude/codex.$$ /usr/local/libexec/wsp-terminal/claude/codex && printf %s '[ -z "${WSP_TERMINAL:-}" ] || [ ! -d /usr/local/libexec/wsp-terminal/"$WSP_TERMINAL" ] || PATH=/usr/local/libexec/wsp-terminal/"$WSP_TERMINAL":$PATH
' > /etc/profile.d/wsp-terminal.sh.$$ && chmod 644 /etc/profile.d/wsp-terminal.sh.$$ && mv -f /etc/profile.d/wsp-terminal.sh.$$ /etc/profile.d/wsp-terminal.sh"####;
pub const TS_CLAUDE_EMPTY: &str = r####"mkdir -p /usr/local/libexec/wsp-terminal/claude && printf %s '' > /usr/local/libexec/wsp-terminal/claude/claude.$$ && chmod 755 /usr/local/libexec/wsp-terminal/claude/claude.$$ && mv -f /usr/local/libexec/wsp-terminal/claude/claude.$$ /usr/local/libexec/wsp-terminal/claude/claude && printf %s '#!/bin/sh
printf '\''%s\n'\'' '\''wsp: Codex cannot sign in on this machine. This terminal'\''\'\'''\''s thread runs Claude Code, and your computer signs in only that agent here. Start a thread on Codex to use it.'\'' >&2
exit 1
' > /usr/local/libexec/wsp-terminal/claude/codex.$$ && chmod 755 /usr/local/libexec/wsp-terminal/claude/codex.$$ && mv -f /usr/local/libexec/wsp-terminal/claude/codex.$$ /usr/local/libexec/wsp-terminal/claude/codex && printf %s '[ -z "${WSP_TERMINAL:-}" ] || [ ! -d /usr/local/libexec/wsp-terminal/"$WSP_TERMINAL" ] || PATH=/usr/local/libexec/wsp-terminal/"$WSP_TERMINAL":$PATH
' > /etc/profile.d/wsp-terminal.sh.$$ && chmod 644 /etc/profile.d/wsp-terminal.sh.$$ && mv -f /etc/profile.d/wsp-terminal.sh.$$ /etc/profile.d/wsp-terminal.sh"####;

fn put(path: &str, text: &str, mode: &str) -> String {
    format!("printf %s {} > {path}.$$ && chmod {mode} {path}.$$ && mv -f {path}.$$ {path}", shell_quote(text))
}

/// brokered.ts:47-58 for the harness claude, the rest of the line taken from what node printed.
fn ts_terminal_line_claude(own_program: Option<&str>) -> String {
    let file = "/usr/local/libexec/wsp-terminal/claude/claude";
    let mine = match own_program {
        Some(text) => put(file, text, "755"),
        None => format!("rm -f {file}"),
    };
    let rest = TS_CLAUDE_UNDEFINED
        .strip_prefix("mkdir -p /usr/local/libexec/wsp-terminal/claude && rm -f /usr/local/libexec/wsp-terminal/claude/claude && ")
        .expect("node's line has this head");
    format!("mkdir -p /usr/local/libexec/wsp-terminal/claude && {mine} && {rest}")
}

struct Lit(Rc<Sim>);

#[async_trait(?Send)]
impl ChannelsCtx for Lit {
    fn terminal_sign_in(&self, harness: &str, entry: &Entry, token: &str) -> Option<TerminalSignIn> {
        match harness {
            "claude" => Some(TerminalSignIn {
                program: None,
                env: [("SIGN_IN".to_string(), format!("claude:{token}"))].into_iter().collect(),
            }),
            _ => self.0.terminal_sign_in(harness, entry, token),
        }
    }
    fn terminal_line(&self, harness: &str, program: &str) -> String {
        assert_eq!(harness, "claude");
        ts_terminal_line_claude(Some(program))
    }
    fn index_start(&self, workspace_id: &str, thread_id: &str) -> Option<String> {
        self.0.index_start(workspace_id, thread_id)
    }
    fn index_cut(&self, workspace_id: &str, thread_id: &str) -> Option<bool> {
        self.0.index_cut(workspace_id, thread_id)
    }
    fn index_fact(&self, workspace_id: &str, resume: &str, fact: &str) -> Option<String> {
        self.0.index_fact(workspace_id, resume, fact)
    }
    fn session_rows(&self) -> Vec<ChannelRow> {
        self.0.session_rows()
    }
    fn thread_record(&self, thread_id: &str) -> Option<ThreadRecord> {
        self.0.thread_record(thread_id)
    }
    fn thread_ids(&self) -> Vec<String> {
        self.0.thread_ids()
    }
    fn latest_harness(&self, thread_id: &str) -> Option<String> {
        self.0.latest_harness(thread_id)
    }
    fn home_dir(&self, record: &WorkspaceRecord) -> Option<String> {
        self.0.home_dir(record)
    }
    async fn daemon_road(&self, entry: &Entry) -> AccountResult<DaemonReachView> {
        self.0.daemon_road(entry).await
    }
    fn turn_road(&self, kind: WorkspaceKind) -> Option<ScopedRoad> {
        self.0.turn_road(kind)
    }
    fn has_door(&self) -> bool {
        self.0.has_door()
    }
    fn offer_of(&self, place_id: &str) -> Option<String> {
        self.0.offer_of(place_id)
    }
    fn wired(&self) -> String {
        self.0.wired()
    }
    fn settings_at(&self, place_id: &str) -> Option<PlaceSettings> {
        self.0.settings_at(place_id)
    }
    fn turn_limit_at(&self, place_id: &str) -> Option<Option<i64>> {
        self.0.turn_limit_at(place_id)
    }
    fn name_of(&self, place_id: &str) -> String {
        self.0.name_of(place_id)
    }
    fn joined(&self, place_id: &str) -> bool {
        self.0.joined(place_id)
    }
    async fn report_of(&self, place_id: &str) -> Option<PlaceReport> {
        self.0.report_of(place_id).await
    }
    async fn rows(&self) -> Vec<PlaceView> {
        self.0.rows().await
    }
    fn link_channel(&self, place_id: &str, on_event: Rc<dyn Fn(Value)>) -> Option<Rc<dyn DaemonChannel>> {
        self.0.link_channel(place_id, on_event)
    }
    async fn spend_today(&self, rows: &[PlaceView], now: i64) -> Vec<(String, Option<f64>)> {
        self.0.spend_today(rows, now).await
    }
    fn projects_held(&self) -> IndexMap<String, ProjectView> {
        self.0.projects_held()
    }
    async fn open_channel(&self, url: &str, token: &str, on_event: Rc<dyn Fn(Value)>) -> AccountResult<Rc<dyn DaemonChannel>> {
        self.0.open_channel(url, token, on_event).await
    }
    fn local_has_daemon(&self) -> bool {
        self.0.local_has_daemon()
    }
    fn local_home_dir(&self) -> String {
        self.0.local_home_dir()
    }
    async fn local_road(&self) -> AccountResult<DaemonReachView> {
        self.0.local_road().await
    }
    async fn adopt_pause(&self, entry: &Entry) {
        self.0.adopt_pause(entry).await
    }
    fn broker_ready(&self) -> bool {
        self.0.broker_ready()
    }
    fn root_of(&self, thread_id: &str) -> String {
        self.0.root_of(thread_id)
    }
    fn device_door(&self) -> Rc<dyn DeviceDoor> {
        self.0.device_door()
    }
    fn host_pid(&self) -> i64 {
        self.0.host_pid()
    }
    fn host_parent_pid(&self) -> i64 {
        self.0.host_parent_pid()
    }
    fn has_state_path(&self) -> bool {
        self.0.has_state_path()
    }
    fn state_folder(&self) -> String {
        self.0.state_folder()
    }
    fn read_link(&self, path: &str) -> Option<String> {
        self.0.read_link(path)
    }
    fn hostname(&self) -> String {
        self.0.hostname()
    }
    fn pid_alive(&self, pid: i64) -> bool {
        self.0.pid_alive(pid)
    }
    fn agent_bins(&self) -> Vec<String> {
        self.0.agent_bins()
    }
    fn agent_pid_of(&self, run: &str) -> Option<i64> {
        self.0.agent_pid_of(run)
    }
    fn group_exists(&self, pgid: i64) -> bool {
        self.0.group_exists(pgid)
    }
    fn proc_scopes_moved(&self) {
        self.0.proc_scopes_moved()
    }
    fn roots_recheck(&self) -> Rc<RefCell<Option<Cancel>>> {
        self.0.roots_recheck()
    }
    fn box_panels_ctx(&self) -> Rc<dyn BoxPanelsCtx> {
        self.0.box_panels_ctx()
    }
    async fn store_get(&self, collection: &str, id: &str) -> AccountResult<Option<Value>> {
        self.0.store_get(collection, id).await
    }
    async fn store_put(&self, collection: &str, id: &str, value: Value) -> AccountResult<()> {
        self.0.store_put(collection, id, value).await
    }
    async fn store_delete(&self, collection: &str, id: &str) -> AccountResult<()> {
        self.0.store_delete(collection, id).await
    }
    async fn store_keys(&self, collection: &str) -> AccountResult<Vec<String>> {
        self.0.store_keys(collection).await
    }
    fn on_workspace_deleted(&self, f: Rc<dyn Fn(&str)>) {
        self.0.on_workspace_deleted(f)
    }
    fn live_has(&self, workspace_id: &str) -> bool {
        self.0.live_has(workspace_id)
    }
    async fn thread_launch(&self, entry: &Entry, thread_id: &str, root_thread_id: &str) -> AccountResult<ThreadLaunch> {
        self.0.thread_launch(entry, thread_id, root_thread_id).await
    }
    fn brokered_env(&self, token: &str, entry: &Entry) -> IndexMap<String, String> {
        self.0.brokered_env(token, entry)
    }
    async fn thread_folder(&self, entry: &Entry) -> String {
        self.0.thread_folder(entry).await
    }
    fn thread_env(&self, entry: &Entry) -> IndexMap<String, String> {
        self.0.thread_env(entry)
    }
    async fn persist(&self, record: &WorkspaceRecord) -> AccountResult<()> {
        self.0.persist(record).await
    }
    fn read_checkout_soon(&self, entry: &Entry) {
        self.0.read_checkout_soon(entry)
    }
    async fn delete_workspace(&self, workspace_id: &str) -> AccountResult<()> {
        self.0.delete_workspace(workspace_id).await
    }
    fn git_origin(&self, path: &str, timeout_ms: i64) -> (Option<i32>, String) {
        self.0.git_origin(path, timeout_ms)
    }
    fn now(&self) -> i64 {
        self.0.now()
    }
    fn schedule(&self, ms: i64, f: Box<dyn FnOnce()>) -> Cancel {
        self.0.schedule(ms, f)
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        self.0.spawn(f)
    }
    fn warn(&self, line: &str) {
        self.0.warn(line)
    }
}

#[test]
fn the_port_of_the_ts_line_is_the_one_node_printed() {
    assert_eq!(ts_terminal_line_claude(None), TS_CLAUDE_UNDEFINED);
    assert_eq!(ts_terminal_line_claude(Some("")), TS_CLAUDE_EMPTY);
}

#[test]
fn a_claude_sign_in_terminal_has_its_program_taken_away_and_never_replaced_by_an_empty_one() {
    let fixture = oracle("channels_copy");
    let world = fixture["scenarios"]["c6 terminal token env order"]["world"].clone();
    run(async move {
        let b = build(&world);
        let area = ChannelsArea::new(Rc::new(Lit(b.sim.clone())));
        let env = area.shell_sign_ins(&b.entry, "t1", "tok1").await;
        assert_eq!(env.get("WSP_TERMINAL").map(String::as_str), Some("claude"));
        let execs: Vec<Value> = b.sim.fx.all().into_iter().filter(|e| e[0] == json!("exec")).collect();
        assert_eq!(execs.len(), 1, "one write of the terminal programs");
        assert_eq!(
            execs[0][1],
            json!(TS_CLAUDE_UNDEFINED),
            "the machine is told what the TS tells it for claude (rm -f of claude's program)"
        );
    })
}

// ---------------------------------------------------------------------------------------------------------------
// checkoutOf (channels.ts:114-121) on odd and hostile paths: node's `join` and `relative` are the oracle. Printed by node
// (`oracle/zz-review-accounts-checkout.oracle.ts`). The paths are a caller's (a project's folder, a worktree's path); the
// answer is the cwd every daemon op there is sent. These pass.

const CHECKOUT_CASES: &str = r####"[{"path": "/home/u/wsp", "top": "/home/u/wsp", "tree": "/wt/a", "answer": "/wt/a"}, {"path": "/home/u/wsp/sub/dir", "top": "/home/u/wsp", "tree": "/wt/a", "answer": "/wt/a/sub/dir"}, {"path": "/home/u/wsp", "top": "/home/u/wsp/sub", "tree": "/wt/a", "answer": "/wt"}, {"path": "/home/u/a/../b", "top": "/home/u/b", "tree": "/wt/a", "answer": "/wt/a"}, {"path": "/home/u/b/", "top": "/home/u/b", "tree": "/wt/a/", "answer": "/wt/a/"}, {"path": "/x/y", "top": "/z", "tree": "/wt/a", "answer": "/wt/x/y"}, {"path": "rel/p", "top": "rel", "tree": "/wt/a", "answer": "/wt/a/p"}, {"path": "/home/u/wsp", "top": "/home/u/wsp", "tree": "/wt/a'; touch /tmp/x; '", "answer": "/wt/a'; touch /tmp/x; '"}, {"path": "/home/u/wsp/$(id)", "top": "/home/u/wsp", "tree": "/wt/a", "answer": "/wt/a/$(id)"}, {"path": "/home/u/wsp/sub", "top": "/home/u/wsp", "tree": "", "answer": "sub"}, {"path": "/home/u/wsp", "top": "/home/u/wsp", "tree": "", "answer": "."}, {"path": "/a/b", "top": "/a/b/c/d", "tree": "/wt", "answer": "/"}, {"path": "/a/b/x", "top": "/a/b", "tree": "/", "answer": "/x"}, {"path": "/home//u/wsp/sub", "top": "/home/u/wsp", "tree": "/wt//a", "answer": "/wt/a/sub"}, {"path": "/home/u/wsp/sub", "top": null, "tree": "/wt/a", "answer": "/wt/a"}, {"path": "/home/u/wsp/./sub/../sub2", "top": "/home/u/wsp", "tree": "/wt/./a", "answer": "/wt/a/sub2"}, {"path": "/home/u/wsp/sub", "top": "/home/u/wsp", "tree": "wt/a", "answer": "wt/a/sub"}, {"path": "/home/u/wsp/sub", "top": "/home/u/wsp", "tree": "../wt", "answer": "../wt/sub"}, {"path": "/home/u/wsp/sub", "top": "/home/u/wsp", "tree": "/wt/a\nb", "answer": "/wt/a\nb/sub"}, {"path": "/home/u/wsp/sub\^@x", "top": "/home/u/wsp", "tree": "/wt/a", "answer": "/wt/a/sub\^@x"}, {"path": "/", "top": "/", "tree": "/wt/a", "answer": "/wt/a"}, {"path": "/home/u/wsp/..", "top": "/home/u/wsp", "tree": "/wt/a", "answer": "/wt"}, {"path": "/home/u/wsp/sub/", "top": "/home/u/wsp/", "tree": "/wt/a/", "answer": "/wt/a/sub"}, {"path": "/home/u/wsp", "top": "/home/u/wsp/", "tree": "/wt/a", "answer": "/wt/a"}, {"path": "C:\\work\\p", "top": "C:\\work", "tree": "/wt/a", "answer": "/wt/C:\\work\\p"}]"####;

#[test]
fn checkout_of_is_nodes_join_and_relative_on_odd_and_hostile_paths() {
    let fixture = oracle("channels_copy");
    let base = fixture["scenarios"]["c13 the status is read at the worktree"]["world"].clone();
    run(async move {
        let cases: Vec<Value> = serde_json::from_str(CHECKOUT_CASES).unwrap();
        let mut wrong = vec![];
        for c in &cases {
            let mut w = base.clone();
            w["record"]["worktree"] = json!({ "path": c["tree"], "made": true });
            w["projects"][0]["path"] = c["path"].clone();
            w["projects"][0]["source"] = json!({ "kind": "folder", "path": c["path"] });
            w["projects"][0]["remote"] = json!("");
            if c["top"].is_null() {
                w["projects"][0].as_object_mut().unwrap().remove("git");
            } else {
                w["projects"][0]["git"] = json!({ "top": c["top"] });
            }
            let b = build(&w);
            let got = b.area.checkout_of(&record(&w["record"])).ok();
            if got.as_deref() != c["answer"].as_str() {
                wrong.push(format!("path {:?} top {:?} tree {:?}: node {:?}, rust {:?}", c["path"], c["top"], c["tree"], c["answer"], got));
            }
        }
        assert!(wrong.is_empty(), "checkout_of differs from node:\n{}", wrong.join("\n"));
    })
}
