// SPDX-License-Identifier: AGPL-3.0-only
//! Ports the six resolver blocks of packages/engine/test/project-state.test.ts (claude, pi, codex, hermes, gemini,
//! opencode): each fixture is the tree an agent left after one headless run, moved and compared byte for byte.
mod ps_fixtures;

use std::path::Path;

use serde_json::json;

use ps_fixtures::*;

fn keys(t: &Tree) -> Vec<String> {
    t.keys().cloned().collect()
}

// --- Claude Code ------------------------------------------------------------------------------------------------------

// TS: packages/engine/test/project-state.test.ts:120
#[tokio::test]
async fn claude_renames_dashed_directories_rewrites_cwd_on_every_line_and_leaves_sibling_alone() {
    let s = scratch();
    let home = claude_home(&s.root);
    let done = resolver_for("claude").move_state(&home, FROM, TO).await.unwrap();
    assert_eq!(tree(&home), claude_after("-root-work-b-2-x"));
    assert!(!Path::new(&j(&home, &["projects", "-private-tmp-wsp-r212-proj-b-2-x"])).exists());
    assert!(!Path::new(&j(&home, &["projects", "-private-tmp-wsp-r212-proj-b-2-x-sub"])).exists());
    let root = j(&home, &["projects", "-root-work-b-2-x"]);
    let sub = j(&home, &["projects", "-root-work-b-2-x-sub"]);
    assert_eq!(
        done,
        vec![
            moved(
                "session transcripts",
                vec![
                    root.clone(),
                    j(&root, &["S1.jsonl"]),
                    j(&root, &["S1", "subagents", "agent-a1.jsonl"]),
                    sub.clone(),
                    j(&sub, &["S3.jsonl"])
                ],
                6
            ),
            moved("auto memory", vec![j(&root, &["memory"])], 1),
        ]
    );
}

// TS: packages/engine/test/project-state.test.ts:134
#[tokio::test]
async fn claude_moves_a_directory_with_no_transcript_only_when_its_key_is_the_projects_own() {
    let s = scratch();
    let home = j(&s.root, &["claude"]);
    write(&j(&home, &["projects", "-private-tmp-wsp-r212-proj-b-2-x", "memory", "MEMORY.md"]), "notes\n");
    write(&j(&home, &["projects", "-private-tmp-wsp-r212-proj-b-2-x-sub", "memory", "MEMORY.md"]), "sub or sibling, no way to tell\n");
    resolver_for("claude").move_state(&home, FROM, TO).await.unwrap();
    let mut got = keys(&tree(&home));
    got.sort();
    assert_eq!(got, vec!["projects/-private-tmp-wsp-r212-proj-b-2-x-sub/memory/MEMORY.md", "projects/-root-work-b-2-x/memory/MEMORY.md"]);
}

// TS: packages/engine/test/project-state.test.ts:142
#[tokio::test]
async fn claude_finds_nothing_when_the_project_never_ran_there() {
    let s = scratch();
    let home = claude_home(&s.root);
    let before = tree(&home);
    assert_eq!(resolver_for("claude").move_state(&home, "/Users/me/never", TO).await.unwrap(), vec![]);
    assert_eq!(tree(&home), before);
}

// TS: packages/engine/test/project-state.test.ts:149
#[tokio::test]
async fn claude_refuses_to_move_onto_a_directory_that_already_exists_rather_than_merge_into_it() {
    let s = scratch();
    let home = claude_home(&s.root);
    write(&j(&home, &["projects", "-root-work-b-2-x", "S9.jsonl"]), &lines(&[&claude_user(TO)]));
    let before = tree(&home);
    let err = resolver_for("claude").move_state(&home, FROM, TO).await.unwrap_err();
    assert!(err.to_string().contains("already exists"), "{err}");
    assert_eq!(tree(&home), before);
}

// --- Pi ---------------------------------------------------------------------------------------------------------------

// TS: packages/engine/test/project-state.test.ts:174
#[tokio::test]
async fn pi_renames_double_dashed_directories_of_root_and_sub_folder_and_rewrites_header_cwd() {
    let s = scratch();
    let home = pi_home(&s.root);
    let done = resolver_for("pi").move_state(&home, FROM, TO).await.unwrap();
    let mut want = Tree::new();
    want.insert("sessions/--root-work-b_2.x--/2026-09-05T21-56-00-000Z_s1.jsonl".into(), txt(&lines(&[&pi_header(TO), PI_MESSAGE])));
    want.insert(
        "sessions/--root-work-b_2.x-sub--/2026-09-05T21-57-00-000Z_s2.jsonl".into(),
        txt(&lines(&[&pi_header(TO_SUB), PI_MESSAGE])),
    );
    want.insert(
        "sessions/--private-tmp-wsp-r212-proj-b_2.x-old--/2026-09-05T21-40-00-000Z_s3.jsonl".into(),
        txt(&lines(&[&pi_header(DECOY), PI_MESSAGE])),
    );
    want.insert("sessions/--Users-me-other--/2026-09-05T21-50-00-000Z_s0.jsonl".into(), txt(&lines(&[&pi_header(OTHER), PI_MESSAGE])));
    want.insert("trust.json".into(), txt(&format!("{{\"{FROM}\":true}}\n")));
    assert_eq!(tree(&home), want);
    let root = j(&home, &["sessions", "--root-work-b_2.x--"]);
    let sub = j(&home, &["sessions", "--root-work-b_2.x-sub--"]);
    assert_eq!(
        done,
        vec![moved(
            "sessions",
            vec![
                root.clone(),
                j(&root, &["2026-09-05T21-56-00-000Z_s1.jsonl"]),
                sub.clone(),
                j(&sub, &["2026-09-05T21-57-00-000Z_s2.jsonl"])
            ],
            4
        )]
    );
}

// TS: packages/engine/test/project-state.test.ts:191
#[tokio::test]
async fn pi_keeps_dots_and_underscores_dashes_colons_and_backslashes_as_measured() {
    let s = scratch();
    let home = j(&s.root, &["pi"]);
    let cwd = "C:\\Users\\me\\a_b.c";
    write(&j(&home, &["sessions", "--C--Users-me-a_b.c--", "x.jsonl"]), &lines(&[&json!({ "type": "session", "cwd": cwd }).to_string()]));
    resolver_for("pi").move_state(&home, cwd, "/home/me/a_b.c").await.unwrap();
    let mut want = Tree::new();
    want.insert(
        "sessions/--home-me-a_b.c--/x.jsonl".into(),
        txt(&lines(&[&json!({ "type": "session", "cwd": "/home/me/a_b.c" }).to_string()])),
    );
    assert_eq!(tree(&home), want);
}

// --- Codex ------------------------------------------------------------------------------------------------------------

// TS: packages/engine/test/project-state.test.ts:229
#[tokio::test]
async fn codex_updates_threads_cwd_and_rollouts_for_root_and_sub_folder_leaving_rollout_path_and_sibling() {
    let s = scratch();
    let home = codex_home(&s.root);
    let [t1, t2, t3, t4] = codex_rollouts(&home);
    let done = resolver_for("codex").move_state(&home, FROM, TO).await.unwrap();
    let mut want = Tree::new();
    want.insert(
        "sessions/2026/09/05/rollout-2026-09-05T21-58-00-t1.jsonl".into(),
        txt(&lines(&[&codex_meta(TO), CODEX_ITEM, &codex_turn(TO), CODEX_EVENT])),
    );
    want.insert(
        "sessions/2026/09/04/rollout-2026-09-04T10-00-00-t2.jsonl".into(),
        txt(&lines(&[&codex_meta(OTHER), CODEX_ITEM, &codex_turn(OTHER)])),
    );
    want.insert(
        "sessions/2026/09/05/rollout-2026-09-05T22-00-00-t3.jsonl".into(),
        txt(&lines(&[&codex_meta(TO_SUB), CODEX_ITEM, &codex_turn(TO_SUB)])),
    );
    want.insert(
        "sessions/2026/09/05/rollout-2026-09-05T22-01-00-t4.jsonl".into(),
        txt(&lines(&[&codex_meta(DECOY), CODEX_ITEM, &codex_turn(DECOY)])),
    );
    want.insert(
        "state_5.sqlite".into(),
        json!({
            "threads": [
                { "id": "t1", "rollout_path": t1, "cwd": TO, "archived": 0, "updated_at": 1 },
                { "id": "t2", "rollout_path": t2, "cwd": OTHER, "archived": 0, "updated_at": 2 },
                { "id": "t3", "rollout_path": t3, "cwd": TO_SUB, "archived": 0, "updated_at": 3 },
                { "id": "t4", "rollout_path": t4, "cwd": DECOY, "archived": 0, "updated_at": 4 },
            ]
        }),
    );
    want.insert("config.toml".into(), txt(&format!("[projects.\"{FROM}\"]\ntrust_level = \"trusted\"\n")));
    assert_eq!(tree(&home), want);
    assert_eq!(done, vec![moved("thread index", vec![j(&home, &["state_5.sqlite"])], 2), moved("rollout transcript", vec![t1, t3], 4)]);
}

// TS: packages/engine/test/project-state.test.ts:257
#[tokio::test]
async fn codex_finds_nothing_in_a_home_with_no_index_and_no_rollouts() {
    let s = scratch();
    let home = j(&s.root, &["codex"]);
    write(&j(&home, &["config.toml"]), "");
    assert_eq!(resolver_for("codex").move_state(&home, FROM, TO).await.unwrap(), vec![]);
}

// TS: packages/engine/test/project-state.test.ts:263
#[tokio::test]
async fn codex_counts_indexed_rollouts_it_could_not_reach_and_moves_their_rows_and_nothing_else() {
    let s = scratch();
    let home = j(&s.root, &["codex"]);
    let t1 = j("sessions", &["2026", "09", "05", "rollout-2026-09-05T21-58-00-t1.jsonl"]);
    let archived = j(&home, &["archived_sessions", "rollout-2026-09-01T10-00-00-t7.jsonl"]);
    write(&j(&home, &[&t1]), &lines(&[&codex_meta(FROM), CODEX_ITEM]));
    write(&archived, &lines(&[&codex_meta(FROM), CODEX_ITEM]));
    seed(
        &j(&home, &["state_5.sqlite"]),
        CODEX_SCHEMA,
        vec![
            ("insert into threads values (?, ?, ?, 0, 1)", vec![t("t1"), t(&j(&home, &[&t1])), t(FROM)]),
            ("insert into threads values (?, ?, ?, 1, 2)", vec![t("t7"), t(&archived), t(FROM)]),
            (
                "insert into threads values (?, ?, ?, 0, 3)",
                vec![t("t8"), t(&j(&home, &["sessions", "2026", "09", "02", "rollout-gone-t8.jsonl"])), t(FROM_SUB)],
            ),
            ("insert into threads values (?, ?, ?, 0, 4)", vec![t("t2"), t(&j(&home, &["sessions", "x", "rollout-t2.jsonl"])), t(OTHER)]),
        ],
    );
    let done = resolver_for("codex").move_state(&home, FROM, TO).await.unwrap();
    let mut index = moved("thread index", vec![j(&home, &["state_5.sqlite"])], 3);
    index.skipped = Some(2);
    assert_eq!(done, vec![index, moved("rollout transcript", vec![j(&home, &[&t1])], 1)]);
    assert_eq!(
        tree(&home).get("archived_sessions/rollout-2026-09-01T10-00-00-t7.jsonl"),
        Some(&txt(&lines(&[&codex_meta(FROM), CODEX_ITEM])))
    );
    let fresh = scratch();
    let all_home = codex_home(&fresh.root);
    let all = resolver_for("codex").move_state(&all_home, FROM, TO).await.unwrap();
    assert_eq!(all[0].skipped, None);
}

// TS: packages/engine/test/project-state.test.ts:284
#[tokio::test]
async fn codex_reads_only_rollouts_the_index_names_under_this_home_even_when_recorded_under_another() {
    let s = scratch();
    let home = j(&s.root, &["codex"]);
    let t1 = j("sessions", &["2026", "09", "05", "rollout-2026-09-05T21-58-00-t1.jsonl"]);
    let orphan = j(&home, &["sessions", "2026", "09", "05", "rollout-2026-09-05T23-00-00-t9.jsonl"]);
    write(&j(&home, &[&t1]), &lines(&[&codex_meta(FROM), CODEX_ITEM, &codex_turn(FROM)]));
    write(&orphan, &lines(&[&codex_meta(FROM), CODEX_ITEM]));
    let recorded = j("/Users/me/.codex", &[&t1]);
    seed(
        &j(&home, &["state_5.sqlite"]),
        CODEX_SCHEMA,
        vec![("insert into threads values (?, ?, ?, 0, 1)", vec![t("t1"), t(&recorded), t(FROM)])],
    );
    let done = resolver_for("codex").move_state(&home, FROM, TO).await.unwrap();
    let mut want = Tree::new();
    want.insert(t1.clone(), txt(&lines(&[&codex_meta(TO), CODEX_ITEM, &codex_turn(TO)])));
    want.insert("sessions/2026/09/05/rollout-2026-09-05T23-00-00-t9.jsonl".into(), txt(&lines(&[&codex_meta(FROM), CODEX_ITEM])));
    want.insert(
        "state_5.sqlite".into(),
        json!({ "threads": [{ "id": "t1", "rollout_path": recorded, "cwd": TO, "archived": 0, "updated_at": 1 }] }),
    );
    assert_eq!(tree(&home), want);
    assert_eq!(
        done,
        vec![moved("thread index", vec![j(&home, &["state_5.sqlite"])], 1), moved("rollout transcript", vec![j(&home, &[&t1])], 2)]
    );
}

// --- Hermes -----------------------------------------------------------------------------------------------------------

// TS: packages/engine/test/project-state.test.ts:325
#[tokio::test]
async fn hermes_updates_cwd_and_git_repo_root_on_rows_at_or_under_the_path_leaving_sibling_alone() {
    let s = scratch();
    let home = hermes_home(&s.root);
    let done = resolver_for("hermes").move_state(&home, FROM, TO).await.unwrap();
    let mut want = Tree::new();
    want.insert(
        "state.db".into(),
        json!({
            "sessions": [
                { "id": "20260906_030000_aaaaaa", "cwd": OTHER, "git_repo_root": OTHER, "message_count": 1 },
                { "id": "20260906_033144_55e3e2", "cwd": TO, "git_repo_root": TO, "message_count": 2 },
                { "id": "20260906_034000_bbbbbb", "cwd": TO, "git_repo_root": null, "message_count": 1 },
                { "id": "20260906_035000_cccccc", "cwd": TO_SUB, "git_repo_root": TO, "message_count": 1 },
                { "id": "20260906_036000_dddddd", "cwd": DECOY, "git_repo_root": DECOY, "message_count": 1 },
            ],
            "messages": [
                { "id": 1, "session_id": "20260906_033144_55e3e2", "content": "hi" },
                { "id": 2, "session_id": "20260906_033144_55e3e2", "content": "there" },
                { "id": 3, "session_id": "20260906_030000_aaaaaa", "content": "other" },
            ],
        }),
    );
    want.insert("sessions/request_dump_20260906_033144_55e3e2_1.json".into(), txt("{}\n"));
    assert_eq!(tree(&home), want);
    assert_eq!(done, vec![moved("sessions", vec![j(&home, &["state.db"])], 3)]);
}

// --- Gemini -----------------------------------------------------------------------------------------------------------

// TS: packages/engine/test/project-state.test.ts:368
#[tokio::test]
async fn gemini_rewrites_registry_keys_of_root_and_sub_folder_and_project_root_files_keeping_slugs_and_chat() {
    let s = scratch();
    let home = gemini_home(&s.root);
    let done = resolver_for("gemini").move_state(&home, FROM, TO).await.unwrap();
    let mut want = Tree::new();
    want.insert("projects.json".into(), txt(&gemini_registry(TO)));
    want.insert("tmp/b_2.x/.project_root".into(), txt(TO));
    want.insert("tmp/b_2.x/chats/session-2026-09-05T21-59-a1b2c3d4.jsonl".into(), txt(GEMINI_CHAT));
    want.insert("history/b_2.x/.project_root".into(), txt(&format!("{TO}\n")));
    want.insert("tmp/other/.project_root".into(), txt(OTHER));
    want.insert("tmp/sub/.project_root".into(), txt(TO_SUB));
    want.insert("tmp/b_2.x-old/.project_root".into(), txt(DECOY));
    want.insert("trustedFolders.json".into(), txt(&format!("{{\"{FROM}\":\"TRUST_FOLDER\"}}\n")));
    assert_eq!(tree(&home), want);
    assert_eq!(
        done,
        vec![
            moved("project registry", vec![j(&home, &["projects.json"])], 2),
            moved("project temp dir", vec![j(&home, &["tmp", "b_2.x", ".project_root"]), j(&home, &["tmp", "sub", ".project_root"])], 2),
            moved("shell history", vec![j(&home, &["history", "b_2.x", ".project_root"])], 1),
        ]
    );
}

// TS: packages/engine/test/project-state.test.ts:403
#[tokio::test]
async fn gemini_moves_the_registry_alone_when_the_project_never_opened_a_shell() {
    let s = scratch();
    let home = gemini_home(&s.root);
    std::fs::remove_dir_all(j(&home, &["history"])).unwrap();
    let done = resolver_for("gemini").move_state(&home, FROM, TO).await.unwrap();
    assert_eq!(done.iter().map(|m| m.state.as_str()).collect::<Vec<_>>(), vec!["project registry", "project temp dir"]);
}

// TS: packages/engine/test/project-state.test.ts:410
#[tokio::test]
async fn gemini_refuses_a_destination_the_registry_already_knows() {
    let s = scratch();
    let home = gemini_home(&s.root);
    write(&j(&home, &["projects.json"]), &json!({ "projects": { FROM: "b_2.x", TO: "b_2.x-1" } }).to_string());
    let err = resolver_for("gemini").move_state(&home, FROM, TO).await.unwrap_err();
    assert!(err.to_string().contains("already exists"), "{err}");
}

// TS: packages/engine/test/project-state.test.ts:416
#[tokio::test]
async fn gemini_finds_nothing_when_the_registry_does_not_know_the_path() {
    let s = scratch();
    let home = gemini_home(&s.root);
    assert_eq!(resolver_for("gemini").move_state(&home, "/Users/me/never", TO).await.unwrap(), vec![]);
}

// --- OpenCode ---------------------------------------------------------------------------------------------------------

// TS: packages/engine/test/project-state.test.ts:466
#[tokio::test]
async fn opencode_updates_worktree_project_directory_and_session_directory_at_or_under_path_and_clears_sandboxes() {
    let s = scratch();
    let home = opencode_home(&s.root);
    let done = resolver_for("opencode").move_state(&home, FROM, TO).await.unwrap();
    let mut db = json!({
        "project": [
            { "id": HASH, "worktree": TO, "vcs": "git", "name": "b_2.x", "sandboxes": "[]", "time_created": 1 },
            { "id": "deadbeef", "worktree": DECOY, "vcs": "git", "name": "b_2.x-old", "sandboxes": "[]", "time_created": 2 },
            { "id": "global", "worktree": "/", "vcs": null, "name": null, "sandboxes": "[]", "time_created": 0 },
        ],
        "project_directory": [
            { "project_id": HASH, "directory": TO },
            { "project_id": HASH, "directory": TO_SUB },
            { "project_id": "deadbeef", "directory": DECOY },
            { "project_id": "global", "directory": "/Users/me/scratch" },
        ],
        "session": [
            { "id": "ses_1", "project_id": HASH, "directory": TO, "title": "first" },
            { "id": "ses_2", "project_id": "global", "directory": "/Users/me/scratch", "title": "loose" },
            { "id": "ses_3", "project_id": HASH, "directory": TO_SUB, "title": "deeper" },
            { "id": "ses_4", "project_id": "deadbeef", "directory": DECOY, "title": "sibling" },
        ],
    });
    for (table, rows) in opencode_messages() {
        db.as_object_mut().unwrap().insert(table.to_string(), rows);
    }
    let mut want = Tree::new();
    want.insert("opencode.db".into(), db);
    assert_eq!(tree(&home), want);
    let file = j(&home, &["opencode.db"]);
    assert_eq!(
        done,
        vec![
            moved("project", vec![file.clone()], 1),
            moved("project directories", vec![file.clone()], 2),
            moved("sessions", vec![file], 2)
        ]
    );
}
