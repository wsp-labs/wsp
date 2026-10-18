// SPDX-License-Identifier: AGPL-3.0-only
//! Every word the Usage page shows, as the web writes them: `USAGE_WORDS` and the words functions of
//! `packages/protocol/src/usage.ts`, `USAGE_PAGE_WORDS`, `RESET_LINE_WORDS` and `ABOUT_WORDS.readWhen` of
//! `apps/web/src/settings/format.ts`, the units of `words/units.ts`, and the chart's date words. Dates read in the zone
//! passed in: `chrono::Local` in the app, a fixed offset in tests.

use chrono::{DateTime, Datelike, TimeZone, Timelike};
use wsp_host_client::protocol::{
    AccountsAnswerAccountsItemNote, AccountsAnswerAccountsItemStatus, AccountsAnswerAccountsItemWindowsItemKind,
};

use super::model::{Range, Split};

pub const NOT_PRICED: &str = "not priced";
pub const UNREAD_NOTE: &str = "not read yet: shows after its next turn";
/// `USAGE_WORDS.reached`, the state word; the pool's verdict says `REACHED`.
pub const LIMIT_REACHED: &str = "limit reached";
pub const NO_USE: &str = "Nothing used in this range";

pub const SETTINGS: &str = "Settings";
pub const TITLE: &str = "Usage";
pub const TAB_USED: &str = "Usage";
pub const TAB_LIMITS: &str = "Limits";
pub const READ_AGAIN: &str = "Read usage and limits again";
pub const TOKENS: &str = "Tokens";
pub const FRESH: &str = "Fresh in";
pub const OUT: &str = "Out";
pub const CACHED: &str = "Cached";
pub const NO_ACCOUNTS: &str = "No agent is signed in on any computer.";
pub const REACHED: &str = "Limit reached";
pub const LEFT: &str = "left";
pub const NO_PLAN_LIMIT: &str = "No plan limit";
pub const ESTIMATE: &str = "API estimate";
pub const AT_LIST_PRICE: &str = "at list price";
pub const TURNS: &str = "Turns";
pub const TURNS_NOTE: &str = "in threads wsp ran";
pub const CACHE_HIT: &str = "Cache hit";
pub const CACHE_WRITE: &str = "Cache write";
pub const MIX: &str = "Where the tokens went";
pub const AGENT_AND_MODEL: &str = "Agent and model";

pub const RESETS: &str = "Resets";
pub const USE_RESET: &str = "Use reset";
pub const USING_RESET: &str = "Using reset";
pub const CANCEL: &str = "Cancel";

pub fn limits_refused(said: &str) -> String {
    format!("The plan limits could not be read: {said}")
}

pub fn used_refused(said: &str) -> String {
    format!("The usage could not be read: {said}")
}

/// A window's name, or its wire id where the page names none (`overage`).
pub fn window_word(kind: AccountsAnswerAccountsItemWindowsItemKind) -> String {
    use AccountsAnswerAccountsItemWindowsItemKind as K;
    match kind {
        K::Session => "5-hour".into(),
        K::Week => "Week".into(),
        K::WeekOpus => "Week, Opus".into(),
        K::WeekSonnet => "Week, Sonnet".into(),
        K::Month => "Month".into(),
        K::Overage => kind.to_string(),
    }
}

pub fn runs_out(when: &str) -> String {
    format!("Runs out {when}")
}

fn with_resets(head: &str, resets: &str) -> String {
    if resets.is_empty() {
        head.to_string()
    } else {
        format!("{head}, {resets}")
    }
}

pub fn ahead_of_pace(resets: &str) -> String {
    with_resets("Ahead of pace", resets)
}

pub fn under_pace(resets: &str) -> String {
    with_resets("Under pace", resets)
}

pub fn on_pace(resets: &str) -> String {
    with_resets("On pace", resets)
}

pub fn accounts(n: usize, agent: &str) -> String {
    format!("{n} {agent} accounts")
}

pub fn burn(tokens: &str, threads: i64) -> String {
    format!("Using {tokens} tokens a minute across {threads} {}", if threads == 1 { "thread" } else { "threads" })
}

pub fn checked(when: &str) -> String {
    format!("checked {when}")
}

pub fn no_limit(agents: &[impl AsRef<str>]) -> String {
    format!("{} {} no plan limit.", list_words(agents), if agents.len() == 1 { "reports" } else { "report" })
}

pub fn range_word(range: Range) -> &'static str {
    match range {
        Range::Day => "Today",
        Range::Week => "7 days",
        Range::Month => "30 days",
    }
}

pub fn split_word(split: Split) -> &'static str {
    match split {
        Split::Agent => "Agent",
        Split::Account => "Account",
        Split::Computer => "Computer",
        Split::Project => "Project",
        Split::Model => "Model",
        Split::Source => "Source",
    }
}

pub fn chart_head(range: Range) -> &'static str {
    match range {
        Range::Day => "Tokens an hour",
        Range::Week | Range::Month => "Tokens a day",
    }
}

/// The one legend entry of a chart drawn as a single total line.
pub fn all_of(split: Split) -> &'static str {
    match split {
        Split::Agent => "All agents together",
        Split::Account => "All accounts together",
        Split::Computer => "All computers together",
        Split::Project => "All projects together",
        Split::Model => "All models together",
        Split::Source => "All sources together",
    }
}

pub fn by(split: &str) -> String {
    format!("By {}", split.to_lowercase())
}

pub fn saved(amount: &str) -> String {
    format!("{amount} saved")
}

/// What the totals count: wsp's threads, and the agents' own logs beside them where the range read any.
pub fn counts(outside: bool) -> &'static str {
    if outside {
        "in wsp threads and outside wsp"
    } else {
        "in wsp threads only"
    }
}

/// The catalog's name for an agent; an id the catalog does not know reads as itself.
pub fn agent_name(agent: &str) -> String {
    crate::ui::catalog::name(agent)
}

/// `accountWords({ agentName, ownOn })`: how a computer's own login of an agent is titled.
pub fn signed_in_on(agent_name: &str, computer: &str) -> String {
    format!("{agent_name} signed in on {computer}")
}

/// A state word as the page draws it, capitalised: the wire's words are the command line's, in its lower case.
pub fn state_word(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

pub fn account_state(status: Option<AccountsAnswerAccountsItemStatus>, note: Option<AccountsAnswerAccountsItemNote>) -> String {
    match (status, note) {
        (Some(AccountsAnswerAccountsItemStatus::Reached), _) => LIMIT_REACHED.to_string(),
        (_, Some(note)) => note.to_string(),
        _ => String::new(),
    }
}

pub fn reset_question(who: &str, count: Option<u64>) -> String {
    let which = match count {
        None => "a reset".to_string(),
        Some(1) => "the one reset".to_string(),
        Some(n) => format!("one of the {n} resets"),
    };
    format!("Use {which} banked on {who}? Its five-hour and weekly windows start again now. There is no undo.")
}

pub fn list_words(words: &[impl AsRef<str>]) -> String {
    match words {
        [] => String::new(),
        [one] => one.as_ref().to_string(),
        [head @ .., last] => {
            let head: Vec<&str> = head.iter().map(AsRef::as_ref).collect();
            format!("{} and {}", head.join(", "), last.as_ref())
        }
    }
}

pub fn logs_line(agents: &[impl AsRef<str>], computers: &[impl AsRef<str>]) -> String {
    format!("Counts what {} logged on {}, wsp's threads there included.", list_words(agents), list_words(computers))
}

/// JS `Math.round`: halves go up, toward positive infinity.
pub(crate) fn js_round(x: f64) -> f64 {
    if x - x.floor() == 0.5 {
        x.ceil()
    } else {
        x.round()
    }
}

/// A magnitude's decimal digits rounded half up at `places`, off its integer and fraction digits.
pub(crate) fn round_half_up(int: &str, frac: &str, places: usize) -> String {
    let mut digits: Vec<u8> = int.bytes().chain(frac.bytes().chain(std::iter::repeat(b'0')).take(places)).collect();
    if frac.as_bytes().get(places).is_some_and(|d| *d >= b'5') {
        let mut i = digits.len();
        loop {
            if i == 0 {
                digits.insert(0, b'1');
                break;
            }
            i -= 1;
            if digits[i] == b'9' {
                digits[i] = b'0';
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    let split = digits.len() - places;
    let (whole, fraction) = digits.split_at(split);
    let whole = String::from_utf8_lossy(whole);
    if places == 0 {
        whole.into_owned()
    } else {
        format!("{whole}.{}", String::from_utf8_lossy(fraction))
    }
}

/// `Number.prototype.toFixed` for finite values under 1e21: the double's exact value rounded, a tie up.
pub(crate) fn to_fixed(x: f64, places: usize) -> String {
    let magnitude = x.abs();
    let bits = magnitude.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i64;
    // A double is m * 2^q, and its exact decimal expansion has -q digits after the point.
    let q = if exponent == 0 { -1074 } else { exponent - 1075 };
    let exact = format!("{magnitude:.*}", (-q).max(0) as usize);
    let (int, frac) = exact.split_once('.').unwrap_or((&exact, ""));
    let fixed = round_half_up(int, frac, places);
    if x < 0.0 {
        format!("-{fixed}")
    } else {
        fixed
    }
}

pub(crate) fn group_thousands(int: &str) -> String {
    let mut out = String::with_capacity(int.len() + int.len() / 3);
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A token count in three figures and its unit, no trailing zero: 4.27k, 22.6M, 7.37B, 687M; under a thousand as is.
pub fn fmt_tokens(n: f64) -> String {
    let count = if n.is_finite() && n > 0.0 { js_round(n) } else { 0.0 };
    if count < 1_000.0 {
        return format!("{count}");
    }
    let (value, unit) = if count < 999_500.0 {
        (count / 1_000.0, "k")
    } else if count < 999_500_000.0 {
        (count / 1_000_000.0, "M")
    } else {
        (count / 1_000_000_000.0, "B")
    };
    let places = match value {
        v if v < 10.0 => 2,
        v if v < 100.0 => 1,
        _ => 0,
    };
    let fixed = to_fixed(value, places);
    let trimmed = if fixed.contains('.') { fixed.trim_end_matches('0').trim_end_matches('.') } else { &fixed };
    format!("{trimmed}{unit}")
}

/// Dollars as en-US `toLocaleString` writes them with two decimals: grouped, halves away from zero, "$-1.00".
pub fn fmt_cost(usd: f64) -> String {
    if usd.is_nan() {
        return "$NaN".into();
    }
    let sign = if usd.is_sign_negative() { "-" } else { "" };
    if usd.is_infinite() {
        return format!("${sign}\u{221e}");
    }
    // Intl rounds the shortest decimal that reads back as the double, which Rust's Display writes.
    let shortest = format!("{}", usd.abs());
    let (int, frac) = shortest.split_once('.').unwrap_or((&shortest, ""));
    let fixed = round_half_up(int, frac, 2);
    let (int, frac) = fixed.split_once('.').unwrap_or((&fixed, "00"));
    format!("${sign}{}.{frac}", group_thousands(int))
}

/// A whole count as en-US `toLocaleString` writes it: 12,345.
pub fn fmt_count(n: i64) -> String {
    let grouped = group_thousands(&n.unsigned_abs().to_string());
    if n < 0 {
        format!("-{grouped}")
    } else {
        grouped
    }
}

/// `part` of `whole` to one decimal, as `toFixed(1)` writes it: "87.5%"; none of nothing reads "0%".
pub fn percent(part: f64, whole: f64) -> String {
    if whole == 0.0 {
        return "0%".into();
    }
    format!("{}%", to_fixed(part / whole * 100.0, 1))
}

/// How long since, coarsely: "3 min", "2 h", "4 d".
pub fn offline_for(ms: f64) -> String {
    let minutes = (ms / 60_000.0).floor().max(0.0);
    if minutes < 60.0 {
        return format!("{minutes} min");
    }
    let hours = (minutes / 60.0).floor();
    if hours < 24.0 {
        format!("{hours} h")
    } else {
        format!("{} d", (hours / 24.0).floor())
    }
}

pub fn read_when(ms: f64) -> String {
    if ms < 60_000.0 {
        "just now".into()
    } else {
        format!("{} ago", offline_for(ms))
    }
}

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
/// en-GB's short months, which write September as "Sept".
const MONTHS_GB: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sept", "Oct", "Nov", "Dec"];

/// A ms epoch in the zone, as `new Date(ms)` takes it; nothing for a moment past chrono's range.
fn local<Tz: TimeZone>(ms: f64, tz: &Tz) -> Option<DateTime<Tz>> {
    if !ms.is_finite() {
        return None;
    }
    DateTime::from_timestamp_millis(ms.trunc() as i64).map(|t| t.with_timezone(tz))
}

fn weekday<Tz: TimeZone>(t: &DateTime<Tz>) -> &'static str {
    WEEKDAYS[t.weekday().num_days_from_monday() as usize]
}

fn hh_mm<Tz: TimeZone>(t: &DateTime<Tz>) -> String {
    format!("{:02}:{:02}", t.hour(), t.minute())
}

/// en-GB `{ hour: "2-digit", minute: "2-digit", hourCycle: "h23" }`: "14:05".
pub fn time_word<Tz: TimeZone>(at: f64, tz: &Tz) -> String {
    local(at, tz).map(|t| hh_mm(&t)).unwrap_or_default()
}

/// A moment as a clock reads it: the time on `now`'s day, else the weekday and time.
pub fn clock_word<Tz: TimeZone>(at: f64, now: f64, tz: &Tz) -> String {
    let (Some(at), Some(now)) = (local(at, tz), local(now, tz)) else { return String::new() };
    if at.date_naive() == now.date_naive() {
        hh_mm(&at)
    } else {
        format!("{} {}", weekday(&at), hh_mm(&at))
    }
}

/// When a limit was last read, as the row's quiet word: "as of 14:05", else "as of Mon 14:05".
pub fn as_of_word<Tz: TimeZone>(read_at: f64, now: f64, tz: &Tz) -> String {
    format!("as of {}", clock_word(read_at, now, tz))
}

pub fn resets_word<Tz: TimeZone>(resets_at: f64, now: f64, tz: &Tz) -> String {
    let left = resets_at - now;
    if left <= 0.0 {
        return "reset".into();
    }
    if left < 3_600_000.0 {
        return format!("resets in {} min", js_round(left / 60_000.0).max(1.0));
    }
    if left < 86_400_000.0 {
        return format!("resets in {} h", (left / 3_600_000.0).floor());
    }
    format!("resets on {}", local(resets_at, tz).map(|t| weekday(&t)).unwrap_or_default())
}

fn span_word(left: f64) -> String {
    if left < 3_600_000.0 {
        format!("{} min", js_round(left / 60_000.0).max(1.0))
    } else if left < 86_400_000.0 {
        format!("{} h", (left / 3_600_000.0).floor())
    } else {
        format!("{} d", (left / 86_400_000.0).floor())
    }
}

/// The banked resets: "none banked", "2 banked", "1 banked, expires in 3 d".
pub fn credits_word(count: u64, next_expires_at: Option<f64>, now: f64) -> String {
    if count == 0 {
        return "none banked".into();
    }
    let banked = format!("{count} banked");
    match next_expires_at {
        Some(at) if at > now => {
            format!("{banked}, {} in {}", if count == 1 { "expires" } else { "first expires" }, span_word(at - now))
        }
        _ => banked,
    }
}

/// When a pool runs out: the time under a day away, else en-US's weekday and 24-hour time, "Fri 14:00".
pub fn runs_out_when<Tz: TimeZone>(out: f64, now: f64, tz: &Tz) -> String {
    let Some(t) = local(out, tz) else { return String::new() };
    if out - now < 86_400_000.0 {
        hh_mm(&t)
    } else {
        format!("{} {}", weekday(&t), hh_mm(&t))
    }
}

/// A segment's tooltip under its account: what it used, and when it was read where stale, else when it resets.
pub fn segment_tip<Tz: TimeZone>(used: f64, stale_at: Option<f64>, resets_at: Option<f64>, now: f64, tz: &Tz) -> String {
    let after = match (stale_at, resets_at) {
        (Some(at), _) => format!(", {}", as_of_word(at, now, tz)),
        (None, Some(at)) => format!(", {}", resets_word(at, now, tz)),
        (None, None) => String::new(),
    };
    format!("{}% used{after}", js_round(used))
}

/// A tick under the chart: the hour in a day ("06"), the weekday in a week ("Mon"), the day in a month ("17 Oct").
pub fn tick_word<Tz: TimeZone>(range: Range, t: f64, tz: &Tz) -> String {
    let Some(t) = local(t, tz) else { return String::new() };
    match range {
        Range::Day => format!("{:02}", t.hour()),
        Range::Week => weekday(&t).to_string(),
        Range::Month => format!("{} {}", t.day(), MONTHS_GB[t.month0() as usize]),
    }
}

/// The step a chart tooltip names: its hour in a day ("14:00"), its day otherwise ("Fri 17 Oct").
pub fn step_word<Tz: TimeZone>(range: Range, t: f64, tz: &Tz) -> String {
    let Some(t) = local(t, tz) else { return String::new() };
    match range {
        Range::Day => hh_mm(&t),
        Range::Week | Range::Month => format!("{} {} {}", weekday(&t), t.day(), MONTHS_GB[t.month0() as usize]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn utc() -> FixedOffset {
        FixedOffset::east_opt(0).unwrap()
    }

    /// Sat 17 Oct 2026 14:05 UTC.
    const NOW: f64 = 1_792_245_900_000.0;
    const MIN: f64 = 60_000.0;
    const HOUR: f64 = 3_600_000.0;
    const DAY: f64 = 86_400_000.0;

    #[test]
    fn tokens_read_in_three_figures_as_node_writes_them() {
        for (n, word) in [
            (999.0, "999"),
            (999.5, "1k"),
            (999_499.0, "999k"),
            (999_500.0, "1M"),
            (4270.0, "4.27k"),
            (22_600_000.0, "22.6M"),
            (7.37e9, "7.37B"),
            (687e6, "687M"),
            (0.0, "0"),
            (-5.0, "0"),
            (f64::NAN, "0"),
            (1000.0, "1k"),
            (1005.0, "1k"),
            (1015.0, "1.01k"),
            (1125.0, "1.13k"),
            (10_050.0, "10.1k"),
            (99_950.0, "100k"),
            (100_500.0, "101k"),
            (999_499_999.0, "999M"),
            (999_500_000.0, "1B"),
            (1234.5, "1.24k"),
            (4999.5, "5k"),
            (1.5e12, "1500B"),
        ] {
            assert_eq!(fmt_tokens(n), word, "{n}");
        }
    }

    #[test]
    fn costs_round_half_away_on_the_shortest_decimal_and_group_like_en_us() {
        for (usd, word) in [
            (0.0, "$0.00"),
            (1.005, "$1.01"),
            (2.675, "$2.68"),
            (4301.745, "$4,301.75"),
            (4301.74, "$4,301.74"),
            (1_234_567.891, "$1,234,567.89"),
            (-1.0, "$-1.00"),
            (-0.004, "$-0.00"),
            (-0.005, "$-0.01"),
            (0.125, "$0.13"),
            (999.995, "$1,000.00"),
            (12.5, "$12.50"),
            (0.1 + 0.2, "$0.30"),
            (-1234.5, "$-1,234.50"),
            (123_456_789_012.345, "$123,456,789,012.35"),
            (1e-7, "$0.00"),
            (f64::NAN, "$NaN"),
        ] {
            assert_eq!(fmt_cost(usd), word, "{usd}");
        }
    }

    #[test]
    fn percent_rounds_like_to_fixed_on_the_exact_double() {
        for (part, whole, word) in [
            (1.0, 3.0, "33.3%"),
            (2.0, 3.0, "66.7%"),
            (1.0, 8.0, "12.5%"),
            (1.0, 16.0, "6.3%"),
            (0.0, 5.0, "0.0%"),
            (5.0, 0.0, "0%"),
            (2_100_000.0, 2_400_000.0, "87.5%"),
            (7.0, 1000.0, "0.7%"),
            (1.0, 2000.0, "0.1%"),
            (3.0, 2000.0, "0.1%"),
            (1.0, 1.0, "100.0%"),
        ] {
            assert_eq!(percent(part, whole), word, "{part}/{whole}");
        }
        assert_eq!(to_fixed(-1.25, 1), "-1.3");
        assert_eq!(to_fixed(-0.04, 1), "-0.0");
        assert_eq!(to_fixed(-0.0, 1), "0.0");
        assert_eq!(to_fixed(1.005 * 100.0, 1), "100.5");
    }

    #[test]
    fn counts_group_by_thousands() {
        assert_eq!(fmt_count(0), "0");
        assert_eq!(fmt_count(999), "999");
        assert_eq!(fmt_count(1234), "1,234");
        assert_eq!(fmt_count(1_234_567), "1,234,567");
        assert_eq!(fmt_count(-12_345), "-12,345");
    }

    #[test]
    fn lists_join_with_commas_and_a_last_and() {
        assert_eq!(list_words(&[] as &[&str]), "");
        assert_eq!(list_words(&["Codex"]), "Codex");
        assert_eq!(list_words(&["Codex", "Claude Code"]), "Codex and Claude Code");
        assert_eq!(list_words(&["a", "b", "c"]), "a, b and c");
        assert_eq!(
            logs_line(&["Claude Code", "Codex"], &["Boat"]),
            "Counts what Claude Code and Codex logged on Boat, wsp's threads there included."
        );
        assert_eq!(no_limit(&["OpenCode"]), "OpenCode reports no plan limit.");
        assert_eq!(no_limit(&["OpenCode", "Cursor"]), "OpenCode and Cursor report no plan limit.");
    }

    #[test]
    fn read_words_go_from_just_now_to_days() {
        assert_eq!(read_when(0.0), "just now");
        assert_eq!(read_when(59_999.0), "just now");
        assert_eq!(read_when(3.0 * MIN), "3 min ago");
        assert_eq!(read_when(2.0 * HOUR + 5.0 * MIN), "2 h ago");
        assert_eq!(read_when(49.0 * HOUR), "2 d ago");
        assert_eq!(offline_for(-5.0), "0 min");
        assert_eq!(checked(&read_when(0.0)), "checked just now");
    }

    #[test]
    fn resets_say_minutes_hours_then_the_weekday() {
        let tz = utc();
        assert_eq!(resets_word(NOW, NOW, &tz), "reset");
        assert_eq!(resets_word(NOW + 10_000.0, NOW, &tz), "resets in 1 min");
        assert_eq!(resets_word(NOW + 90_000.0, NOW, &tz), "resets in 2 min");
        assert_eq!(resets_word(NOW + 59.0 * MIN, NOW, &tz), "resets in 59 min");
        assert_eq!(resets_word(NOW + 2.5 * HOUR, NOW, &tz), "resets in 2 h");
        assert_eq!(resets_word(NOW + 3.0 * DAY, NOW, &tz), "resets on Tue");
        // 11:00 UTC on Tue is still Mon in UTC-12.
        assert_eq!(resets_word(NOW + 3.0 * DAY - 3.0 * HOUR, NOW, &FixedOffset::west_opt(12 * 3600).unwrap()), "resets on Mon");
    }

    #[test]
    fn credits_say_how_many_and_when_the_first_lapses() {
        assert_eq!(credits_word(0, Some(NOW + DAY), NOW), "none banked");
        assert_eq!(credits_word(2, None, NOW), "2 banked");
        assert_eq!(credits_word(2, Some(NOW - 1.0), NOW), "2 banked");
        assert_eq!(credits_word(1, Some(NOW + 21.0 * DAY), NOW), "1 banked, expires in 21 d");
        assert_eq!(credits_word(2, Some(NOW + 5.0 * HOUR), NOW), "2 banked, first expires in 5 h");
        assert_eq!(credits_word(2, Some(NOW + 30_000.0), NOW), "2 banked, first expires in 1 min");
    }

    #[test]
    fn as_of_reads_the_time_today_else_the_weekday() {
        let tz = utc();
        assert_eq!(as_of_word(NOW - 5.0 * MIN, NOW, &tz), "as of 14:00");
        assert_eq!(as_of_word(NOW - DAY, NOW, &tz), "as of Fri 14:05");
        assert_eq!(as_of_word(NOW - 14.0 * HOUR - 5.0 * MIN, NOW, &tz), "as of 00:00");
        // In UTC+10 now is Sun 00:05 and five minutes before is Sat 23:55.
        let east = FixedOffset::east_opt(10 * 3600).unwrap();
        assert_eq!(as_of_word(NOW - 10.0 * MIN, NOW, &east), "as of Sat 23:55");
        assert_eq!(state_word(&as_of_word(NOW - DAY, NOW, &tz)), "As of Fri 14:05");
    }

    #[test]
    fn states_capitalise_the_wire_words() {
        use AccountsAnswerAccountsItemNote as N;
        use AccountsAnswerAccountsItemStatus as S;
        assert_eq!(account_state(Some(S::Reached), Some(N::ReportsNoPlanLimit)), "limit reached");
        assert_eq!(account_state(Some(S::Ok), Some(N::PaysPerTokenNoPlanLimit)), "pays per token, no plan limit");
        assert_eq!(account_state(None, Some(N::NotReadYetShowsAfterItsNextTurn)), UNREAD_NOTE);
        assert_eq!(account_state(Some(S::Warning), None), "");
        assert_eq!(state_word("limit reached"), "Limit reached");
        assert_eq!(state_word(""), "");
        assert_eq!(state_word("resets in 2 h"), "Resets in 2 h");
    }

    #[test]
    fn reset_question_names_the_count() {
        assert_eq!(
            reset_question("Codex with ChatGPT Plus", Some(2)),
            "Use one of the 2 resets banked on Codex with ChatGPT Plus? Its five-hour and weekly windows start again now. There is no undo."
        );
        assert!(reset_question("x", Some(1)).starts_with("Use the one reset banked on x?"));
        assert!(reset_question("x", None).starts_with("Use a reset banked on x?"));
    }

    #[test]
    fn agents_go_by_the_catalogs_names() {
        assert_eq!(agent_name("claude"), "Claude Code");
        assert_eq!(agent_name("gemini"), "Gemini CLI");
        assert_eq!(agent_name("hermes"), "Hermes Agent");
        assert_eq!(agent_name("qwen"), "Qwen Code");
        assert_eq!(agent_name("unknown-agent"), "unknown-agent");
        assert_eq!(signed_in_on(&agent_name("opencode"), "Boat"), "OpenCode signed in on Boat");
    }

    #[test]
    fn page_words_match_the_web() {
        assert_eq!(runs_out("Fri 14:00"), "Runs out Fri 14:00");
        assert_eq!(ahead_of_pace(""), "Ahead of pace");
        assert_eq!(under_pace("resets in 2 h"), "Under pace, resets in 2 h");
        assert_eq!(on_pace("resets on Tue"), "On pace, resets on Tue");
        assert_eq!(accounts(2, "Codex"), "2 Codex accounts");
        assert_eq!(burn("4.27k", 1), "Using 4.27k tokens a minute across 1 thread");
        assert_eq!(burn("22.6M", 3), "Using 22.6M tokens a minute across 3 threads");
        assert_eq!(by(split_word(Split::Computer)), "By computer");
        assert_eq!(saved("$1.20"), "$1.20 saved");
        assert_eq!(window_word(AccountsAnswerAccountsItemWindowsItemKind::Session), "5-hour");
        assert_eq!(window_word(AccountsAnswerAccountsItemWindowsItemKind::WeekOpus), "Week, Opus");
        assert_eq!(window_word(AccountsAnswerAccountsItemWindowsItemKind::Overage), "overage");
    }

    #[test]
    fn segment_tips_say_use_then_staleness_or_reset() {
        let tz = utc();
        assert_eq!(segment_tip(62.4, None, Some(NOW + 2.5 * HOUR), NOW, &tz), "62% used, resets in 2 h");
        assert_eq!(segment_tip(62.5, Some(NOW - DAY), Some(NOW - HOUR), NOW, &tz), "63% used, as of Fri 14:05");
        assert_eq!(segment_tip(10.0, None, None, NOW, &tz), "10% used");
    }

    /// Node 22: `new Intl.DateTimeFormat(locale, options).format(t)` with `TZ=UTC`.
    #[test]
    fn chart_words_match_node() {
        let tz = utc();
        let at = |m: u32, d: u32, h: u32, min: u32| tz.with_ymd_and_hms(2026, m, d, h, min, 0).unwrap().timestamp_millis() as f64;
        assert_eq!(tick_word(Range::Day, at(10, 17, 6, 0), &tz), "06");
        assert_eq!(tick_word(Range::Day, at(10, 17, 0, 0), &tz), "00");
        assert_eq!(tick_word(Range::Week, at(9, 7, 0, 3), &tz), "Mon");
        assert_eq!(tick_word(Range::Month, at(10, 7, 0, 0), &tz), "7 Oct");
        assert_eq!(tick_word(Range::Month, at(9, 7, 0, 0), &tz), "7 Sept");
        assert_eq!(step_word(Range::Day, at(9, 7, 0, 3), &tz), "00:03");
        assert_eq!(step_word(Range::Week, at(10, 7, 0, 0), &tz), "Wed 7 Oct");
        assert_eq!(step_word(Range::Month, at(9, 7, 0, 0), &tz), "Mon 7 Sept");
        assert_eq!(step_word(Range::Month, at(1, 1, 0, 0), &tz), "Thu 1 Jan");
        assert_eq!(runs_out_when(at(10, 17, 14, 5) + 3.0 * DAY, NOW, &tz), "Tue 14:05");
        assert_eq!(runs_out_when(at(10, 18, 0, 5), NOW, &tz), "00:05");
        assert_eq!(runs_out_when(at(10, 24, 0, 5), NOW, &tz), "Sat 00:05");
        assert_eq!(time_word(NOW, &tz), "14:05");
    }

    /// Node 22's en-GB `{ day: "numeric", month: "short" }` and `{ weekday: "short", day: "numeric", month: "short" }` on
    /// the 18th of each month of 2026, as Chrome writes them: September is "Sept".
    #[test]
    fn every_month_reads_as_en_gb_writes_it() {
        let tz = utc();
        let ticks =
            ["18 Jan", "18 Feb", "18 Mar", "18 Apr", "18 May", "18 Jun", "18 Jul", "18 Aug", "18 Sept", "18 Oct", "18 Nov", "18 Dec"];
        let steps = [
            "Sun 18 Jan",
            "Wed 18 Feb",
            "Wed 18 Mar",
            "Sat 18 Apr",
            "Mon 18 May",
            "Thu 18 Jun",
            "Sat 18 Jul",
            "Tue 18 Aug",
            "Fri 18 Sept",
            "Sun 18 Oct",
            "Wed 18 Nov",
            "Fri 18 Dec",
        ];
        for (m, (tick, step)) in ticks.into_iter().zip(steps).enumerate() {
            let t = tz.with_ymd_and_hms(2026, m as u32 + 1, 18, 12, 0, 0).unwrap().timestamp_millis() as f64;
            assert_eq!(tick_word(Range::Month, t, &tz), tick);
            assert_eq!(step_word(Range::Week, t, &tz), step);
        }
    }
}
