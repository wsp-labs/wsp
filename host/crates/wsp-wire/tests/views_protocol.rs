// SPDX-License-Identifier: AGPL-3.0-only
//! Cases of packages/protocol/test/protocol.test.ts at a3b870a6e that read the views and the top-level shapes:
//! parses of the view types and the functions of views/ and the top-level files. What the wire's unions parse
//! (`EventUnion`, `RuntimeRequest`, `DaemonRequest`) is left to the wire writer.

use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use wsp_wire::types::harness_picks::{context_windows_for, efforts_for, marked_default, PickOption};
use wsp_wire::types::{parse, Check, JsNumber};
use wsp_wire::views::golden_image::{
    agent_of_row, golden_head, golden_image, package_of, recipe_pins, tool_row_id, tool_row_prefix, GoldenImage, GoldenManifest,
    GoldenStage, GoldenVersion, ImageSpec, Recipe, RecipeDigest, RecipeRow, RecipeSource, SealedImage, SealedVault, ToolPin,
    BREW_ID_PREFIX,
};
use wsp_wire::views::harness::{
    keeps_rename, mcp_servers_blocked, no_mcp_servers_line, takes_mcp_servers, HarnessCatalog, HarnessOption, McpServerSpec,
};
use wsp_wire::views::place::{DoctorLineEvent, PlaceView};
use wsp_wire::views::project_bundle::{ProjectExportEvent, ProjectExportResult, ProjectImportResult, ProjectPlan};
use wsp_wire::views::session::{SessionOrigin, SessionView, SubagentView, ThreadView};
use wsp_wire::views::session_events::{is_session_event, SessionEvent, SESSION_EVENT_TYPES};
use wsp_wire::views::snapshot_lineage::{ProjectGolden, SnapshotLineage, SnapshotRollbackResult};
use wsp_wire::views::workspace::{DaemonReachView, PortReachView, WorkspaceListing, WorkspaceOut, WorkspaceStatus, WorkspaceView};

/// A float that holds a whole number is that integer: JavaScript has one kind of number, and the TS compares by value.
fn whole(v: Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() && f.fract() == 0.0 && f.abs() < 9e15 => Value::from(f as i64),
            _ => Value::Number(n),
        },
        Value::Array(a) => Value::Array(a.into_iter().map(whole).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, whole(v))).collect()),
        other => other,
    }
}

/// What a value writes as, read back from its JSON text.
fn back<T: Serialize>(x: &T) -> Value {
    whole(serde_json::from_str(&serde_json::to_string(x).expect("serializes")).expect("is JSON"))
}

/// zod's `X.parse(v)` written back, for the cases the TS compares with `toEqual`.
fn parsed<T: DeserializeOwned + Serialize + Check>(v: &Value) -> Value {
    back(&parse::<T>(v).unwrap_or_else(|e| panic!("refused {v}: {e:?}")))
}

fn refused<T: DeserializeOwned + Check>(v: &Value) -> bool {
    parse::<T>(v).is_err()
}

/// `{ ...base, [key]: value }`.
fn with(base: &Value, key: &str, value: Value) -> Value {
    let mut out = base.clone();
    out.as_object_mut().expect("an object").insert(key.to_string(), value);
    out
}

/// `{ ...base, [key]: undefined }` as the parse sees it, and the rest-spread that leaves a key out.
fn without(base: &Value, key: &str) -> Value {
    let mut out = base.clone();
    out.as_object_mut().expect("an object").remove(key);
    out
}

fn texts<O: PickOption>(options: Vec<O>) -> Vec<String> {
    options.iter().map(|o| o.value().to_string()).collect()
}

mod a_copy_of_the_image_beside_the_record {
    use super::*;

    fn image() -> Value {
        json!({ "name": "default", "version": 2, "hash": "a".repeat(64), "recipeHash": "rh", "logins": [], "sealedAt": "t", "sealedFrom": "h1" })
    }

    // packages/protocol/test/protocol.test.ts:121 at a3b870a6e
    #[test]
    fn keeps_the_pins_beside_the_recipe_one_per_tick_that_recorded_one_in_id_order_with() {
        // The TS digest here has no `files`, which the Rust RecipeDigest requires: an empty list is added.
        let digest: RecipeDigest = serde_json::from_value(json!({
            "ticks": [
                { "id": "tools/npm/wrangler", "version": "4.1.0", "road": "npm", "installer": "i", "pin": { "tag": "4.1.0" } },
                { "id": "agents/codex" },
                { "id": "tools/catalog/gh", "road": "release", "installer": "k", "pin": { "tag": "v2.86.0", "sha256": "b".repeat(64) } },
                { "id": "tools/catalog/tmux", "road": "apt", "installer": "j", "pin": { "tag": "3.3a-3", "latest": true } },
            ],
            "files": [],
        }))
        .expect("a digest");
        let pins = back(&recipe_pins(&digest, None));
        assert_eq!(
            pins,
            json!([
                { "id": "tools/catalog/gh", "tag": "v2.86.0", "sha256": "b".repeat(64), "road": "release" },
                { "id": "tools/catalog/tmux", "tag": "3.3a-3", "latest": true, "road": "apt" },
                { "id": "tools/npm/wrangler", "tag": "4.1.0", "road": "npm" },
            ])
        );
        assert_eq!(parsed::<SealedImage>(&with(&image(), "pins", pins.clone()))["pins"], pins);
        assert!(parsed::<SealedImage>(&image()).get("pins").is_none());
        // A caller's key names the row across computers; the pins sort by that key, so two rows for one tool cannot hide behind their order.
        let keyed_digest: RecipeDigest = serde_json::from_value(json!({
            "ticks": [
                { "id": "tools/npm/wrangler", "pin": { "tag": "4.1.0" } },
                { "id": "tools/catalog/gh", "pin": { "tag": "v2.86.0" } },
            ],
            "files": [],
        }))
        .expect("a digest");
        let last_word = |id: &str| id[id.rfind('/').map_or(0, |i| i + 1)..].to_string();
        let keyed = recipe_pins(&keyed_digest, Some(&last_word));
        assert_eq!(keyed.into_iter().map(|p| p.id).collect::<Vec<_>>(), vec!["gh", "wrangler"]);
        // The one rule for the agents rung: the agent an agents row names, nothing for an MCP server's row or another rung.
        assert_eq!(agent_of_row("agents/codex"), Some("codex".to_string()));
        assert_eq!(agent_of_row("agents/mcp/claude/wsp"), None);
        assert_eq!(agent_of_row("tools/npm/codex"), None);
        // The one pin shape: a version, a sum where a road hashed one, the latest mark; a sum alone or a bare object is none.
        assert_eq!(parsed::<ToolPin>(&json!({ "tag": "4.1.0" })), json!({ "tag": "4.1.0" }));
        assert!(refused::<ToolPin>(&json!({ "sha256": "b".repeat(64) })));
        assert!(refused::<ToolPin>(&json!({ "tag": "4.1.0", "latest": false })));
    }
}

mod what_a_computer_says_about_its_agents_and_what_a_person_reads_off_it {
    use super::*;

    // packages/protocol/test/protocol.test.ts:231 at a3b870a6e
    #[test]
    fn a_computer_s_row_carries_those_versions_and_the_one_word_the_host_worked_out_per() {
        let row = json!({
            "id": "p_1",
            "kind": "computer",
            "name": "spoo",
            "default": false,
            "agents": ["claude", "codex"],
            "agentVersions": { "claude": "2.1.270 (Claude Code)" },
            "signIns": { "claude": "vault-key", "codex": "none" },
        });
        assert_eq!(parsed::<PlaceView>(&row), row);
        assert!(refused::<PlaceView>(&with(&row, "signIns", json!({ "codex": "maybe" }))));
    }
}

mod the_recipe_s_pins {
    use super::*;

    // packages/protocol/test/protocol.test.ts:274 at a3b870a6e
    #[test]
    fn a_recipe_row_and_a_digest_tick_carry_one_pin_shape_the_version_with_the_sum_wher() {
        let pin = json!({ "tag": "v2.86.0", "sha256": "b".repeat(64) });
        assert_eq!(parsed::<ToolPin>(&pin), pin);
        assert_eq!(parsed::<ToolPin>(&json!({ "tag": "4.1.0" })), json!({ "tag": "4.1.0" }));
        assert!(refused::<ToolPin>(&json!({ "sha256": "b".repeat(64) })));
        let row =
            json!({ "id": "gh", "kind": "tool", "on": true, "source": { "kind": "popular", "sessions": 1, "images": 1 }, "pin": pin });
        assert_eq!(parsed::<RecipeRow>(&row), row);
        let tick = json!({ "id": "tools/catalog/gh", "road": "release", "installer": "a".repeat(64), "pin": pin });
        let digest = json!({ "ticks": [tick, { "id": "agents/claude" }], "files": [] });
        assert_eq!(parsed::<RecipeDigest>(&digest), digest);
    }
}

mod protocol_views {
    use super::*;

    fn workspace() -> Value {
        json!({
            "id": "ws_1",
            "name": "task-1",
            "machineId": "m1",
            "phase": "running",
            "golden": "snap_g",
            "createdAt": "2026-09-01T00:00:00.000Z",
            "project": { "id": "pr_1a2b3c4d", "name": "task-1", "path": "/root/task-1", "computer": "here" },
        })
    }

    // packages/protocol/test/protocol.test.ts:287 at a3b870a6e
    #[test]
    fn parses_a_workspaceview_and_rejects_a_bad_phase() {
        let ws = workspace();
        assert_eq!(parsed::<WorkspaceView>(&ws), ws);
        assert!(refused::<WorkspaceView>(&with(&ws, "phase", json!("hibernating"))));
    }

    // packages/protocol/test/protocol.test.ts:301 at a3b870a6e
    #[test]
    fn parses_a_sessionview() {
        let s = json!({ "id": "0b6a9c1e-0000-4000-8000-000000000000", "workspaceId": "ws_1", "harness": "claude", "status": "running" });
        assert_eq!(parsed::<SessionView>(&s), s);
        assert!(refused::<SessionView>(&with(&s, "status", json!("done"))));
    }

    // packages/protocol/test/protocol.test.ts:312 at a3b870a6e
    #[test]
    fn workspaceview_and_workspacestatus_carry_the_desktop_stream_as_screen_streamurl_a() {
        let view = with(&workspace(), "screen", json!({ "streamUrl": "wss://stream.example/m1" }));
        assert_eq!(parsed::<WorkspaceView>(&view), view);
        let text = serde_json::to_string(&view).unwrap();
        assert_eq!(parsed::<WorkspaceView>(&serde_json::from_str(&text).unwrap()), view);
        let headless = without(&view, "screen");
        assert_eq!(parsed::<WorkspaceView>(&headless), headless);
        assert!(refused::<WorkspaceView>(&with(&view, "screen", json!({}))));

        let mut status = view.clone();
        for (key, value) in [
            ("machineState", json!("running")),
            ("reach", json!({ "state": "unsupported" })),
            ("size", json!({ "cpu": 2, "memMb": 4096 })),
            ("rateUsdPerHour", json!(0.11)),
        ] {
            status = with(&status, key, value);
        }
        assert_eq!(parsed::<WorkspaceStatus>(&status), status);
    }

    // packages/protocol/test/protocol.test.ts:334 at a3b870a6e
    #[test]
    fn the_listing_every_command_line_and_tool_reads_carries_what_the_machine_said_it_i() {
        let facts = json!({ "os": "Ubuntu 24.04.3 LTS", "uptimeMs": 90_061_000, "folder": "/home/dev" });
        let status = json!({
            "id": "ws_1",
            "name": "box",
            "machineId": "m_box",
            "phase": "running",
            "golden": "",
            "createdAt": "2026-09-11T00:00:00.000Z",
            "project": { "id": "pr_1a2b3c4d", "name": "box", "path": "/home/dev/box", "computer": "pl_box" },
            "kind": "cloud",
            "machineState": "running",
            "reach": { "state": "reachable", "url": "http://127.0.0.1:40000", "expiresAt": 1789041249000u64 },
            "size": { "cpu": 8, "memMb": 16384 },
            "rateUsdPerHour": 0,
            "facts": facts,
        });
        let listed = parsed::<WorkspaceListing>(&status);
        // The three the Machine pane draws, so a person at a terminal or an agent through the tools reads the same ones.
        assert_eq!(listed["facts"], facts);
        // What the pick is for: the reach's state without the route the app dials, and nothing minted riding out.
        assert_eq!(listed["reach"], json!({ "state": "reachable" }));
        // A machine that has not said what it is leaves the field off rather than carrying an empty one.
        let quiet = without(&status, "facts");
        assert!(parsed::<WorkspaceListing>(&quiet).get("facts").is_none());
    }
}

mod a_workspace_s_look {
    use super::*;

    fn view() -> Value {
        json!({
            "id": "ws_1",
            "name": "task-1",
            "machineId": "m1",
            "phase": "running",
            "golden": "snap_g",
            "createdAt": "2026-09-01T00:00:00.000Z",
            "project": { "id": "pr_1a2b3c4d", "name": "task-1", "path": "/root/task-1", "computer": "here" },
        })
    }

    fn theme() -> Value {
        json!({ "dots": [{ "angle": 200, "radius": 0.5 }, { "angle": 20, "radius": 0.5 }], "harmony": "complementary", "grain": 0.25, "opacity": 0.6, "mode": "auto" })
    }

    // packages/protocol/test/protocol.test.ts:374 at a3b870a6e
    #[test]
    fn the_view_carries_the_theme_and_the_glyph_absent_is_none_and_a_theme_outside_the() {
        let view = view();
        let theme = theme();
        assert_eq!(parsed::<WorkspaceView>(&view), view);
        let looked = with(&with(&view, "theme", theme.clone()), "glyph", json!("flask"));
        assert_eq!(parsed::<WorkspaceView>(&looked), looked);
        let themed = |over: Value| with(&view, "theme", over);
        assert!(refused::<WorkspaceView>(&themed(with(&theme, "dots", json!([])))));
        let dots = theme["dots"].as_array().unwrap().clone();
        let doubled: Vec<Value> = dots.iter().chain(dots.iter()).cloned().collect();
        assert!(refused::<WorkspaceView>(&themed(with(&theme, "dots", Value::Array(doubled)))));
        assert!(refused::<WorkspaceView>(&themed(with(&theme, "opacity", json!(0)))));
        assert!(refused::<WorkspaceView>(&themed(with(&theme, "grain", json!(2)))));
        assert!(refused::<WorkspaceView>(&themed(with(&theme, "mode", json!("sepia")))));
        assert!(!refused::<WorkspaceView>(&with(&view, "tint", json!("cyan"))));
        assert_eq!(parsed::<WorkspaceView>(&with(&view, "tint", json!("cyan"))), view);
        assert!(refused::<WorkspaceView>(&with(&view, "glyph", json!("🚀"))));
    }
}

mod protocol_event_union {
    use super::*;

    // packages/protocol/test/protocol.test.ts:462 at a3b870a6e
    #[test]
    fn session_steer_is_a_session_event_the_message_the_person_sent_into_a_running_turn() {
        let steered = json!({ "type": "session.steer", "workspaceId": "ws_1", "sessionId": "s1", "turnId": "turn_0001", "threadId": "thread_0001", "at": 1756687889412u64, "prompt": "and say pineapple", "requestId": "req_7" });
        assert_eq!(parsed::<SessionEvent>(&steered), steered);
        // left to the wire writer: EventUnion.parse of the steer after a JSON round trip, and of the steer with seq 9
        let plain = without(&steered, "requestId");
        assert_eq!(parsed::<SessionEvent>(&plain), plain);
        assert!(refused::<SessionEvent>(&without(&steered, "prompt")));
        assert!(refused::<SessionEvent>(&with(&steered, "prompt", json!(7))));
    }
}

// a_create_s_stage_lines_on_the_wire: packages/protocol/test/protocol.test.ts:484 at a3b870a6e is not written. The case is
// creationAwaits alone (hostname-set is a note, every other stage is a wait), and the Rust interface has no
// creation_awaits: views/workspace_events.rs holds no function.

mod a_relayed_permission_prompt_on_the_wire {
    use super::*;

    fn ask() -> Value {
        json!({
            "type": "session.permission",
            "workspaceId": "ws_1",
            "sessionId": "s1",
            "turnId": "turn_0001",
            "threadId": "thread_0001",
            "at": 1757320000000u64,
            "askId": "d9aa99d3-be4e-4a2b-8766-1b9494cde4f6",
            "toolName": "Write",
            "toolUseId": "toolu_1",
            "input": r#"{"file_path":"/root/out.txt","content":"hi"}"#,
            "detail": "out.txt",
            "options": [
                { "id": "allow", "label": "Allow", "effect": "allow" },
                { "id": "deny", "label": "Deny", "effect": "deny" },
                { "id": "mode:acceptEdits", "label": "Allow, then Accept edits", "effect": "mode", "mode": "acceptEdits" },
            ],
        })
    }

    fn closed() -> Value {
        json!({
            "type": "session.permission.closed",
            "workspaceId": "ws_1",
            "sessionId": "s1",
            "turnId": "turn_0001",
            "threadId": "thread_0001",
            "at": 1757320005000u64,
            "askId": ask()["askId"],
            "outcome": "allowed",
            "optionId": "allow",
        })
    }

    // packages/protocol/test/protocol.test.ts:513 at a3b870a6e
    #[test]
    fn both_halves_are_session_events_so_a_transcript_replays_a_prompt_and_how_it_close() {
        for e in [ask(), closed()] {
            assert_eq!(parsed::<SessionEvent>(&e), e);
            // left to the wire writer: EventUnion.parse of each half after a JSON round trip, and with seq 12
        }
        // Every session type is read off the union, so an added event is never missed by a client's own list.
        assert!(SESSION_EVENT_TYPES.contains(&"session.permission"));
        assert!(SESSION_EVENT_TYPES.contains(&"session.permission.closed"));
        assert!(!SESSION_EVENT_TYPES.contains(&"session.queued"));
        // The one predicate every reader that folds a thread's events out of the whole channel makes.
        assert!(is_session_event(ask()["type"].as_str().unwrap()));
        assert!(!is_session_event("workspace.napped"));
    }

    // packages/protocol/test/protocol.test.ts:528 at a3b870a6e
    #[test]
    fn a_prompt_with_nothing_the_harness_did_not_name_still_parses_and_one_missing_what() {
        let ask = ask();
        let bare = without(&without(&ask, "detail"), "toolUseId");
        assert_eq!(parsed::<SessionEvent>(&bare), bare);
        for key in ["askId", "toolName", "input", "options"] {
            assert!(refused::<SessionEvent>(&without(&ask, key)), "{key} is required");
        }
        assert!(refused::<SessionEvent>(&with(&ask, "options", json!([{ "id": "allow", "label": "Allow", "effect": "maybe" }]))));
    }

    // packages/protocol/test/protocol.test.ts:537 at a3b870a6e
    #[test]
    fn a_close_names_its_outcome_from_the_four_and_the_option_only_where_one_closed_it() {
        let closed = closed();
        for outcome in ["allowed", "denied", "unanswered", "cancelled"] {
            assert_eq!(parsed::<SessionEvent>(&with(&closed, "outcome", json!(outcome)))["outcome"], json!(outcome));
        }
        assert!(refused::<SessionEvent>(&with(&closed, "outcome", json!("timedout"))));
        let no_option = without(&closed, "optionId");
        assert_eq!(parsed::<SessionEvent>(&no_option), no_option);
        assert!(refused::<SessionEvent>(&without(&closed, "askId")));
    }
}

mod session_wire_fields_the_face_reads {
    use super::*;

    // packages/protocol/test/protocol.test.ts:583 at a3b870a6e
    #[test]
    fn every_session_event_may_carry_at_ms_epoch_and_turnid_both_survive_a_json_round_t() {
        let scope = |extra: Value| {
            let mut e = json!({ "workspaceId": "ws_1", "sessionId": "s1", "at": 1756687889412u64, "turnId": "turn_0001" });
            for (k, v) in extra.as_object().unwrap() {
                e = with(&e, k, v.clone());
            }
            e
        };
        let events = [
            scope(json!({ "type": "session.start", "prompt": "hello" })),
            scope(json!({ "type": "session.delta", "kind": "text", "text": "hi" })),
            scope(json!({ "type": "session.done", "result": { "status": "completed" } })),
            scope(json!({ "type": "session.end", "exitCode": 0, "sawResult": true })),
        ];
        for e in &events {
            assert_eq!(&parsed::<SessionEvent>(e), e);
            // left to the wire writer: EventUnion.parse of each event after a JSON round trip
        }
        assert!(refused::<SessionEvent>(&with(&events[0], "at", json!("2026-09-01T00:00:00Z"))));
        assert!(refused::<SessionEvent>(&with(&events[0], "turnId", json!(7))));
    }

    // packages/protocol/test/protocol.test.ts:599 at a3b870a6e
    #[test]
    fn every_session_event_may_carry_a_threadid_a_transcript_without_one_still_parses() {
        let scope = |extra: Value| {
            let mut e = json!({ "workspaceId": "ws_1", "sessionId": "s1", "turnId": "turn_0001", "threadId": "thread_0001" });
            for (k, v) in extra.as_object().unwrap() {
                e = with(&e, k, v.clone());
            }
            e
        };
        let events = [
            scope(json!({ "type": "session.start", "prompt": "hello" })),
            scope(json!({ "type": "session.delta", "kind": "text", "text": "hi" })),
            scope(json!({ "type": "session.done", "result": { "status": "completed" } })),
            scope(json!({ "type": "session.end", "exitCode": 0, "sawResult": true })),
        ];
        for e in &events {
            assert_eq!(&parsed::<SessionEvent>(e), e);
            // left to the wire writer: EventUnion.parse of each event after a JSON round trip
            let before = without(e, "threadId");
            assert_eq!(parsed::<SessionEvent>(&before), before);
        }
        assert!(refused::<SessionEvent>(&with(&events[0], "threadId", json!(7))));
    }

    // packages/protocol/test/protocol.test.ts:616 at a3b870a6e
    #[test]
    fn session_start_carries_the_harness_catalog_from_system_init() {
        let started = json!({
            "type": "session.start",
            "workspaceId": "ws_1",
            "sessionId": "s1",
            "harness": { "slashCommands": ["compact", "review"], "permissionMode": "bypassPermissions", "agents": ["general-purpose"] },
        });
        assert_eq!(parsed::<SessionEvent>(&started), started);
        let partial = with(&started, "harness", json!({ "permissionMode": "default" }));
        assert_eq!(parsed::<SessionEvent>(&partial), partial);
        assert!(refused::<SessionEvent>(&with(&started, "harness", json!({ "slashCommands": "compact" }))));
    }

    // packages/protocol/test/protocol.test.ts:629 at a3b870a6e
    #[test]
    fn session_start_may_say_the_thread_s_previous_turn_was_cut_and_only_as_true_the_fa() {
        let started = json!({ "type": "session.start", "workspaceId": "ws_1", "sessionId": "s1", "afterCut": true });
        assert_eq!(parsed::<SessionEvent>(&started), started);
        assert!(refused::<SessionEvent>(&with(&started, "afterCut", json!(false))));
    }

    // packages/protocol/test/protocol.test.ts:635 at a3b870a6e
    #[test]
    fn sessionview_carries_prompt_startedat_and_endedat_so_the_sidebar_can_title_and_so() {
        let running = json!({
            "id": "0b6a9c1e-0000-4000-8000-000000000000",
            "workspaceId": "ws_1",
            "harness": "claude",
            "status": "running",
            "prompt": "fix the flaky test",
            "startedAt": 1756687889412u64,
        });
        assert_eq!(parsed::<SessionView>(&running), running);
        let ended = with(&with(&running, "status", json!("completed")), "endedAt", json!(1756687899870u64));
        assert_eq!(parsed::<SessionView>(&ended), ended);
        assert!(refused::<SessionView>(&with(&running, "startedAt", json!("soon"))));
    }
}

mod runtime_wire_types {
    use super::*;

    fn catalog() -> Value {
        json!({
            "harness": "claude",
            "label": "Claude Code",
            "source": "harness",
            "version": "2.1.257",
            "models": [
                { "value": "claude-opus-5", "label": "Opus 5", "isDefault": true, "contextWindows": ["200k", "1m"] },
                { "value": "claude-haiku-4-5", "label": "Haiku", "efforts": [], "contextWindows": [] },
            ],
            "efforts": [{ "value": "high", "label": "High", "isDefault": true }],
            "contextWindows": [{ "value": "200k", "label": "200k" }, { "value": "1m", "label": "1M", "isDefault": true }],
            "permissionModes": [{ "value": "acceptEdits", "label": "Accept edits", "description": "Edits files without asking" }],
            "steers": true,
            "renames": true,
            "images": true,
        })
    }

    // packages/protocol/test/protocol.test.ts:1011 at a3b870a6e
    #[test]
    fn a_harness_catalog_lists_what_each_picker_may_offer_says_where_the_lists_came_fro() {
        let catalog = catalog();
        assert_eq!(parsed::<HarnessCatalog>(&catalog), catalog);
        assert_eq!(parsed::<HarnessCatalog>(&with(&catalog, "isDefault", json!(true))), with(&catalog, "isDefault", json!(true)));
        let bare = json!({ "harness": "pi", "label": "Pi", "source": "table", "version": null, "models": [], "efforts": [], "contextWindows": [], "permissionModes": [], "steers": false, "renames": false, "images": false });
        assert_eq!(parsed::<HarnessCatalog>(&bare), bare);
        // steers says whether a running turn of this harness takes a message; the composer decides send-now from it before the click
        // screenCommands names the CLI's commands that work only in its own terminal, each with the wsp control that serves
        // it; the composer lists none of them and sends nothing for one. Absent is none, a catalog from before the field.
        let screen =
            with(&catalog, "screenCommands", json!([{ "name": "login", "control": "sign-in" }, { "name": "model", "control": "model" }]));
        assert_eq!(parsed::<HarnessCatalog>(&screen), screen);
        assert!(refused::<HarnessCatalog>(&with(&catalog, "screenCommands", json!(["login"]))));
        assert!(refused::<HarnessCatalog>(&with(&catalog, "screenCommands", json!([{ "name": "login", "control": "terminal" }]))));
        assert!(refused::<HarnessCatalog>(&without(&catalog, "steers")));
        assert!(refused::<HarnessCatalog>(&with(&catalog, "steers", json!("yes"))));
        // renames says whether a name of a person's survives in the harness's own store; a client offers the rename from it
        assert!(refused::<HarnessCatalog>(&without(&catalog, "renames")));
        assert!(refused::<HarnessCatalog>(&with(&catalog, "renames", json!("yes"))));
        // images says whether a message to this harness may carry one; the composer offers its picker from it
        assert!(refused::<HarnessCatalog>(&without(&catalog, "images")));
        assert!(refused::<HarnessCatalog>(&with(&catalog, "images", json!("yes"))));
        assert!(refused::<HarnessCatalog>(&with(&catalog, "models", json!([{ "value": "x" }]))));
        assert!(refused::<HarnessCatalog>(&without(&catalog, "efforts")));
        assert!(refused::<HarnessCatalog>(&with(&catalog, "source", json!("guess"))));
        let old = without(&without(&without(&catalog, "source"), "version"), "contextWindows");
        assert!(refused::<HarnessCatalog>(&old));
    }

    fn row(over: Value) -> HarnessCatalog {
        let mut base = json!({
            "harness": "claude",
            "label": "Claude Code",
            "source": "harness",
            "version": "2.1.263",
            "models": [],
            "efforts": [],
            "contextWindows": [],
            "permissionModes": [],
            "steers": true,
            "renames": true,
            "images": true,
        });
        for (k, v) in over.as_object().unwrap() {
            base = with(&base, k, v.clone());
        }
        serde_json::from_value(base).expect("a catalog")
    }

    // packages/protocol/test/protocol.test.ts:1051 at a3b870a6e
    #[test]
    fn whether_a_rename_is_kept_is_the_adapter_s_answer_a_row_the_runtime_s_table_stood() {
        assert!(keeps_rename(Some(&row(json!({})))));
        // The machine answered no: no client offers the rename.
        assert!(!keeps_rename(Some(&row(json!({ "renames": false })))));
        // The table stood in, so nobody has asked the machine yet: the client offers it and the runtime answers.
        assert!(keeps_rename(Some(&row(json!({ "source": "table", "renames": false })))));
        // null and undefined are both None here.
        assert!(keeps_rename(None));
        // Whether a launch may carry MCP servers is not the machine's answer but this host's adapter, so a table row is
        // the answer and absent is a no: a client offering an agent whose launch drops them is the fault, not the fix.
        assert!(takes_mcp_servers(Some(&row(json!({ "mcpServers": true })))));
        assert!(takes_mcp_servers(Some(&row(json!({ "source": "table", "mcpServers": true })))));
        assert!(!takes_mcp_servers(Some(&row(json!({})))));
        assert!(!takes_mcp_servers(Some(&row(json!({ "mcpServers": false })))));
        assert!(!takes_mcp_servers(None));
    }

    // packages/protocol/test/protocol.test.ts:1083 at a3b870a6e
    #[test]
    fn servers_named_for_an_agent_whose_adapter_renders_none_are_refused_in_that_agent() {
        let mut wsp: IndexMap<String, McpServerSpec> = IndexMap::new();
        wsp.insert(
            "wsp".to_string(),
            McpServerSpec {
                command: "/usr/local/bin/node".to_string(),
                args: vec!["/opt/wsp/bin.js".to_string(), "mcp".to_string()],
                no_slate: None,
            },
        );
        assert_eq!(mcp_servers_blocked(Some(&wsp), false, "Codex"), Some(no_mcp_servers_line("Codex")));
        assert_eq!(mcp_servers_blocked(Some(&wsp), true, "Claude Code"), None);
        assert_eq!(mcp_servers_blocked(None, false, "Codex"), None);
        assert_eq!(mcp_servers_blocked(Some(&IndexMap::new()), false, "Codex"), None);
        assert!(no_mcp_servers_line("Codex").contains("Codex"));
    }

    // packages/protocol/test/protocol.test.ts:1092 at a3b870a6e
    #[test]
    fn a_model_without_its_own_lists_takes_the_catalog_s_a_model_with_lists_narrows_the() {
        let catalog: HarnessCatalog = serde_json::from_value(json!({
            "harness": "claude",
            "label": "Claude Code",
            "source": "harness",
            "version": "2.1.257",
            "models": [
                { "value": "claude-opus-5", "label": "Opus 5", "isDefault": true, "contextWindows": ["200k", "1m"] },
                { "value": "claude-sonnet-5", "label": "Sonnet 5", "contextWindows": [] },
                { "value": "claude-haiku-4-5", "label": "Haiku", "efforts": [], "contextWindows": [] },
                { "value": "claude-next", "label": "Next", "efforts": ["high"] },
            ],
            "efforts": [{ "value": "low", "label": "Low" }, { "value": "high", "label": "High" }],
            "contextWindows": [{ "value": "200k", "label": "200k" }, { "value": "1m", "label": "1M", "isDefault": true }],
            "permissionModes": [{ "value": "acceptEdits", "label": "Accept edits" }],
            "steers": true,
            "renames": true,
            "images": true,
        }))
        .expect("a catalog");
        let m = &catalog.models;
        assert_eq!(texts(efforts_for(&catalog, Some(&m[0]))), vec!["low", "high"]);
        assert_eq!(texts(efforts_for(&catalog, Some(&m[3]))), vec!["high"]);
        assert_eq!(efforts_for(&catalog, Some(&m[2])), Vec::<HarnessOption>::new());
        assert_eq!(efforts_for(&catalog, None), catalog.efforts);
        assert_eq!(texts(context_windows_for(&catalog, Some(&m[0]))), vec!["200k", "1m"]);
        assert_eq!(context_windows_for(&catalog, Some(&m[1])), Vec::<HarnessOption>::new());
        assert_eq!(texts(context_windows_for(&catalog, Some(&m[3]))), vec!["200k", "1m"]);
        assert_eq!(context_windows_for(&catalog, None), Vec::<HarnessOption>::new());
        assert_eq!(marked_default(&catalog.models).map(|o| o.value.as_str()), Some("claude-opus-5"));
        assert_eq!(marked_default(&catalog.context_windows).map(|o| o.value.as_str()), Some("1m"));
        assert!(marked_default(&catalog.permission_modes).is_none());
        assert!(marked_default::<HarnessOption>(&[]).is_none());
    }

    // packages/protocol/test/protocol.test.ts:1125 at a3b870a6e
    #[test]
    fn sessionview_records_the_model_effort_permission_mode_and_context_window_the_sess() {
        let view = json!({ "id": "s1", "workspaceId": "ws_1", "harness": "claude", "status": "running", "model": "claude-opus-5", "effort": "high", "permissionMode": "bypassPermissions", "contextWindow": "1m" });
        assert_eq!(parsed::<SessionView>(&view), view);
        assert!(refused::<SessionView>(&with(&view, "model", json!(5))));
    }

    // packages/protocol/test/protocol.test.ts:1159 at a3b870a6e
    #[test]
    fn snapshots_list_snapshots_rollback_parse_and_snapshotlineage_is_the_manifest_plus() {
        // left to the wire writer: RuntimeRequest.parse of snapshots.list, snapshots.list with a name and snapshots.rollback with a version, and the refusal of snapshots.rollback with no version
        let version = json!({
            "version": 11,
            "snapshotId": "snap_golden-v11",
            "baseTemplate": "base",
            "setupSha": "abc",
            "createdAt": "2026-08-14T00:00:00.000Z",
            "smoke": { "cmd": "claude --version", "exitCode": 0 },
        });
        let lineage = json!({ "name": "default", "head": 11, "versions": [version] });
        assert_eq!(parsed::<SnapshotLineage>(&lineage), lineage);
        let empty = json!({ "name": "default", "head": null, "versions": [] });
        assert_eq!(parsed::<SnapshotLineage>(&empty), empty);
        let rolled = json!({ "lineage": lineage, "existingWorkspaces": "untouched" });
        assert_eq!(parsed::<SnapshotRollbackResult>(&rolled), rolled);
        assert!(refused::<SnapshotRollbackResult>(&json!({ "lineage": lineage, "existingWorkspaces": "upgraded" })));
    }

    // packages/protocol/test/protocol.test.ts:1181 at a3b870a6e
    #[test]
    fn workspaces_portreach_names_the_workspace_and_one_guest_port_portreachview_is_the() {
        // left to the wire writer: RuntimeRequest.parse of workspaces.portReach, its refusal with no port, and its refusal of ports 0, 65536 and 30.5
        let reach = json!({ "url": "https://m-3000.preview.example/?pt_token=edge", "expiresAt": 1_700_000_000_000u64 });
        assert_eq!(parsed::<PortReachView>(&reach), reach);
        assert_eq!(parsed::<PortReachView>(&with(&reach, "daemonToken", json!("d"))), reach);
        assert!(refused::<PortReachView>(&json!({ "url": "x" })));
    }

    // packages/protocol/test/protocol.test.ts:1192 at a3b870a6e
    #[test]
    fn daemonreachview_carries_the_preview_route_its_expiry_and_the_daemon_token_when_t() {
        let full = json!({ "url": "https://m-7070.preview.example/?pt_token=edge", "expiresAt": 1_700_000_000_000u64, "daemonToken": "d" });
        assert_eq!(parsed::<DaemonReachView>(&full), full);
        let bare = json!({ "url": "https://m-7070.preview.example/?pt_token=edge", "expiresAt": 1 });
        assert_eq!(parsed::<DaemonReachView>(&bare), bare);
        assert!(refused::<DaemonReachView>(&json!({ "url": "x" })));
    }
}

mod golden_wire_schemas {
    use super::*;

    // packages/protocol/test/protocol.test.ts:1219 at a3b870a6e
    #[test]
    fn a_manifest_sealed_before_kind_was_recorded_still_parses_new_ones_carry_the_kind() {
        let old = json!({ "version": 1, "snapshotId": "snap_a", "baseTemplate": "base", "setupSha": "x", "createdAt": "2026-09-01T00:00:00Z", "smoke": { "cmd": "true", "exitCode": 0 } });
        let manifest = json!({ "head": 2, "versions": [old, with(&with(&old, "version", json!(2)), "kind", json!("desktop"))] });
        let parsed = parse::<GoldenManifest>(&manifest).expect("a manifest");
        assert!(parsed.versions[0].kind.is_none());
        assert_eq!(back(&parsed.versions[1].kind), json!("desktop"));
    }
}

mod golden_version_logins {
    use super::*;

    fn base() -> Value {
        json!({ "version": 1, "snapshotId": "snap_1", "baseTemplate": "base", "setupSha": "x", "createdAt": "t", "smoke": { "cmd": "true", "exitCode": 0 } })
    }

    // packages/protocol/test/protocol.test.ts:1395 at a3b870a6e
    #[test]
    fn carries_name_and_state_per_login_when_the_seal_was_given_them_and_stays_optional() {
        let base = base();
        let logins = json!([{ "name": "GitHub CLI login", "state": "signed-in" }, { "name": "Codex login", "state": "skipped" }]);
        let sealed = with(&base, "logins", logins);
        assert_eq!(parsed::<GoldenVersion>(&sealed), sealed);
        assert!(parse::<GoldenVersion>(&base).expect("a version").logins.is_none());
        assert!(refused::<GoldenVersion>(&with(&base, "logins", json!([{ "name": "x", "state": "done" }]))));
    }

    // packages/protocol/test/protocol.test.ts:1402 at a3b870a6e
    #[test]
    fn carries_the_tools_missing_from_the_image_with_the_cause_and_reason_and_stays_opt() {
        let base = base();
        let missing = json!([{ "id": "tools/brew/gopls", "name": "gopls", "outcome": "skipped", "note": "no Linux bottle" }, { "id": "tools/homebrew", "name": "Homebrew", "outcome": "failed", "note": "git: not found" }]);
        let sealed = with(&base, "missingTools", missing);
        assert_eq!(parsed::<GoldenVersion>(&sealed), sealed);
        assert!(parse::<GoldenVersion>(&base).expect("a version").missing_tools.is_none());
        assert!(refused::<GoldenVersion>(&with(
            &base,
            "missingTools",
            json!([{ "id": "tools/brew/gopls", "name": "gopls", "note": "no Linux bottle" }])
        )));
        assert!(refused::<GoldenVersion>(&with(
            &base,
            "missingTools",
            json!([{ "name": "gopls", "outcome": "skipped", "note": "no Linux bottle" }])
        )));
        assert!(refused::<GoldenVersion>(&with(
            &base,
            "missingTools",
            json!([{ "id": "tools/brew/gopls", "name": "gopls", "outcome": "installed", "note": "" }])
        )));
    }

    // packages/protocol/test/protocol.test.ts:1411 at a3b870a6e
    #[test]
    fn carries_what_the_pack_left_off_the_image_by_row_path_and_note_and_stays_optional() {
        let base = base();
        let left = json!([{ "id": "agents/claude", "path": "~/.claude/settings.json", "note": "hook left behind: /opt/homebrew/bin/terminal-notifier" }]);
        let sealed = with(&base, "leftBehind", left);
        assert_eq!(parsed::<GoldenVersion>(&sealed), sealed);
        assert!(parse::<GoldenVersion>(&base).expect("a version").left_behind.is_none());
        assert!(refused::<GoldenVersion>(&with(&base, "leftBehind", json!([{ "id": "agents/claude", "note": "hook left behind: x" }]))));
        assert!(refused::<GoldenVersion>(&with(&base, "leftBehind", json!(["hook left behind: x"]))));
    }
}

mod goldenimage {
    use super::*;

    fn version(extra: Value) -> GoldenVersion {
        let mut v = json!({ "version": 1, "snapshotId": "snap_a", "baseTemplate": "base", "setupSha": "x", "createdAt": "2026-09-01T00:00:00Z", "smoke": { "cmd": "true", "exitCode": 0 } });
        for (k, val) in extra.as_object().unwrap() {
            v = with(&v, k, val.clone());
        }
        serde_json::from_value(v).expect("a version")
    }

    // packages/protocol/test/protocol.test.ts:1421 at a3b870a6e
    // The TS passes the two fields alone; the Rust function takes a whole GoldenVersion, so the rest is filled in.
    #[test]
    fn boots_a_version_from_its_template_once_one_is_recorded_and_gives_it_no_word_a_ve() {
        assert_eq!(
            golden_image(&version(json!({ "templateId": "tpl_a" }))),
            GoldenImage { spec: ImageSpec::Template("tpl_a".to_string()), marks: vec![] }
        );
        assert_eq!(
            golden_image(&version(json!({}))),
            GoldenImage { spec: ImageSpec::FromSnapshot("snap_a".to_string()), marks: vec!["volatile"] }
        );
    }

    // packages/protocol/test/protocol.test.ts:1426 at a3b870a6e
    #[test]
    fn a_version_record_takes_an_optional_templateid_and_parses_without_one_as_every_ve() {
        let v = json!({ "version": 1, "snapshotId": "snap_a", "baseTemplate": "base", "setupSha": "x", "createdAt": "2026-09-01T00:00:00Z", "smoke": { "cmd": "true", "exitCode": 0 } });
        assert!(parse::<GoldenVersion>(&v).expect("a version").template_id.is_none());
        assert_eq!(
            parse::<GoldenVersion>(&with(&v, "templateId", json!("tpl_a"))).expect("a version").template_id,
            Some("tpl_a".to_string())
        );
        assert_eq!(back(&GoldenStage::Promoting), json!("promoting"));
    }
}

mod the_sentences_a_vault_archive_is_refused_with {
    use super::*;

    // packages/protocol/test/protocol.test.ts:1460 at a3b870a6e
    #[test]
    fn a_sealed_vault_reads_with_the_paths_it_held_and_one_sealed_before_the_list_was_k() {
        let vault = json!({ "sha256": "a".repeat(64), "bytes": 10, "paths": 2, "takenAt": "2026-09-01T00:00:00Z" });
        assert!(parse::<SealedVault>(&vault).expect("a vault").held.is_none());
        let held = with(&vault, "held", json!(["/root/.codex/auth.json"]));
        assert_eq!(parse::<SealedVault>(&held).expect("a vault").held, Some(vec!["/root/.codex/auth.json".to_string()]));
    }
}

mod goldenhead {
    use super::*;

    fn v1() -> GoldenVersion {
        serde_json::from_value(json!({ "version": 1, "snapshotId": "snap_1", "baseTemplate": "base", "setupSha": "x", "createdAt": "2026-09-01T00:00:00Z", "smoke": { "cmd": "true", "exitCode": 0 } }))
            .expect("a version")
    }

    // packages/protocol/test/protocol.test.ts:1470 at a3b870a6e
    #[test]
    fn is_nothing_without_a_manifest() {
        assert!(golden_head(None).is_none());
    }

    // packages/protocol/test/protocol.test.ts:1474 at a3b870a6e
    #[test]
    fn is_nothing_when_the_head_names_a_version_the_manifest_lacks() {
        let manifest = GoldenManifest { head: JsNumber(2.0), versions: vec![v1()] };
        assert!(golden_head(Some(&manifest)).is_none());
    }

    // packages/protocol/test/protocol.test.ts:1478 at a3b870a6e
    #[test]
    fn is_the_version_the_head_names() {
        let mut v2 = v1();
        v2.version = JsNumber(2.0);
        v2.snapshot_id = "snap_2".to_string();
        let manifest = GoldenManifest { head: JsNumber(2.0), versions: vec![v1(), v2.clone()] };
        let head = golden_head(Some(&manifest)).expect("the head's version");
        assert_eq!(head, &v2);
        assert!(std::ptr::eq(head, &manifest.versions[1]));
    }
}

mod thread_provenance {
    use super::*;

    fn row() -> Value {
        json!({ "id": "s1", "workspaceId": "ws_1", "harness": "claude", "status": "completed" })
    }

    // packages/protocol/test/protocol.test.ts:1487 at a3b870a6e
    #[test]
    fn sessionview_carries_who_asked_for_the_turn_person_cli_or_agent_and_stays_optiona() {
        let row = row();
        assert_eq!(parse::<SessionView>(&with(&row, "startedBy", json!("cli"))).expect("a view").started_by, Some(SessionOrigin::Cli));
        assert_eq!(parse::<SessionView>(&with(&row, "startedBy", json!("agent"))).expect("a view").started_by, Some(SessionOrigin::Agent));
        assert_eq!(parse::<SessionView>(&row).expect("a view").started_by, None);
        assert!(refused::<SessionView>(&with(&row, "startedBy", json!("robot"))));
        assert_eq!(back(&[SessionOrigin::Person, SessionOrigin::Cli, SessionOrigin::Agent]), json!(["person", "cli", "agent"]));
    }

    // packages/protocol/test/protocol.test.ts:1545 at a3b870a6e
    #[test]
    fn a_start_carries_the_attempt_it_was_sent_under_and_a_thread_reads_the_attempt_off() {
        // left to the wire writer: RuntimeRequest.parse of a sessions.start carrying attempt att_1
        // left to the words writer: foldThreads of the three rows, grouped.attempt being att_1 and the lone thread's row having no attempt key
        // The thread foldThreads answered for the first two rows (evaluated with node at a3b870a6e), which ThreadView must read back whole.
        let grouped = json!({
            "id": "thr_a",
            "threadId": "thr_a",
            "workspaceId": "ws_1",
            "harness": "claude",
            "startedBy": "person",
            "status": "completed",
            "title": "s1",
            "sessionId": "s2",
            "turns": 2,
            "ran": false,
            "attempt": "att_1",
        });
        assert_eq!(parsed::<ThreadView>(&grouped), grouped);
        let attempted = with(&row(), "attempt", json!("att_1"));
        assert_eq!(parsed::<SessionView>(&attempted)["attempt"], json!("att_1"));
    }

    // packages/protocol/test/protocol.test.ts:1769 at a3b870a6e
    #[test]
    fn folded_is_a_mark_a_paired_device_sends_and_the_view_carries_what_an_agent_s_suba() {
        // left to the wire writer: RuntimeRequest.safeParse of sessions.mark with folded true (accepted) and folded "yes" (refused)
        let child = json!({ "id": "a1", "title": "count", "state": "done", "model": "claude-haiku-4-5", "asked": "Count to three.", "startedAt": 1, "endedAt": 2, "lastLine": "3" });
        assert_eq!(parsed::<SubagentView>(&child), child);
        let event = json!({ "type": "session.subagent", "workspaceId": "ws_a", "sessionId": "s1", "task": "a1", "state": "running", "model": "claude-haiku-4-5", "asked": "Count to three." });
        let read = parsed::<SessionEvent>(&event);
        assert_eq!(read["model"], json!("claude-haiku-4-5"));
        assert_eq!(read["asked"], json!("Count to three."));
    }
}

mod the_small_recipe {
    use super::*;

    fn row() -> Value {
        json!({ "id": "gh", "kind": "tool", "on": true, "source": { "kind": "used", "sessions": 100, "calls": 7919 }, "signIn": "copy" })
    }

    fn recipe() -> Value {
        json!({ "version": 1, "at": "2026-09-06T03:00:00.000Z", "histories": [{ "agent": "claude", "state": "read", "sessions": 149, "calls": 87593 }], "rows": [row()] })
    }

    // packages/protocol/test/protocol.test.ts:1894 at a3b870a6e
    #[test]
    fn is_catalog_ids_with_a_tick_and_the_source_of_it_and_nothing_a_session_or_a_file() {
        let recipe = recipe();
        let row = row();
        assert_eq!(parsed::<Recipe>(&recipe), recipe);
        assert!(refused::<Recipe>(&with(&recipe, "version", json!(2))));
        assert!(refused::<Recipe>(&with(&recipe, "rows", json!([with(&row, "source", json!({ "kind": "guess" }))]))));
        assert!(refused::<Recipe>(&with(&recipe, "rows", json!([with(&row, "signIn", json!("maybe"))]))));
        assert!(refused::<Recipe>(&with(
            &recipe,
            "histories",
            json!([{ "agent": "claude", "state": "read", "sessions": -1, "calls": 0 }])
        )));
        let installed = json!({ "kind": "installed", "paths": ["~/.claude/settings.json"], "bin": true });
        assert_eq!(parsed::<RecipeSource>(&installed), installed);
        assert!(refused::<RecipeSource>(&json!({ "kind": "installed", "paths": ["~/.claude/settings.json"] })));
    }
}

mod the_project_plan {
    use super::*;

    // packages/protocol/test/protocol.test.ts:1920 at a3b870a6e
    #[test]
    fn names_each_agent_with_sessions_for_the_folder_and_how_its_state_travels() {
        let agent = json!({ "agent": "claude", "name": "Claude Code", "sessions": 2, "bytes": 4096, "carry": "moves" });
        let plan = json!({ "source": "/Users/dev/proj", "repo": true, "files": 1, "bytes": 2, "secrets": [], "excluded": [], "skipped": [], "agents": [agent] });
        assert_eq!(parsed::<ProjectPlan>(&plan), plan);
        assert!(refused::<ProjectPlan>(&with(&plan, "agents", json!([with(&agent, "carry", json!("maybe"))]))));
        let unreadable = json!({ "agent": "opencode", "name": "OpenCode", "sessions": 0, "bytes": 0, "carry": "transcript-only", "error": "file is not a database" });
        assert_eq!(parsed::<ProjectPlan>(&with(&plan, "agents", json!([unreadable])))["agents"], json!([unreadable]));
        assert!(refused::<ProjectPlan>(&without(&plan, "agents")));
    }
}

mod the_project_export {
    use super::*;

    // packages/protocol/test/protocol.test.ts:1964 at a3b870a6e
    #[test]
    fn its_events_ride_the_union_with_the_stages_in_order_and_the_result_counts_session() {
        // left to the wire writer: EventUnion.parse of a project.export event at each stage, packing, downloading, landing, done and failed
        // The same stages through the event's own shape, which the union's member is: each reads back as written, less the type key the enum holds.
        for stage in ["packing", "downloading", "landing", "done", "failed"] {
            let e = json!({ "type": "project.export", "workspaceId": "ws_1", "source": "/root/work/proj", "dest": "/Users/dev/proj", "stage": stage, "message": "x", "elapsedMs": 3, "seq": 1 });
            let read = parsed::<ProjectExportEvent>(&e);
            assert_eq!(read["stage"], json!(stage));
            assert_eq!(read["message"], json!("x"));
        }
        assert!(refused::<ProjectExportEvent>(
            &json!({ "type": "project.export", "workspaceId": "w", "source": "/a", "dest": "/b", "stage": "uploading", "message": "", "elapsedMs": 0 })
        ));
        let result = json!({
            "dest": "/Users/dev/proj",
            "files": 12,
            "bytes": 4096,
            "excluded": ["node_modules", "dist"],
            "agents": [
                { "agent": "claude", "files": 3, "bytes": 40, "outcome": "moved", "sessions": 2 },
                { "agent": "codex", "files": 1, "bytes": 40, "outcome": "transcript-only", "sessions": 2, "skipped": 1 },
                { "agent": "hermes", "files": 0, "bytes": 0, "outcome": "nothing", "sessions": 1 },
                { "agent": "pi", "files": 0, "bytes": 0, "outcome": "failed", "error": "not a database" },
            ],
        });
        assert_eq!(parsed::<ProjectExportResult>(&result), result);
        assert!(refused::<ProjectExportResult>(&without(&result, "excluded")));
        let imported = parsed::<ProjectImportResult>(&json!({
            "dest": "/root/p",
            "files": 1,
            "bytes": 1,
            "parts": 1,
            "cut": [],
            "rewritten": [],
            "agents": [{ "agent": "codex", "files": 1, "bytes": 1, "outcome": "transcript-only", "skipped": 2 }],
            "project": { "name": "p", "dest": "/root/p", "importedAt": "2026-09-12T10:00:00.000Z" },
        }));
        assert_eq!(imported["agents"][0]["skipped"], json!(2));
    }
}

mod project_goldens {
    use super::*;

    // packages/protocol/test/protocol.test.ts:1989 at a3b870a6e
    #[test]
    fn a_project_golden_names_its_snapshot_the_projects_it_carries_the_golden_version_i() {
        let golden = json!({
            "snapshotId": "snap_project-proj",
            "projects": [{ "name": "proj", "dest": "/root/work/proj", "importedAt": "2026-09-06T10:00:00.000Z" }],
            "golden": "snap_golden-v12",
            "version": 12,
            "workspaceId": "ws_1",
            "workspaceName": "task",
            "createdAt": "2026-09-06T10:05:00.000Z",
        });
        assert_eq!(parsed::<ProjectGolden>(&golden), golden);
        let unversioned = without(&golden, "version");
        assert_eq!(parsed::<ProjectGolden>(&unversioned), unversioned);
        assert!(refused::<ProjectGolden>(&without(&golden, "projects")));
        assert!(refused::<ProjectGolden>(&with(&golden, "version", json!("12"))));
    }
}

mod packageof {
    use super::*;

    // packages/protocol/test/protocol.test.ts:2030 at a3b870a6e
    #[test]
    fn is_what_follows_the_manager_in_a_tools_row_s_id_a_tap_formula_s_slashes_kept_it() {
        assert_eq!(package_of("tools/brew/gh"), "gh");
        assert_eq!(package_of("tools/brew/zingzy/tap/diskbloom"), "zingzy/tap/diskbloom");
        assert_eq!(package_of("tools/npm/@scope/name"), "@scope/name");
        assert_eq!(package_of(&format!("{BREW_ID_PREFIX}yq")), "yq");
        // The collector's id for a manager's package, which a scan row of the same manager and package stands for.
        assert_eq!(tool_row_id("brew", "zingzy/tap/diskbloom"), format!("{BREW_ID_PREFIX}zingzy/tap/diskbloom"));
        assert_eq!(package_of(&tool_row_id("npm", "@scope/name")), "@scope/name");
        // Where a manager's rows sit, which is what the engine tests a row's id against and what the brew prefix is.
        assert_eq!(tool_row_prefix("npm"), "tools/npm/");
        assert_eq!(BREW_ID_PREFIX, tool_row_prefix("brew"));
        assert!(tool_row_id("uv", "ruff").starts_with(&tool_row_prefix("uv")));
    }
}

mod bringing_work_back {
    use super::*;

    // packages/protocol/test/protocol.test.ts:2128 at a3b870a6e
    #[test]
    fn a_workspace_says_which_workspace_it_was_forked_out_of_and_every_door_hands_that() {
        let view = json!({
            "id": "ws_child",
            "name": "pricing-page-fork",
            "machineId": "m2",
            "phase": "running",
            "golden": "snap_g",
            "createdAt": "2026-09-17T00:00:00.000Z",
            "project": { "id": "pr_1a2b3c4d", "name": "landing", "path": "/root/landing", "computer": "box" },
            "parentWorkspaceId": "ws_parent",
        });
        assert_eq!(parsed::<WorkspaceView>(&view), view);
        assert_eq!(parse::<WorkspaceOut>(&view).expect("an out view").parent_workspace_id, Some("ws_parent".to_string()));
        // A workspace a person made is nobody's child and carries none.
        let own = without(&view, "parentWorkspaceId");
        assert_eq!(parse::<WorkspaceOut>(&own).expect("an out view").parent_workspace_id, None);
        assert!(refused::<WorkspaceView>(&with(&view, "parentWorkspaceId", json!(7))));
    }
}

mod the_doctor_s_computer_road_on_the_wire {
    use super::*;

    // packages/protocol/test/protocol.test.ts:2240 at a3b870a6e
    #[test]
    fn one_line_of_a_road_carries_which_run_it_is_the_words_and_which_stream_said_them() {
        for stream in ["out", "err"] {
            let line = json!({ "type": "doctor.line", "doctorId": "d_abc", "line": "spoo answers", "stream": stream });
            // The event's struct holds no type key: the enum that holds it writes the tag.
            assert_eq!(parsed::<DoctorLineEvent>(&line), without(&line, "type"));
        }
        assert_eq!(DoctorLineEvent::TYPE, "doctor.line");
        assert!(refused::<DoctorLineEvent>(
            &json!({ "type": "doctor.line", "doctorId": "d_abc", "line": "spoo answers", "stream": "log" })
        ));
        // left to the wire writer: EventUnion.parse refusing a doctor.line, which a host source's event is and no member of the runtime's own union
    }
}
