// SPDX-License-Identifier: AGPL-3.0-only
//! Spending a banked reset: which computer it runs on, what is refused before anything runs there, the read that
//! checks the login before the spend, the key kept across a spend nobody answered, and the words each answer says.
//! Ported from packages/runtime/test/usage-reset.test.ts at 65f6b89c7 (19 cases), plus the cases spec/usage_reset.md
//! pins beyond them. Every sentence is the TS test's own text.
#[path = "support/base.rs"]
mod base;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use async_trait::async_trait;
use base::{run, MemStore};
use futures::channel::oneshot;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_account::usage_reset::{
    usage_resets, FileAsk, PlaceOf, ResetAsk, ResetPlace, ResetPlaceKind, Resets, UsageResetDeps, UsageResets, PENDING,
};
use wsp_adapters::port::{ResetReading, ResetRoad, ResetSpend};
use wsp_core::types::pending::Store;
use wsp_wire::types::usage::{AccountLimit, AccountRow, HarnessLimit, ResetAnswer, ResetOutcome};
use wsp_words::exit::{HostError, Kind};

const KEY: &str = "codex:acct_a";

fn named(over: Value) -> AccountLimit {
    let mut v = json!({ "key": KEY, "agent": "codex", "label": "a@example.com", "road": "named", "plan": "plus",
        "windows": [{ "kind": "session", "usedPercent": 100 }], "readAt": 1, "computers": ["pl_spoo"] });
    base::merge(&mut v, over);
    serde_json::from_value(v).unwrap()
}

fn limit(count: Option<u64>, id: &str, label: Option<&str>) -> HarnessLimit {
    let mut v = json!({ "windows": [{ "kind": "session", "usedPercent": 100 }], "account": { "id": id } });
    if let Some(l) = label {
        v["account"]["label"] = json!(l);
    }
    if let Some(c) = count {
        v["credits"] = json!({ "count": c, "credits": [] });
    }
    serde_json::from_value(v).unwrap()
}

fn reading(count: Option<u64>) -> ResetReading {
    ResetReading { keyed: false, limit: Some(limit(count, "acct_a", Some("a@example.com"))) }
}

fn reading_as(count: Option<u64>, id: &str, label: Option<&str>) -> ResetReading {
    ResetReading { keyed: false, limit: Some(limit(count, id, label)) }
}

fn spent(outcome: &str, left: Option<u64>) -> ResetSpend {
    let limit = left.map(|l| {
        serde_json::from_value(json!({ "windows": [{ "kind": "session", "usedPercent": 0 }], "credits": { "count": l } })).unwrap()
    });
    ResetSpend::Outcome { outcome: outcome.to_owned(), limit }
}

fn place(id: &str, name: &str, kind: ResetPlaceKind, connected: bool, signed_in: bool) -> ResetPlace {
    ResetPlace { id: id.to_owned(), name: name.to_owned(), kind, connected, signed_in: Rc::new(move |_| signed_in) }
}

fn places() -> PlaceOf {
    Rc::new(|id: &str| match id {
        "here" => place("here", "Mac", ResetPlaceKind::Here, true, true),
        "pl_spoo" => place("pl_spoo", "spoo", ResetPlaceKind::Box, true, true),
        "pl_boat" => place("pl_boat", "Boat", ResetPlaceKind::Box, false, true),
        "pl_out" => place("pl_out", "Out", ResetPlaceKind::Box, true, false),
        "solari" => place("solari", "solari", ResetPlaceKind::Provider, true, true),
        other => place(other, other, ResetPlaceKind::Box, false, false),
    })
}

/// A spend's answer: a reply, a failed run, or one held until the test releases it.
enum Spend {
    Reply(ResetSpend),
    Fails(&'static str),
    Held(oneshot::Receiver<ResetSpend>),
}

struct Scripts {
    reads: RefCell<VecDeque<Option<ResetReading>>>,
}

impl Resets for Scripts {
    fn read_command(&self, road: &ResetRoad) -> Result<String, String> {
        if !road.home.starts_with('/') {
            return Err(format!("home must be an absolute path, got \"{}\"", road.home));
        }
        Ok(format!("READ {}", road.home))
    }
    fn parse_read(&self, _stdout: &str) -> Option<ResetReading> {
        self.reads.borrow_mut().pop_front().flatten()
    }
    fn spend_command(&self, _road: &ResetRoad, key: &str, credit_id: Option<&str>) -> Result<String, String> {
        if credit_id.is_some_and(|c| c.contains('\'')) {
            return Err(format!("creditId must be a plain slug, got \"{}\"", credit_id.unwrap()));
        }
        Ok(format!("SPEND {key} {}", credit_id.unwrap_or("-")))
    }
    fn parse_spend(&self, stdout: &str) -> ResetSpend {
        let v: Value = serde_json::from_str(stdout).unwrap();
        match v["kind"].as_str().unwrap() {
            "unanswered" => ResetSpend::Unanswered,
            "refused" => {
                ResetSpend::Refused { refused: v["refused"].as_str().unwrap().to_owned(), too_old: v["tooOld"].as_bool().unwrap() }
            }
            _ => ResetSpend::Outcome {
                outcome: v["outcome"].as_str().unwrap().to_owned(),
                limit: v.get("limit").filter(|l| !l.is_null()).map(|l| serde_json::from_value(l.clone()).unwrap()),
            },
        }
    }
    fn too_old(&self) -> String {
        "is older than 0.141 and has no resets".to_owned()
    }
}

fn spend_json(s: &ResetSpend) -> String {
    match s {
        ResetSpend::Unanswered => json!({ "kind": "unanswered" }).to_string(),
        ResetSpend::Refused { refused, too_old } => json!({ "kind": "refused", "refused": refused, "tooOld": too_old }).to_string(),
        ResetSpend::Outcome { outcome, limit } => json!({ "kind": "outcome", "outcome": outcome, "limit": limit }).to_string(),
    }
}

/// The deps over a store and the answers each script gets, recording every script run and every reading filed.
struct Deps {
    store: Rc<MemStore>,
    limits: Vec<AccountLimit>,
    rows: Vec<AccountRow>,
    scripts: Rc<Scripts>,
    spends: RefCell<VecDeque<Spend>>,
    runs: RefCell<Vec<(String, String)>>,
    filed: RefCell<Vec<Option<u64>>>,
    minted: RefCell<u32>,
    relative_home: Cell<bool>,
}

#[async_trait(?Send)]
impl UsageResetDeps for Deps {
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    async fn limits(&self) -> Result<Vec<AccountLimit>, HostError> {
        Ok(self.limits.clone())
    }
    fn agent_name(&self, agent: &str) -> String {
        if agent == "codex" {
            "Codex".to_owned()
        } else {
            agent.to_owned()
        }
    }
    fn resets(&self, agent: &str) -> Option<Rc<dyn Resets>> {
        (agent == "codex").then(|| self.scripts.clone() as Rc<dyn Resets>)
    }
    async fn places(&self) -> Result<PlaceOf, HostError> {
        Ok(places())
    }
    async fn road(&self, _agent: &str, place: &ResetPlace) -> Result<ResetRoad, HostError> {
        let root = if self.relative_home.get() { "" } else { "/" };
        Ok(ResetRoad { home: format!("{root}logins/{}/codex", place.id), env: None, launch: None })
    }
    async fn run(&self, place: &ResetPlace, script: &str, _agent: &str) -> Result<String, HostError> {
        self.runs.borrow_mut().push((place.id.clone(), script.to_owned()));
        if !script.starts_with("SPEND") {
            return Ok(String::new());
        }
        let next = self.spends.borrow_mut().pop_front();
        match next {
            None => Ok(spend_json(&spent("reset", Some(1)))),
            Some(Spend::Reply(s)) => Ok(spend_json(&s)),
            Some(Spend::Fails(why)) => Err(base::plain(why)),
            Some(Spend::Held(rx)) => Ok(spend_json(&rx.await.unwrap())),
        }
    }
    async fn file(&self, ask: FileAsk<'_>) -> Result<String, HostError> {
        self.filed.borrow_mut().push(ask.limit.credits.as_ref().map(|c| c.count.as_f64() as u64));
        Ok(KEY.to_owned())
    }
    async fn row(&self, key: &str) -> Result<Option<AccountRow>, HostError> {
        if let Some(r) = self.rows.iter().find(|r| r.key == key) {
            return Ok(Some(r.clone()));
        }
        Ok((key == KEY).then(|| {
            serde_json::from_value(json!({ "key": key, "agent": "codex", "label": "Codex with ChatGPT Plus", "computers": ["spoo"] }))
                .unwrap()
        }))
    }
    fn uuid(&self) -> String {
        *self.minted.borrow_mut() += 1;
        format!("key-{}", self.minted.borrow())
    }
}

struct Harness {
    deps: Rc<Deps>,
    reset: UsageResets,
}

impl Harness {
    fn new(limits: Option<Vec<AccountLimit>>, rows: Vec<AccountRow>, reads: Vec<Option<ResetReading>>, spends: Vec<Spend>) -> Harness {
        Harness::over(MemStore::new(), limits, rows, reads, spends)
    }
    fn over(
        store: Rc<MemStore>,
        limits: Option<Vec<AccountLimit>>,
        rows: Vec<AccountRow>,
        reads: Vec<Option<ResetReading>>,
        spends: Vec<Spend>,
    ) -> Harness {
        let deps = Rc::new(Deps {
            store,
            limits: limits.unwrap_or_else(|| vec![named(json!({}))]),
            rows,
            scripts: Rc::new(Scripts { reads: RefCell::new(reads.into()) }),
            spends: RefCell::new(spends.into()),
            runs: RefCell::new(vec![]),
            filed: RefCell::new(vec![]),
            minted: RefCell::new(0),
            relative_home: Cell::new(false),
        });
        let reset = usage_resets(deps.clone());
        Harness { deps, reset }
    }
    async fn ask(&self, account: &str) -> Result<ResetAnswer, HostError> {
        self.reset.reset(ResetAsk { account: account.to_owned(), credit_id: None, on: None }).await
    }
    async fn ask_on(&self, account: &str, on: &str) -> Result<ResetAnswer, HostError> {
        self.reset.reset(ResetAsk { account: account.to_owned(), credit_id: None, on: Some(on.to_owned()) }).await
    }
    fn places_run(&self) -> Vec<String> {
        self.deps.runs.borrow().iter().map(|r| r.0.clone()).collect()
    }
    fn scripts_run(&self) -> Vec<String> {
        self.deps.runs.borrow().iter().map(|r| r.1.clone()).collect()
    }
    fn spend_runs(&self) -> Vec<String> {
        self.scripts_run().into_iter().filter(|s| s.starts_with("SPEND")).collect()
    }
    fn pending(&self) -> Option<Value> {
        self.deps.store.row(PENDING, KEY)
    }
    fn pending_ids(&self) -> Vec<String> {
        self.deps.store.rows.borrow().get(PENDING).map(|c| c.keys().cloned().collect()).unwrap_or_default()
    }
}

/// `absentComputer(name, null).sentence` (protocol/src/workspace-state.ts:345-351).
fn absent(name: &str) -> String {
    format!("{name} is not answering; it connects on its own when it is on")
}

fn row_of(answer: &ResetAnswer) -> String {
    answer.account.as_ref().map(|a| a.key.clone()).unwrap_or_default()
}

mod which_computer_a_reset_runs_on {
    use super::*;

    #[test]
    fn the_first_of_the_accounts_computers_that_holds_its_login_and_is_connected() {
        run(async {
            let h = Harness::new(
                Some(vec![named(json!({ "computers": ["solari", "pl_boat", "pl_out", "pl_spoo"] }))]),
                vec![],
                vec![Some(reading(Some(2)))],
                vec![],
            );
            h.ask(KEY).await.unwrap();
            assert_eq!(h.places_run(), vec!["pl_spoo", "pl_spoo"]);
            assert_eq!(h.scripts_run()[0], "READ /logins/pl_spoo/codex");
        });
    }

    #[test]
    fn the_one_the_person_named_refused_with_the_computers_own_sentence_when_away() {
        run(async {
            let h = Harness::new(
                Some(vec![named(json!({ "computers": ["pl_spoo", "pl_boat"] }))]),
                vec![],
                vec![Some(reading(Some(2)))],
                vec![],
            );
            let away = h.ask_on(KEY, "pl_boat").await.unwrap_err();
            assert_eq!(away.message, absent("Boat"));
            assert_eq!(away.kind, None);
            let not_on = h.ask_on(KEY, "here").await.unwrap_err();
            assert_eq!(not_on.message, "Codex with ChatGPT Plus is not signed in on Mac; it is on spoo and Boat");
            assert_eq!(not_on.kind, Some(Kind::Usage));
            assert!(h.scripts_run().is_empty());
        });
    }

    #[test]
    fn a_box_that_is_away_when_it_is_the_only_one_refused_with_nothing_run_and_no_key() {
        run(async {
            let h = Harness::new(Some(vec![named(json!({ "computers": ["pl_boat"] }))]), vec![], vec![], vec![]);
            let e = h.ask(KEY).await.unwrap_err();
            assert_eq!(e.message, absent("Boat"));
            assert_eq!(e.kind, None);
            assert!(h.scripts_run().is_empty());
            assert!(h.pending_ids().is_empty());
        });
    }

    #[test]
    fn the_computer_an_own_login_is_kept_on() {
        run(async {
            let h = Harness::new(Some(vec![]), vec![], vec![Some(reading_as(Some(2), "acct_z", None))], vec![]);
            h.ask("codex@here").await.unwrap();
            assert_eq!(h.places_run(), vec!["here", "here"]);
        });
    }

    #[test]
    fn refuses_a_login_that_lives_only_on_a_providers_machines_naming_them() {
        run(async {
            let h = Harness::new(Some(vec![named(json!({ "computers": ["solari"] }))]), vec![], vec![], vec![]);
            let e = h.ask(KEY).await.unwrap_err();
            assert_eq!(e.message, "Codex with ChatGPT Plus lives only on Solari machines; sign Codex in on a computer of yours");
            assert_eq!(e.kind, Some(Kind::Usage));
            assert!(h.scripts_run().is_empty());
        });
    }

    #[test]
    fn no_computers_at_all_reads_signed_out() {
        run(async {
            let h = Harness::new(Some(vec![named(json!({ "computers": [] }))]), vec![], vec![], vec![]);
            let e = h.ask(KEY).await.unwrap_err();
            assert_eq!(e.message, "Codex with ChatGPT Plus is signed in on none of your computers now; sign Codex in on one of them");
            assert_eq!(e.kind, Some(Kind::Usage));
            assert!(h.scripts_run().is_empty());
        });
    }
}

mod what_is_refused_before_anything_is_spent {
    use super::*;

    #[test]
    fn an_account_signed_in_by_key_before_any_process_runs() {
        run(async {
            for l in [named(json!({ "keyed": true })), named(json!({ "key": "codex:vault-key", "road": "vault" }))] {
                let key = l.key.clone();
                let h = Harness::new(Some(vec![l]), vec![], vec![], vec![]);
                let e = h.ask(&key).await.unwrap_err();
                assert_eq!(e.kind, Some(Kind::Usage));
                assert!(e.message.ends_with("pays per token; an API key has no reset to use"), "{}", e.message);
                assert!(h.scripts_run().is_empty());
            }
        });
    }

    #[test]
    fn the_vaults_key_which_no_turn_has_read_a_limit_for_yet() {
        run(async {
            let row: AccountRow = serde_json::from_value(json!({ "key": "codex:vault-key", "agent": "codex", "label": "Codex with an API key", "computers": ["spoo"], "note": "pays per token, no plan limit" })).unwrap();
            let h = Harness::new(Some(vec![]), vec![row], vec![], vec![]);
            let e = h.ask("codex:vault-key").await.unwrap_err();
            assert_eq!(e.message, "Codex with an API key pays per token; an API key has no reset to use");
            assert!(h.scripts_run().is_empty());
        });
    }

    #[test]
    fn a_computer_whose_login_reads_as_a_key_now() {
        run(async {
            let h = Harness::new(None, vec![], vec![Some(ResetReading { keyed: true, limit: None })], vec![]);
            let e = h.ask(KEY).await.unwrap_err();
            assert_eq!(e.message, "Codex with ChatGPT Plus pays per token; an API key has no reset to use");
            assert_eq!(e.kind, Some(Kind::Usage));
            assert!(h.spend_runs().is_empty());
        });
    }

    #[test]
    fn a_computer_signed_in_as_another_account_now() {
        run(async {
            let h = Harness::new(None, vec![], vec![Some(reading_as(Some(2), "acct_b", Some("b@example.com")))], vec![]);
            let e = h.ask(KEY).await.unwrap_err();
            assert_eq!(e.message, "Codex on spoo is now signed in as b@example.com, not a@example.com; nothing was spent");
            assert_eq!(e.kind, None);
            assert!(h.spend_runs().is_empty());
            assert!(h.deps.filed.borrow().is_empty(), "nothing is filed before the login is checked");
        });
    }

    #[test]
    fn an_account_no_limit_names_and_an_agent_that_banks_none() {
        run(async {
            let e = Harness::new(None, vec![], vec![], vec![]).ask("nope").await.unwrap_err();
            assert_eq!(e.kind, Some(Kind::NotFound));
            assert_eq!(e.message, "no account nope; wsp usage lists every account");
            let h = Harness::new(Some(vec![named(json!({ "key": "claude:x", "agent": "claude" }))]), vec![], vec![], vec![]);
            let e = h.ask("claude:x").await.unwrap_err();
            assert_eq!(e.message, "claude banks no resets");
            assert_eq!(e.kind, Some(Kind::Usage));
        });
    }

    #[test]
    fn a_credit_id_the_agent_would_not_take_as_a_usage_refusal_with_nothing_kept() {
        run(async {
            let h = Harness::new(None, vec![], vec![Some(reading(Some(2)))], vec![]);
            let e = h.reset.reset(ResetAsk { account: KEY.to_owned(), credit_id: Some("rc'1".to_owned()), on: None }).await.unwrap_err();
            assert_eq!(e.kind, Some(Kind::Usage));
            assert_eq!(e.message, "creditId must be a plain slug, got \"rc'1\"");
            assert!(h.spend_runs().is_empty());
            assert!(h.pending_ids().is_empty());
        });
    }

    #[test]
    fn a_home_the_agent_would_not_read_is_a_plain_refusal_with_nothing_run() {
        run(async {
            let h = Harness::new(None, vec![], vec![Some(reading(Some(2)))], vec![]);
            h.deps.relative_home.set(true);
            let e = h.ask(KEY).await.unwrap_err();
            assert_eq!(e.kind, None);
            assert_eq!(e.message, "home must be an absolute path, got \"logins/pl_spoo/codex\"");
            assert!(h.scripts_run().is_empty());
            assert!(h.pending_ids().is_empty());
        });
    }

    #[test]
    fn nothing_banked_on_the_read_with_no_spend() {
        run(async {
            let h = Harness::new(None, vec![], vec![Some(reading(Some(0)))], vec![]);
            let a = h.ask(KEY).await.unwrap();
            assert_eq!(a.outcome, ResetOutcome::NoCredit);
            assert_eq!(a.said, "No reset banked on Codex with ChatGPT Plus on spoo");
            assert!(h.spend_runs().is_empty());
        });
    }

    #[test]
    fn a_build_whose_read_carries_no_resets_at_all() {
        run(async {
            let h = Harness::new(None, vec![], vec![Some(reading(None))], vec![]);
            let e = h.ask(KEY).await.unwrap_err();
            assert_eq!(e.message, "Codex on spoo is older than 0.141 and has no resets");
            assert!(h.spend_runs().is_empty());
            assert_eq!(*h.deps.filed.borrow(), vec![None], "the reading is filed before the count is read");
        });
    }

    #[test]
    fn a_read_with_no_limit_says_unread() {
        run(async {
            let h = Harness::new(None, vec![], vec![None], vec![]);
            let e = h.ask(KEY).await.unwrap_err();
            assert_eq!(e.message, "Codex on spoo did not read Codex with ChatGPT Plus's limits; nothing was spent");
            assert_eq!(e.kind, None);
            assert!(h.spend_runs().is_empty());
        });
    }
}

mod a_spend {
    use super::*;

    #[test]
    fn says_each_answer_in_its_words_the_reading_after_it_filed() {
        run(async {
            let cases = [
                (
                    spent("reset", Some(1)),
                    ResetOutcome::Reset,
                    "Reset used: Codex with ChatGPT Plus on spoo's windows start again now, 1 left",
                ),
                (
                    spent("nothingToReset", None),
                    ResetOutcome::NothingToReset,
                    "No window of Codex with ChatGPT Plus on spoo is in use right now, so the credit was kept",
                ),
                (spent("noCredit", None), ResetOutcome::NoCredit, "No reset banked on Codex with ChatGPT Plus on spoo"),
                (
                    spent("alreadyRedeemed", None),
                    ResetOutcome::AlreadyRedeemed,
                    "That reset of Codex with ChatGPT Plus on spoo was already used",
                ),
                (
                    spent("resetPartly", None),
                    ResetOutcome::Unknown,
                    "Codex answered resetPartly for Codex with ChatGPT Plus on spoo; the account was read again",
                ),
            ];
            for (answer, outcome, said) in cases {
                let h = Harness::new(None, vec![], vec![Some(reading(Some(2)))], vec![Spend::Reply(answer)]);
                let a = h.ask(KEY).await.unwrap();
                assert_eq!(a.outcome, outcome, "{said}");
                assert_eq!(a.said, said);
                assert_eq!(row_of(&a), KEY);
                assert!(h.pending().is_none(), "an answered spend lets go of its key");
            }
            let h = Harness::new(None, vec![], vec![Some(reading(Some(2)))], vec![Spend::Reply(spent("reset", Some(1)))]);
            h.reset.reset(ResetAsk { account: KEY.to_owned(), credit_id: Some("rc_9".to_owned()), on: None }).await.unwrap();
            assert_eq!(h.spend_runs()[0], "SPEND key-1 rc_9");
            assert_eq!(*h.deps.filed.borrow(), vec![Some(2), Some(1)]);
        });
    }

    #[test]
    fn a_refusal_past_400_units_is_marked_cut() {
        run(async {
            let long = "é".repeat(450);
            let h = Harness::new(
                None,
                vec![],
                vec![Some(reading(Some(2)))],
                vec![Spend::Reply(ResetSpend::Refused { refused: long.clone(), too_old: false })],
            );
            let e = h.ask(KEY).await.unwrap_err();
            assert_eq!(
                e.message,
                format!("Codex on spoo refused the reset of Codex with ChatGPT Plus: {} (cut 50 characters)", "é".repeat(400))
            );
        });
    }

    #[test]
    fn joins_a_second_press_to_the_one_in_flight() {
        run(async {
            let (tx, rx) = oneshot::channel();
            let h = Harness::new(None, vec![], vec![Some(reading(Some(2)))], vec![Spend::Held(rx)]);
            let first = h.reset.reset(ResetAsk { account: KEY.to_owned(), credit_id: None, on: None });
            let second = h.reset.reset(ResetAsk { account: KEY.to_owned(), credit_id: None, on: None });
            let a = tokio::task::spawn_local(first);
            let b = tokio::task::spawn_local(second);
            tokio::task::yield_now().await;
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            tx.send(spent("reset", Some(1))).unwrap();
            let (a, b) = (a.await.unwrap().unwrap(), b.await.unwrap().unwrap());
            assert_eq!(a, b);
            assert_eq!(h.scripts_run(), vec!["READ /logins/pl_spoo/codex", "SPEND key-1 -"]);
        });
    }

    #[test]
    fn a_press_after_a_failed_one_runs_anew() {
        run(async {
            let h = Harness::new(None, vec![], vec![None, Some(reading(Some(0)))], vec![]);
            assert!(h.ask(KEY).await.is_err());
            let a = h.ask(KEY).await.unwrap();
            assert_eq!(a.outcome, ResetOutcome::NoCredit);
            assert_eq!(h.scripts_run(), vec!["READ /logins/pl_spoo/codex", "READ /logins/pl_spoo/codex"]);
        });
    }

    #[test]
    fn keeps_the_key_when_the_spend_went_unanswered_and_lets_go_once_answered() {
        run(async {
            let h = Harness::new(
                None,
                vec![],
                vec![Some(reading(Some(2))), Some(reading(Some(2)))],
                vec![Spend::Reply(ResetSpend::Unanswered), Spend::Reply(spent("reset", Some(1)))],
            );
            let e = h.ask(KEY).await.unwrap_err();
            assert_eq!(e.message, "Codex on spoo did not answer; the same request goes again on the next press");
            assert_eq!(h.pending(), Some(json!({ "key": "key-1", "before": 2 })));
            assert_eq!(serde_json::to_string(&h.pending().unwrap()).unwrap(), r#"{"key":"key-1","before":2}"#);
            h.ask(KEY).await.unwrap();
            assert_eq!(h.spend_runs(), vec!["SPEND key-1 -", "SPEND key-1 -"]);
            assert!(h.pending().is_none());
        });
    }

    #[test]
    fn keeps_the_key_across_a_run_that_failed_outright() {
        run(async {
            let h = Harness::new(None, vec![], vec![Some(reading(Some(2)))], vec![Spend::Fails("spoo is not answering")]);
            let e = h.ask(KEY).await.unwrap_err();
            assert_eq!(e.message, "spoo is not answering");
            assert_eq!(h.pending(), Some(json!({ "key": "key-1", "before": 2 })));
        });
    }

    #[test]
    fn says_the_count_fell_since_an_unanswered_press_and_spends_nothing() {
        run(async {
            let h = Harness::new(
                None,
                vec![],
                vec![Some(reading(Some(2))), Some(reading(Some(1)))],
                vec![Spend::Reply(ResetSpend::Unanswered)],
            );
            assert!(h.ask(KEY).await.unwrap_err().message.contains("did not answer"));
            let a = h.ask(KEY).await.unwrap();
            assert_eq!(a.outcome, ResetOutcome::Earlier);
            assert_eq!(a.said, "The count on Codex with ChatGPT Plus on spoo fell since the earlier press: it may have gone through, or a reset expired or was used elsewhere. Nothing more was spent; 1 left.");
            // The second press read the account and sent no consume at all.
            let first_words: Vec<String> = h.scripts_run().iter().map(|s| s.split(' ').next().unwrap().to_owned()).collect();
            assert_eq!(first_words, vec!["READ", "SPEND", "READ"]);
            assert!(h.pending().is_none());
        });
    }

    #[test]
    fn lets_go_of_the_key_on_a_refusal_and_says_a_build_with_no_such_request_is_too_old() {
        run(async {
            let h = Harness::new(
                None,
                vec![],
                vec![Some(reading(Some(2))), Some(reading(Some(2)))],
                vec![
                    Spend::Reply(ResetSpend::Refused { refused: "Method not found".to_owned(), too_old: true }),
                    Spend::Reply(ResetSpend::Refused { refused: "chatgpt authentication required".to_owned(), too_old: false }),
                ],
            );
            assert_eq!(h.ask(KEY).await.unwrap_err().message, "Codex on spoo is older than 0.141 and has no resets");
            assert!(h.pending().is_none());
            assert_eq!(
                h.ask(KEY).await.unwrap_err().message,
                "Codex on spoo refused the reset of Codex with ChatGPT Plus: chatgpt authentication required"
            );
            assert_eq!(h.spend_runs(), vec!["SPEND key-1 -", "SPEND key-2 -"]);
        });
    }

    #[test]
    fn a_new_resets_over_the_same_store_sends_the_kept_key() {
        run(async {
            // What survives a restart: the pending row, not the joins. A new host over the same store reuses the key.
            let store = MemStore::new();
            let first =
                Harness::over(store.clone(), None, vec![], vec![Some(reading(Some(2)))], vec![Spend::Reply(ResetSpend::Unanswered)]);
            assert!(first.ask(KEY).await.is_err());
            let again =
                Harness::over(store.clone(), None, vec![], vec![Some(reading(Some(2)))], vec![Spend::Reply(spent("reset", Some(1)))]);
            again.ask(KEY).await.unwrap();
            assert_eq!(again.spend_runs(), vec!["SPEND key-1 -"]);
            assert_eq!(*again.deps.minted.borrow(), 0, "no new key is minted");
            assert!(store.row(PENDING, KEY).is_none());
        });
    }
}

#[allow(dead_code)]
fn unused(_: IndexMap<String, String>) {}
