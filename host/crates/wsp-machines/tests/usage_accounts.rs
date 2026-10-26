// SPDX-License-Identifier: AGPL-3.0-only
//! usage accounts (spec `usage_accounts.md`, rules C1-C10). Ported from packages/runtime/test/usage-ledger.test.ts: the
//! last two cases of "an account's banked resets", "the rate table", "a computer in the usage records", the first case
//! of "an account's key and label", "the account rows" and "what an account draws right now". Each test names its rule
//! and the TS line it stands for; a test with no TS line stands for a rule of usage.ts the TS file never reads.

mod rest_support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::IndexMap;
use serde_json::{json, Value};

use rest_support::*;
use wsp_core::clock::Clock;
use wsp_machines::usage::accounts::*;
use wsp_machines::usage::{Vaulted, BURN_WINDOW_MS, RESET_DETAILS_MS};
use wsp_store::store::Store;
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::types::usage::{AccountLimit, AccountLimitCredits, AccountRoad, AccountRowBurn};
use wsp_wire::views::place_more::PlaceView;
use wsp_words::exit::HostError;

const NOON: i64 = 1_790_683_200_000;
const MINUTE: i64 = 60_000;
const HOUR: i64 = 3_600_000;
const DAY: i64 = 86_400_000;

fn s(x: &str) -> String {
    x.to_string()
}

fn limit(extra: Value) -> AccountLimit {
    let mut base = json!({ "key": "codex:acct_1", "agent": "codex", "label": "dev@example.com", "windows": [], "readAt": NOON, "computers": ["here"] });
    for (k, v) in extra.as_object().unwrap() {
        base[k] = v.clone();
    }
    serde_json::from_value(base).unwrap()
}

fn pv(v: Value) -> PlaceView {
    serde_json::from_value(v).unwrap()
}

fn mac() -> PlaceView {
    pv(
        json!({ "id": "here", "kind": "computer", "name": "zingzys-macbook-pro.local", "label": "zingzy's MacBook Pro", "default": true }),
    )
}

fn boat() -> PlaceView {
    pv(json!({ "id": "box", "kind": "provider", "name": "box", "default": false }))
}

fn name_of_places(id: &str) -> String {
    match id {
        "here" => s("zingzy's MacBook Pro"),
        "pl_boat" => s("Boat"),
        "pl_spoo" => s("spoo"),
        other => s(other),
    }
}

fn agent_name_of(agent: &str) -> String {
    match agent {
        "claude" => s("Claude Code"),
        "codex" => s("Codex"),
        "opencode" => s("OpenCode"),
        other => s(other),
    }
}

fn plan_brand_of(agent: &str) -> Option<String> {
    match agent {
        "codex" => Some(s("ChatGPT")),
        "claude" => Some(s("Claude")),
        _ => None,
    }
}

/// The TS tests' defaults: names are ids, every agent is "Codex" and prints limits, nothing is in the vault.
fn ask(limits: Vec<AccountLimit>) -> AccountRowsAsk {
    AccountRowsAsk {
        limits,
        places: vec![],
        name_of: Rc::new(|id: &str| id.to_string()),
        agent_name: Rc::new(|_: &str| "Codex".to_string()),
        plan_brand: None,
        vaulted: Rc::new(|_: &str| None),
        prints_limits: Rc::new(|_: &str| true),
        burn: None,
    }
}

/// The settings of the "account rows" group: the three computers, Claude Code and Codex printing limits.
fn named_ask(limits: Vec<AccountLimit>) -> AccountRowsAsk {
    AccountRowsAsk {
        name_of: Rc::new(name_of_places),
        agent_name: Rc::new(agent_name_of),
        plan_brand: Some(Rc::new(plan_brand_of)),
        prints_limits: Rc::new(|agent: &str| agent == "claude" || agent == "codex"),
        ..ask(limits)
    }
}

fn signed(id: &str, name: &str, ins: &[(&str, AgentSignInState)]) -> SignedInPlace {
    SignedInPlace {
        id: s(id),
        name: s(name),
        sign_ins: Some(ins.iter().map(|(a, st)| (s(a), *st)).collect()),
    }
}

fn three_places() -> Vec<SignedInPlace> {
    use AgentSignInState::*;
    vec![
        signed(
            "here",
            "zingzy's MacBook Pro",
            &[
                ("claude", VaultKey),
                ("codex", SignedIn),
                ("opencode", SignedIn),
            ],
        ),
        signed(
            "pl_boat",
            "Boat",
            &[("claude", VaultKey), ("codex", SignedIn)],
        ),
        signed("pl_spoo", "spoo", &[("claude", SignedIn), ("cursor", None)]),
    ]
}

fn vault(kind: Option<Vaulted>, agent: &'static str) -> Rc<dyn Fn(&str) -> Option<Vaulted>> {
    Rc::new(move |a: &str| if a == agent { kind } else { None })
}

fn rows_json(a: &AccountRowsAsk) -> Vec<Value> {
    account_rows(a)
        .iter()
        .map(|r| serde_json::to_value(r).unwrap())
        .collect()
}

fn pick(rows: &[Value], fields: &[&str]) -> Vec<Value> {
    rows.iter()
        .map(|r| {
            Value::Array(
                fields
                    .iter()
                    .map(|f| r.get(*f).cloned().unwrap_or(Value::Null))
                    .collect(),
            )
        })
        .collect()
}

fn labels(rows: &[Value]) -> Vec<Value> {
    rows.iter().map(|r| r["label"].clone()).collect()
}

// ---------------------------------------------------------------------------------------------------------------
// C1 to C4: accounts and computer names
// ---------------------------------------------------------------------------------------------------------------

// C1: usage-ledger.test.ts:407. A named account, else the vault's token or key, else the computer's own login.
#[test]
fn names_the_account_a_harness_named_else_the_vaults_token_or_key_else_the_computers_login() {
    let computer = ComputerName {
        id: s("here"),
        name: s("zingzy's MacBook Pro"),
    };
    let acct = |key: &str, label: &str, road| Account {
        key: s(key),
        label: s(label),
        road,
    };
    let maya = NamedAccount {
        id: s("maya@example.com"),
        label: None,
    };
    assert_eq!(
        account_of(
            "codex",
            "Codex",
            Some(&maya),
            Some(Vaulted::Token),
            &computer
        ),
        acct(
            "codex:maya@example.com",
            "maya@example.com",
            AccountRoad::Named
        )
    );
    assert_eq!(
        account_of(
            "claude",
            "Claude Code",
            None,
            Some(Vaulted::Token),
            &computer
        ),
        acct(
            "claude:vault-token",
            "Claude Code with your sign-in",
            AccountRoad::Vault
        )
    );
    assert_eq!(
        account_of("claude", "Claude Code", None, Some(Vaulted::Key), &computer),
        acct(
            "claude:vault-key",
            "Claude Code with an API key",
            AccountRoad::Vault
        )
    );
    assert_eq!(
        account_of("opencode", "OpenCode", None, None, &computer),
        acct(
            "opencode@here",
            "OpenCode signed in on zingzy's MacBook Pro",
            AccountRoad::Own
        )
    );
    let labelled = NamedAccount {
        id: s("acct_9"),
        label: Some(s("Maya's team")),
    };
    assert_eq!(
        account_of("codex", "Codex", Some(&labelled), None, &computer),
        acct("codex:acct_9", "Maya's team", AccountRoad::Named)
    );
}

// C2: no TS case; usage.ts:127-130. The account a computer's turns named, else the vault key they ran on, else the login.
#[test]
fn the_account_on_a_computer_is_the_one_its_turns_named_else_the_vault_key_else_its_login() {
    let mac = ComputerName {
        id: s("here"),
        name: s("Mac"),
    };
    let on = |limits: &[AccountLimit], vaulted| {
        account_on_computer("claude", "Claude Code", &mac, limits, vaulted)
    };
    let key = |k: &str, l: &str| (s(k), s(l));
    let own_login = key("claude@here", "Claude Code signed in on Mac");

    let vault_key = limit(
        json!({ "key": "claude:vault-key", "agent": "claude", "label": "Claude Code with an API key", "road": "vault", "keyed": true }),
    );
    let named = limit(
        json!({ "key": "claude:maya", "agent": "claude", "label": "maya@example.com", "road": "named" }),
    );
    let own = limit(
        json!({ "key": "claude@here", "agent": "claude", "label": "Claude Code signed in on Mac", "road": "own" }),
    );
    let no_road = limit(json!({ "key": "claude:old", "agent": "claude", "label": "old" }));
    let elsewhere = limit(
        json!({ "key": "claude@pl_x", "agent": "claude", "label": "x", "road": "own", "computers": ["pl_x"] }),
    );
    let other_agent =
        limit(json!({ "key": "codex:z", "agent": "codex", "label": "z", "road": "named" }));

    assert_eq!(on(&[], None), own_login);
    assert_eq!(on(&[], Some(Vaulted::Key)), own_login);
    assert_eq!(
        on(std::slice::from_ref(&vault_key), Some(Vaulted::Key)),
        key("claude:vault-key", "Claude Code with an API key")
    );
    assert_eq!(
        on(std::slice::from_ref(&vault_key), Some(Vaulted::Token)),
        own_login,
        "a vault token stays apart from a login"
    );
    assert_eq!(on(std::slice::from_ref(&vault_key), None), own_login);
    assert_eq!(
        on(&[vault_key.clone(), named.clone()], Some(Vaulted::Key)),
        key("claude:maya", "maya@example.com"),
        "a turn's own account comes before the key"
    );
    assert_eq!(
        on(
            &[elsewhere, other_agent, own.clone(), named],
            Some(Vaulted::Key)
        ),
        key("claude@here", "Claude Code signed in on Mac"),
        "another computer or agent is not this one's, and the first match wins"
    );
    assert_eq!(
        on(&[no_road], None),
        key("claude:old", "old"),
        "a reading with no road is not the vault's"
    );
}

// C3: usage-ledger.test.ts:386. A computer reads as its name, never a hostname or an id.
#[test]
fn a_computer_reads_as_its_name_never_a_hostname_or_id() {
    let kept = IndexMap::new();
    let (mac, boat) = (mac(), boat());
    let got = vec![
        usage_computer_name(&[mac.clone(), boat.clone()], "here", &kept),
        usage_computer_name(&[mac.clone(), boat.clone()], "box", &kept),
        usage_computer_name(std::slice::from_ref(&mac), "box", &kept),
    ];
    assert_eq!(got, vec!["zingzy's MacBook Pro", "Boat", "Boat"]);
    assert_eq!(
        usage_computer_name(&[], "here", &kept),
        "this computer",
        "this computer before its row is listed"
    );
    let mut kept = IndexMap::new();
    kept.insert(s("p_0a1b2c3d4e5f6a7b"), s("lab"));
    assert_eq!(
        usage_computer_name(std::slice::from_ref(&mac), "p_0a1b2c3d4e5f6a7b", &kept),
        "lab"
    );
    assert_eq!(
        usage_computer_name(&[mac], "p_9f8e7d6c5b4a3f2e", &kept),
        "a removed computer"
    );
}

// C4: usage-ledger.test.ts:390. A joined computer's name is kept, and only a change is written.
#[test]
fn keeps_a_joined_computers_name_and_writes_only_a_change() {
    run(async {
        let store = MemStore::new();
        store.seed(
            "usage-computers",
            "p_old",
            json!({ "id": "p_old", "name": "old" }),
        );
        store.seed("usage-computers", "junk", json!({ "id": 1, "name": "x" }));
        let lab_id = "p_0a1b2c3d4e5f6a7b";
        let lab = |name: &str| {
            pv(json!({ "id": lab_id, "kind": "computer", "name": name, "default": false }))
        };
        let pairs = |kept: &IndexMap<String, String>| {
            kept.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>()
        };

        let kept = kept_computer_names(store.as_ref(), &[mac(), boat(), lab("lab")])
            .await
            .unwrap();
        assert_eq!(
            pairs(&kept),
            vec![(s("p_old"), s("old")), (s(lab_id), s("lab"))],
            "the stored first, new ones after; a malformed document is no name"
        );
        assert_eq!(
            store.calls(),
            vec![format!("put usage-computers/{lab_id}")],
            "this computer and a provider key are not kept"
        );
        assert_eq!(
            store.doc("usage-computers", lab_id),
            Some(json!({ "id": lab_id, "name": "lab" }))
        );

        let kept = kept_computer_names(store.as_ref(), &[mac(), boat()])
            .await
            .unwrap();
        assert_eq!(
            store.calls().len(),
            1,
            "a name that did not change is not written"
        );
        assert_eq!(usage_computer_name(&[mac()], lab_id, &kept), "lab");
        assert_eq!(
            usage_computer_name(&[mac()], "p_9f8e7d6c5b4a3f2e", &kept),
            "a removed computer"
        );

        let kept = kept_computer_names(store.as_ref(), &[lab("lab 2")])
            .await
            .unwrap();
        assert_eq!(store.calls().len(), 2);
        assert_eq!(
            store.doc("usage-computers", lab_id),
            Some(json!({ "id": lab_id, "name": "lab 2" }))
        );
        assert_eq!(kept.get(lab_id).map(String::as_str), Some("lab 2"));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// C5, C6: banked resets
// ---------------------------------------------------------------------------------------------------------------

// C5: usage-ledger.test.ts:336. A read in full is due where there never was one or it was a day ago; never on a key.
#[test]
fn is_due_a_read_in_full_where_never_or_a_day_ago_and_never_on_a_key() {
    assert_eq!(RESET_DETAILS_MS, DAY);
    let held = |extra: Value| limit(extra);
    assert!(reset_details_due(None, NOON));
    assert!(
        reset_details_due(Some(&held(json!({}))), NOON),
        "no credits held"
    );
    assert!(reset_details_due(
        Some(&held(json!({ "credits": { "count": 2, "readAt": NOON } }))),
        NOON
    ));
    assert!(!reset_details_due(
        Some(&held(
            json!({ "credits": { "count": 2, "readAt": NOON, "detailAt": NOON - DAY + 60_000 } })
        )),
        NOON
    ));
    assert!(reset_details_due(
        Some(&held(
            json!({ "credits": { "count": 2, "readAt": NOON, "detailAt": NOON - DAY } })
        )),
        NOON
    ));
    assert!(!reset_details_due(
        Some(&held(json!({ "keyed": true }))),
        NOON
    ));
}

// C6: usage-ledger.test.ts:345. The resets on a row carry the soonest an available one lapses.
#[test]
fn puts_the_resets_on_the_row_with_the_soonest_available_lapse() {
    let credits = json!({
        "count": 2,
        "credits": [
            { "id": "rc_0", "status": "redeeming", "expiresAt": NOON + DAY },
            { "id": "rc_1", "status": "available", "expiresAt": NOON + 21 * DAY },
            { "id": "rc_2", "status": "available", "expiresAt": NOON + 30 * DAY }
        ],
        "readAt": NOON,
        "detailAt": NOON
    });
    let codex = |credits: Value| {
        limit(
            json!({ "key": "codex:acct_1", "agent": "codex", "label": "dev@example.com", "road": "named", "plan": "plus", "windows": [{ "kind": "session", "usedPercent": 34 }], "credits": credits }),
        )
    };
    let rows = rows_json(&ask(vec![codex(credits.clone())]));
    let mut want = credits.clone();
    want["nextExpiresAt"] = json!(NOON + 21 * DAY);
    assert_eq!(rows[0]["credits"], want);
    let rows = rows_json(&ask(vec![codex(json!({ "count": 2, "readAt": NOON }))]));
    assert_eq!(rows[0]["credits"], json!({ "count": 2, "readAt": NOON }));

    let held: AccountLimitCredits = serde_json::from_value(credits).unwrap();
    assert_eq!(serde_json::to_value(row_credits(&held)).unwrap(), want);
    let spent: AccountLimitCredits = serde_json::from_value(json!({ "count": 1, "credits": [{ "id": "rc_9", "status": "redeemed", "expiresAt": NOON + DAY }, { "id": "rc_8", "status": "available" }], "readAt": NOON })).unwrap();
    assert_eq!(
        serde_json::to_value(row_credits(&spent)).unwrap(),
        json!({ "count": 1, "credits": [{ "id": "rc_9", "status": "redeemed", "expiresAt": NOON + DAY }, { "id": "rc_8", "status": "available" }], "readAt": NOON }),
        "no available credit with a lapse, no nextExpiresAt"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// C7: the burn
// ---------------------------------------------------------------------------------------------------------------

fn call(account: &str, thread: &str, tokens: f64, replayed: Option<(&str, i64)>) -> BurnCall {
    BurnCall {
        account: s(account),
        thread_id: s(thread),
        tokens,
        replayed: replayed.map(|(run, at)| ReplayedStamp { run: s(run), at }),
    }
}

fn burn_json(b: &Burn, account: &str) -> Value {
    serde_json::to_value(b.of(account)).unwrap()
}

fn rate(tokens_per_minute: f64, threads: i64) -> Value {
    let per_minute = if tokens_per_minute.fract() == 0.0 {
        json!(tokens_per_minute as i64)
    } else {
        json!(tokens_per_minute)
    };
    json!({ "tokensPerMinute": per_minute, "threads": threads })
}

// C7: usage-ledger.test.ts:498. What a thread drew in fifteen minutes, per minute, and how many threads drew it.
#[test]
fn draws_the_last_fifteen_minutes_per_minute_and_counts_threads() {
    assert_eq!(BURN_WINDOW_MS, 15 * MINUTE);
    let clock = TestClock::at(NOON);
    let burn = Burn::new(clock.clone());
    burn.add(call("claude:vault-token", "t1", 9_000.0, None));
    clock.advance(10 * MINUTE);
    burn.add(call("claude:vault-token", "t2", 6_000.0, None));
    burn.add(call("claude:vault-token", "t1", 15_000.0, None));
    burn.add(call("codex:acct_1", "t3", 1_500.0, None));
    assert_eq!(burn_json(&burn, "claude:vault-token"), rate(2_000.0, 2));
    assert_eq!(burn_json(&burn, "codex:acct_1"), rate(100.0, 1));
    assert_eq!(burn_json(&burn, "claude:vault-key"), Value::Null);
    clock.advance(6 * MINUTE);
    assert_eq!(burn_json(&burn, "claude:vault-token"), rate(1_400.0, 2));
    clock.advance(10 * MINUTE);
    assert_eq!(burn_json(&burn, "claude:vault-token"), Value::Null);

    // usage.ts:481-482: a call filed at or before now minus the window is gone.
    let clock = TestClock::at(NOON);
    let burn = Burn::new(clock.clone());
    burn.add(call("a", "t1", 1_500.0, None));
    clock.advance(BURN_WINDOW_MS - 1);
    assert_eq!(burn_json(&burn, "a"), rate(100.0, 1));
    clock.advance(1);
    assert_eq!(burn_json(&burn, "a"), Value::Null);
}

// C7: no TS case; usage.ts:476-491. A re-read run's calls are filed by their own stamps, counted back from when the
// newest arrived; a stamp older than the newest moves nothing.
#[test]
fn files_a_replayed_run_by_its_own_stamps() {
    const S0: i64 = 5_000_000;
    let clock = TestClock::at(NOON);
    let burn = Burn::new(clock.clone());
    let acct = "claude:vault-token";
    burn.add(call(acct, "t1", 3_000.0, Some(("r1", S0))));
    burn.add(call(acct, "t1", 6_000.0, Some(("r1", S0 + 5 * MINUTE))));
    burn.add(call(acct, "t2", 9_000.0, Some(("r1", S0 + 14 * MINUTE))));
    // Filed at 14, 9 and 0 minutes before they arrived.
    assert_eq!(burn_json(&burn, acct), rate(1_200.0, 2));
    clock.advance(MINUTE - 1);
    assert_eq!(burn_json(&burn, acct), rate(1_200.0, 2));
    clock.advance(1);
    assert_eq!(burn_json(&burn, acct), rate(1_000.0, 2));
    clock.advance(5 * MINUTE - 1);
    assert_eq!(burn_json(&burn, acct), rate(1_000.0, 2));
    clock.advance(1);
    assert_eq!(burn_json(&burn, acct), rate(600.0, 1));
    clock.advance(9 * MINUTE - 1);
    assert_eq!(burn_json(&burn, acct), rate(600.0, 1));
    clock.advance(1);
    assert_eq!(burn_json(&burn, acct), Value::Null);

    // A stamp older than the newest is filed against the newest and moves it nowhere.
    let clock = TestClock::at(NOON);
    let burn = Burn::new(clock.clone());
    burn.add(call(acct, "t1", 3_000.0, Some(("r1", S0))));
    burn.add(call(acct, "t1", 6_000.0, Some(("r1", S0 + 14 * MINUTE))));
    burn.add(call(acct, "t1", 9_000.0, Some(("r1", S0 + 7 * MINUTE))));
    assert_eq!(burn_json(&burn, acct), rate(1_200.0, 1));
    clock.advance(MINUTE);
    assert_eq!(burn_json(&burn, acct), rate(1_000.0, 1));
    clock.advance(7 * MINUTE);
    assert_eq!(burn_json(&burn, acct), rate(400.0, 1));
    clock.advance(7 * MINUTE);
    assert_eq!(burn_json(&burn, acct), Value::Null);
}

// C7: no TS case; usage.ts:483-484. A run with no call left forgets its newest stamp.
#[test]
fn forgets_a_runs_newest_stamp_once_its_calls_are_gone() {
    const S0: i64 = 5_000_000;
    let clock = TestClock::at(NOON);
    let burn = Burn::new(clock.clone());
    let acct = "claude:vault-token";
    burn.add(call(acct, "t1", 1_500.0, Some(("r1", S0 + HOUR))));
    burn.add(call(acct, "t1", 1_500.0, Some(("r1", S0 + HOUR + 1_000))));
    assert!(burn.newest.borrow().contains_key("r1"));
    clock.advance(BURN_WINDOW_MS);
    assert_eq!(burn_json(&burn, acct), Value::Null);
    assert!(burn.newest.borrow().is_empty());
    // A later call stamped before the old newest one is a new run's first, filed when it arrived.
    burn.add(call(acct, "t1", 3_000.0, Some(("r1", S0))));
    assert_eq!(burn_json(&burn, acct), rate(200.0, 1));
}

// ---------------------------------------------------------------------------------------------------------------
// C8: the price table
// ---------------------------------------------------------------------------------------------------------------

type Fetch = Rc<dyn Fn() -> LocalBoxFuture<'static, Result<Value, HostError>>>;

fn gpt() -> Value {
    json!({ "gpt-5.5": { "input_cost_per_token": 5e-6, "output_cost_per_token": 3e-5, "litellm_provider": "openai" } })
}

fn fetch_of(fetched: &Rc<Cell<u32>>, offline: &Rc<Cell<bool>>, body: &Rc<RefCell<Value>>) -> Fetch {
    let (fetched, offline, body) = (fetched.clone(), offline.clone(), body.clone());
    Rc::new(move || {
        fetched.set(fetched.get() + 1);
        let answer = if offline.get() {
            Err(HostError::new("offline"))
        } else {
            Ok(body.borrow().clone())
        };
        async move { answer }.boxed_local()
    })
}

fn table_over(
    store: &Rc<MemStore>,
    clock: &Rc<TestClock>,
    fetch: Fetch,
    warned: &Rc<Warned>,
) -> Rc<PriceTable> {
    let w = warned.clone();
    let store: Rc<dyn Store> = store.clone();
    PriceTable::new(
        store,
        clock.clone() as Rc<dyn Clock>,
        fetch,
        Rc::new(move |line: &str| w.push(line)),
    )
}

fn keys(table: &wsp_words::usage::RateTable) -> Vec<String> {
    table.keys().cloned().collect()
}

fn kept_blob(store: &MemStore) -> Option<Value> {
    store
        .blob("prices", "litellm")
        .map(|b| serde_json::from_slice(&b).unwrap())
}

// C8: usage-ledger.test.ts:356. Read once a day, kept for when the network is gone.
#[test]
fn is_read_once_a_day_and_kept_for_when_the_network_is_gone() {
    run(async {
        let clock = TestClock::at(NOON);
        let store = MemStore::new();
        let warned = Rc::new(Warned::default());
        let (fetched, offline, body) = (
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(false)),
            Rc::new(RefCell::new(gpt())),
        );
        let table = table_over(&store, &clock, fetch_of(&fetched, &offline, &body), &warned);
        assert_eq!(keys(&table.get().await), vec!["gpt-5.5"]);
        table.get().await;
        assert_eq!(fetched.get(), 1);
        assert_eq!(
            kept_blob(&store),
            Some(
                json!({ "fetchedAt": NOON, "table": { "gpt-5.5": { "input": 5e-6, "output": 3e-5, "provider": "openai" } } })
            )
        );
        clock.advance(25 * HOUR);
        offline.set(true);
        assert_eq!(keys(&table.get().await), vec!["gpt-5.5"]);
        assert_eq!(fetched.get(), 2);
        assert_eq!(
            warned.lines(),
            vec!["the price table was not read (offline); the last one read is used"]
        );
        // A host started later over the same store reads the kept copy before it asks.
        let again_fetched = Rc::new(Cell::new(0));
        let again = table_over(
            &store,
            &clock,
            fetch_of(&again_fetched, &offline, &body),
            &warned,
        );
        assert_eq!(keys(&again.get().await), vec!["gpt-5.5"]);
        assert_eq!(
            again_fetched.get(),
            1,
            "the kept copy is a day old, so it asks"
        );
        assert_eq!(warned.lines().len(), 2);
        assert_eq!(
            kept_blob(&store).unwrap()["fetchedAt"],
            json!(NOON),
            "a failed read leaves the kept copy as it was"
        );

        // usage.ts:506, 515: fresh for a day to the millisecond; a kept copy with a fresh stamp is used, one whose stamp is no number is not.
        let clock = TestClock::at(NOON);
        let store = MemStore::new();
        let (fetched, offline, body) = (
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(false)),
            Rc::new(RefCell::new(gpt())),
        );
        let table = table_over(&store, &clock, fetch_of(&fetched, &offline, &body), &warned);
        table.get().await;
        clock.advance(DAY - 1);
        table.get().await;
        assert_eq!(fetched.get(), 1);
        clock.advance(1);
        table.get().await;
        assert_eq!(fetched.get(), 2);

        let store = MemStore::new();
        let kept =
            json!({ "fetchedAt": NOON - 1_000, "table": { "m": { "input": 1, "output": 2 } } });
        store.seed_blob("prices", "litellm", kept.to_string().as_bytes());
        let fetched = Rc::new(Cell::new(0));
        let table = table_over(
            &store,
            &TestClock::at(NOON),
            fetch_of(&fetched, &offline, &body),
            &warned,
        );
        assert_eq!(keys(&table.get().await), vec!["m"]);
        assert_eq!(fetched.get(), 0, "a fresh kept copy answers with no read");
        let store = MemStore::new();
        store.seed_blob(
            "prices",
            "litellm",
            json!({ "fetchedAt": "x", "table": { "m": { "input": 1, "output": 2 } } })
                .to_string()
                .as_bytes(),
        );
        let fetched = Rc::new(Cell::new(0));
        let table = table_over(
            &store,
            &TestClock::at(NOON),
            fetch_of(&fetched, &offline, &body),
            &warned,
        );
        assert_eq!(keys(&table.get().await), vec!["gpt-5.5"]);
        assert_eq!(fetched.get(), 1);

        // With nothing held a failed read warns that no row is priced and answers an empty table.
        let before = warned.lines().len();
        offline.set(true);
        let table = table_over(
            &MemStore::new(),
            &TestClock::at(NOON),
            fetch_of(&Rc::new(Cell::new(0)), &offline, &body),
            &warned,
        );
        assert!(table.get().await.is_empty());
        assert_eq!(
            warned.lines()[before..],
            ["the price table was not read (offline); no row is priced"]
        );
    });
}

// C8: no TS case; usage.ts:531-535. A second caller joins the read in flight.
#[test]
fn two_callers_share_one_read() {
    run(async {
        let clock = TestClock::at(NOON);
        let store = MemStore::new();
        let warned = Rc::new(Warned::default());
        let fetched = Rc::new(Cell::new(0u32));
        let counter = fetched.clone();
        let fetch: Fetch = Rc::new(move || {
            counter.set(counter.get() + 1);
            async move {
                tokio::task::yield_now().await;
                tokio::task::yield_now().await;
                Ok(gpt())
            }
            .boxed_local()
        });
        let table = table_over(&store, &clock, fetch, &warned);
        let (a, b) = futures::join!(table.get(), table.get());
        assert_eq!(fetched.get(), 1);
        assert_eq!(keys(&a), vec!["gpt-5.5"]);
        assert_eq!(a, b);
        assert!(warned.lines().is_empty());
    });
}

// C8: no TS case; usage.ts:521. A fetch with no priced rows is ignored silently; nothing is written for it.
#[test]
fn an_empty_fetch_keeps_the_last_table_quietly() {
    run(async {
        let clock = TestClock::at(NOON);
        let store = MemStore::new();
        let warned = Rc::new(Warned::default());
        let (fetched, offline, body) = (
            Rc::new(Cell::new(0)),
            Rc::new(Cell::new(false)),
            Rc::new(RefCell::new(gpt())),
        );
        let table = table_over(&store, &clock, fetch_of(&fetched, &offline, &body), &warned);
        table.get().await;
        clock.advance(25 * HOUR);
        *body.borrow_mut() = json!({ "sample_spec": { "input_cost_per_token": 1 } });
        assert_eq!(keys(&table.get().await), vec!["gpt-5.5"]);
        assert_eq!(fetched.get(), 2);
        assert!(warned.lines().is_empty());
        assert_eq!(kept_blob(&store).unwrap()["fetchedAt"], json!(NOON));

        let store = MemStore::new();
        let table = table_over(
            &store,
            &clock,
            fetch_of(
                &Rc::new(Cell::new(0)),
                &offline,
                &Rc::new(RefCell::new(json!({}))),
            ),
            &warned,
        );
        assert!(table.get().await.is_empty());
        assert!(warned.lines().is_empty());
        assert!(kept_blob(&store).is_none());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// C9, C10: names and rows
// ---------------------------------------------------------------------------------------------------------------

fn plus(id: &str, email: &str, computer: &str) -> AccountLimit {
    limit(
        json!({ "key": format!("codex:{id}"), "agent": "codex", "label": email, "road": "named", "plan": "plus", "windows": [{ "kind": "session", "usedPercent": 10 }], "computers": [computer] }),
    )
}

fn names_json(a: &AccountRowsAsk) -> Value {
    serde_json::to_value(account_names(a)).unwrap()
}

// C9: usage-ledger.test.ts:464. Two accounts of one agent that read the same keep the address each signed in as.
#[test]
fn keeps_the_address_where_two_accounts_would_read_the_same() {
    let pro = limit(
        json!({ "key": "codex:acct_c", "agent": "codex", "label": "c@example.com", "road": "named", "plan": "pro", "windows": [{ "kind": "session", "usedPercent": 10 }] }),
    );
    let a = named_ask(vec![
        plus("acct_a", "a@example.com", "here"),
        plus("acct_b", "b@example.com", "pl_boat"),
        pro,
    ]);
    let rows = rows_json(&a);
    assert_eq!(
        labels(&rows),
        vec![
            json!("Codex with ChatGPT Plus as a@example.com"),
            json!("Codex with ChatGPT Plus as b@example.com"),
            json!("Codex with ChatGPT Pro")
        ]
    );
    assert_eq!(
        names_json(&a),
        json!({ "codex:acct_a": "Codex with ChatGPT Plus as a@example.com", "codex:acct_b": "Codex with ChatGPT Plus as b@example.com", "codex:acct_c": "Codex with ChatGPT Pro" })
    );

    // usage.ts:555: a name that already ends so is left as it is.
    let same = |key: &str| {
        limit(json!({ "key": key, "agent": "codex", "label": "dev@example.com", "road": "named" }))
    };
    let twin = named_ask(vec![same("codex:a1"), same("codex:a2")]);
    assert_eq!(
        names_json(&twin),
        json!({ "codex:a1": "Codex as dev@example.com", "codex:a2": "Codex as dev@example.com" })
    );
}

// C9: usage-ledger.test.ts:471. Two own logins and the vault's key that read the same keep the computer; the vault stays.
#[test]
fn keeps_the_computer_where_two_own_logins_would_read_the_same() {
    let vault_key = limit(
        json!({ "key": "claude:vault-key", "agent": "claude", "label": "Claude Code with an API key", "road": "vault", "keyed": true, "computers": ["pl_boat"] }),
    );
    let own = |computer: &str| {
        limit(
            json!({ "key": format!("claude@{computer}"), "agent": "claude", "label": format!("Claude Code signed in on {}", name_of_places(computer)), "road": "own", "keyed": true, "computers": [computer] }),
        )
    };
    let a = AccountRowsAsk {
        vaulted: vault(Some(Vaulted::Key), "claude"),
        ..named_ask(vec![vault_key, own("here"), own("pl_spoo")])
    };
    let rows = rows_json(&a);
    assert_eq!(
        labels(&rows),
        vec![
            json!("Claude Code with an API key"),
            json!("Claude Code with an API key on spoo"),
            json!("Claude Code with an API key on zingzy's MacBook Pro")
        ]
    );
    let by_key: serde_json::Map<String, Value> = rows
        .iter()
        .map(|r| (r["key"].as_str().unwrap().to_string(), r["label"].clone()))
        .collect();
    assert_eq!(names_json(&a), Value::Object(by_key));
}

// C10: usage-ledger.test.ts:433. The vault's token once across every computer, a computer's own login apart, and the
// account a turn named across its computers.
#[test]
fn lists_the_vault_token_once_across_computers() {
    let codex = limit(json!({
        "key": "codex:acct_7f3a", "agent": "codex", "label": "dev@example.com", "plan": "plus",
        "windows": [{ "kind": "session", "usedPercent": 34 }], "status": "ok", "computers": ["here", "pl_boat"]
    }));
    let a = AccountRowsAsk {
        places: three_places(),
        vaulted: vault(Some(Vaulted::Token), "claude"),
        ..named_ask(vec![codex])
    };
    let rows = rows_json(&a);
    let unread = "not read yet: shows after its next turn";
    assert_eq!(
        pick(&rows, &["key", "label", "computers", "note"]),
        vec![
            json!([
                "claude@pl_spoo",
                "Claude Code signed in on spoo",
                ["spoo"],
                unread
            ]),
            json!([
                "claude:vault-token",
                "Claude Code with your sign-in",
                ["zingzy's MacBook Pro", "Boat"],
                unread
            ]),
            json!([
                "codex:acct_7f3a",
                "Codex with ChatGPT Plus",
                ["zingzy's MacBook Pro", "Boat"],
                null
            ]),
            json!([
                "opencode@here",
                "OpenCode signed in on zingzy's MacBook Pro",
                ["zingzy's MacBook Pro"],
                "reports no plan limit"
            ]),
        ]
    );
    let codex_row = rows
        .iter()
        .find(|r| r["key"] == json!("codex:acct_7f3a"))
        .unwrap();
    assert_eq!(codex_row["plan"], json!("plus"));
    assert_eq!(
        codex_row["windows"],
        json!([{ "kind": "session", "usedPercent": 34 }])
    );
    assert_eq!(codex_row["status"], json!("ok"));
    assert_eq!(codex_row["readAt"], json!(NOON));

    // usage.ts:613-616: each row's burn where it has one, a row made from a sign-in included.
    let drawing: AccountRowBurn =
        serde_json::from_value(json!({ "tokensPerMinute": 2000, "threads": 2 })).unwrap();
    let burning = AccountRowsAsk {
        burn: Some(Rc::new(move |key: &str| {
            if key == "claude:vault-token" {
                Some(drawing.clone())
            } else {
                None
            }
        })),
        ..a
    };
    let rows = rows_json(&burning);
    assert_eq!(
        pick(&rows, &["key", "burn"])
            .iter()
            .map(|p| p[1].clone())
            .collect::<Vec<_>>(),
        vec![
            Value::Null,
            json!({ "tokensPerMinute": 2000, "threads": 2 }),
            Value::Null,
            Value::Null
        ]
    );
}

// C10: usage-ledger.test.ts:445. A named account carries the address it signed in as.
#[test]
fn carries_the_address_a_named_account_signed_in_as() {
    let codex = limit(
        json!({ "key": "codex:acct_7f3a", "agent": "codex", "label": "dev@example.com", "road": "named", "plan": "plus" }),
    );
    let own = limit(
        json!({ "key": "codex@pl_boat", "agent": "codex", "label": "Codex signed in on Boat", "road": "own", "computers": ["pl_boat"] }),
    );
    let rows = rows_json(&named_ask(vec![codex, own]));
    assert_eq!(
        pick(&rows, &["key", "address"]),
        vec![
            json!(["codex@pl_boat", null]),
            json!(["codex:acct_7f3a", "dev@example.com"])
        ]
    );
}

// C10: usage-ledger.test.ts:455. A vault key or a key a turn named reads as a sign-in with no plan window.
#[test]
fn reads_a_vault_key_as_a_sign_in_with_no_plan_window() {
    let keyed = limit(
        json!({ "key": "claude@pl_spoo", "agent": "claude", "label": "spoo", "keyed": true, "computers": ["pl_spoo"] }),
    );
    let a = AccountRowsAsk {
        places: three_places(),
        vaulted: vault(Some(Vaulted::Key), "claude"),
        ..named_ask(vec![keyed])
    };
    let rows: Vec<Value> = rows_json(&a)
        .into_iter()
        .filter(|r| r["agent"] == json!("claude"))
        .collect();
    let note = "pays per token, no plan limit";
    assert_eq!(
        pick(&rows, &["key", "label", "note"]),
        vec![
            json!(["claude@pl_spoo", "Claude Code with an API key", note]),
            json!(["claude:vault-key", "Claude Code with an API key", note])
        ]
    );
    assert!(
        rows[0].get("windows").is_none() && rows[0].get("readAt").is_none(),
        "no windows read, so neither windows nor readAt"
    );
}

// C10: usage-ledger.test.ts:480. A computer's own login folds into the vault key its turns ran on: one key, one row.
#[test]
fn folds_a_computers_login_into_the_vault_key_its_turns_ran_on() {
    use AgentSignInState::*;
    let key = limit(
        json!({ "key": "claude:vault-key", "agent": "claude", "label": "Claude Code with an API key", "road": "vault", "keyed": true, "computers": ["pl_boat", "here"] }),
    );
    let codex = limit(json!({
        "key": "codex:acct_7f3a", "agent": "codex", "label": "dev@example.com", "road": "named", "plan": "plus",
        "windows": [{ "kind": "session", "usedPercent": 12 }, { "kind": "week", "usedPercent": 40 }]
    }));
    let mine = vec![
        signed(
            "here",
            "zingzy's MacBook Pro",
            &[
                ("claude", SignedIn),
                ("codex", SignedIn),
                ("opencode", SignedIn),
            ],
        ),
        signed("pl_boat", "Boat", &[("claude", VaultKey)]),
    ];
    let a = AccountRowsAsk {
        places: mine,
        vaulted: vault(Some(Vaulted::Key), "claude"),
        ..named_ask(vec![key, codex])
    };
    let rows = rows_json(&a);
    assert_eq!(
        pick(&rows, &["label", "computers", "note"]),
        vec![
            json!([
                "Claude Code with an API key",
                ["Boat", "zingzy's MacBook Pro"],
                "pays per token, no plan limit"
            ]),
            json!(["Codex with ChatGPT Plus", ["zingzy's MacBook Pro"], null]),
            json!([
                "OpenCode signed in on zingzy's MacBook Pro",
                ["zingzy's MacBook Pro"],
                "reports no plan limit"
            ]),
        ]
    );
}

// C10: usage-ledger.test.ts:398. Each computer of an account once, two removed ones too.
#[test]
fn names_each_computer_once_two_removed_ones_too() {
    let own = limit(json!({
        "key": "claude@here", "agent": "claude", "label": "Claude Code signed in on here", "road": "own", "keyed": true,
        "computers": ["here", "p_0a1b2c3d4e5f6a7b", "p_9f8e7d6c5b4a3f2e"]
    }));
    let places = vec![mac()];
    let a = AccountRowsAsk {
        name_of: Rc::new(move |id: &str| usage_computer_name(&places, id, &IndexMap::new())),
        agent_name: Rc::new(|_: &str| "Claude Code".to_string()),
        ..ask(vec![own])
    };
    let rows = rows_json(&a);
    assert_eq!(
        rows[0]["computers"],
        json!(["zingzy's MacBook Pro", "a removed computer"])
    );
}

// C10: no TS case; usage.ts:617. Sorted by agent, then label, in the root collation, not by code unit.
#[test]
fn sorts_by_agent_then_label() {
    let own = |agent: &str, computer: &str| {
        limit(
            json!({ "key": format!("{agent}@{computer}"), "agent": agent, "label": "x", "road": "own", "computers": [computer] }),
        )
    };
    let a = AccountRowsAsk {
        name_of: Rc::new(|id: &str| match id {
            "c1" => s("Zed"),
            "c2" => s("alpha"),
            other => s(other),
        }),
        agent_name: Rc::new(agent_name_of),
        ..ask(vec![
            own("opencode", "c1"),
            own("codex", "c1"),
            own("codex", "c2"),
            own("claude", "c1"),
        ])
    };
    let rows = rows_json(&a);
    assert_eq!(
        pick(&rows, &["key", "label"]),
        vec![
            json!(["claude@c1", "Claude Code signed in on Zed"]),
            json!(["codex@c2", "Codex signed in on alpha"]),
            json!(["codex@c1", "Codex signed in on Zed"]),
            json!(["opencode@c1", "OpenCode signed in on Zed"]),
        ]
    );
}
