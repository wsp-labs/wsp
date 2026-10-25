// SPDX-License-Identifier: AGPL-3.0-only
//! What a plan's limits and a range's use read as: who an account is, the sentences a reset ends in, the list price
//! of a turn's tokens off LiteLLM's table, the clock words a row's windows carry, and the words of a turn a limit
//! stopped. Port of the functions of packages/protocol/src/usage.ts at a3b870a6e; its shapes and flat constants are
//! wsp_wire::types::usage. See spec/usage.md.

use wsp_wire::types::usage::AccountRow;
use wsp_wire::types::usage::LimitKind;
use wsp_wire::types::usage::LimitWindow;
use wsp_wire::types::usage::UsageRange;
use wsp_wire::types::usage::UsageSplit;
use wsp_wire::types::usage::UsedAnswer;
use wsp_wire::types::usage::UsedAnswerLogs;
use wsp_wire::types::usage::UsedRow;

use indexmap::IndexMap;
use serde::Serialize;

use wsp_wire::types::usage::AccountNote;
use wsp_wire::types::usage::LimitStatus;
use wsp_wire::types::usage::USAGE_WORDS;

use crate::js::js_round;
use crate::js::{number_string, upper_first};
use crate::words::units::fmt_cost;
use crate::words::units::fmt_tokens;
use crate::words::units::LIST_PRICE_WORD;

use jiff::civil::Weekday;
use jiff::tz::TimeZone;
use jiff::Timestamp;
use jiff::Zoned;

fn usage_word(key: &str) -> &'static str {
    USAGE_WORDS.iter().find(|(k, _)| *k == key).map(|(_, v)| *v).expect("a word the usage table holds")
}

fn zoned(at: f64, time_zone: Option<&str>) -> Zoned {
    let zone = match time_zone {
        Some(name) => TimeZone::get(name).expect("an IANA zone name"),
        None => TimeZone::system(),
    };
    Timestamp::from_millisecond(at as i64).expect("an instant in range").to_zoned(zone)
}

fn weekday_short(day: Weekday) -> &'static str {
    match day {
        Weekday::Monday => "Mon",
        Weekday::Tuesday => "Tue",
        Weekday::Wednesday => "Wed",
        Weekday::Thursday => "Thu",
        Weekday::Friday => "Fri",
        Weekday::Saturday => "Sat",
        Weekday::Sunday => "Sun",
    }
}

// The windows a plan counts use over, as wsp names them: an agent's five-hour window is the session, its seven-day
// the week, a per-model week its own kind, and anything longer the month.
// usage.ts:11
// in wsp_wire::types::usage: LIMIT_KINDS

// usage.ts:12
// in wsp_wire::types::usage: LimitKind

// usage.ts:13
// in wsp_wire::types::usage: LimitKind

// One window of a plan: how much of it is used, 0 to 100 and past 100 where the agent ran over, and when it
// starts again, ms epoch, where the agent said.
// usage.ts:17
// in wsp_wire::types::usage: LimitWindow

// usage.ts:18
// in wsp_wire::types::usage: LimitWindow

// usage.ts:20
// in wsp_wire::types::usage: LimitStatus

// usage.ts:21
// in wsp_wire::types::usage: LimitStatus

// usage.ts:23
// in wsp_wire::types::usage: RESET_CREDIT_STATUSES

// One reset a plan has banked, as its agent lists it: an id the agent spends it by, whether it can still be spent,
// and when it lapses, ms epoch, where it does.
// usage.ts:27
// in wsp_wire::types::usage: ResetCredit

// usage.ts:28
// in wsp_wire::types::usage: ResetCredit

// The resets a plan has banked: how many can be spent, and each one where the agent was asked for them in full,
// which it may cut short of the count.
// usage.ts:32
// in wsp_wire::types::usage: BankedResets

// usage.ts:33
// in wsp_wire::types::usage: BankedResets

// A plan's limits as a harness printed them during a turn. account is the harness's own name for the sign-in
// where it gives one; keyed is a sign-in by API key, which has no plan window at all. credits is absent on a reading
// that did not ask for them, which leaves the last one standing.
// usage.ts:38
// in wsp_wire::types::usage: HarnessLimit

// usage.ts:46
// in wsp_wire::types::usage: HarnessLimit

/// A window's length in minutes as a limit kind: five hours is the session and seven days the week.
// usage.ts:49
// export function limitKindOfMinutes(minutes: number): LimitKind {
//   if (minutes <= 300) return "session";
//   if (minutes <= 10_080) return "week";
//   return "month";
// }
pub fn limit_kind_of_minutes(minutes: f64) -> LimitKind {
    if minutes <= 300.0 {
        LimitKind::Session
    } else if minutes <= 10_080.0 {
        LimitKind::Week
    } else {
        LimitKind::Month
    }
}

// How a turn reached its account: one the harness named, the vault's token or key handed to every computer, or
// the login a computer keeps of its own.
// usage.ts:57
// in wsp_wire::types::usage: AccountRoad

// usage.ts:58
// in wsp_wire::types::usage: AccountRoad

// An account's limits as the host keeps them: the last reading any computer's turn gave, and every computer whose
// turns ran on it.
// usage.ts:62
// in wsp_wire::types::usage: AccountLimit

// usage.ts:76
// in wsp_wire::types::usage: AccountLimit

// Every sentence a usage row may carry.
// usage.ts:79
// in wsp_wire::types::usage: USAGE_WORDS

// Why an account row carries no windows: its agent reports none, it signs in with a key, or no turn has run on it.
// usage.ts:95
// in wsp_wire::types::usage: AccountNote

// usage.ts:96
// in wsp_wire::types::usage: AccountNote

// One sign-in as the Usage page lists it: the same account on three computers is one row naming all three.
// usage.ts:99
// in wsp_wire::types::usage: AccountRow

// usage.ts:117
// in wsp_wire::types::usage: AccountRow

// An account's row after a turn's limit reading folded into it, so a bound meter moves at once.
// usage.ts:120
// in wsp_wire::types::usage: UsageAccountEvent

// usage.ts:121
// in wsp_wire::types::usage: UsageAccountEvent

// usage.ts:123
// in wsp_wire::types::usage: AccountsAnswer

// usage.ts:124
// in wsp_wire::types::usage: AccountsAnswer

// What a press to spend a banked reset came to: the agent's four answers, a count that fell since an earlier press
// nobody answered, which spends nothing more, or an answer this code does not know, which is never read as a reset.
// usage.ts:128
// in wsp_wire::types::usage: RESET_OUTCOMES

// usage.ts:129
// in wsp_wire::types::usage: ResetOutcome

// usage.ts:130
// in wsp_wire::types::usage: ResetOutcome

// The outcome, the sentence that says it, and the account's row as the read after it left it.
// usage.ts:133
// in wsp_wire::types::usage: ResetAnswer

// usage.ts:134
// in wsp_wire::types::usage: ResetAnswer

/// The account a reset sentence is about: its label, and the computer it runs on where the label does not say.
// usage.ts:137
// export const resetWho = (o: { label: string; own: boolean; computer: string }): string => (o.own ? o.label : `${o.label} on ${o.computer}`);
pub fn reset_who(label: &str, own: bool, computer: &str) -> String {
    if own {
        label.to_string()
    } else {
        format!("{label} on {computer}")
    }
}

/// Every sentence a reset may end in, by what it came to.
// usage.ts:140
// export const RESET_WORDS = {
//   reset: (who: string, left?: number) => `Reset used: ${who}'s windows start again now${left === undefined ? "" : `, ${left} left`}`,
//   nothingToReset: (who: string) => `No window of ${who} is in use right now, so the credit was kept`,
//   noCredit: (who: string) => `No reset banked on ${who}`,
//   alreadyRedeemed: (who: string) => `That reset of ${who} was already used`,
//   earlier: (who: string, left: number) =>
//     `The count on ${who} fell since the earlier press: it may have gone through, or a reset expired or was used elsewhere. Nothing more was spent; ${left} left.`,
//   unknown: (agent: string, outcome: string, who: string) => `${agent} answered ${outcome} for ${who}; the account was read again`,
// } as const;
/// The TS RESET_WORDS, one function per entry.
pub mod reset_words {
    use crate::js::number_string;

    /// `left` None is the TS undefined.
    pub fn reset(who: &str, left: Option<f64>) -> String {
        let left = left.map(|n| format!(", {} left", number_string(n))).unwrap_or_default();
        format!("Reset used: {who}'s windows start again now{left}")
    }
    pub fn nothing_to_reset(who: &str) -> String {
        format!("No window of {who} is in use right now, so the credit was kept")
    }
    pub fn no_credit(who: &str) -> String {
        format!("No reset banked on {who}")
    }
    pub fn already_redeemed(who: &str) -> String {
        format!("That reset of {who} was already used")
    }
    pub fn earlier(who: &str, left: f64) -> String {
        format!("The count on {who} fell since the earlier press: it may have gone through, or a reset expired or was used elsewhere. Nothing more was spent; {} left.", number_string(left))
    }
    pub fn unknown(agent: &str, outcome: &str, who: &str) -> String {
        format!("{agent} answered {outcome} for {who}; the account was read again")
    }
}

/// The one question a spend asks first, in the command line and the app alike.
// usage.ts:151
// export function resetQuestion(who: string, count: number | undefined): string {
//   const which = count === undefined ? "a reset" : count === 1 ? "the one reset" : `one of the ${count} resets`;
//   return `Use ${which} banked on ${who}? Its five-hour and weekly windows start again now. There is no undo.`;
// }
pub fn reset_question(who: &str, count: Option<f64>) -> String {
    let which = match count {
        None => "a reset".to_string(),
        Some(1.0) => "the one reset".to_string(),
        Some(n) => format!("one of the {} resets", number_string(n)),
    };
    format!("Use {which} banked on {who}? Its five-hour and weekly windows start again now. There is no undo.")
}

// usage.ts:156
// export const resetKeyedLine = (label: string): string => `${label} pays per token; an API key has no reset to use`;
pub fn reset_keyed_line(label: &str) -> String {
    format!("{label} pays per token; an API key has no reset to use")
}

// usage.ts:157
// export const resetSilentLine = (agent: string, computer: string): string => `${agent} on ${computer} did not answer; the same request goes again on the next press`;
pub fn reset_silent_line(agent: &str, computer: &str) -> String {
    format!("{agent} on {computer} did not answer; the same request goes again on the next press")
}

// usage.ts:158
// export const resetUnreadLine = (agent: string, computer: string, label: string): string => `${agent} on ${computer} did not read ${label}'s limits; nothing was spent`;
pub fn reset_unread_line(agent: &str, computer: &str, label: &str) -> String {
    format!("{agent} on {computer} did not read {label}'s limits; nothing was spent")
}

// usage.ts:159
// export const resetMismatchLine = (agent: string, computer: string, other: string, label: string): string => `${agent} on ${computer} is now signed in as ${other}, not ${label}; nothing was spent`;
pub fn reset_mismatch_line(agent: &str, computer: &str, other: &str, label: &str) -> String {
    format!("{agent} on {computer} is now signed in as {other}, not {label}; nothing was spent")
}

// usage.ts:160
// export const resetRefusedLine = (agent: string, computer: string, label: string, said: string): string => `${agent} on ${computer} refused the reset of ${label}: ${said}`;
pub fn reset_refused_line(agent: &str, computer: &str, label: &str, said: &str) -> String {
    format!("{agent} on {computer} refused the reset of {label}: {said}")
}

// usage.ts:161
// export const resetProviderOnlyLine = (label: string, provider: string, agent: string): string => `${label} lives only on ${provider} machines; sign ${agent} in on a computer of yours`;
pub fn reset_provider_only_line(label: &str, provider: &str, agent: &str) -> String {
    format!("{label} lives only on {provider} machines; sign {agent} in on a computer of yours")
}

// usage.ts:162
// export const resetSignedOutLine = (label: string, agent: string): string => `${label} is signed in on none of your computers now; sign ${agent} in on one of them`;
pub fn reset_signed_out_line(label: &str, agent: &str) -> String {
    format!("{label} is signed in on none of your computers now; sign {agent} in on one of them")
}

// usage.ts:163
// export const resetNotOnLine = (label: string, computer: string, on: readonly string[]): string => `${label} is not signed in on ${computer}; it is on ${listWords(on)}`;
pub fn reset_not_on_line(label: &str, computer: &str, on: &[impl AsRef<str>]) -> String {
    format!("{label} is not signed in on {computer}; it is on {}", list_words(on))
}

// usage.ts:164
// export const resetNoLoginsLine = (computer: string, agent: string): string =>
//   `${computer} has not said where it keeps its logins, so ${agent}'s there cannot be reached; wsp add ${computer} --update puts a newer agent on it`;
pub fn reset_no_logins_line(computer: &str, agent: &str) -> String {
    format!("{computer} has not said where it keeps its logins, so {agent}'s there cannot be reached; wsp add {computer} --update puts a newer agent on it")
}

// usage.ts:166
// export const resetNoneLine = (agent: string): string => `${agent} banks no resets`;
pub fn reset_none_line(agent: &str) -> String {
    format!("{agent} banks no resets")
}

// usage.ts:167
// export const noSuchAccountLine = (word: string): string => `no account ${word}; wsp usage lists every account`;
pub fn no_such_account_line(word: &str) -> String {
    format!("no account {word}; wsp usage lists every account")
}

// usage.ts:169
// in wsp_wire::types::usage: USAGE_RANGES

// usage.ts:170
// in wsp_wire::types::usage: UsageRange

// usage.ts:171
// in wsp_wire::types::usage: UsageRange

// usage.ts:173
// in wsp_wire::types::usage: USAGE_SPLITS

// usage.ts:174
// in wsp_wire::types::usage: UsageSplit

// usage.ts:175
// in wsp_wire::types::usage: UsageSplit

/// The read of what was used that the Usage page, wsp usage and its tool all send, so every split counts the same:
/// wsp's threads and the work the agents logged outside wsp, where the person has not turned that reading off.
// usage.ts:179
// export const usedAsk = (range: UsageRange, split: UsageSplit): { range: UsageRange; split: UsageSplit; outside: true } => ({ range, split, outside: true });
/// The TS `{ range, split, outside: true }`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UsedAsk {
    pub range: UsageRange,
    pub split: UsageSplit,
    pub outside: bool,
}

pub fn used_ask(range: &UsageRange, split: &UsageSplit) -> UsedAsk {
    UsedAsk { range: *range, split: *split, outside: true }
}

// The days a range reaches back over, today included.
// usage.ts:182
// in wsp_wire::types::usage: RANGE_DAYS

// The ranges a computer's readings are read over: the last fifteen minutes the Computers list draws live, and the
// Usage page's own.
// usage.ts:186
// in wsp_wire::types::usage: READINGS_RANGES

// usage.ts:187
// in wsp_wire::types::usage: ReadingsRange

// usage.ts:188
// in wsp_wire::types::usage: ReadingsRange

// How far back each readings range reaches from now.
// usage.ts:191
// in wsp_wire::types::usage: READINGS_SPAN_MS

// The step a computer's chart folds its readings into for each range, so an answer is a few hundred points. Under a
// minute the daemon answers its fifteen-second reads themselves.
// usage.ts:195
// in wsp_wire::types::usage: READINGS_STEP_MS

// What a usage row is filed under: the day and hour in this computer's zone, and the four splits and the model.
// usage.ts:198
// in wsp_wire::types::usage: UsageKey

// usage.ts:207
// in wsp_wire::types::usage: UsageKey

// usage.ts:209
// in wsp_wire::types::usage: UsageTokens

// usage.ts:210
// in wsp_wire::types::usage: UsageTokens

// One key's use: its turns and tokens, the cost the harness itself reported where it did, and where it was read:
// a turn wsp ran, or a harness's own log of work done outside wsp on this computer. unreported holds the part of the
// tokens that came with no cost in a row that also holds a reported cost, so they are still priced at list.
// usage.ts:215
// in wsp_wire::types::usage: UsageRow

// usage.ts:222
// in wsp_wire::types::usage: UsageRow

// A day's rows, one document per day, with the harness sessions wsp's own turns were filed under that day: what a
// read of the logs leaves out, so no turn counts twice.
// usage.ts:226
// in wsp_wire::types::usage: UsageDay

// usage.ts:227
// in wsp_wire::types::usage: UsageDay

// One split value's use over a range, wsp's own turns and the agents' logs together. input counts every token the
// model read, the cached ones included. costList is the rate table's figure for the tokens no harness put a cost on,
// and priced is false where some of them have no entry in it.
// usage.ts:232
// in wsp_wire::types::usage: UsedRow

// usage.ts:248
// in wsp_wire::types::usage: UsedRow

// usage.ts:250
// in wsp_wire::types::usage: UsedAnswer

// usage.ts:264
// in wsp_wire::types::usage: UsedAnswer

/// The tokens a row read fresh, the cached and the written ones taken out of its input, so fresh, written, cached and
/// out add up to the row.
// usage.ts:268
// export const freshIn = (tokens: { input: number; cached: number; cacheWrite?: number | undefined }): number => Math.max(0, tokens.input - tokens.cached - (tokens.cacheWrite ?? 0));
pub fn fresh_in(input: f64, cached: f64, cache_write: Option<f64>) -> f64 {
    (input - cached - cache_write.unwrap_or(0.0)).max(0.0)
}

/// A list as a sentence reads it: "a", "a and b", "a, b and c".
// usage.ts:271
// export const listWords = (words: readonly string[]): string => (words.length < 2 ? (words[0] ?? "") : `${words.slice(0, -1).join(", ")} and ${words[words.length - 1]}`);
pub fn list_words(words: &[impl AsRef<str>]) -> String {
    match words {
        [] => String::new(),
        [only] => only.as_ref().to_string(),
        [init @ .., last] => {
            let head: Vec<&str> = init.iter().map(AsRef::as_ref).collect();
            format!("{} and {}", head.join(", "), last.as_ref())
        }
    }
}

/// What the logs a range counted are, said once under its numbers: the agents that kept them and where.
// usage.ts:274
// export const logsLine = (logs: { agents: readonly string[]; computers: readonly string[] }): string => `Counts what ${listWords(logs.agents)} logged on ${listWords(logs.computers)}, wsp's threads there included.`;
pub fn logs_line(logs: &UsedAnswerLogs) -> String {
    format!("Counts what {} logged on {}, wsp's threads there included.", list_words(&logs.agents), list_words(&logs.computers))
}

// usage.ts:276
// private in the TS: port it as a private helper
// const RANGE_WORDS: Record<UsageRange, string> = { day: "today", week: "in the last 7 days", month: "in the last 30 days" };

/// A range's use in one line: the tokens, what they cost and how much of it came from cache.
// usage.ts:279
// export function usedHeadline(used: Pick<UsedAnswer, "range" | "rows">): string {
//   const sum = (pick: (row: UsedRow) => number): number => used.rows.reduce((n, row) => n + pick(row), 0);
//   const tokens = sum(row => row.tokens.input + row.tokens.output);
//   if (tokens === 0) return USAGE_WORDS.noUse;
//   const priced = used.rows.some(r => r.costReported !== undefined || r.costList !== undefined);
//   const listed = used.rows.some(r => (r.costList ?? 0) > 0);
//   const cost = priced ? `, ${fmtCost(sum(r => (r.costReported ?? 0) + (r.costList ?? 0)))}${listed ? ` at ${LIST_PRICE_WORD}` : ""}` : "";
//   const cached = sum(row => row.tokens.cached);
//   const unpriced = used.rows.some(r => !r.priced) ? " Some of it has no price to read, so the figure leaves it out." : "";
//   return `${fmtTokens(tokens)} tokens ${RANGE_WORDS[used.range]}${cost}${cached > 0 ? `, ${fmtTokens(cached)} of it read from cache` : ""}.${unpriced}`;
// }
pub fn used_headline(used: &UsedAnswer) -> String {
    let sum = |pick: &dyn Fn(&UsedRow) -> f64| used.rows.iter().fold(0.0, |n, row| n + pick(row));
    let tokens = sum(&|r: &UsedRow| r.tokens.input.0 + r.tokens.output.0);
    if tokens == 0.0 {
        return usage_word("noUse").to_string();
    }
    let priced = used.rows.iter().any(|r| r.cost_reported.is_some() || r.cost_list.is_some());
    let listed = used.rows.iter().any(|r| r.cost_list.as_ref().map_or(0.0, |n| n.0) > 0.0);
    let cost = if priced {
        let at = if listed { format!(" at {LIST_PRICE_WORD}") } else { String::new() };
        let total = sum(&|r: &UsedRow| r.cost_reported.as_ref().map_or(0.0, |n| n.0) + r.cost_list.as_ref().map_or(0.0, |n| n.0));
        format!(", {}{at}", fmt_cost(total))
    } else {
        String::new()
    };
    let cached = sum(&|r: &UsedRow| r.tokens.cached.0);
    let cache = if cached > 0.0 { format!(", {} of it read from cache", fmt_tokens(cached)) } else { String::new() };
    let unpriced = if used.rows.iter().any(|r| !r.priced) { " Some of it has no price to read, so the figure leaves it out." } else { "" };
    let range = match used.range {
        UsageRange::Day => "today",
        UsageRange::Week => "in the last 7 days",
        UsageRange::Month => "in the last 30 days",
    };
    format!("{} tokens {range}{cost}{cache}.{unpriced}", fmt_tokens(tokens))
}

/// An account as a person reads it: the agent and how it pays, by key or by its plan, else the address it signed in
/// as, else your sign-in wsp hands every computer, else the login one computer keeps of its own.
// usage.ts:293
// export function accountWords(o: { agentName: string; keyed?: boolean | undefined; plan?: string | undefined; planBrand?: string | undefined; named?: string | undefined; vaulted?: boolean | undefined; ownOn?: string | undefined }): string {
//   if (o.keyed === true) return `${o.agentName} with an API key`;
//   if (o.plan !== undefined && o.plan !== "") {
//     const plan = o.plan[0]!.toUpperCase() + o.plan.slice(1);
//     return `${o.agentName} with ${o.planBrand === undefined ? plan : `${o.planBrand} ${plan}`}`;
//   }
//   if (o.named !== undefined) return `${o.agentName} as ${o.named}`;
//   if (o.vaulted === true) return `${o.agentName} with your sign-in`;
//   return o.ownOn === undefined ? o.agentName : `${o.agentName} signed in on ${o.ownOn}`;
// }
/// The TS object `accountWords` reads; None is undefined.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountWho {
    pub agent_name: String,
    pub keyed: Option<bool>,
    pub plan: Option<String>,
    pub plan_brand: Option<String>,
    pub named: Option<String>,
    pub vaulted: Option<bool>,
    pub own_on: Option<String>,
}

pub fn account_words(o: &AccountWho) -> String {
    if o.keyed == Some(true) {
        return format!("{} with an API key", o.agent_name);
    }
    if let Some(plan) = o.plan.as_deref().filter(|p| !p.is_empty()) {
        let plan = upper_first(plan);
        let text = match &o.plan_brand {
            None => plan,
            Some(brand) => format!("{brand} {plan}"),
        };
        return format!("{} with {text}", o.agent_name);
    }
    if let Some(named) = &o.named {
        return format!("{} as {named}", o.agent_name);
    }
    if o.vaulted == Some(true) {
        return format!("{} with your sign-in", o.agent_name);
    }
    match &o.own_on {
        None => o.agent_name.clone(),
        Some(on) => format!("{} signed in on {on}", o.agent_name),
    }
}

// How long a price table is used before the host reads LiteLLM's file again.
// usage.ts:306
// in wsp_wire::types::usage: PRICES_TTL_MS

// usage.ts:307
// in wsp_wire::types::usage: PRICES_URL

/// A model's per-token prices in dollars. cacheRead and cacheWrite are absent where the table names none.
// usage.ts:310
// export interface Rate {
//   input: number;
//   output: number;
//   cacheRead?: number;
//   cacheWrite?: number;
//   provider?: string;
// }
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rate {
    pub input: f64,
    pub output: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

// usage.ts:317
// export type RateTable = Record<string, Rate>;
/// Models in the order the price file names them.
pub type RateTable = IndexMap<String, Rate>;

// usage.ts:319
// private in the TS: port it as a private helper
// const price = (value: unknown): number | undefined => (typeof value === "number" && Number.isFinite(value) && value >= 0 ? value : undefined);

/// The table off LiteLLM's price file: each model's four base per-token prices. The long-context, batch, flex and
/// priority twins beside them are not what a turn pays and are left out, and so is any entry with no input or output
/// price, the file's own description of its fields among them.
// usage.ts:324
// export function parseRateTable(document: unknown): RateTable {
//   if (typeof document !== "object" || document === null || Array.isArray(document)) return {};
//   const table: RateTable = {};
//   for (const [model, raw] of Object.entries(document)) {
//     if (typeof raw !== "object" || raw === null || model === "sample_spec") continue;
//     const entry = raw as Record<string, unknown>;
//     const input = price(entry["input_cost_per_token"]);
//     const output = price(entry["output_cost_per_token"]);
//     if (input === undefined || output === undefined) continue;
//     const cacheRead = price(entry["cache_read_input_token_cost"]);
//     const cacheWrite = price(entry["cache_creation_input_token_cost"]);
//     const provider = typeof entry["litellm_provider"] === "string" ? entry["litellm_provider"] : undefined;
//     table[model] = { input, output, ...(cacheRead !== undefined ? { cacheRead } : {}), ...(cacheWrite !== undefined ? { cacheWrite } : {}), ...(provider !== undefined ? { provider } : {}) };
//   }
//   return table;
// }
pub fn parse_rate_table(document: &serde_json::Value) -> RateTable {
    let Some(entries) = document.as_object() else {
        return RateTable::new();
    };
    let price = |v: Option<&serde_json::Value>| v.and_then(serde_json::Value::as_f64).filter(|n| *n >= 0.0);
    let mut table = RateTable::new();
    for (model, raw) in entries {
        let Some(entry) = raw.as_object() else {
            continue;
        };
        if model == "sample_spec" {
            continue;
        }
        let (Some(input), Some(output)) = (price(entry.get("input_cost_per_token")), price(entry.get("output_cost_per_token"))) else {
            continue;
        };
        let provider = entry.get("litellm_provider").and_then(serde_json::Value::as_str).map(str::to_string);
        let rate = Rate {
            input,
            output,
            cache_read: price(entry.get("cache_read_input_token_cost")),
            cache_write: price(entry.get("cache_creation_input_token_cost")),
            provider,
        };
        table.insert(model.clone(), rate);
    }
    table
}

/// A model as its maker names it, without the context window Claude Code writes after it, as in [1m].
// usage.ts:342
// export const baseModel = (model: string): string => model.replace(/\[[^\]]*\]$/, "");
pub fn base_model(model: &str) -> String {
    if !model.ends_with(']') {
        return model.to_string();
    }
    let end = model.len() - 1;
    let from = model[..end].rfind(']').map_or(0, |i| i + 1);
    match model[from..end].find('[') {
        Some(i) => model[..from + i].to_string(),
        None => model.to_string(),
    }
}

/// A model's entry as a harness names it: as it is, without a provider in front, or without a date or a context
/// window after it.
// usage.ts:346
// export function rateOf(model: string, table: RateTable): Rate | undefined {
//   const bare = baseModel(model.includes("/") ? model.slice(model.lastIndexOf("/") + 1) : model);
//   return table[model] ?? table[bare] ?? table[bare.replace(/-\d{8}$/, "")];
// }
pub fn rate_of(model: &str, table: &RateTable) -> Option<Rate> {
    let after_provider = match model.rfind('/') {
        Some(i) => &model[i + 1..],
        None => model,
    };
    let bare = base_model(after_provider);
    let undated = match bare.len().checked_sub(9) {
        Some(at) if bare.as_bytes()[at] == b'-' && bare.as_bytes()[at + 1..].iter().all(u8::is_ascii_digit) => bare[..at].to_string(),
        _ => bare.clone(),
    };
    table.get(model).or_else(|| table.get(bare.as_str())).or_else(|| table.get(undated.as_str())).cloned()
}

/// The list price of a turn's tokens, dollars, or nothing where the table has no entry for its model. input counts
/// every token the model read, the cached and the written part included, so each part is priced at its own rate.
// usage.ts:353
// export function priceOf(model: string, tokens: { input: number; output: number; cached?: number; cacheWrite?: number }, table: RateTable): number | undefined {
//   const rate = rateOf(model, table);
//   if (rate === undefined) return undefined;
//   const cached = tokens.cached ?? 0;
//   const written = tokens.cacheWrite ?? 0;
//   const fresh = Math.max(0, tokens.input - cached - written);
//   return fresh * rate.input + cached * (rate.cacheRead ?? rate.input) + written * (rate.cacheWrite ?? rate.input) + tokens.output * rate.output;
// }
/// The tokens `priceOf` prices; None is undefined.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PricedTokens {
    pub input: f64,
    pub output: f64,
    pub cached: Option<f64>,
    pub cache_write: Option<f64>,
}

pub fn price_of(model: &str, tokens: &PricedTokens, table: &RateTable) -> Option<f64> {
    let rate = rate_of(model, table)?;
    let cached = tokens.cached.unwrap_or(0.0);
    let written = tokens.cache_write.unwrap_or(0.0);
    let fresh = (tokens.input - cached - written).max(0.0);
    Some(
        fresh * rate.input
            + cached * rate.cache_read.unwrap_or(rate.input)
            + written * rate.cache_write.unwrap_or(rate.input)
            + tokens.output * rate.output,
    )
}

/// The calendar day an instant falls on in a zone, YYYY-MM-DD; this computer's own zone where none is named.
// usage.ts:363
// export function dayKeyOf(at: number, timeZone?: string): string {
//   const parts = new Intl.DateTimeFormat("en-CA", { year: "numeric", month: "2-digit", day: "2-digit", ...(timeZone !== undefined ? { timeZone } : {}) }).formatToParts(at);
//   const part = (type: string): string => parts.find(p => p.type === type)?.value ?? "";
//   return `${part("year")}-${part("month")}-${part("day")}`;
// }
pub fn day_key_of(at: f64, time_zone: Option<&str>) -> String {
    let z = zoned(at, time_zone);
    format!("{:04}-{:02}-{:02}", z.year(), z.month(), z.day())
}

/// The hour of the day an instant falls on in a zone, 0 to 23.
// usage.ts:370
// export function hourOf(at: number, timeZone?: string): number {
//   const hour = new Intl.DateTimeFormat("en-GB", { hour: "2-digit", hourCycle: "h23", ...(timeZone !== undefined ? { timeZone } : {}) }).format(at);
//   return Number(hour) % 24;
// }
pub fn hour_of(at: f64, time_zone: Option<&str>) -> f64 {
    zoned(at, time_zone).hour() as f64
}

/// When a window starts again, as a row's quiet word: minutes under an hour, hours under a day, then the weekday. The
/// one formatter for a reset wherever it shows: the Usage page's meters, the limit strip's fact and the armed tile.
// usage.ts:377
// export function resetsWord(resetsAt: number, now: number, timeZone?: string): string {
//   const left = resetsAt - now;
//   if (left <= 0) return "reset";
//   if (left < 3_600_000) return `resets in ${Math.max(1, Math.round(left / 60_000))} min`;
//   if (left < 86_400_000) return `resets in ${Math.floor(left / 3_600_000)} h`;
//   const weekday = new Intl.DateTimeFormat("en-US", { weekday: "short", ...(timeZone !== undefined ? { timeZone } : {}) }).format(resetsAt);
//   return `resets on ${weekday}`;
// }
pub fn resets_word(resets_at: f64, now: f64, time_zone: Option<&str>) -> String {
    let left = resets_at - now;
    if left <= 0.0 {
        return "reset".to_string();
    }
    if left < 3_600_000.0 {
        return format!("resets in {} min", number_string(js_round(left / 60_000.0).max(1.0)));
    }
    if left < 86_400_000.0 {
        return format!("resets in {} h", number_string((left / 3_600_000.0).floor()));
    }
    format!("resets on {}", weekday_short(zoned(resets_at, time_zone).weekday()))
}

// How long until a moment, as a row's quiet word: minutes under an hour, hours under a day, then days.
// usage.ts:387
// private in the TS: port it as a private helper
// const spanWord = (left: number): string => (left < 3_600_000 ? `${Math.max(1, Math.round(left / 60_000))} min` : left < 86_400_000 ? `${Math.floor(left / 3_600_000)} h` : `${Math.floor(left / 86_400_000)} d`);

/// The resets a plan has banked, as a row's word: how many, and when the first one lapses where the agent said.
// usage.ts:390
// export function creditsWord(credits: { count: number; nextExpiresAt?: number | undefined }, now: number): string {
//   if (credits.count === 0) return "none banked";
//   const banked = `${credits.count} banked`;
//   if (credits.nextExpiresAt === undefined || credits.nextExpiresAt <= now) return banked;
//   return `${banked}, ${credits.count === 1 ? "expires" : "first expires"} in ${spanWord(credits.nextExpiresAt - now)}`;
// }
/// `count` and `next_expires_at` are the TS `credits` object's fields.
pub fn credits_word(count: f64, next_expires_at: Option<f64>, now: f64) -> String {
    if count == 0.0 {
        return "none banked".to_string();
    }
    let banked = format!("{} banked", number_string(count));
    let span = |left: f64| {
        if left < 3_600_000.0 {
            format!("{} min", number_string(js_round(left / 60_000.0).max(1.0)))
        } else if left < 86_400_000.0 {
            format!("{} h", number_string((left / 3_600_000.0).floor()))
        } else {
            format!("{} d", number_string((left / 86_400_000.0).floor()))
        }
    };
    match next_expires_at {
        Some(next) if next > now => {
            let which = if count == 1.0 { "expires" } else { "first expires" };
            format!("{banked}, {which} in {}", span(next - now))
        }
        _ => banked,
    }
}

/// A moment as a clock reads it: the time on `now`'s day, else the weekday and time.
// usage.ts:398
// export function clockWord(at: number, now: number, timeZone?: string): string {
//   const zone = timeZone !== undefined ? { timeZone } : {};
//   const time = new Intl.DateTimeFormat("en-GB", { hour: "2-digit", minute: "2-digit", hourCycle: "h23", ...zone }).format(at);
//   if (dayKeyOf(at, timeZone) === dayKeyOf(now, timeZone)) return time;
//   return `${new Intl.DateTimeFormat("en-US", { weekday: "short", ...zone }).format(at)} ${time}`;
// }
pub fn clock_word(at: f64, now: f64, time_zone: Option<&str>) -> String {
    let z = zoned(at, time_zone);
    let time = format!("{:02}:{:02}", z.hour(), z.minute());
    if day_key_of(at, time_zone) == day_key_of(now, time_zone) {
        return time;
    }
    format!("{} {time}", weekday_short(z.weekday()))
}

/// When a limit was last read, as the row's quiet word: the time today, else the weekday and time.
// usage.ts:406
// export function asOfWord(readAt: number, now: number, timeZone?: string): string {
//   return `as of ${clockWord(readAt, now, timeZone)}`;
// }
pub fn as_of_word(read_at: f64, now: f64, time_zone: Option<&str>) -> String {
    format!("as of {}", clock_word(read_at, now, time_zone))
}

/// A used row's price as a row reads it: the figure, with not priced where any of its tokens had no price, since
/// the figure then leaves them out, else list price where any of it came off the table; nothing where no turn
/// carried a price at all.
// usage.ts:413
// export function usedPrice(row: Pick<UsedRow, "costReported" | "costList" | "priced">): { figure?: string; word?: string } {
//   const word = !row.priced ? USAGE_WORDS.notPriced : row.costList !== undefined && row.costList > 0 ? USAGE_WORDS.listPrice : undefined;
//   const figure = row.costReported === undefined && row.costList === undefined ? undefined : fmtCost((row.costReported ?? 0) + (row.costList ?? 0));
//   return { ...(figure !== undefined ? { figure } : {}), ...(word !== undefined ? { word } : {}) };
// }
/// The TS `{ figure?, word? }`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct UsedPrice {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub figure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word: Option<String>,
}

pub fn used_price(row: &UsedRow) -> UsedPrice {
    let word = if !row.priced {
        Some(usage_word("notPriced"))
    } else if row.cost_list.as_ref().is_some_and(|n| n.0 > 0.0) {
        Some(usage_word("listPrice"))
    } else {
        None
    };
    let figure = if row.cost_reported.is_none() && row.cost_list.is_none() {
        None
    } else {
        Some(fmt_cost(row.cost_reported.as_ref().map_or(0.0, |n| n.0) + row.cost_list.as_ref().map_or(0.0, |n| n.0)))
    };
    UsedPrice { figure, word: word.map(str::to_string) }
}

/// A window as a row's cell: how much of it is used and when it starts again.
// usage.ts:420
// export function windowCell(w: LimitWindow, now: number, timeZone?: string): string {
//   const used = `${Math.round(w.usedPercent)}%`;
//   return w.resetsAt === undefined ? used : `${used} ${resetsWord(w.resetsAt, now, timeZone)}`;
// }
pub fn window_cell(w: &LimitWindow, now: f64, time_zone: Option<&str>) -> String {
    let used = format!("{}%", number_string(js_round(w.used_percent.0)));
    match w.resets_at.as_ref() {
        None => used,
        Some(at) => format!("{used} {}", resets_word(at.0, now, time_zone)),
    }
}

/// An account row's state cell: limit reached where the agent said so, else the note that says why it has no windows.
// usage.ts:426
// export function accountState(row: Pick<AccountRow, "status" | "note">): string {
//   return row.status === "reached" ? USAGE_WORDS.reached : (row.note ?? "");
// }
pub fn account_state(row: &AccountRow) -> String {
    if row.status == Some(LimitStatus::Reached) {
        return usage_word("reached").to_string();
    }
    match row.note {
        None => String::new(),
        Some(AccountNote::ReportsNoPlanLimit) => usage_word("noLimit").to_string(),
        Some(AccountNote::PaysPerTokenNoPlanLimit) => usage_word("keyed").to_string(),
        Some(AccountNote::NotReadYetShowsAfterItsNextTurn) => usage_word("unread").to_string(),
    }
}

// A turn an agent's usage limit stopped: the agent said it may not run more now, and when its plan starts again
// where it said that too, ms epoch. Read off the readings the adapters already take, never off the agent's words.
// usage.ts:432
// in wsp_wire::types::usage: TurnLimit

// usage.ts:433
// in wsp_wire::types::usage: TurnLimit

/// Every sentence a turn stopped by a usage limit carries, in the thread, on its tile and in the timeline.
// usage.ts:436
// export const LIMIT_WORDS = {
//   /** The strip's title while the turn stands stopped. */
//   reached: "Usage limit reached",
//   /** The strip's title once Resume at reset is armed. */
//   resuming: "Resumes at the reset",
//   /** The tile's state word, in the warning ink the Usage page reads a reached limit in. */
//   tile: "At limit",
//   /** Under the title: the agent stopped this turn, and whether it said when it can go on. */
//   stopped: (agent: string): string => `${agent} stopped this turn until its plan resets.`,
//   stoppedUnknown: (agent: string): string => `${agent} stopped this turn and did not say when its plan resets.`,
//   armed: (agent: string): string => `${agent} goes on with this turn when its plan resets.`,
//   /** The drawer's row: the clock time the agent's plan lets it go on, from `clockWord()`. */
//   resumesAt: (agent: string, clock: string): string => `${agent} resumes at ${clock}`,
//   resume: "Resume at reset",
//   cancel: "Cancel",
//   /** The timeline's record of the turn going on, in the runtime notice's mono; the fold above it says when. */
//   resumed: "resumed at the reset",
// } as const;
/// The TS LIMIT_WORDS: its fixed words as constants, its sentences as functions.
pub mod limit_words {
    pub const REACHED: &str = "Usage limit reached";
    pub const RESUMING: &str = "Resumes at the reset";
    pub const TILE: &str = "At limit";
    pub const RESUME: &str = "Resume at reset";
    pub const CANCEL: &str = "Cancel";
    pub const RESUMED: &str = "resumed at the reset";
    pub fn stopped(agent: &str) -> String {
        format!("{agent} stopped this turn until its plan resets.")
    }
    pub fn stopped_unknown(agent: &str) -> String {
        format!("{agent} stopped this turn and did not say when its plan resets.")
    }
    pub fn armed(agent: &str) -> String {
        format!("{agent} goes on with this turn when its plan resets.")
    }
    pub fn resumes_at(agent: &str, clock: &str) -> String {
        format!("{agent} resumes at {clock}")
    }
}

// usage.ts:457
// in wsp_wire::types::usage: LIMIT_RESUME_PROMPT

/// The reset a plan's latest reading still holds the agent behind at `now`: the last of the windows at or past their
/// cap that have not started again yet. Undefined where none is, which is a plan the agent may run on.
// usage.ts:461
// export function heldUntil(windows: readonly LimitWindow[], now: number): number | undefined {
//   const held = windows.filter(w => w.usedPercent >= 100 && w.resetsAt !== undefined && w.resetsAt > now).map(w => w.resetsAt!);
//   return held.length === 0 ? undefined : Math.max(...held);
// }
pub fn held_until(windows: &[LimitWindow], now: f64) -> Option<f64> {
    let held: Vec<f64> = windows
        .iter()
        .filter(|w| w.used_percent.0 >= 100.0)
        .filter_map(|w| w.resets_at.as_ref().map(|r| r.0))
        .filter(|r| *r > now)
        .collect();
    if held.is_empty() {
        None
    } else {
        Some(held.iter().copied().fold(f64::NEG_INFINITY, f64::max))
    }
}
