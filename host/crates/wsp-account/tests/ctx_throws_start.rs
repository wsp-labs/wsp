// SPDX-License-Identifier: AGPL-3.0-only
//! The context's `namedMode` throws through the start area's picks check, where an access word names a mode the harness
//! lacks (threads/agents.ts:428). The throw is the answer, and no pick check runs after it.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/start.rs"]
mod start;

use std::rc::Rc;

use base::{err_answer, kinded, project, run, FakeBase};
use serde_json::{json, Value};
use start::{catalog_json, FakeStart};
use wsp_account::projects::start_from::{start_from_area, StartFrom, StartFromCtx};
use wsp_core::types::harness::StartPicksAsked;
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_wire::views::workspace::ProjectView;
use wsp_words::exit::{HostError, Kind};

const REMOTE: &str = "https://github.com/dev/spoo.git";
const TOP: &str = "/home/me/spoo";

struct H {
    fake: Rc<FakeStart>,
    area: Rc<StartFrom>,
}

impl H {
    fn new() -> H {
        let fake = FakeStart::new();
        let ctx: Rc<dyn StartFromCtx> = fake.clone();
        let area = start_from_area(ctx);
        H { fake, area }
    }
    fn base(&self) -> &Rc<FakeBase> {
        &self.fake.base
    }
}

fn here_project() -> ProjectView {
    project("pr_1", "spoo", "here", json!({ "remote": REMOTE, "path": TOP, "git": { "top": TOP } }))
}

fn prefs() -> wsp_wire::views::preferences::Preferences {
    serde_json::from_value(json!({"theme":"dark","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false})).expect("preferences read")
}

fn open_picks() -> Value {
    json!({ "catalog": catalog_json("codex"), "openModel": "sonnet", "openEffort": "high" })
}

// threads/agents.ts:428 (namedMode): the refusal is the answer, and no pick is checked after it.
#[test]
fn ctx_throws_named_mode_passes_through_picks_hold() {
    run(async {
        let h = H::new();
        *h.base().prefs.borrow_mut() = Some(prefs());
        h.base().answer_with("resolved_picks", vec![open_picks()]);
        let said = "codex takes no access word auto-edit";
        h.base().answer_with("named_mode", vec![err_answer(said, Some("usage"))]);
        let asked = StartPicksAsked { harness: Some("codex".into()), access: Some(AccessChoice::AutoEdit), ..Default::default() };
        let got: Result<(), HostError> = h.area.picks_hold(&here_project(), &asked).await;
        assert_eq!(got.expect_err("namedMode throws"), kinded(said, Kind::Usage));
        assert!(h.base().args_of("start_picks_refusal").is_empty(), "no pick check after the throw");
    });
}
