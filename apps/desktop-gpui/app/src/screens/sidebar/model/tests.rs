// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

const NOW: f64 = 1_800_000_000_000.0;
const MIN: f64 = 60_000.0;
const HOUR: f64 = 3_600_000.0;

fn row(id: &str, status: Status, started: f64) -> Row {
    Row { id: id.into(), workspace_id: "w".into(), harness: "claude".into(), status, started_at: Some(started), ..Row::default() }
}

fn done(id: &str, started: f64) -> Row {
    Row { ended_at: Some(started + MIN), read_at: Some(started + MIN), ..row(id, Status::Completed, started) }
}

/// Finished and not yet seen: live in the list, working nowhere.
fn fresh(id: &str, started: f64) -> Row {
    Row { ended_at: Some(started + MIN), ..row(id, Status::Completed, started) }
}

/// The rows under the heads, for a test of the trees alone.
fn under_heads(lines: Vec<Line>) -> Vec<Line> {
    lines.into_iter().filter(|l| !matches!(l.item, Item::Head { .. })).collect()
}

fn child(mut r: Row, parent: &str) -> Row {
    r.parent_thread_id = Some(parent.into());
    r
}

fn view(rows: &[Row]) -> Vec<Thread> {
    fold_threads(rows)
}

fn keys(all: &[Thread], trees: &[Tree]) -> Vec<String> {
    trees.iter().map(|t| t.thread.map_or_else(|| t.key.clone(), |i| all[i].key.clone())).collect()
}

fn tiles(lines: &[Line], all: &[Thread]) -> Vec<(String, u8)> {
    lines.iter().filter_map(|l| if let Item::Tile(t) = &l.item { Some((all[t.thread].key.clone(), l.depth)) } else { None }).collect()
}

#[test]
fn fold_takes_latest_state_and_opening_provenance() {
    let mut first = row("a1", Status::Completed, NOW - HOUR);
    first.thread_id = Some("t".into());
    first.prompt = Some("Fix the login page. Then write tests for it.".into());
    first.parent_thread_id = Some("lead".into());
    first.cost_usd = Some(0.5);
    let mut second = row("a2", Status::Running, NOW);
    second.thread_id = Some("t".into());
    second.cost_usd = Some(0.25);
    let all = fold_threads(&[first.clone(), second.clone(), row("solo", Status::Completed, NOW)]);
    assert_eq!(all.len(), 2);
    assert_eq!((all[0].key.as_str(), all[0].session_id.as_str()), ("t", "a2"));
    assert_eq!(all[0].status, Status::Running);
    assert_eq!(all[0].title, "Fix the login page.");
    assert_eq!(all[0].parent.as_deref(), Some("lead"));
    assert_eq!(all[0].cost_usd, Some(0.75));
    assert_eq!(all[1].cost_usd, None);
    second.harness_title = Some("  Login\n fix ".into());
    assert_eq!(fold_threads(&[first, second])[0].title, "Login");
    assert_eq!(fold_threads(&[row("bare", Status::Completed, NOW)])[0].title, "bare");
}

#[test]
fn needs_you_skips_a_stops_end_but_not_a_finish_or_held_work() {
    let all = view(&[
        Row { ended_at: Some(NOW), ..row("fin", Status::Completed, NOW - MIN) },
        Row { ended_at: Some(NOW), ..row("stop", Status::Interrupted, NOW - MIN) },
        Row { sub: Some(Sub { state: SubState::Napping, held: true }), ..done("held", NOW - HOUR) },
        Row { woke_at: Some(NOW), ..done("woke", NOW - HOUR) },
    ]);
    let needs: Vec<bool> = all.iter().map(|t| t.needs_you).collect();
    assert_eq!(needs, [true, false, true, true]);
    assert!(all[1].unread, "a stop nobody saw is still unread");
    assert!(!all[2].unread);
}

#[test]
fn settle_by_hand_by_quiet_and_never_for_failed_or_unseen() {
    let settle = Some(SETTLE_MS_DEFAULT);
    let read = &view(&[done("r", NOW - 5.0 * HOUR)])[0];
    assert!(thread_settled(read, NOW, settle, false));
    assert!(!thread_settled(read, NOW, settle, true), "held never folds by time");
    assert!(!thread_settled(read, NOW, None, false), "never means only a hand settles");
    let recent = &view(&[done("r", NOW - HOUR)])[0];
    assert!(!thread_settled(recent, NOW, settle, false));
    let unseen = &view(&[Row { read_at: None, ..done("u", NOW - 5.0 * HOUR) }])[0];
    assert!(!thread_settled(unseen, NOW, settle, false));
    let failed = &view(&[Row { status: Status::Failed, ..done("f", NOW - 5.0 * HOUR) }])[0];
    assert!(!thread_settled(failed, NOW, settle, false));
    let hand = &view(&[Row { settled_at: Some(NOW - HOUR + 2.0 * MIN), ..done("h", NOW - HOUR) }])[0];
    assert_eq!(settle_moment(hand, settle, true), Some(SettleMoment { ms: NOW - HOUR + 2.0 * MIN, by_hand: true }));
    let since = &view(&[Row { settled_at: Some(NOW - 3.0 * HOUR), ..done("s", NOW - HOUR) }])[0];
    assert!(!thread_settled(since, NOW, settle, false), "activity after a hand settle brings it back");
}

#[test]
fn kinds_read_in_registry_order() {
    let k = |r: Row| kind_of(&view(&[r])[0], NOW);
    let running = row("r", Status::Running, NOW);
    assert_eq!(k(Row { asking: true, ..running.clone() }), Kind::NeedsYou);
    assert_eq!(k(Row { waiting_on: true, ..running.clone() }), Kind::NeedsYou);
    assert_eq!(k(Row { capped: true, ..running.clone() }), Kind::Waiting);
    assert_eq!(k(Row { sub: Some(Sub { state: SubState::Starting, held: false }), ..running.clone() }), Kind::Starting);
    assert_eq!(k(running.clone()), Kind::Working);
    let failed = Row { ended_at: Some(NOW), ..row("f", Status::Failed, NOW - MIN) };
    assert_eq!(k(Row { limit: true, ..failed.clone() }), Kind::Limited);
    assert_eq!(k(Row { limit: true, resume_at: Some(NOW + HOUR), ..failed.clone() }), Kind::Resuming);
    assert_eq!(k(failed), Kind::Failed);
    let released = Some(Sub { state: SubState::Released, held: false });
    assert_eq!(k(Row { sub: released.clone(), ..done("x", NOW - HOUR) }), Kind::Released);
    assert_eq!(k(Row { sub: released, read_at: None, ..done("x", NOW - HOUR) }), Kind::Done, "unread reads Done first");
    assert_eq!(k(Row { disk_kept_until: Some(NOW - 1.0), ..done("x", NOW - HOUR) }), Kind::Expired);
    assert_eq!(k(Row { ended_at: Some(NOW), ..row("s", Status::Interrupted, NOW - MIN) }), Kind::Stopped, "a stop outranks Done");
    assert_eq!(k(Row { sub: Some(Sub { state: SubState::Napping, held: true }), ..done("x", NOW - HOUR) }), Kind::NotPushed);
    assert_eq!(k(done("x", NOW - HOUR)), Kind::Resting);
}

#[test]
fn slots_and_receding_titles() {
    let working = &view(&[row("w", Status::Running, NOW - 42_000.0)])[0];
    let s = slot(working, Kind::Working, NOW, None);
    assert_eq!((s.text.as_deref(), s.tone, s.glyph, s.crab), (Some("42s"), Some(Tone::Working), None, true));
    let resting = &view(&[done("r", NOW - 3.0 * HOUR)])[0];
    assert_eq!(slot(resting, Kind::Resting, NOW, Some(NOW - 5.0 * MIN)).text.as_deref(), Some("5m"));
    assert_eq!(
        slot(resting, Kind::Failed, NOW, None),
        Slot { glyph: Some("circle-alert"), crab: false, tone: Some(Tone::Failed), text: None }
    );
    assert_eq!(word(resting, Kind::Limited, NOW, 0.0), "At limit");
    let refused = &view(&[Row { setup_refusal: Some("config folder outside home".into()), ..done("x", NOW - HOUR) }])[0];
    let tile = Tile {
        thread: 0,
        shape: Shape::Full,
        under: Under::None,
        moves: true,
        inbox: false,
        started_by: Vec::new(),
        snoozed_working: None,
        age_at: None,
    };
    assert_eq!(tile_slot(refused, &tile, NOW).text.as_deref(), Some("Refused"));
    assert_eq!(tile_slot(refused, &Tile { shape: Shape::Settled, ..tile }, NOW).text, None);
    assert!(recedes(Kind::Working) && recedes(Kind::Resting) && recedes(Kind::Stopped));
    assert!(!recedes(Kind::NeedsYou) && !recedes(Kind::Done) && !recedes(Kind::Failed) && !recedes(Kind::NotPushed));
}

#[test]
fn running_time_and_resting_age_formats() {
    assert_eq!(running_time(42_000.0), "42s");
    assert_eq!(running_time(17.0 * MIN + 5_000.0), "17m");
    assert_eq!(running_time(65.0 * MIN), "1h 5m");
    assert_eq!(running_time(26.0 * HOUR), "1d 2h");
    assert_eq!(running_time(-5.0), "0s");
    assert_eq!(resting_age(NOW - 30_000.0, NOW), "now");
    assert_eq!(resting_age(NOW + 5_000.0, NOW), "now");
    assert_eq!(resting_age(NOW - 5.0 * MIN, NOW), "5m");
    assert_eq!(resting_age(NOW - 3.0 * HOUR, NOW), "3h");
    assert_eq!(resting_age(NOW - 50.0 * HOUR, NOW), "2d");
    assert_eq!(resets_word(NOW + 14.0 * MIN, NOW, 0.0), "resets in 14 min");
    assert_eq!(resets_word(NOW + 5.0 * HOUR, NOW, 0.0), "resets in 5 h");
    assert_eq!(resets_word(0.0 + 3.0 * 24.0 * HOUR, 0.0, 0.0), "resets on Sun");
}

#[test]
fn sections_split_pinned_inbox_list_and_settled_newest_first() {
    let all = view(&[
        row("old", Status::Running, NOW - 3.0 * HOUR),
        row("new", Status::Running, NOW - HOUR),
        Row { pinned_at: Some(NOW), ..row("pin", Status::Running, NOW - 4.0 * HOUR) },
        Row { asking: true, ..row("ask", Status::Running, NOW - 2.0 * HOUR) },
        done("quiet", NOW - 9.0 * HOUR),
    ]);
    let v = sidebar(&all, &Prefs::default(), NOW, None, Vec::new());
    assert_eq!(keys(&all, &v.pinned), ["pin"]);
    assert_eq!(keys(&all, &v.needs_you), ["ask"], "a lone root that asks stands in the inbox alone");
    assert_eq!(keys(&all, &v.working), ["new", "old"], "running roots stand in Working, the pinned one in Pinned");
    assert!(v.threads.is_empty());
    assert_eq!(keys(&all, &v.settled), ["quiet"]);
}

#[test]
fn a_moved_key_reorders_the_list() {
    let all = view(&[fresh("a", NOW - 3.0 * HOUR), Row { order: Some(NOW), ..fresh("b", NOW - 5.0 * HOUR) }]);
    assert_eq!(keys(&all, &sidebar(&all, &Prefs::default(), NOW, None, Vec::new()).threads), ["b", "a"]);
}

#[test]
fn a_tree_with_an_asking_child_shows_in_both_inbox_and_list() {
    let all = view(&[fresh("lead", NOW - HOUR), Row { asking: true, ..child(row("kid", Status::Running, NOW), "lead") }]);
    let v = sidebar(&all, &Prefs::default(), NOW, None, Vec::new());
    assert_eq!(keys(&all, &v.threads), ["lead"]);
    assert_eq!(keys(&all, &v.needs_you), ["kid"]);
    assert_eq!(v.needs_you[0].inbox.as_deref(), Some(&[0usize][..]));
    let lines = v.lines(&Folds::default());
    let inbox = lines.iter().find_map(|l| if let Item::Tile(t) = &l.item { t.inbox.then_some(t) } else { None }).unwrap();
    assert_eq!((inbox.started_by.as_slice(), inbox.moves, inbox.under), (&[0usize][..], false, Under::None));
}

#[test]
fn placement_holds_while_the_state_holds() {
    let mut ask = Row { asking: true, ..row("ask", Status::Running, NOW - 2.0 * HOUR) };
    let all = view(std::slice::from_ref(&ask));
    let v = sidebar(&all, &Prefs::default(), NOW, None, Vec::new());
    let marks = v.drop_marks("ask", Section::Threads).unwrap().marks;
    assert_eq!(marks, json!({ "section": { "name": "idle", "whileState": "needs-you:ask" } }));
    ask.section = Some(Placement { name: ThreadSection::Idle, while_state: "needs-you:ask".into() });
    let all = view(std::slice::from_ref(&ask));
    let v = sidebar(&all, &Prefs::default(), NOW, None, Vec::new());
    assert!(v.needs_you.is_empty());
    assert_eq!(keys(&all, &v.threads), ["ask"]);
    ask.section.as_mut().unwrap().while_state = "needs-you:older-turn".into();
    let all = view(&[ask]);
    assert_eq!(keys(&all, &sidebar(&all, &Prefs::default(), NOW, None, Vec::new()).needs_you), ["ask"], "a new turn lapses it");
    assert_eq!(
        sidebar(&all, &Prefs::default(), NOW, None, Vec::new()).drop_marks("ask", Section::Pinned).map(|m| m.marks),
        Some(json!({ "pinned": true }))
    );
}

#[test]
fn attempts_group_under_one_head_oldest_first() {
    let a = |id: &str, at: f64| Row { attempt: Some("x".into()), prompt: Some(format!("{id} task")), ..fresh(id, at) };
    let all = view(&[a("m2", NOW - MIN), a("m1", NOW - 2.0 * MIN), fresh("solo", NOW)]);
    let v = sidebar(&all, &Prefs::default(), NOW, None, Vec::new());
    assert_eq!(keys(&all, &v.threads), ["solo", "attempt:x"]);
    assert_eq!(keys(&all, &v.threads[1].children), ["m1", "m2"]);
    let pin = v.drop_marks("attempt:x", Section::Pinned).unwrap();
    assert_eq!((pin.thread_ids, pin.marks), (vec!["m1".to_owned(), "m2".to_owned()], json!({ "pinned": true })), "a group pins as one");
    assert_eq!(v.drop_marks("attempt:x", Section::NeedsYou), None);
    let lines = v.lines(&Folds::default());
    let group = lines.iter().find_map(|l| if let Item::Group { title_of, copies, .. } = l.item { Some((title_of, copies)) } else { None });
    assert_eq!(group.map(|(t, c)| (all[t].title.as_str(), c)), Some(("m1 task", 2)));
}

#[test]
fn a_snoozed_tree_hides_unless_it_works() {
    let snooze = |r: Row| Row { snoozed_until: Some(NOW + HOUR), ..r };
    let all = view(&[snooze(row("busy", Status::Running, NOW - HOUR)), snooze(done("idle", NOW - HOUR)), fresh("plain", NOW - 2.0 * HOUR)]);
    let v = sidebar(&all, &Prefs::default(), NOW, None, Vec::new());
    assert_eq!(keys(&all, &v.threads), ["plain", "busy"]);
    assert_eq!(v.threads[1].snoozed_working, Some(1));
    assert!(v.settled.is_empty());
    assert_eq!(v.root_place("busy"), None, "a snoozed tree at the foot does not move");
    let lines = v.lines(&Folds::default());
    let Item::Tile(busy) = &lines[1].item else { panic!("busy is the second row") };
    assert_eq!(tile_slot(&all[0], busy, NOW).text.as_deref(), Some("1 working"));
}

#[test]
fn lines_head_counts_and_settled_paging() {
    let mut rows: Vec<Row> = (0..30).map(|i| done(&format!("s{i:02}"), NOW - 10.0 * HOUR - f64::from(i) * MIN)).collect();
    rows.push(Row { pinned_at: Some(NOW), ..row("pin", Status::Running, NOW) });
    let all = view(&rows);
    let v = sidebar(&all, &Prefs::default(), NOW, None, Vec::new());
    let lines = v.lines(&Folds::default());
    let heads: Vec<_> = lines
        .iter()
        .filter_map(|l| if let Item::Head { section, count, folded } = l.item { Some((section, count, folded)) } else { None })
        .collect();
    assert_eq!(heads, [(Section::Pinned, 1, false), (Section::Settled, 30, true)]);
    assert_eq!(lines.len(), 3, "Settled starts folded");
    let open = Folds { sections: HashSet::new(), ..Folds::default() };
    let lines = v.lines(&open);
    assert_eq!(tiles(&lines, &all).len(), 26);
    assert_eq!(lines.last().map(|l| &l.item), Some(&Item::More { n: 5, lead: None }));
    let to_open = Folds { open: Some("s29".into()), ..open.clone() };
    assert_eq!(tiles(&v.lines(&to_open), &all).len(), 31, "the open thread's settled tree stays on the page");
    let folded_pin = Folds { sections: HashSet::from([Section::Pinned, Section::Settled]), ..Folds::default() };
    assert_eq!(v.lines(&folded_pin).len(), 2);
}

#[test]
fn lines_nest_two_levels_then_stand_flat_with_rails() {
    let all = view(&[
        row("a", Status::Running, NOW - 5.0 * HOUR),
        child(row("b", Status::Running, NOW - 4.0 * HOUR), "a"),
        child(row("c", Status::Running, NOW - 3.0 * HOUR), "b"),
        child(row("d", Status::Running, NOW - 2.0 * HOUR), "c"),
        child(row("e", Status::Running, NOW - HOUR), "a"),
    ]);
    let lines = under_heads(sidebar(&all, &Prefs::default(), NOW, None, Vec::new()).lines(&Folds::default()));
    let drawn = tiles(&lines, &all);
    let expect: Vec<(String, u8)> = [("a", 0), ("e", 1), ("b", 1), ("c", 2), ("d", 2)].iter().map(|&(k, d)| (k.into(), d)).collect();
    assert_eq!(drawn, expect, "live children newest first, then flat at the second level");
    let at = |k: &str| &lines[drawn.iter().position(|(key, _)| key == k).unwrap()];
    assert!(!at("e").last && at("b").last);
    assert!(!at("c").last && at("d").last, "c's own children stand after it in its list");
    assert_eq!(at("c").rails, 0, "b is last, so no rail runs at depth 1 past its tree");
}

#[test]
fn a_folded_tile_counts_its_rows_and_finished_children_fold() {
    let lead = Row { folded_at: Some(NOW), ..row("lead", Status::Running, NOW - 3.0 * HOUR) };
    let fin = child(Row { read_at: None, ..done("fin", NOW - HOUR) }, "lead");
    let live = child(row("live", Status::Running, NOW - HOUR), "lead");
    let all = view(&[lead.clone(), fin.clone(), live.clone()]);
    let v = sidebar(&all, &Prefs::default(), NOW, None, Vec::new());
    let first = under_heads(v.lines(&Folds::default())).remove(0);
    assert!(matches!(first.item, Item::Tile(Tile { under: Under::Folded(2), .. })), "one live child and the Finished row");
    let all = view(&[Row { folded_at: None, ..lead }, fin, live]);
    let v = sidebar(&all, &Prefs::default(), NOW, None, Vec::new());
    let shut = v.lines(&Folds::default());
    assert_eq!(tiles(&shut, &all).len(), 2);
    assert!(shut.iter().any(|l| l.item == Item::Finished { lead: 0, count: 1, open: false } && l.depth == 1 && l.last));
    let open = Folds { finished_open: HashMap::from([("lead".into(), FINISHED_PAGE)]), ..Folds::default() };
    let lines = v.lines(&open);
    let fin_line = lines.iter().find_map(|l| if let Item::Tile(t) = &l.item { (all[t.thread].key == "fin").then_some(t) } else { None });
    assert_eq!(fin_line.map(|t| t.shape), Some(Shape::Finished));
    assert_eq!(tile_kind(&all[1], fin_line.unwrap(), NOW), Kind::Done);
}

#[test]
fn key_at_lands_above_below_between_and_rekeys() {
    assert_eq!(key_at(&[], 0, NOW).0, NOW);
    assert_eq!(key_at(&[100.0, 50.0], 0, NOW).0, 101.0);
    assert_eq!(key_at(&[100.0, 50.0], 2, NOW).0, 50.0 - BELOW_LAST_MS);
    assert_eq!(key_at(&[100.0, 50.0], 1, NOW), (75.0, vec![]));
    // The figures treeOrder.ts keyAt answers for the same keys.
    assert_eq!(key_at(&[NOW, NOW, NOW - 10.0], 1, 0.0), (1799999999996.6667, vec![(1, 1799999999993.3333)]));
    assert_eq!(key_at(&[NOW; 4], 2, 0.0), (1799999985000.0, vec![(2, 1799999970000.0), (3, 1799999955000.0)]));
    let close = [NOW + 0.0001, NOW, NOW - 0.0002, NOW - 5.0];
    assert_eq!(key_at(&close, 1, 0.0), (1799999999998.75, vec![(1, 1799999999997.5), (2, 1799999999996.25)]));
}

#[test]
fn moves_write_order_keys_and_cross_out_of_pinned() {
    let all = view(&[
        fresh("a", NOW - HOUR),
        fresh("b", NOW - 2.0 * HOUR),
        fresh("c", NOW - 3.0 * HOUR),
        Row { pinned_at: Some(NOW), ..fresh("p", NOW - 4.0 * HOUR) },
    ]);
    let v = sidebar(&all, &Prefs::default(), NOW, None, Vec::new());
    assert_eq!(v.root_place("c"), Some(RootPlace { section: Section::Threads, index: 2, count: 3 }));
    let top = v.move_root("c", Move::Top).unwrap();
    assert_eq!(top, [MoveMark { thread_ids: vec!["c".into()], marks: json!({ "order": NOW - HOUR + ABOVE_TOP_MS }) }]);
    let up = v.move_root("c", Move::Up).unwrap();
    assert_eq!(up[0].marks, json!({ "order": NOW - 1.5 * HOUR }));
    assert_eq!(v.move_root("a", Move::Up), None, "the top moves up nowhere");
    assert_eq!(v.move_root("c", Move::Down), None);
    let out = v.move_root("p", Move::To { section: Section::Threads, at: 3 }).unwrap();
    assert_eq!(out[0].marks, json!({ "pinned": false, "order": NOW - 3.0 * HOUR - BELOW_LAST_MS }));
    let pin = v.move_root("a", Move::To { section: Section::Pinned, at: 0 }).unwrap();
    assert_eq!(pin[0].marks, json!({ "pinned": NOW + ABOVE_TOP_MS }));
}

#[test]
fn subagents_stand_under_their_lead_and_count() {
    let sub = |id: &str, state| Subagent {
        id: id.into(),
        title: id.into(),
        state,
        started_at: NOW - MIN,
        ended_at: None,
        call: None,
        model: None,
        asked: None,
        failure: None,
        last_line: None,
        tokens: None,
        duration_ms: None,
    };
    let lead = Row {
        subagents: vec![sub("s1", SubagentState::Running), sub("s2", SubagentState::Done)].into(),
        ..row("lead", Status::Running, NOW - HOUR)
    };
    let all = view(&[lead]);
    let lines = under_heads(sidebar(&all, &Prefs::default(), NOW, None, Vec::new()).lines(&Folds::default()));
    assert!(matches!(lines[0].item, Item::Tile(Tile { under: Under::Open, .. })));
    assert_eq!(lines[1].item, Item::Subagent { lead: 0, index: 0, part: Part::Live });
    assert_eq!(lines[2].item, Item::Finished { lead: 0, count: 1, open: false });
    assert_eq!(subagent_kind(&all[0].subagents[1], Part::Finished), Kind::Finished);
}

#[test]
fn project_pick_keeps_children_wherever_they_run() {
    let mine = |r: Row, p: &str| Row { project_id: Some(p.into()), ..r };
    let all = view(&[
        mine(row("a", Status::Running, NOW - HOUR), "p1"),
        mine(child(row("k", Status::Running, NOW), "a"), "p2"),
        mine(row("b", Status::Running, NOW), "p2"),
    ]);
    let v = sidebar(&all, &Prefs { project: Some("p1".into()), ..Prefs::default() }, NOW, None, Vec::new());
    assert_eq!(keys(&all, &v.working), ["a"]);
    assert_eq!(keys(&all, &v.working[0].children), ["k"]);
}
