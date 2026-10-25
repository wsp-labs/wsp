// SPDX-License-Identifier: AGPL-3.0-only
//! The pieces a slate is read at a glance by, drawn against a host started for this binary alone, at the
//! narrow panel's 400 px and a widened 640: a ring and a meter beside numbers as tiles of one strip, progress, choices
//! as a titled pick card, the stat row's narrow layouts, a decision, and the gallery's renderer findings.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "slate/fixture.rs"]
mod fixture;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use common::{shared, Throwaway};
use desktop_gpui::screens::slate::SlateScreen;
use desktop_gpui::screens::{Host, Opened, Screen};
use desktop_gpui::ui::{self, Side, Still};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{prelude::*, px, size, Bounds, Entity, Pixels, TestAppContext, VisualTestContext};
use wsp_host_client::HostClient;

fn bench() -> (&'static Throwaway, &'static PathBuf) {
    static FOLDER: OnceLock<PathBuf> = OnceLock::new();
    let host = &shared().host;
    (host, FOLDER.get_or_init(|| fixture::project(host)))
}

/// The panel at `width` on a fresh thread holding `slate`, drawn with every formula answered and its widths measured.
fn open<'a>(cx: &'a mut TestAppContext, slate: &str, width: f32) -> &'a mut VisualTestContext {
    open_screen(cx, slate, width).1
}

/// `open`, keeping the screen to read the slate's values from.
fn open_screen<'a>(cx: &'a mut TestAppContext, slate: &str, width: f32) -> (Entity<SlateScreen>, &'a mut VisualTestContext) {
    let (host, folder) = bench();
    let thread = fixture::staged(host, folder, &fixture::State { name: "glance", file: None });
    fixture::write(host, &thread, slate);
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        cx.set_global(Still(true));
    });
    cx.executor().allow_parking();
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let handle = Host::new(client, runtime);
    let opened = Opened { thread_id: Some(thread), workspace_id: None, tab: Some("slate".into()), arg: None };
    let mut made: Option<Entity<SlateScreen>> = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| SlateScreen::new(handle.clone(), opened, window, cx));
        made = Some(screen.clone());
        Root::new(screen, window, cx)
    });
    cx.simulate_resize(size(px(width), px(900.)));
    let screen = made.unwrap();
    let limit = Instant::now() + Duration::from_secs(30);
    while !cx.read(|cx| screen.read(cx).slate().is_some_and(|s| s.formulas().iter().all(|f| s.resolved.contains_key(f)))) {
        assert!(Instant::now() < limit, "the slate never drew with its formulas answered");
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    // A layout that reads its own width draws again on the next frame.
    for _ in 0..4 {
        cx.update(|window, cx| window.render_frame(cx));
        cx.run_until_parked();
    }
    (screen, cx)
}

fn value_of(cx: &mut VisualTestContext, screen: &Entity<SlateScreen>, path: &str) -> Option<serde_json::Value> {
    cx.read(|cx| screen.read(cx).slate().and_then(|s| s.value(path).cloned()))
}

fn wait_for(
    cx: &mut VisualTestContext,
    screen: &Entity<SlateScreen>,
    what: &str,
    done: impl Fn(&mut VisualTestContext, &Entity<SlateScreen>) -> bool,
) {
    let limit = Instant::now() + Duration::from_secs(30);
    loop {
        cx.update(|window, cx| window.render_frame(cx));
        cx.run_until_parked();
        if done(cx, screen) {
            return;
        }
        assert!(Instant::now() < limit, "never: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn click_at(cx: &mut VisualTestContext, at: gpui_kit::Point<Pixels>) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.simulate_click(at, gpui_kit::Modifiers::none());
    cx.run_until_parked();
}

fn at(cx: &mut VisualTestContext, selector: &str) -> Bounds<Pixels> {
    cx.debug_bounds(Box::leak(selector.to_owned().into_boxed_str())).unwrap_or_else(|| panic!("{selector} is drawn"))
}

fn drawn(cx: &mut VisualTestContext, selector: &str) -> bool {
    cx.debug_bounds(Box::leak(selector.to_owned().into_boxed_str())).is_some()
}

fn level(a: Pixels, b: Pixels) -> bool {
    (a - b).abs() < px(0.5)
}

const STRIP: &str = r#"<slate title="Strip">
  <column>
    <row id="r">
      <number id="n" label="Tests" value={209} />
      <ring id="g" label="Passing" value={209} max={214} format="fraction" />
      <meter id="m" label="Disk" value={62} note="of 1 TB" />
    </row>
  </column>
</slate>
"#;

#[gpui_kit::test]
fn a_ring_and_a_meter_beside_numbers_draw_as_tiles_of_one_strip(cx: &mut TestAppContext) {
    let cx = open(cx, STRIP, 640.);
    assert!(drawn(cx, "slate-strip"), "the three figures stand in one strip");
    assert!(drawn(cx, "slate-g-tile-ring") && drawn(cx, "slate-m-tile-ring"), "the ring and the meter draw their share as a ring");
    assert!(!drawn(cx, "slate-fill"), "no meter track in a strip");
    let figures = [at(cx, "slate-n-figure"), at(cx, "slate-g-tile-figure"), at(cx, "slate-m-tile-figure")];
    assert!(figures.iter().all(|f| level(f.top(), figures[0].top())), "the figures share one line: {figures:?}");
    let ring = at(cx, "slate-g-tile-ring");
    assert!(level(ring.left() - figures[1].right(), px(10.)), "the ring sits 10 px right of its figure: {ring:?} {:?}", figures[1]);
}

const PROGRESS: &str = r#"<slate title="Progress">
  <column>
    <progress id="p" label="screens in the app" items={[{ n: "Settings", in: true }, { n: "Usage", in: true }, { n: "Slate", in: false, why: "strip tiles" }, { n: "Thread", in: true }, { n: "Browser", in: false }]} key={item.n} name={item.n} done={item.in} note={item.why} />
  </column>
</slate>
"#;

#[gpui_kit::test]
fn progress_counts_every_row_and_lists_only_the_rows_not_done(cx: &mut TestAppContext) {
    let cx = open(cx, PROGRESS, 400.);
    assert!(drawn(cx, "slate-progress-3/5"), "three of the five rows are done");
    assert!(drawn(cx, "slate-progress-label"), "the label stands beside the count");
    assert!(drawn(cx, "slate-p-open-Slate") && drawn(cx, "slate-p-open-Browser"), "the two rows not done are listed");
    assert!(!drawn(cx, "slate-p-open-Usage"), "a done row is not");
    let open = [at(cx, "slate-p-open-Slate"), at(cx, "slate-p-open-Browser")];
    assert!(open.iter().all(|r| r.size.height >= px(44.)), "each row not done keeps the 44 px floor: {open:?}");
    assert!(open[0].bottom() <= open[1].top(), "in their order");
    let fill = at(cx, "slate-progress-fill");
    let count = at(cx, "slate-progress-count");
    assert!(fill.top() > count.bottom(), "the track stands under the count");
    assert!(!drawn(cx, "slate-progress-more"));
}

#[gpui_kit::test]
fn progress_with_every_row_done_says_all_and_lists_nothing(cx: &mut TestAppContext) {
    let all = PROGRESS.replace("in: false, why: \"strip tiles\"", "in: true").replace("in: false", "in: true");
    let cx = open(cx, &all, 400.);
    assert!(drawn(cx, "slate-progress-5/5"));
    assert!(!drawn(cx, "slate-p-open-Slate"));
}

#[gpui_kit::test]
fn progress_with_no_rows_says_none_yet_alone(cx: &mut TestAppContext) {
    let none = PROGRESS.replace(r#"items={[{ n: "Settings", in: true }, { n: "Usage", in: true }, { n: "Slate", in: false, why: "strip tiles" }, { n: "Thread", in: true }, { n: "Browser", in: false }]}"#, "items={[]}");
    assert_ne!(none, PROGRESS);
    let cx = open(cx, &none, 400.);
    assert!(drawn(cx, "slate-progress-0/0"));
    assert!(!drawn(cx, "slate-progress-label"), "None yet, not None yet screens in the app");
}

#[gpui_kit::test]
fn progress_past_eight_rows_not_done_says_how_many_more(cx: &mut TestAppContext) {
    let rows: Vec<String> = (1..=11).map(|i| format!("{{ n: \"step {i}\", in: {} }}", i == 1)).collect();
    let slate = format!(
        "<slate title=\"Many\">\n  <column>\n    <progress id=\"p\" label=\"steps\" items={{[{}]}} key={{item.n}} name={{item.n}} done={{item.in}} />\n  </column>\n</slate>\n",
        rows.join(", ")
    );
    let cx = open(cx, &slate, 400.);
    assert!(drawn(cx, "slate-progress-1/11"));
    assert!(drawn(cx, "slate-p-open-step 9") && !drawn(cx, "slate-p-open-step 10"), "eight rows not done are listed");
    assert!(drawn(cx, "slate-progress-more"), "and the two past them are counted");
}

const CHOICES: &str = r#"<slate title="Quiz">
  <value name="pick" start={null} />
  <column>
    <choices id="c" label="Which layer routes packets?" value={$pick} options={["Link", "Network", "Transport"]} answer="Network" />
  </column>
</slate>
"#;

#[gpui_kit::test]
fn choices_stand_as_a_title_over_a_pick_card_of_their_own(cx: &mut TestAppContext) {
    let cx = open(cx, CHOICES, 400.);
    let title = at(cx, "slate-choices-title");
    let card = at(cx, "slate-choices-card");
    assert!(title.bottom() <= card.top(), "the question stands over its card, not as its first row: {title:?} {card:?}");
    let row = at(cx, "slate-choice-open");
    assert!(card.contains(&row.origin), "the answers are the card's rows");
    assert!(row.size.height >= px(48.), "each answer is a 48 px row: {row:?}");
    let radio = at(cx, "slate-choice-radio");
    assert!(level(radio.left() - card.left(), px(17.)), "the radio leads the row at the card's inset: {radio:?} {card:?}");
}

fn stats(n: usize) -> String {
    let all = [
        r#"<stat label="Checks passing on the main branch today" value={212} max={214} window="this push" />"#,
        r#"<stat label="Spent" value={4.64} format="usd" window="this week, every thread on every computer" />"#,
        r#"<stat label="Open pull requests" value={7} window="across three repos" />"#,
        r#"<stat label="Deploys" value={3} window="today" />"#,
    ];
    format!(
        "<slate title=\"Stats\">\n  <column>\n    <stats id=\"s\">\n      {}\n    </stats>\n  </column>\n</slate>\n",
        all[..n].join("\n      ")
    )
}

fn labels_on_one_line(cx: &mut VisualTestContext, n: usize) {
    for i in 0..n {
        let label = at(cx, &format!("slate-s-stat-{i}-label"));
        assert_eq!(label.size.height, px(20.), "label {i} keeps to one line");
        if let Some(window) = cx.debug_bounds(Box::leak(format!("slate-s-stat-{i}-window").into_boxed_str())) {
            assert_eq!(window.size.height, px(16.), "window {i} keeps to one line");
        }
    }
}

#[gpui_kit::test]
fn three_stats_stand_across_at_640_on_one_baseline(cx: &mut TestAppContext) {
    let cx = open(cx, &stats(3), 640.);
    labels_on_one_line(cx, 3);
    let figures: Vec<_> = (0..3).map(|i| at(cx, &format!("slate-s-stat-{i}-figure"))).collect();
    assert!(figures.iter().all(|f| level(f.top(), figures[0].top())), "one baseline across: {figures:?}");
    assert!(figures[0].right() < figures[1].left() && figures[1].right() < figures[2].left(), "side by side");
    let ring = at(cx, "slate-s-stat-0-ring");
    assert!(level(ring.left() - figures[0].right(), px(10.)), "the ring sits beside its figure");
}

#[gpui_kit::test]
fn three_stats_under_480_stand_one_a_line(cx: &mut TestAppContext) {
    let cx = open(cx, &stats(3), 400.);
    labels_on_one_line(cx, 3);
    let labels: Vec<_> = (0..3).map(|i| at(cx, &format!("slate-s-stat-{i}-label"))).collect();
    let figures: Vec<_> = (0..3).map(|i| at(cx, &format!("slate-s-stat-{i}-figure"))).collect();
    for i in 0..3 {
        assert!(figures[i].left() > labels[i].left(), "tile {i}: the label at the left, the figure at the right");
        if i > 0 {
            assert!(labels[i].top() >= figures[i - 1].bottom(), "tile {i} stands under tile {}", i - 1);
        }
    }
    let ring = at(cx, "slate-s-stat-0-ring");
    assert!(level(ring.left() - figures[0].right(), px(10.)), "the ring stays beside its figure");
}

#[gpui_kit::test]
fn four_stats_under_480_stand_two_a_row_each_row_on_one_baseline(cx: &mut TestAppContext) {
    let cx = open(cx, &stats(4), 400.);
    labels_on_one_line(cx, 4);
    let figures: Vec<_> = (0..4).map(|i| at(cx, &format!("slate-s-stat-{i}-figure"))).collect();
    assert!(level(figures[0].top(), figures[1].top()) && level(figures[2].top(), figures[3].top()), "{figures:?}");
    assert!(figures[2].top() > figures[0].bottom(), "two rows");
}

const DECISION: &str = r#"<slate title="Ship">
  <value name="ship" start="" />
  <value name="n" start={0} />
  <column>
    <decision id="d" label="Ship it now?" value={$ship} pick="now" onChange={set($n, $n + 1)}>
      <option value="now" label="Now" note="Checks are green" />
      <option value="later" label="Later" note="Waits for the e2e run" />
    </decision>
  </column>
</slate>
"#;

#[gpui_kit::test]
fn a_decision_says_every_answers_note(cx: &mut TestAppContext) {
    let cx = open(cx, DECISION, 400.);
    assert!(drawn(cx, "slate-decision-open"));
    assert!(drawn(cx, "slate-answer-note"), "the other answer's note stands under the pick, not in a tooltip");
}

#[gpui_kit::test]
fn a_decision_marks_the_agents_pick_on_its_answer(cx: &mut TestAppContext) {
    let cx = open(cx, DECISION, 400.);
    let decision = at(cx, "slate-decision-open");
    let mine = at(cx, "slate-answer-mine");
    // A selector drawn twice reads as the last drawn: Later, the answer after the pick.
    let later = at(cx, "button");
    assert!(
        level(mine.left(), decision.left()) && mine.right() < later.left(),
        "the pick, Now, the first answer, carries the mark: {mine:?} {later:?}"
    );
}

const DECIDE: &str = r#"<slate title="Ship">
  <value name="ship" start="" />
  <value name="n" start={0} />
  <column>
    <decision id="d" label="Ship it now?" value={$ship} onChange={set($n, $n + 1)}>
      <option value="now" label="Now" />
    </decision>
  </column>
</slate>
"#;

#[gpui_kit::test]
fn change_clears_a_decision_and_raises_no_change_for_the_empty_answer(cx: &mut TestAppContext) {
    let (screen, cx) = open_screen(cx, DECIDE, 400.);
    // The one answer, then Change: the only button on the page each time.
    let now = at(cx, "button");
    click_at(cx, now.center());
    wait_for(cx, &screen, "Now answered and change raised once", |cx, s| {
        drawn(cx, "slate-decision-answered") && value_of(cx, s, "$n") == Some(serde_json::json!(1))
    });
    let change = at(cx, "button");
    click_at(cx, change.center());
    wait_for(cx, &screen, "the answer cleared", |cx, s| {
        value_of(cx, s, "$ship") == Some(serde_json::json!("")) && drawn(cx, "slate-decision-open")
    });
    for _ in 0..25 {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(value_of(cx, &screen, "$n"), Some(serde_json::json!(1)), "clearing the answer sends nothing, as the web's decision does");
}

const HELD_DECISION: &str = r#"<slate title="Ship">
  <value name="ship" start="" />
  <column>
    <decision id="d" label="Ship it now?" value={$ship} held="Checks are still running" onChange={send("Ship", $ship)}>
      <option value="now" label="Now" />
      <option value="later" label="Later" />
    </decision>
  </column>
</slate>
"#;

fn held_decision_says_why(cx: &mut TestAppContext, start: &str, state: &str) {
    let slate = HELD_DECISION.replace(r#"start="""#, start);
    let cx = open(cx, &slate, 400.);
    let decision = at(cx, &format!("slate-decision-{state}"));
    assert!(drawn(cx, "slate-held-why"), "{state}: the reason a decision is held is drawn, not left to a hover");
    let why = at(cx, "slate-held-why");
    let answers = at(cx, "button");
    assert!(decision.contains(&why.origin) && why.top() >= answers.bottom(), "{state}: under the decision's answers: {why:?} {answers:?}");
}

#[gpui_kit::test]
fn a_held_decision_says_why_on_screen_as_a_held_button_does(cx: &mut TestAppContext) {
    held_decision_says_why(cx, r#"start="""#, "open");
}

#[gpui_kit::test]
fn a_held_decision_once_answered_still_says_why_beside_its_change(cx: &mut TestAppContext) {
    held_decision_says_why(cx, r#"start="now""#, "answered");
}

const SMALL_CHART: &str = r#"<slate title="Errors">
  <column>
    <chart id="ch" label="Errors" height="small" items={[{ t: 1, v: 3 }, { t: 2, v: 9 }, { t: 3, v: 4 }]} x={item.t} value={item.v} />
  </column>
</slate>
"#;

#[gpui_kit::test]
fn a_small_chart_keeps_its_small_height_in_a_widened_panel(cx: &mut TestAppContext) {
    let cx = open(cx, SMALL_CHART, 640.);
    let plot = at(cx, "chart-plot");
    assert!(plot.size.height <= px(120.), "a small chart's plot fits its 120 px at 640: {plot:?}");
}

const STRIP_AFTER_NUMBER: &str = r#"<slate title="Strip">
  <column>
    <heading id="h" level="section">Build</heading>
    <number id="n" label="Queue" value={3} />
    <status id="st" tone="warning">slow</status>
    <row id="g">
      <number id="a" label="Passed" value={209} />
      <number id="b" label="Failed" value={5} />
    </row>
  </column>
</slate>
"#;

#[gpui_kit::test]
fn a_strip_after_a_number_and_its_status_is_a_card_of_its_own(cx: &mut TestAppContext) {
    let cx = open(cx, STRIP_AFTER_NUMBER, 400.);
    let strip = at(cx, "slate-strip");
    // The last card drawn holds the strip: its own card starts at the strip, the number's card starts above it.
    let card = at(cx, "card");
    assert!(card.contains(&strip.origin) && strip.top() - card.top() < px(4.), "the strip is a card of its own: {strip:?} {card:?}");
}

const CHART: &str = r#"<slate title="Errors">
  <column>
    <chart id="first" label="Requests" items={[{ t: 1, v: 3 }, { t: 2, v: 5 }, { t: 3, v: 4 }]} x={item.t} value={item.v} />
    <chart id="ch" label="Errors" items={[{ t: 1, v: 3 }, { t: 2, v: 9 }, { t: 3, v: 4 }]} x={item.t} value={item.v} tone="bad" />
  </column>
</slate>
"#;

#[gpui_kit::test]
fn a_chart_draws_the_tone_it_names_past_the_slates_one_accent(cx: &mut TestAppContext) {
    let cx = open(cx, CHART, 640.);
    assert!(drawn(cx, "slate-first-line-accent"), "the first chart naming no tone carries the accent");
    assert!(drawn(cx, "slate-ch-line-bad"), "a second chart naming bad draws bad, not muted grey");
    let plot = at(cx, "chart-plot");
    assert!(plot.size.width > px(400.), "the plot spans its box rather than collapse to its gutter: {plot:?}");
}

const STATUS: &str = r#"<slate title="Deploy">
  <column>
    <heading id="h" level="section">Deploy</heading>
    <status id="st" tone="good">Live</status>
  </column>
</slate>
"#;

#[gpui_kit::test]
fn a_status_alone_stands_bare_rather_than_fill_a_card(cx: &mut TestAppContext) {
    let cx = open(cx, STATUS, 400.);
    let heading = at(cx, "slate-piece-h-heading");
    let status = at(cx, "slate-piece-st-status");
    assert!(level(status.left(), heading.left()), "the state word stands at the page's edge, in no card: {status:?} {heading:?}");
}

const TABLE: &str = r#"<slate title="Files">
  <column>
    <table id="t" items={[{ p: "apps/desktop-gpui/app/src/screens/slate/pieces/numbers.rs", a: "tiles", n: 96 }, { p: "apps/web/src/slate/pieces/tile.tsx", a: "one tile", n: 83 }]} key={item.p}>
      <col title="File" value={item.p} mono />
      <col title="Change" value={item.a} />
      <col title="Lines" value={item.n} />
    </table>
  </column>
</slate>
"#;

#[gpui_kit::test]
fn a_table_under_its_longest_word_folds_named_by_its_mono_first_column(cx: &mut TestAppContext) {
    let cx = open(cx, TABLE, 400.);
    assert!(drawn(cx, "slate-table-folded"), "at 400 px the path would break mid-word, so the table folds");
    assert!(drawn(cx, "slate-table-name-0"), "the mono column of paths names each row");
}

#[gpui_kit::test]
fn a_wide_table_keeps_its_header_inside_its_card(cx: &mut TestAppContext) {
    let cx = open(cx, TABLE, 760.);
    let table = at(cx, "slate-table");
    let head = at(cx, "slate-table-head");
    assert!(head.top() > table.top(), "the header is the card's first row, inside its edge: {head:?} {table:?}");
}

const FACTS: &str = r#"<slate title="Branch">
  <column>
    <facts id="f">
      <fact label="Branch" value="feat/2468-gpui-slate" mono />
      <fact label="Review" value="Approved" tone="good" />
      <fact label="Checks" value="24/24" />
    </facts>
  </column>
</slate>
"#;

#[gpui_kit::test]
fn facts_at_760_share_one_line(cx: &mut TestAppContext) {
    let wide = open(cx, FACTS, 760.);
    let one_line = at(wide, "slate-facts").size.height;
    assert!(one_line <= px(24.), "wide, the facts share a line: {one_line:?}");
}

#[gpui_kit::test]
fn facts_at_400_stand_one_a_line(cx: &mut TestAppContext) {
    let narrow = open(cx, FACTS, 400.);
    let lines = at(narrow, "slate-facts").size.height;
    assert!(lines >= px(3. * 16. + 2. * 6.), "three facts, one a line: {lines:?}");
}

const FACTS_IN_ROW: &str = r#"<slate title="Checkout incident">
  <column>
    <row id="r" wrap>
      <status tone="warning">Investigating</status>
      <facts id="f" layout="line">
        <fact label="Started" value="14:10" emphasis="strong" />
        <fact label="Scope" value="Checkout API" />
        <fact label="Impact" value="Customers receive 502 during checkout" tone="bad" />
      </facts>
    </row>
  </column>
</slate>
"#;

#[gpui_kit::test]
fn facts_in_a_row_at_760_share_one_line(cx: &mut TestAppContext) {
    // Their own width in a row is their content's, under 480 px, which must not turn them one a line.
    let wide = open(cx, FACTS_IN_ROW, 760.);
    let one_line = at(wide, "slate-facts").size.height;
    assert!(one_line <= px(24.), "in a row the facts share a line: {one_line:?}");
}
