// SPDX-License-Identifier: AGPL-3.0-only
//! usage ledger (spec `usage_ledger.md`, rules U1-U14). Ported from packages/runtime/test/usage-ledger.test.ts: the
//! groups "the ledger of what was used", "work read from the logs", "what each account may still use", the first four
//! cases of "an account's banked resets" and the label case of "an account's key and label". Each test names its rule
//! and the TS line it stands for; a test with no TS line stands for a rule of usage.ts the TS file never reads.

mod rest_support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::{FutureExt, LocalBoxFuture};
use serde_json::{json, Value};

use rest_support::*;
use wsp_machines::usage::ledger::{UsageLedger, UsageLedgerOptions};
use wsp_machines::usage::{EntryTokens, LimitReading, UsageEntry, UsageSource, UsedQuery};
use wsp_store::store::{BlobMark, Store};
use wsp_wire::types::usage::{AccountRoad, HarnessLimit, UsageRange, UsageSplit, UsedAnswer};
use wsp_words::exit::HostError;
use wsp_words::usage::{parse_rate_table, RateTable};

const NOON: i64 = 1_790_683_200_000;
const HOUR: i64 = 3_600_000;
const DAY: i64 = 86_400_000;
const MIDNIGHT: i64 = NOON - 12 * HOUR;

type Prices = Rc<dyn Fn() -> LocalBoxFuture<'static, RateTable>>;

fn table() -> RateTable {
    parse_rate_table(&json!({ "gpt-5.5": { "input_cost_per_token": 5e-6, "output_cost_per_token": 3e-5, "cache_read_input_token_cost": 5e-7, "litellm_provider": "openai" } }))
}

fn prices_of(table: RateTable) -> Prices {
    Rc::new(move || {
        let t = table.clone();
        async move { t }.boxed_local()
    })
}

fn make(store: Rc<dyn Store>, clock: &Rc<TestClock>, prices: Prices, retention_days: Option<u32>) -> Rc<UsageLedger> {
    UsageLedger::new(UsageLedgerOptions { store, clock: clock.clone(), time_zone: Some("UTC".into()), prices, retention_days })
}

struct Rig {
    usage: Rc<UsageLedger>,
    store: Rc<MemStore>,
    clock: Rc<TestClock>,
}

fn ledger() -> Rig {
    ledger_with(NOON, table(), None)
}

fn ledger_with(start: i64, table: RateTable, retention_days: Option<u32>) -> Rig {
    let store = MemStore::new();
    let clock = TestClock::at(start);
    let usage = make(store.clone(), &clock, prices_of(table), retention_days);
    Rig { usage, store, clock }
}

fn s(x: &str) -> String {
    x.to_string()
}

fn m(x: &str) -> Option<String> {
    Some(x.to_string())
}

fn toks(input: f64, output: f64, cached: f64) -> Option<EntryTokens> {
    Some(EntryTokens { input: Some(input), output: Some(output), cached: Some(cached), ..Default::default() })
}

fn io(input: f64, output: f64) -> Option<EntryTokens> {
    Some(EntryTokens { input: Some(input), output: Some(output), ..Default::default() })
}

/// The TS file's `turn`: a Codex turn of the account dev@example.com on this computer.
fn turn(at: i64) -> UsageEntry {
    UsageEntry {
        at,
        agent: s("codex"),
        account: s("codex:acct_1"),
        account_label: s("dev@example.com"),
        computer: s("here"),
        project: s("p_spoo"),
        model: m("gpt-5.5"),
        turns: None,
        tokens: toks(1_000.0, 100.0, 400.0),
        cost_usd: None,
        session: None,
        source: UsageSource::Wsp,
    }
}

fn split_name(split: UsageSplit) -> String {
    serde_json::to_value(split).unwrap().as_str().unwrap().to_string()
}

fn q(range: UsageRange, split: UsageSplit) -> UsedQuery {
    UsedQuery { range, split, label: Rc::new(|split, value| format!("{}:{value}", split_name(split))), outside: None }
}

async fn used(usage: &Rc<UsageLedger>, range: UsageRange, split: UsageSplit) -> Value {
    j(&usage.used(q(range, split)).await.unwrap())
}

fn j(answer: &UsedAnswer) -> Value {
    serde_json::to_value(answer).unwrap()
}

async fn day_of(usage: &Rc<UsageLedger>, day: &str) -> Value {
    serde_json::to_value(usage.day(day).await.unwrap().expect("a day")).unwrap()
}

fn rows(v: &Value) -> Vec<Value> {
    v["rows"].as_array().unwrap().clone()
}

fn close(v: &Value, want: f64) {
    let got = v.as_f64().unwrap_or_else(|| panic!("a number, got {v}"));
    assert!((got - want).abs() < 1e-12, "{got} is not {want}");
}

fn hl(v: Value) -> HarnessLimit {
    serde_json::from_value(v).unwrap()
}

fn reading() -> Value {
    json!({ "windows": [{ "kind": "session", "usedPercent": 40, "resetsAt": NOON + HOUR }], "plan": "plus", "status": "ok" })
}

fn lr(limit: Value, computer: &str) -> LimitReading {
    LimitReading { key: s("codex:acct_1"), agent: s("codex"), label: s("dev@example.com"), road: AccountRoad::Named, computer: s(computer), limit: hl(limit), at: None }
}

async fn limits_json(usage: &Rc<UsageLedger>) -> Value {
    serde_json::to_value(usage.limits().await.unwrap()).unwrap()
}

/// A store that gives the executor a turn at every read and write, so two writers of one ledger really interleave,
/// and that fails the first `put` into one named collection.
struct YieldStore {
    inner: Rc<MemStore>,
    fail_once: RefCell<Option<String>>,
}

#[async_trait(?Send)]
impl Store for YieldStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        tokio::task::yield_now().await;
        self.inner.get(collection, id).await
    }
    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        tokio::task::yield_now().await;
        let fail = self.fail_once.borrow().as_deref() == Some(collection);
        if fail {
            *self.fail_once.borrow_mut() = None;
            return Err(HostError::new(format!("put {collection} refused")));
        }
        self.inner.put(collection, id, value).await
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
        tokio::task::yield_now().await;
        self.inner.list(collection).await
    }
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError> {
        self.inner.keys(collection).await
    }
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        tokio::task::yield_now().await;
        self.inner.delete(collection, id).await
    }
    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        tokio::task::yield_now().await;
        self.inner.get_blob(collection, id).await
    }
    async fn put_blob(&self, collection: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        tokio::task::yield_now().await;
        self.inner.put_blob(collection, id, bytes).await
    }
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), HostError> {
        tokio::task::yield_now().await;
        self.inner.delete_blob(collection, id).await
    }
    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
        self.inner.stat_blob(collection, id).await
    }
}

// ---------------------------------------------------------------------------------------------------------------
// U1 to U5: the documents, the write chain and the fold
// ---------------------------------------------------------------------------------------------------------------

// U1: no TS case; usage.ts:42-50, 273-282. One blob a day under its own text, the index beside it.
#[test]
fn files_a_turn_under_its_day_blob_and_lists_the_day_in_the_index() {
    run(async {
        let r = ledger();
        r.usage.add(turn(NOON - HOUR)).await.unwrap();
        let raw = String::from_utf8(r.store.blob("usage-days", "2026-09-29").expect("the day's blob")).unwrap();
        assert_eq!(
            raw,
            r#"{"day":"2026-09-29","rows":[{"day":"2026-09-29","hour":11,"agent":"codex","account":"codex:acct_1","computer":"here","project":"p_spoo","model":"gpt-5.5","source":"wsp","turns":1,"tokens":{"input":1000,"output":100,"cached":400,"cacheWrite":0,"reasoning":0}}]}"#
        );
        assert_eq!(r.store.doc("usage-index", "days"), Some(json!({ "days": ["2026-09-29"] })));
        assert_eq!(r.store.doc("usage-accounts", "codex:acct_1"), Some(json!({ "key": "codex:acct_1", "label": "dev@example.com" })));
        assert_eq!(r.store.blob_ids("usage-days"), vec!["2026-09-29"]);
    });
}

// U2: no TS case; usage.ts:211-216. Two writers on one day, each reading the day the other left.
#[test]
fn two_turns_at_once_both_land() {
    run(async {
        let inner = MemStore::new();
        let clock = TestClock::at(NOON);
        let store = Rc::new(YieldStore { inner: inner.clone(), fail_once: RefCell::new(None) });
        let usage = make(store, &clock, prices_of(table()), None);
        let a = UsageEntry { tokens: io(500.0, 50.0), ..turn(NOON - HOUR) };
        let b = UsageEntry { tokens: io(300.0, 30.0), ..turn(NOON - HOUR) };
        let (ra, rb, asked) = futures::join!(usage.add(a), usage.add(b), usage.used(q(UsageRange::Day, UsageSplit::Agent)));
        ra.unwrap();
        rb.unwrap();
        let day = serde_json::to_value(usage.day("2026-09-29").await.unwrap().unwrap()).unwrap();
        assert_eq!(day["rows"].as_array().unwrap().len(), 1);
        assert_eq!(day["rows"][0]["turns"], json!(2));
        assert_eq!(day["rows"][0]["tokens"], json!({ "input": 800, "output": 80, "cached": 0, "cacheWrite": 0, "reasoning": 0 }));
        let answer = j(&asked.unwrap());
        assert_eq!(rows(&answer)[0]["tokens"]["input"], json!(800), "a read asked after two writes waits for both");
    });
}

// U2: no TS case; usage.ts:212-216. A write that fails fails its own caller; the one queued behind it runs.
#[test]
fn a_failed_write_fails_its_caller_and_the_next_runs() {
    run(async {
        let inner = MemStore::new();
        let clock = TestClock::at(NOON);
        let store = Rc::new(YieldStore { inner: inner.clone(), fail_once: RefCell::new(Some(s("usage-index"))) });
        let usage = make(store, &clock, prices_of(table()), None);
        let a = UsageEntry { tokens: io(500.0, 50.0), ..turn(NOON - HOUR) };
        let b = UsageEntry { tokens: io(300.0, 30.0), ..turn(NOON - HOUR) };
        let (ra, rb) = futures::join!(usage.add(a), usage.add(b));
        assert_eq!(ra.expect_err("the first write fails").message, "put usage-index refused");
        rb.unwrap();
        let day = serde_json::to_value(usage.day("2026-09-29").await.unwrap().unwrap()).unwrap();
        assert_eq!(day["rows"][0]["turns"], json!(2), "the failed write's blob landed before its index failed, and the next folded onto it");
        assert_eq!(inner.doc("usage-index", "days"), Some(json!({ "days": ["2026-09-29"] })));
    });
}

// U3: no TS case; usage.ts:218-223. No blob, and a blob that parses but is no UsageDay, read as none.
#[test]
fn a_day_that_is_not_a_usage_day_reads_as_none() {
    run(async {
        let r = ledger();
        assert!(r.usage.day("2026-09-29").await.unwrap().is_none());
        for body in ["[]", r#"{"nope":1}"#, r#""x""#] {
            r.store.seed_blob("usage-days", "2026-09-29", body.as_bytes());
            assert!(r.usage.day("2026-09-29").await.unwrap().is_none(), "{body}");
        }
        let answer = used(&r.usage, UsageRange::Day, UsageSplit::Agent).await;
        assert_eq!(answer["rows"], json!([]), "a day that is no UsageDay counts no rows");
    });
}

// U3 and the after-the-port note: no TS case; usage.ts:219-221. A blob that is not JSON fails the call.
#[test]
fn a_day_that_is_not_json_fails_the_read() {
    run(async {
        let r = ledger();
        r.store.seed_blob("usage-days", "2026-09-29", b"not json");
        assert!(r.usage.day("2026-09-29").await.is_err());
        assert!(r.usage.used(q(UsageRange::Day, UsageSplit::Agent)).await.is_err(), "every used over that day fails");
    });
}

// U3: no TS case; usage.ts:224-227. Strings only, in the order held; anything else reads empty.
#[test]
fn an_index_that_is_not_a_list_reads_empty() {
    run(async {
        let r = ledger();
        assert_eq!(r.usage.days().await.unwrap(), Vec::<String>::new());
        r.store.seed("usage-index", "days", json!({ "days": "nope" }));
        assert_eq!(r.usage.days().await.unwrap(), Vec::<String>::new());
        r.store.seed("usage-index", "days", json!({ "other": ["2026-09-29"] }));
        assert_eq!(r.usage.days().await.unwrap(), Vec::<String>::new());
        r.store.seed("usage-index", "days", json!({ "days": ["2026-09-30", 3, "x"] }));
        assert_eq!(r.usage.days().await.unwrap(), vec!["2026-09-30", "x"]);
    });
}

// U4: usage-ledger.test.ts:50. Two turns of one key are one row; the stored row is the fold's own literal.
#[test]
fn files_a_turn_under_its_day_and_hour_and_two_turns_of_one_key_are_one_row() {
    run(async {
        let r = ledger();
        r.usage.add(turn(NOON - HOUR)).await.unwrap();
        r.usage.add(UsageEntry { tokens: io(500.0, 50.0), ..turn(NOON - HOUR + 60_000) }).await.unwrap();
        let day = day_of(&r.usage, "2026-09-29").await;
        assert_eq!(
            day,
            json!({ "day": "2026-09-29", "rows": [
                { "day": "2026-09-29", "hour": 11, "agent": "codex", "account": "codex:acct_1", "computer": "here", "project": "p_spoo", "model": "gpt-5.5", "turns": 2, "tokens": { "input": 1500, "output": 150, "cached": 400, "cacheWrite": 0, "reasoning": 0 }, "source": "wsp" }
            ] })
        );
        // usage.ts:254, 269: a turn count of its own is kept (0 stays 0), and no model files under "".
        r.usage.add(UsageEntry { model: None, turns: Some(0.0), ..turn(NOON - HOUR) }).await.unwrap();
        let day = day_of(&r.usage, "2026-09-29").await;
        assert_eq!(day["rows"].as_array().unwrap().len(), 2);
        assert_eq!(day["rows"][1]["model"], json!(""));
        assert_eq!(day["rows"][1]["turns"], json!(0));
        assert_eq!(day["rows"][1]["tokens"]["input"], json!(1000));
    });
}

// U4: usage-ledger.test.ts:60. Bare tokens are kept apart only once the row holds a reported cost.
#[test]
fn keeps_bare_tokens_apart_only_once_the_row_holds_a_reported_cost() {
    run(async {
        let list = 600.0 * 5e-6 + 400.0 * 5e-7 + 100.0 * 3e-5;
        for order in [["bare", "reported", "bare"], ["reported", "bare", "bare"]] {
            let r = ledger();
            for (i, kind) in order.iter().enumerate() {
                let entry = UsageEntry { cost_usd: if *kind == "reported" { Some(0.5) } else { None }, ..turn(NOON - HOUR + i as i64 * 60_000) };
                r.usage.add(entry).await.unwrap();
                let day = day_of(&r.usage, "2026-09-29").await;
                if i == 0 {
                    assert!(day["rows"][0].get("unreported").is_none(), "a row of bare tokens alone carries no apart count ({order:?})");
                }
            }
            let day = day_of(&r.usage, "2026-09-29").await;
            let row = &day["rows"][0];
            assert_eq!(row["turns"], json!(3), "{order:?}");
            assert_eq!(row["costReported"], json!(0.5), "{order:?}");
            assert_eq!(row["unreported"], json!({ "input": 2000, "output": 200, "cached": 800, "cacheWrite": 0, "reasoning": 0 }), "{order:?}");
            let answer = used(&r.usage, UsageRange::Day, UsageSplit::Agent).await;
            let line = &rows(&answer)[0];
            assert_eq!(line["costReported"], json!(0.5));
            close(&line["costList"], 2.0 * list);
        }
    });
}

// U4: no TS case; usage.ts:269. A wsp turn's session is noted once, appended; a log row's is not noted.
#[test]
fn notes_a_wsp_turns_session_once() {
    run(async {
        let r = ledger();
        let with = |session: Option<&str>, source| UsageEntry { session: session.map(s), source, ..turn(NOON) };
        r.usage.add(with(Some("s1"), UsageSource::Wsp)).await.unwrap();
        r.usage.add(with(Some("s1"), UsageSource::Wsp)).await.unwrap();
        r.usage.add(with(Some("s2"), UsageSource::Log)).await.unwrap();
        r.usage.add(with(None, UsageSource::Wsp)).await.unwrap();
        let day = day_of(&r.usage, "2026-09-29").await;
        assert_eq!(day["sessions"], json!(["s1"]));
        r.usage.add(with(Some("s3"), UsageSource::Wsp)).await.unwrap();
        let day = day_of(&r.usage, "2026-09-29").await;
        assert_eq!(day["sessions"], json!(["s1", "s3"]));
    });
}

// U5: usage-ledger.test.ts:212. The oldest documents go as a new day is filed.
#[test]
fn keeps_ninety_days_of_documents_the_oldest_dropped_as_a_new_day_is_filed() {
    run(async {
        let r = ledger_with(NOON - 95 * DAY, table(), None);
        r.usage.add(turn(NOON - 95 * DAY)).await.unwrap();
        r.usage.add(turn(NOON - 94 * DAY)).await.unwrap();
        assert_eq!(r.usage.days().await.unwrap(), vec!["2026-06-26", "2026-06-27"]);
        r.clock.advance(95 * DAY);
        r.usage.add(turn(NOON)).await.unwrap();
        assert!(r.usage.day("2026-06-26").await.unwrap().is_none());
        assert!(r.usage.day("2026-06-27").await.unwrap().is_none());
        assert!(r.usage.day("2026-09-29").await.unwrap().is_some());
        assert_eq!(r.usage.days().await.unwrap(), vec!["2026-09-29"]);
        assert_eq!(r.store.blob_ids("usage-days"), vec!["2026-09-29"]);
        assert_eq!(r.store.doc("usage-index", "days"), Some(json!({ "days": ["2026-09-29"] })));
    });
}

// U5 and U1: no TS case; usage.ts:277-281, 209. A turn older than the retention is written and deleted in one write;
// `retention_days` replaces the ninety.
#[test]
fn a_turn_older_than_retention_leaves_no_day() {
    run(async {
        let r = ledger();
        r.usage.add(turn(NOON - 95 * DAY)).await.unwrap();
        assert!(r.usage.day("2026-06-26").await.unwrap().is_none());
        assert_eq!(r.usage.days().await.unwrap(), Vec::<String>::new());
        assert_eq!(
            r.store.calls(),
            vec!["put usage-accounts/codex:acct_1", "putBlob usage-days/2026-06-26", "deleteBlob usage-days/2026-06-26", "put usage-index/days"]
        );

        let r = ledger_with(NOON, table(), Some(2));
        r.usage.add(turn(NOON - DAY)).await.unwrap();
        r.usage.add(turn(NOON - 2 * DAY)).await.unwrap();
        r.usage.add(turn(NOON)).await.unwrap();
        assert_eq!(r.usage.days().await.unwrap(), vec!["2026-09-28", "2026-09-29"]);
        assert_eq!(r.store.blob_ids("usage-days"), vec!["2026-09-28", "2026-09-29"]);
    });
}

// U6: usage-ledger.test.ts:414. One label per key, the last given, written only where it differs.
#[test]
fn keeps_the_label_each_account_was_filed_under() {
    run(async {
        let r = ledger();
        r.usage.add(UsageEntry { account: s("codex:maya@example.com"), account_label: s("maya@example.com"), ..turn(NOON) }).await.unwrap();
        let log = UsageEntry {
            account: s("claude@here"),
            account_label: s("Claude Code on zingzy's MacBook Pro"),
            agent: s("claude"),
            source: UsageSource::Log,
            ..turn(NOON)
        };
        r.usage.file_logs(vec![log], vec![s("here")]).await.unwrap();
        let labels = r.usage.account_labels().await.unwrap();
        assert_eq!(
            serde_json::to_value(&labels).unwrap(),
            json!({ "codex:maya@example.com": "maya@example.com", "claude@here": "Claude Code on zingzy's MacBook Pro" })
        );
        assert_eq!(r.store.doc("usage-accounts", "claude@here"), Some(json!({ "key": "claude@here", "label": "Claude Code on zingzy's MacBook Pro" })));

        // usage.ts:285-288: a label already held is not written again; the last of two for one key is the one kept.
        let before = r.store.calls().iter().filter(|c| c.starts_with("put usage-accounts/")).count();
        r.usage.add(UsageEntry { account: s("codex:maya@example.com"), account_label: s("maya@example.com"), ..turn(NOON) }).await.unwrap();
        let after = r.store.calls().iter().filter(|c| c.starts_with("put usage-accounts/")).count();
        assert_eq!(after, before);
        let named = |label: &str| UsageEntry { account: s("x@y"), account_label: s(label), source: UsageSource::Log, ..turn(NOON) };
        r.usage.file_logs(vec![named("first"), named("second")], vec![s("here")]).await.unwrap();
        assert_eq!(r.store.doc("usage-accounts", "x@y"), Some(json!({ "key": "x@y", "label": "second" })));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// U8: work read from the logs
// ---------------------------------------------------------------------------------------------------------------

// U8: usage-ledger.test.ts:226. A session a wsp turn ran under is skipped; a second read replaces the first.
#[test]
fn skips_a_session_a_wsp_turn_was_filed_under_and_a_second_read_replaces_the_first() {
    run(async {
        let r = ledger();
        let claude = |session: &str, source, input, output| UsageEntry {
            agent: s("claude"),
            model: m("claude-opus-5"),
            session: m(session),
            source,
            tokens: io(input, output),
            ..turn(NOON)
        };
        r.usage.add(claude("s-wsp", UsageSource::Wsp, 100.0, 10.0)).await.unwrap();
        let logs = vec![
            claude("s-wsp", UsageSource::Log, 100.0, 10.0),
            claude("s-outside", UsageSource::Log, 7.0, 3.0),
            UsageEntry { at: NOON - 100 * DAY, ..claude("s-old", UsageSource::Log, 1.0, 1.0) },
        ];
        r.usage.file_logs(logs.clone(), vec![s("here")]).await.unwrap();
        r.usage.file_logs(logs, vec![s("here")]).await.unwrap();
        let answer = used(&r.usage, UsageRange::Day, UsageSplit::Agent).await;
        let got: Vec<Value> = rows(&answer).iter().map(|r| json!([r["key"], r["tokens"]["input"], r["tokens"]["output"]])).collect();
        assert_eq!(got, vec![json!(["claude", 107, 13])], "the wsp turn once and the outside session once, in one row");
        let day = day_of(&r.usage, "2026-09-29").await;
        let stored: Vec<Value> = day["rows"].as_array().unwrap().iter().map(|r| json!([r["source"], r["turns"], r["tokens"]["input"]])).collect();
        assert_eq!(stored, vec![json!(["wsp", 1, 100]), json!(["log", 1, 7])]);
        assert_eq!(day["sessions"], json!(["s-wsp"]), "a log row's session is never noted");
        assert_eq!(r.usage.days().await.unwrap(), vec!["2026-09-29"], "a session older than the retention files no day");
    });
}

// U8: usage-ledger.test.ts:240. A computer's rows are replaced only when it was read.
#[test]
fn replaces_each_computers_rows_only_when_that_computer_was_read() {
    run(async {
        let r = ledger();
        let log = |computer: &str, session: &str, input: f64| UsageEntry {
            agent: s("claude"),
            model: m("claude-opus-5"),
            computer: s(computer),
            session: m(session),
            source: UsageSource::Log,
            tokens: io(input, 1.0),
            ..turn(NOON)
        };
        r.usage.file_logs(vec![log("here", "s-here", 10.0), log("pl_spoo", "s-spoo", 20.0)], vec![s("here"), s("pl_spoo")]).await.unwrap();
        r.usage.file_logs(vec![log("here", "s-here", 15.0)], vec![s("here")]).await.unwrap();
        let by_computer = || async {
            let answer = used(&r.usage, UsageRange::Day, UsageSplit::Computer).await;
            rows(&answer).iter().map(|r| json!([r["key"], r["tokens"]["input"]])).collect::<Vec<_>>()
        };
        assert_eq!(by_computer().await, vec![json!(["pl_spoo", 20]), json!(["here", 15])]);
        r.usage.file_logs(vec![], vec![s("pl_spoo")]).await.unwrap();
        assert_eq!(by_computer().await, vec![json!(["here", 15])]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// U9 to U11: reading the ledger
// ---------------------------------------------------------------------------------------------------------------

// U9: usage-ledger.test.ts:38. The table is read at the first row counted, once, never for an empty range.
#[test]
fn reads_the_rate_table_only_for_a_range_with_a_row_once() {
    run(async {
        let asked = Rc::new(Cell::new(0u32));
        let counter = asked.clone();
        let prices: Prices = Rc::new(move || {
            counter.set(counter.get() + 1);
            let t = table();
            async move { t }.boxed_local()
        });
        let store = MemStore::new();
        let clock = TestClock::at(NOON);
        let usage = make(store, &clock, prices, None);
        let empty = used(&usage, UsageRange::Week, UsageSplit::Agent).await;
        assert_eq!(empty["rows"], json!([]));
        assert_eq!(asked.get(), 0);
        usage.add(UsageEntry { cost_usd: Some(0.5), ..turn(NOON) }).await.unwrap();
        usage.add(UsageEntry { project: s("p_other"), ..turn(NOON) }).await.unwrap();
        let answer = used(&usage, UsageRange::Week, UsageSplit::Agent).await;
        assert_eq!(asked.get(), 1);
        assert_eq!(rows(&answer).len(), 1);
        assert_eq!(rows(&answer)[0]["tokens"]["input"], json!(2000));
        assert_eq!(rows(&answer)[0]["costReported"], json!(0.5));
    });
}

// U9: usage-ledger.test.ts:78. A day, a week and a month reach back over their own days and no further.
#[test]
fn folds_a_day_a_week_and_a_month_over_their_own_days() {
    run(async {
        let r = ledger();
        for back in [0, 3, 6, 7, 29, 30] {
            r.usage.add(UsageEntry { tokens: io(100.0, 10.0), ..turn(NOON - back * DAY) }).await.unwrap();
        }
        let total = |range| {
            let usage = r.usage.clone();
            async move {
                let answer = used(&usage, range, UsageSplit::Agent).await;
                rows(&answer).iter().map(|r| r["tokens"]["input"].as_f64().unwrap()).sum::<f64>()
            }
        };
        assert_eq!(total(UsageRange::Day).await, 100.0);
        assert_eq!(total(UsageRange::Week).await, 300.0);
        assert_eq!(total(UsageRange::Month).await, 500.0);
    });
}

// U9: usage-ledger.test.ts:125. One sign-in across two computers is one row, a computer's own login another.
#[test]
fn splits_by_account() {
    run(async {
        let r = ledger();
        let on = |account: &str, computer: &str, cost: f64| UsageEntry {
            agent: s("claude"),
            account: s(account),
            computer: s(computer),
            model: m("claude-opus-5"),
            cost_usd: Some(cost),
            ..turn(NOON)
        };
        r.usage.add(on("claude:vault-token", "here", 0.2)).await.unwrap();
        r.usage.add(on("claude:vault-token", "pl_boat", 0.3)).await.unwrap();
        r.usage.add(on("claude@pl_spoo", "pl_spoo", 0.1)).await.unwrap();
        let answer = used(&r.usage, UsageRange::Day, UsageSplit::Account).await;
        let got: Vec<Value> = rows(&answer).iter().map(|r| json!([r["key"], r["label"], r["tokens"]["input"], r["costReported"]])).collect();
        assert_eq!(
            got,
            vec![json!(["claude:vault-token", "account:claude:vault-token", 2000, 0.5]), json!(["claude@pl_spoo", "account:claude@pl_spoo", 1000, 0.1])]
        );
    });
}

// U9: usage-ledger.test.ts:137. One model files under each agent; the agent is named on three splits only.
#[test]
fn files_one_model_under_each_agent_and_names_the_agent_on_three_splits() {
    run(async {
        let r = ledger();
        r.usage
            .add(UsageEntry { agent: s("claude"), account: s("claude:vault-token"), model: m("claude-sonnet-4-5"), tokens: io(3_000.0, 10.0), ..turn(NOON) })
            .await
            .unwrap();
        r.usage
            .add(UsageEntry { agent: s("hermes"), account: s("hermes@here"), model: m("claude-sonnet-4-5[1m]"), tokens: io(2_000.0, 10.0), ..turn(NOON) })
            .await
            .unwrap();
        let agent_of = |split| {
            let usage = r.usage.clone();
            async move {
                let answer = used(&usage, UsageRange::Day, split).await;
                rows(&answer).iter().map(|r| json!([r["key"], r["label"], r.get("agent").cloned().unwrap_or(Value::Null)])).collect::<Vec<_>>()
            }
        };
        assert_eq!(
            agent_of(UsageSplit::Model).await,
            vec![json!(["claude:claude-sonnet-4-5", "model:claude-sonnet-4-5", "claude"]), json!(["hermes:claude-sonnet-4-5", "model:claude-sonnet-4-5", "hermes"])]
        );
        assert_eq!(
            agent_of(UsageSplit::Account).await,
            vec![json!(["claude:vault-token", "account:claude:vault-token", "claude"]), json!(["hermes@here", "account:hermes@here", "hermes"])]
        );
        assert_eq!(agent_of(UsageSplit::Agent).await, vec![json!(["claude", "agent:claude", "claude"]), json!(["hermes", "agent:hermes", "hermes"])]);
        assert_eq!(agent_of(UsageSplit::Computer).await, vec![json!(["here", "computer:here", null])]);
        assert_eq!(agent_of(UsageSplit::Project).await, vec![json!(["p_spoo", "project:p_spoo", null])]);
    });
}

// U9: usage-ledger.test.ts:158. The harness's own cost stays; bare tokens price at list; an unpriced model says so.
#[test]
fn keeps_the_harnesss_cost_prices_the_bare_tokens_and_says_what_it_could_not_price() {
    run(async {
        let r = ledger();
        r.usage.add(UsageEntry { agent: s("claude"), model: m("claude-opus-5"), cost_usd: Some(0.42), ..turn(NOON) }).await.unwrap();
        r.usage.add(UsageEntry { agent: s("codex"), model: m("gpt-5.5"), tokens: toks(1_000.0, 100.0, 400.0), ..turn(NOON) }).await.unwrap();
        r.usage.add(UsageEntry { agent: s("opencode"), model: m("some-new-model"), tokens: io(10.0, 1.0), ..turn(NOON) }).await.unwrap();
        let answer = used(&r.usage, UsageRange::Day, UsageSplit::Agent).await;
        let by_key = |key: &str| rows(&answer).into_iter().find(|r| r["key"] == json!(key)).unwrap_or_else(|| panic!("no row {key}"));
        let claude = by_key("claude");
        assert_eq!(claude["costReported"], json!(0.42));
        assert_eq!(claude["priced"], json!(true));
        assert!(claude.get("costList").is_none(), "the harness's figure wins over the table's");
        let codex = by_key("codex");
        close(&codex["costList"], 600.0 * 5e-6 + 400.0 * 5e-7 + 100.0 * 3e-5);
        assert_eq!(codex["priced"], json!(true));
        let opencode = by_key("opencode");
        assert_eq!(opencode["priced"], json!(false));
        assert!(opencode.get("costList").is_none());
    });
}

// U9: usage-ledger.test.ts:173. The whole token mix, wsp's own turns, and the list price of every token.
#[test]
fn returns_the_token_mix_wsps_turns_and_the_list_price() {
    run(async {
        let table = parse_rate_table(&json!({ "claude-opus-5-5": { "input_cost_per_token": 4e-6, "output_cost_per_token": 2e-5, "cache_read_input_token_cost": 2e-7, "cache_creation_input_token_cost": 5e-6 } }));
        let r = ledger_with(NOON, table, None);
        r.usage
            .add(UsageEntry {
                agent: s("claude"),
                model: m("claude-opus-5-5[1m]"),
                cost_usd: Some(9.0),
                tokens: Some(EntryTokens { input: Some(10_000.0), output: Some(100.0), cached: Some(6_000.0), cache_write: Some(3_000.0), reasoning: Some(40.0) }),
                ..turn(NOON)
            })
            .await
            .unwrap();
        r.usage
            .add(UsageEntry {
                agent: s("claude"),
                model: m("claude-opus-5-5"),
                source: UsageSource::Log,
                tokens: Some(EntryTokens { input: Some(1_000.0), output: Some(10.0), cached: Some(0.0), cache_write: Some(0.0), reasoning: None }),
                ..turn(NOON)
            })
            .await
            .unwrap();
        let answer = used(&r.usage, UsageRange::Day, UsageSplit::Agent).await;
        let row = &rows(&answer)[0];
        assert_eq!(row["tokens"], json!({ "input": 11000, "output": 110, "cached": 6000, "cacheWrite": 3000, "reasoning": 40 }));
        assert_eq!(row["turns"], json!(1), "a turn is one wsp ran; the logs keep sessions, not turns");
        assert_eq!(row["costReported"], json!(9));
        close(&row["costList"], 1_000.0 * 4e-6 + 10.0 * 2e-5);
        close(&row["estimate"], 1_000.0 * 4e-6 + 6_000.0 * 2e-7 + 3_000.0 * 5e-6 + 100.0 * 2e-5 + 1_000.0 * 4e-6 + 10.0 * 2e-5);
        close(&row["saved"], 6_000.0 * (4e-6 - 2e-7));
    });
}

// U9: usage-ledger.test.ts:197. Logs count in the same row as wsp's turns, and the answer says whose and where.
#[test]
fn counts_log_rows_with_wsps_and_says_whose_logs() {
    run(async {
        let r = ledger();
        r.usage.add(UsageEntry { agent: s("claude"), model: m("claude-opus-5"), ..turn(NOON) }).await.unwrap();
        r.usage.add(UsageEntry { agent: s("claude"), model: m("claude-opus-5"), source: UsageSource::Log, ..turn(NOON) }).await.unwrap();
        r.usage.add(UsageEntry { agent: s("codex"), source: UsageSource::Log, computer: s("pl_spoo"), ..turn(NOON) }).await.unwrap();
        let answer = used(&r.usage, UsageRange::Day, UsageSplit::Agent).await;
        let got: Vec<Value> = rows(&answer).iter().map(|r| json!([r["key"], r["tokens"]["input"]])).collect();
        assert_eq!(got, vec![json!(["claude", 2000]), json!(["codex", 1000])]);
        assert_eq!(answer["logs"], json!({ "agents": ["agent:claude", "agent:codex"], "computers": ["computer:here", "computer:pl_spoo"] }));
        let without = r.usage.used(UsedQuery { outside: Some(false), ..q(UsageRange::Day, UsageSplit::Agent) }).await.unwrap();
        let without = j(&without);
        let got: Vec<Value> = rows(&without).iter().map(|r| json!([r["key"], r["tokens"]["input"]])).collect();
        assert_eq!(got, vec![json!(["claude", 1000])]);
        assert!(without.get("logs").is_none());
    });
}

// U10: usage-ledger.test.ts:87. Hours for a day, days for a week, a step with nothing at 0.
#[test]
fn draws_the_range_as_a_series() {
    run(async {
        let r = ledger();
        r.usage.add(UsageEntry { tokens: io(100.0, 20.0), ..turn(NOON - 2 * HOUR) }).await.unwrap();
        r.usage.add(UsageEntry { tokens: io(30.0, 0.0), ..turn(NOON - 2 * DAY) }).await.unwrap();
        let day = used(&r.usage, UsageRange::Day, UsageSplit::Agent).await;
        let series = day["series"].as_array().unwrap();
        assert_eq!(series.len(), 24);
        assert_eq!(series[10], json!({ "t": NOON - 2 * HOUR, "tokens": 120 }));
        assert_eq!(series.iter().filter(|p| p["tokens"].as_f64().unwrap() > 0.0).count(), 1);
        assert_eq!(series[0]["t"], json!(MIDNIGHT));
        let week = used(&r.usage, UsageRange::Week, UsageSplit::Agent).await;
        let tokens: Vec<Value> = week["series"].as_array().unwrap().iter().map(|p| p["tokens"].clone()).collect();
        assert_eq!(tokens, vec![json!(0), json!(0), json!(0), json!(0), json!(30), json!(0), json!(120)]);
        assert_eq!(week["since"], json!(MIDNIGHT - 6 * DAY));
        assert_eq!(week["until"], json!(MIDNIGHT + DAY), "the range ends where today does, not at the moment it was read");
    });
}

// U10: usage-ledger.test.ts:102. Each split value's own line on the series' steps, adding up to the series.
#[test]
fn draws_each_split_values_line_on_the_series_steps() {
    run(async {
        let r = ledger();
        r.usage.add(UsageEntry { agent: s("claude"), tokens: io(100.0, 20.0), ..turn(NOON - 2 * HOUR) }).await.unwrap();
        r.usage.add(UsageEntry { agent: s("codex"), tokens: io(30.0, 5.0), ..turn(NOON) }).await.unwrap();
        r.usage.add(UsageEntry { agent: s("codex"), tokens: io(40.0, 0.0), ..turn(NOON - 2 * DAY) }).await.unwrap();
        let day = used(&r.usage, UsageRange::Day, UsageSplit::Agent).await;
        let lines = day["lines"].as_array().unwrap();
        let shape: Vec<Value> = lines.iter().map(|l| json!([l["key"], l["label"], l["points"].as_array().unwrap().len()])).collect();
        assert_eq!(shape, vec![json!(["claude", "agent:claude", 24]), json!(["codex", "agent:codex", 24])]);
        assert_eq!(lines[0]["points"][10], json!(120));
        assert_eq!(lines[1]["points"][12], json!(35));
        let week = used(&r.usage, UsageRange::Week, UsageSplit::Agent).await;
        let points: Vec<Value> = week["lines"].as_array().unwrap().iter().map(|l| l["points"].clone()).collect();
        assert_eq!(points, vec![json!([0, 0, 0, 0, 0, 0, 120]), json!([0, 0, 0, 0, 40, 0, 35])]);
        let series: Vec<f64> = week["series"].as_array().unwrap().iter().map(|p| p["tokens"].as_f64().unwrap()).collect();
        let summed: Vec<f64> = (0..7).map(|i| week["lines"].as_array().unwrap().iter().map(|l| l["points"][i].as_f64().unwrap()).sum()).collect();
        assert_eq!(series, summed);
    });
}

// U11: usage-ledger.test.ts:116. A value that used nothing keeps its row at zero and draws no line.
#[test]
fn draws_no_line_for_a_value_that_used_nothing() {
    run(async {
        let r = ledger();
        let claude = |account: &str, computer: &str, tokens| UsageEntry {
            agent: s("claude"),
            account: s(account),
            computer: s(computer),
            model: m("claude-opus-5"),
            tokens,
            ..turn(NOON)
        };
        r.usage.add(claude("claude@here", "here", toks(1_000.0, 100.0, 400.0))).await.unwrap();
        r.usage.add(claude("claude@pl_hetzner", "pl_hetzner", Some(EntryTokens::default()))).await.unwrap();
        let answer = used(&r.usage, UsageRange::Month, UsageSplit::Account).await;
        let got: Vec<Value> = rows(&answer).iter().map(|r| json!([r["key"], r["tokens"]["input"].as_f64().unwrap() + r["tokens"]["output"].as_f64().unwrap()])).collect();
        assert_eq!(got, vec![json!(["claude@here", 1100.0]), json!(["claude@pl_hetzner", 0.0])]);
        let keys: Vec<Value> = answer["lines"].as_array().unwrap().iter().map(|l| l["key"].clone()).collect();
        assert_eq!(keys, vec![json!("claude@here")]);
    });
}

// U11: usage-ledger.test.ts:188. The answer's keys are the spec's and carry no threads and no counts.
#[test]
fn names_no_threads_and_counts_none() {
    run(async {
        let r = ledger();
        r.usage.add(UsageEntry { tokens: toks(600.0, 0.0, 0.0), ..turn(NOON) }).await.unwrap();
        r.usage.add(UsageEntry { source: UsageSource::Log, session: m("s-log"), tokens: toks(1_000.0, 0.0, 0.0), ..turn(NOON) }).await.unwrap();
        let answer = used(&r.usage, UsageRange::Day, UsageSplit::Agent).await;
        let mut keys: Vec<&str> = answer.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort();
        assert_eq!(keys, vec!["lines", "logs", "range", "rows", "series", "since", "split", "until"]);
        assert!(answer.get("threads").is_none());
        assert!(answer.get("counts").is_none());
    });
}

// U11: no TS case; usage.ts:396. Equal tokens sort by key in the root collation, not by code unit.
#[test]
fn ties_sort_by_key() {
    run(async {
        let r = ledger();
        for agent in ["beta", "Zed", "alpha"] {
            r.usage.add(UsageEntry { agent: s(agent), account: format!("{agent}@here"), tokens: io(10.0, 1.0), ..turn(NOON) }).await.unwrap();
        }
        let answer = used(&r.usage, UsageRange::Day, UsageSplit::Agent).await;
        let keys: Vec<Value> = rows(&answer).iter().map(|r| r["key"].clone()).collect();
        assert_eq!(keys, vec![json!("alpha"), json!("beta"), json!("Zed")]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// U12 to U14: what each account may still use
// ---------------------------------------------------------------------------------------------------------------

// U12: usage-ledger.test.ts:260. The last reading and every computer a reading came from; the ledger untouched.
#[test]
fn keeps_an_accounts_last_reading_and_every_computer() {
    run(async {
        let r = ledger();
        r.usage.limit(lr(reading(), "here")).await.unwrap();
        r.clock.advance(60_000);
        let second = hl(json!({ "windows": [{ "kind": "session", "usedPercent": 55 }], "plan": "plus", "status": "ok" }));
        r.usage.limit(LimitReading { limit: second, ..lr(reading(), "pl_boat") }).await.unwrap();
        let expected = json!([{
            "key": "codex:acct_1", "agent": "codex", "label": "dev@example.com", "road": "named", "plan": "plus",
            "windows": [{ "kind": "session", "usedPercent": 55 }], "status": "ok", "readAt": NOON + 60_000, "computers": ["here", "pl_boat"]
        }]);
        assert_eq!(limits_json(&r.usage).await, expected);
        assert_eq!(r.store.doc("limits", "codex:acct_1"), Some(expected[0].clone()));
        assert_eq!(r.usage.days().await.unwrap(), Vec::<String>::new());
        assert!(r.store.blob_ids("usage-days").is_empty());
        let labels = r.usage.account_labels().await.unwrap();
        assert_eq!(serde_json::to_value(&labels).unwrap(), json!({ "codex:acct_1": "dev@example.com" }));
    });
}

// U12: usage-ledger.test.ts:271. Each reading is answered with the one it replaced.
#[test]
fn answers_each_reading_with_the_one_it_replaced() {
    run(async {
        let r = ledger();
        let first = r.usage.limit(lr(reading(), "here")).await.unwrap();
        assert!(first.before.is_none());
        let next = hl(json!({ "windows": [{ "kind": "session", "usedPercent": 75 }], "plan": "plus", "status": "ok" }));
        let second = r.usage.limit(LimitReading { limit: next, ..lr(reading(), "here") }).await.unwrap();
        assert_eq!(second.before.as_ref(), Some(&first.after));
        assert_eq!(serde_json::to_value(&second.after.windows).unwrap(), json!([{ "kind": "session", "usedPercent": 75 }]));
    });
}

// U12: usage-ledger.test.ts:280. A reading taken earlier is kept at its own moment, never over a newer one.
#[test]
fn keeps_a_reading_taken_earlier_only_over_an_older_one() {
    run(async {
        let r = ledger();
        let file = |used_percent: i64, at: Option<i64>| {
            let usage = r.usage.clone();
            async move {
                usage
                    .limit(LimitReading {
                        key: s("codex@pl_spoo"),
                        agent: s("codex"),
                        label: s("Codex signed in on spoo"),
                        road: AccountRoad::Own,
                        computer: s("pl_spoo"),
                        limit: hl(json!({ "windows": [{ "kind": "session", "usedPercent": used_percent }] })),
                        at,
                    })
                    .await
                    .unwrap()
            }
        };
        file(30, Some(NOON - 600_000)).await;
        let held = limits_json(&r.usage).await;
        assert_eq!(held[0]["windows"], json!([{ "kind": "session", "usedPercent": 30 }]));
        assert_eq!(held[0]["readAt"], json!(NOON - 600_000));
        file(50, None).await;
        let writes = r.store.calls().len();
        let older = file(20, Some(NOON - 60_000)).await;
        assert_eq!(older.before.as_ref(), Some(&older.after));
        assert_eq!(r.store.calls().len(), writes, "an older reading writes nothing, no label either");
        let held = limits_json(&r.usage).await;
        assert_eq!(held[0]["windows"], json!([{ "kind": "session", "usedPercent": 50 }]));
        assert_eq!(held[0]["readAt"], json!(NOON));
    });
}

// U12: usage-ledger.test.ts:291. What a reading leaves out stays: the plan a later reading does not name.
#[test]
fn keeps_the_plan_a_later_reading_leaves_out() {
    run(async {
        let r = ledger();
        r.usage.limit(lr(reading(), "here")).await.unwrap();
        r.usage.limit(lr(json!({ "windows": [{ "kind": "week", "usedPercent": 3 }] }), "here")).await.unwrap();
        let held = limits_json(&r.usage).await;
        assert_eq!(held[0]["plan"], json!("plus"));
        assert_eq!(held[0]["status"], json!("ok"));
        assert_eq!(held[0]["windows"], json!([{ "kind": "week", "usedPercent": 3 }]));
        r.usage.limit(lr(json!({ "windows": [] }), "here")).await.unwrap();
        let held = limits_json(&r.usage).await;
        assert_eq!(held[0]["windows"], json!([{ "kind": "week", "usedPercent": 3 }]), "a reading with no windows keeps the held ones");
    });
}

fn full() -> Value {
    json!({ "count": 2, "credits": [
        { "id": "rc_1", "status": "available", "expiresAt": NOON + 21 * DAY },
        { "id": "rc_2", "status": "available", "expiresAt": NOON + 30 * DAY }
    ] })
}

fn credits_reading(credits: Option<Value>, keyed: Option<bool>) -> Value {
    let mut v = json!({ "windows": [{ "kind": "session", "usedPercent": 40 }] });
    if let Some(c) = credits {
        v["credits"] = c;
    }
    if let Some(k) = keyed {
        v["keyed"] = json!(k);
    }
    v
}

async fn held_credits(usage: &Rc<UsageLedger>) -> Value {
    limits_json(usage).await[0].get("credits").cloned().unwrap_or(Value::Null)
}

fn with(base: Value, extra: Value) -> Value {
    let mut out = base;
    for (k, v) in extra.as_object().unwrap() {
        out[k] = v.clone();
    }
    out
}

// U13: usage-ledger.test.ts:305. A full reading's resets with when they were read, and a same-count reading keeps them.
#[test]
fn keeps_a_full_readings_resets_and_a_same_count_reading_keeps_them() {
    run(async {
        let r = ledger();
        r.usage.limit(lr(credits_reading(Some(full()), None), "here")).await.unwrap();
        assert_eq!(held_credits(&r.usage).await, with(full(), json!({ "readAt": NOON, "detailAt": NOON })));
        r.clock.advance(60_000);
        r.usage.limit(lr(credits_reading(Some(json!({ "count": 2 })), None), "here")).await.unwrap();
        assert_eq!(held_credits(&r.usage).await, with(full(), json!({ "readAt": NOON + 60_000, "detailAt": NOON })));
    });
}

// U13: usage-ledger.test.ts:314. A count that no longer matches drops the details, so the next full read is due.
#[test]
fn drops_details_a_count_no_longer_matches() {
    run(async {
        let r = ledger();
        r.usage.limit(lr(credits_reading(Some(full()), None), "here")).await.unwrap();
        r.clock.advance(60_000);
        r.usage.limit(lr(credits_reading(Some(json!({ "count": 1 })), None), "here")).await.unwrap();
        assert_eq!(held_credits(&r.usage).await, json!({ "count": 1, "readAt": NOON + 60_000 }));
    });
}

// U13: usage-ledger.test.ts:322. A reading that carries no resets at all leaves what is held alone.
#[test]
fn leaves_resets_alone_for_a_reading_without_any() {
    run(async {
        let r = ledger();
        r.usage.limit(lr(credits_reading(Some(full()), None), "here")).await.unwrap();
        r.clock.advance(60_000);
        r.usage.limit(lr(credits_reading(None, None), "here")).await.unwrap();
        assert_eq!(held_credits(&r.usage).await, with(full(), json!({ "readAt": NOON, "detailAt": NOON })));
        let held = limits_json(&r.usage).await;
        assert_eq!(held[0]["readAt"], json!(NOON + 60_000), "the reading itself is still taken");
    });
}

// U13: usage-ledger.test.ts:330. A key banks none.
#[test]
fn holds_none_for_a_keyed_account() {
    run(async {
        let r = ledger();
        r.usage.limit(lr(credits_reading(Some(json!({ "count": 2 })), Some(true)), "here")).await.unwrap();
        assert_eq!(held_credits(&r.usage).await, Value::Null);
        let held = limits_json(&r.usage).await;
        assert_eq!(held[0]["keyed"], json!(true));
    });
}

// U14: no TS case; usage.ts:414-415, 439-443. A reading that does not parse is none, and `limits` skips it.
#[test]
fn limits_skip_a_reading_that_does_not_parse() {
    run(async {
        let r = ledger();
        r.store.seed("limits", "junk", json!({ "key": "junk" }));
        r.store.seed(
            "limits",
            "claude@here",
            json!({ "key": "claude@here", "agent": "claude", "label": "c", "road": "own", "windows": [], "readAt": NOON, "computers": ["here"] }),
        );
        let keys = |v: Value| v.as_array().unwrap().iter().map(|l| l["key"].clone()).collect::<Vec<_>>();
        assert_eq!(keys(limits_json(&r.usage).await), vec![json!("claude@here")]);
        let kept = r
            .usage
            .limit(LimitReading {
                key: s("junk"),
                agent: s("codex"),
                label: s("j"),
                road: AccountRoad::Own,
                computer: s("here"),
                limit: hl(json!({ "windows": [{ "kind": "session", "usedPercent": 3 }] })),
                at: None,
            })
            .await
            .unwrap();
        assert!(kept.before.is_none());
        assert_eq!(
            serde_json::to_value(&kept.after).unwrap(),
            json!({ "key": "junk", "agent": "codex", "label": "j", "road": "own", "windows": [{ "kind": "session", "usedPercent": 3 }], "readAt": NOON, "computers": ["here"] })
        );
        assert_eq!(keys(limits_json(&r.usage).await), vec![json!("junk"), json!("claude@here")], "in store order");
    });
}

// Restart: no TS case; usage-ledger.md "Restart". Everything is in the store; a new ledger over it answers the same.
#[test]
fn a_new_ledger_over_the_same_store_reads_every_day_back() {
    run(async {
        let r = ledger();
        r.usage.add(turn(NOON)).await.unwrap();
        r.usage.add(UsageEntry { agent: s("claude"), model: m("claude-opus-5"), cost_usd: Some(0.3), session: m("s1"), ..turn(NOON - DAY) }).await.unwrap();
        r.usage.limit(lr(credits_reading(Some(full()), None), "here")).await.unwrap();
        let again = make(r.store.clone(), &r.clock, prices_of(table()), None);
        assert_eq!(again.days().await.unwrap(), vec!["2026-09-28", "2026-09-29"]);
        assert_eq!(day_of(&again, "2026-09-28").await, day_of(&r.usage, "2026-09-28").await);
        assert_eq!(day_of(&again, "2026-09-29").await, day_of(&r.usage, "2026-09-29").await);
        assert_eq!(day_of(&again, "2026-09-28").await["sessions"], json!(["s1"]));
        for split in [UsageSplit::Agent, UsageSplit::Account] {
            let before = used(&r.usage, UsageRange::Month, split).await;
            assert_eq!(rows(&before).len(), 2);
            assert_eq!(used(&again, UsageRange::Month, split).await, before);
        }
        assert_eq!(limits_json(&again).await, limits_json(&r.usage).await);
        assert_eq!(held_credits(&again).await, with(full(), json!({ "readAt": NOON, "detailAt": NOON })));
        assert_eq!(
            serde_json::to_value(again.account_labels().await.unwrap()).unwrap(),
            serde_json::to_value(r.usage.account_labels().await.unwrap()).unwrap()
        );
    });
}
