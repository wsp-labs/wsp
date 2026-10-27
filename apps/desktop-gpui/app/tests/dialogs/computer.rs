// SPDX-License-Identifier: AGPL-3.0-only
//! Add a computer's states for `dialogs_shots`, each staged from fixture answers shaped as the protocol's types with no
//! host behind it; `dialogs_add_computer` drives the same fixtures.
#![allow(dead_code)]

use desktop_gpui::screens::dialogs::add_computer::model::AddStep;
use desktop_gpui::screens::dialogs::add_computer::{self, AddComputer};
use gpui_kit::{AnyView, App, Context, Entity, Window};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use wsp_host_client::protocol::{PlaceAddJob, PlaceEstimate, PlaceView, RecipeView, SshHostSuggestion};

use super::{page, State};

/// A fixture read as the protocol's type `T` and written back, so a field the type does not hold fails here.
pub fn shaped<T: DeserializeOwned + Serialize>(v: Value) -> Value {
    let read: T = serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{v}: {e}"));
    serde_json::to_value(read).unwrap()
}

pub const BOX: &str = "nobody@192.0.2.1";

pub fn here() -> Value {
    shaped::<PlaceView>(json!({
        "id": "here", "kind": "computer", "name": "Studio", "default": true, "agents": ["claude", "codex"],
        "agentVersions": { "claude": "2.1.295", "codex": "0.71.0" }, "signIns": {}, "sizes": []
    }))
}

/// The box once it joined, its setup as `setup` and its rows as `rows`.
pub fn joined(setup: Option<Value>, rows: Value) -> Value {
    let mut place = json!({
        "id": "p_box", "kind": "computer", "name": "lab-box", "default": false, "agents": [], "agentVersions": {}, "signIns": {}, "sizes": [],
        "diskFreeBytes": 41_000_000_000u64,
        "picks": picks(),
        "recipe": "none",
        "applied": { "hash": "h", "at": "2026-10-18T10:00:00Z", "rows": rows }
    });
    if let Some(setup) = setup {
        place["setup"] = setup;
    }
    shaped::<PlaceView>(place)
}

pub fn picks() -> Value {
    json!({
        "name": "lab-box",
        "agents": { "claude": { "signin": "vault" }, "codex": { "signin": "machine" } },
        "mcp": { "linear": { "agents": ["claude"] } },
        "clis": { "rg": { "via": "apt" }, "gh": { "via": "apt" } },
        "skills": { "review": { "from": "~/.claude/skills/review" } },
        "plugins": {},
        "folders": { "wsp": { "from": "/home/me/src/wsp", "name": "wsp", "keep": [] } },
        "configs": { "git": {}, "github": { "signin": "machine" } }
    })
}

pub fn options() -> Value {
    json!({
        "agents": [
            { "id": "claude", "name": "Claude Code", "signins": ["vault", "machine", "token"], "bytes": 218_103_808 },
            { "id": "codex", "name": "Codex", "signins": ["machine"], "kind": "device", "bytes": 477_102_080 },
            { "id": "opencode", "name": "OpenCode", "signins": ["machine", "key"] }
        ],
        "mcp": [
            { "name": "linear", "agents": ["claude"], "kind": "oauth" },
            { "name": "sentry", "agents": ["claude", "codex"], "kind": "key", "keys": ["SENTRY_TOKEN"] }
        ],
        "clis": [
            { "name": "gh", "via": "apt", "version": "2.62.0", "bytes": 42_000_000, "calls": 212 },
            { "name": "rg", "via": "apt", "version": "14.1.1", "bytes": 6_100_000, "calls": 87 },
            { "name": "ffmpeg", "via": "apt", "bytes": 310_000_000, "calls": 3 },
            { "name": "duckdb", "via": "source", "needs": ["cc", "make"], "bytes": 1_400_000_000u64 }
        ],
        "skills": [
            { "name": "review", "from": "~/.claude/skills/review", "linked": false },
            { "name": "release-notes", "from": "~/.claude/skills/release-notes", "linked": true }
        ],
        "plugins": [{ "name": "frontend-design@claude-plugins", "on": true }, { "name": "pr-review@claude-plugins" }],
        "configs": [
            { "id": "git", "label": "~/.gitconfig, name and email" },
            { "id": "shell", "label": "~/.zshrc" },
            { "id": "github", "label": "gh", "signins": ["vault", "machine", "skip"], "account": "zingzy", "scopes": ["repo", "read:org"] }
        ],
        "folders": [{ "name": "wsp", "path": "/home/me/src/wsp", "remote": "git@github.com:wsp-labs/wsp.git", "unpushed": 2, "bytes": 380_000_000 }]
    })
}

pub fn projects() -> Vec<Value> {
    vec![
        json!({ "id": "pr1", "name": "wsp", "path": "/home/me/src/wsp", "remote": "git@github.com:wsp-labs/wsp.git", "computer": "here" }),
        json!({ "id": "pr2", "name": "notes", "path": "/home/me/notes", "remote": "", "computer": "here" }),
    ]
}

pub fn job(state: &str, steps: Value) -> Value {
    shaped::<PlaceAddJob>(
        json!({ "addId": "a_fixture", "address": BOX, "startedAt": "2026-10-18T10:00:00Z", "state": state, "steps": steps }),
    )
}

fn hosts(n: usize) -> Vec<Value> {
    let mut out: Vec<Value> = (1..=n)
        .map(|i| {
            shaped::<SshHostSuggestion>(
                json!({ "alias": format!("box-{i:02}"), "hostName": format!("10.0.0.{i}"), "user": "root", "from": "config" }),
            )
        })
        .collect();
    out.push(
        shaped::<SshHostSuggestion>(json!({ "alias": "build-runner-eu-central-1-ephemeral-gpu-large-0042", "hostName": "203.0.113.42", "user": "ubuntu", "from": "config" })),
    );
    out
}

/// A setup on the box at `state`, its steps as `steps` and its waits as `waiting`.
pub fn setup(state: &str, steps: Value, waiting: Value) -> Value {
    let mut s = json!({ "state": state, "addId": "s_fixture", "startedAt": "2026-10-18T10:00:00Z", "steps": steps, "waiting": waiting });
    if state != "running" {
        s["finishedAt"] = json!("2026-10-18T10:04:12Z");
    }
    s
}

pub fn done_steps() -> Value {
    json!([
        { "step": "floor", "state": "done", "ms": 41_200 }, { "step": "agents", "state": "done", "ms": 63_000 },
        { "step": "signins", "state": "done", "ms": 2_400 }, { "step": "mcp", "state": "done", "ms": 900 },
        { "step": "clis", "state": "done", "ms": 12_300 }, { "step": "skills", "state": "done", "ms": 400 },
        { "step": "plugins", "state": "done", "ms": 3_100 }, { "step": "github", "state": "done", "ms": 1_200 },
        { "step": "folders", "state": "done", "ms": 22_000 }, { "step": "configs", "state": "done", "ms": 300 }
    ])
}

pub fn landed_rows() -> Value {
    json!([
        { "id": "floor/curl", "label": "curl", "outcome": "installed", "step": "floor" },
        { "id": "agents/claude", "label": "Claude Code", "outcome": "installed", "step": "agents" },
        { "id": "agents/codex", "label": "Codex", "outcome": "installed", "step": "agents" },
        { "id": "signins/claude", "label": "Claude Code", "outcome": "installed", "step": "signins" },
        { "id": "agents/mcp/linear", "label": "linear", "outcome": "installed", "step": "mcp" },
        { "id": "clis/rg", "label": "rg", "outcome": "installed", "step": "clis" },
        { "id": "clis/gh", "label": "gh", "outcome": "installed", "step": "clis" },
        { "id": "skills/review", "label": "review", "outcome": "installed", "step": "skills" },
        { "id": "github", "label": "GitHub", "outcome": "installed", "step": "github", "note": "signed in on that computer" },
        { "id": "folders/wsp", "label": "wsp", "outcome": "installed", "step": "folders" },
        { "id": "configs/git", "label": "git", "outcome": "installed", "step": "configs" }
    ])
}

/// A dialog staged by `f`, over the page, with the box and this computer listed.
pub fn open(
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(&mut AddComputer, &mut Window, &mut Context<AddComputer>),
) -> Entity<AddComputer> {
    add_computer::staged(window, cx, f)
}

pub fn on_picks(this: &mut AddComputer, step: AddStep, cx: &mut Context<AddComputer>) {
    this.take_places(vec![here(), joined(None, json!([]))], vec![], vec![], cx);
    this.take_projects(projects(), cx);
    this.take_options(options(), cx);
    this.stage(step, BOX, Some(job("done", json!([]))), Some("p_box"), Some(picks()), cx);
}

fn running(this: &mut AddComputer, place: Value, step: AddStep, cx: &mut Context<AddComputer>) {
    this.take_places(vec![here(), place], vec![], vec![], cx);
    this.stage(step, BOX, None, Some("p_box"), None, cx);
}

pub fn states() -> Vec<State> {
    vec![
        State::new("W2-DLG-021", |w, cx| {
            open(w, cx, |this, _, cx| this.take_places(vec![here()], vec![], vec![], cx));
            page(w, cx)
        }),
        State::new("W2-DLG-022", |w, cx| {
            open(w, cx, |this, _, cx| this.take_hosts(Ok(hosts(30)), cx));
            page(w, cx)
        }),
        State::new("W2-DLG-023", |w, cx| {
            open(w, cx, |_, _, _| {});
            page(w, cx)
        })
        .size(600., 800.),
        State::new("W2-DLG-024", |w, cx| {
            open(w, cx, |this, _, cx| {
                this.stage(AddStep::Checks, BOX, Some(job("running", json!([{ "step": "connect", "state": "running" }]))), None, None, cx)
            });
            page(w, cx)
        }),
        State::new("W2-DLG-025", |w, cx| {
            open(w, cx, |this, _, cx| {
                let mut j = job("failed", json!([{ "step": "connect", "state": "failed" }]));
                j["address"] = json!("nobody@wsp-parity.invalid");
                j["said"] = json!("nobody@wsp-parity.invalid has never been reached from this computer and answered no key to a scan; read its host key on the computer itself and run wsp add nobody@wsp-parity.invalid --host-key '<type> <fingerprint>'");
                this.stage(AddStep::Checks, "nobody@wsp-parity.invalid", Some(shaped::<PlaceAddJob>(j)), None, None, cx);
            });
            page(w, cx)
        }),
        State::new("W2-DLG-026-hostkey", |w, cx| {
            open(w, cx, |this, _, cx| {
                this.take_places(vec![here()], vec![], vec![], cx);
                let mut j = job("failed", json!([{ "step": "connect", "state": "failed" }]));
                j["kind"] = json!("host-key");
                j["said"] = json!("the host key is new");
                j["hostKey"] = json!("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl");
                this.stage(AddStep::Checks, BOX, Some(shaped::<PlaceAddJob>(j)), None, None, cx);
            });
            page(w, cx)
        }),
        State::new("W2-DLG-026-sudo", |w, cx| {
            open(w, cx, |this, _, cx| {
                let mut j = job(
                    "failed",
                    json!([{ "step": "connect", "state": "done", "ms": 310 }, { "step": "chip", "state": "done", "ms": 120 }, { "step": "root", "state": "failed" }]),
                );
                j["kind"] = json!("sudo");
                j["said"] = json!("sudo on nobody@192.0.2.1 asks for a password.");
                this.stage(AddStep::Checks, BOX, Some(shaped::<PlaceAddJob>(j)), None, None, cx);
            });
            page(w, cx)
        }),
        State::new("W2-DLG-026-picks", |w, cx| {
            open(w, cx, |this, _, cx| on_picks(this, AddStep::Agents, cx));
            page(w, cx)
        }),
        State::new("W2-DLG-026-servers", |w, cx| {
            open(w, cx, |this, _, cx| {
                on_picks(this, AddStep::Mcp, cx);
                let mut p = picks();
                p["mcp"]["sentry"] = json!({ "agents": ["claude", "codex"] });
                this.stage(AddStep::Mcp, BOX, Some(job("done", json!([]))), Some("p_box"), Some(p), cx);
            });
            page(w, cx)
        }),
        State::new("W2-DLG-026-clis", |w, cx| {
            open(w, cx, |this, _, cx| on_picks(this, AddStep::Clis, cx));
            page(w, cx)
        }),
        State::new("W2-DLG-026-github", |w, cx| {
            open(w, cx, |this, _, cx| on_picks(this, AddStep::Github, cx));
            page(w, cx)
        }),
        State::new("W2-DLG-026-projects", |w, cx| {
            open(w, cx, |this, _, cx| on_picks(this, AddStep::Projects, cx));
            page(w, cx)
        }),
        State::new("W2-DLG-026-summary", |w, cx| {
            open(w, cx, |this, window, cx| {
                on_picks(this, AddStep::Summary, cx);
                this.take_estimate(shaped::<PlaceEstimate>(json!({ "neededBytes": 3_400_000_000u64, "keptBytes": 10_737_418_240u64, "freeBytes": 41_000_000_000u64, "unmeasured": 2 })), cx);
                this.stage_save("Lab boxes", window, cx);
            });
            page(w, cx)
        }),
        State::new("W2-DLG-026-diskshort", |w, cx| {
            open(w, cx, |this, _, cx| {
                on_picks(this, AddStep::Summary, cx);
                this.take_estimate(shaped::<PlaceEstimate>(json!({ "neededBytes": 3_400_000_000u64, "keptBytes": 10_737_418_240u64, "freeBytes": 9_000_000_000u64, "unmeasured": 0 })), cx);
            });
            page(w, cx)
        }),
        State::new("W2-DLG-026-running", |w, cx| {
            open(w, cx, |this, _, cx| {
                let s = setup(
                    "running",
                    json!([{ "step": "floor", "state": "done", "ms": 41_200 }, { "step": "agents", "state": "running" }]),
                    json!([]),
                );
                running(
                    this,
                    joined(Some(s), json!([{ "id": "floor/curl", "label": "curl", "outcome": "installed", "step": "floor" }])),
                    AddStep::Running,
                    cx,
                );
                this.open_row("floor", true, cx);
                this.take_log(
                    &[
                        "2026-10-18T10:00:01Z [floor] Reading package lists...".into(),
                        "2026-10-18T10:00:02Z [floor] Setting up curl (8.5.0-2ubuntu10.6) ...".into(),
                    ],
                    cx,
                );
            });
            page(w, cx)
        }),
        State::new("W2-DLG-026-signin", |w, cx| {
            open(w, cx, |this, _, cx| {
                let s = setup(
                    "running",
                    json!([{ "step": "floor", "state": "done", "ms": 41_200 }, { "step": "agents", "state": "done", "ms": 63_000 }, { "step": "signins", "state": "running" }]),
                    json!([{ "row": "signins/codex", "label": "Codex", "url": "https://auth.openai.com/codex/device", "code": "KXQ4-7PLM", "expiresAt": "2026-10-18T10:20:00Z", "state": "waiting" }]),
                );
                let rows = json!([{ "id": "agents/claude", "label": "Claude Code", "outcome": "installed", "step": "agents" }, { "id": "agents/codex", "label": "Codex", "outcome": "installed", "step": "agents" }]);
                running(this, joined(Some(s), rows), AddStep::Running, cx);
            });
            page(w, cx)
        }),
        State::new("W2-DLG-026-failed", |w, cx| {
            open(w, cx, |this, _, cx| {
                let mut s = setup("failed", json!([{ "step": "floor", "state": "failed", "ms": 8_100 }]), json!([]));
                s["said"] = json!("apt-get could not reach the package mirror (Temporary failure resolving archive.ubuntu.com)");
                running(this, joined(Some(s), json!([])), AddStep::Running, cx);
            });
            page(w, cx)
        }),
        State::new("W2-DLG-026-needsyou", |w, cx| {
            open(w, cx, |this, _, cx| {
                let mut rows = landed_rows();
                rows.as_array_mut().unwrap().retain(|r| r["id"] != "folders/wsp");
                rows.as_array_mut().unwrap().push(json!({ "id": "folders/wsp", "label": "wsp", "outcome": "failed", "step": "folders", "note": "git clone was refused: Permission denied (publickey)", "fix": "Sign GitHub in on lab-box, then press Retry." }));
                rows.as_array_mut().unwrap().push(
                    json!({ "id": "agents/mcp/sentry", "label": "sentry", "outcome": "skipped", "step": "mcp", "keys": ["SENTRY_TOKEN"] }),
                );
                running(this, joined(Some(setup("done", done_steps(), json!([]))), rows), AddStep::Running, cx);
            });
            page(w, cx)
        }),
        State::new("W2-DLG-026-ready", |w, cx| {
            open(w, cx, |this, _, cx| {
                running(this, joined(Some(setup("done", done_steps(), json!([]))), landed_rows()), AddStep::Ready, cx)
            });
            page(w, cx)
        }),
        State::new("W2-DLG-026-done", |w, cx| {
            open(w, cx, |this, _, cx| {
                running(this, joined(Some(setup("done", done_steps(), json!([]))), landed_rows()), AddStep::Running, cx)
            });
            page(w, cx)
        }),
        State::new("W2-DLG-026-startfrom", |w, cx| {
            open(w, cx, |this, _, cx| {
                let recipe = shaped::<RecipeView>(
                    json!({ "name": "Lab boxes", "slug": "lab-boxes", "summary": "2 agents, 2 CLIs, 1 skill", "machines": ["lab-1"], "savedAt": "2026-10-02T09:00:00Z", "file": picks() }),
                );
                this.take_recipes(vec![recipe], cx);
                on_picks(this, AddStep::StartFrom, cx);
            });
            page(w, cx)
        }),
    ]
}

/// The staged view the shots stand on, for a test that drives it.
pub fn page_view(window: &mut Window, cx: &mut App) -> AnyView {
    page(window, cx)
}
