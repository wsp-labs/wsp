// SPDX-License-Identifier: AGPL-3.0-only
//! The Usage page's logic over the host's answers, after `apps/web/src/settings/usage.tsx` and `usageChart.tsx`: the
//! totals, the token mix, the split table, the chart's lines, ticks and inks, and the Limits tab's pools and verdicts.
//! Nothing here draws; the screen reads these and lays them out.

use std::cmp::Ordering;

use chrono::TimeZone;
use gpui_kit::{rgb, Hsla, Rgba};
use wsp_host_client::protocol::{
    AccountsAnswerAccountsItem, AccountsAnswerAccountsItemNote, AccountsAnswerAccountsItemStatus,
    AccountsAnswerAccountsItemWindowsItemKind, ResetAnswerAccount, UsageUsedRequest, UsageUsedRequestRange, UsageUsedRequestSplit,
    UsedAnswer, UsedAnswerRange, UsedAnswerRowsItem, UsedAnswerSplit,
};

use super::words as w;
use crate::ui::catalog;

pub type Row = UsedAnswerRowsItem;
pub type Account = AccountsAnswerAccountsItem;
pub type Kind = AccountsAnswerAccountsItemWindowsItemKind;

/// How far back the Used tab reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Range {
    Day,
    Week,
    Month,
}

impl Range {
    pub const ALL: [Range; 3] = [Range::Day, Range::Week, Range::Month];

    pub fn word(self) -> &'static str {
        w::range_word(self)
    }

    pub fn wire(self) -> UsageUsedRequestRange {
        match self {
            Range::Day => UsageUsedRequestRange::Day,
            Range::Week => UsageUsedRequestRange::Week,
            Range::Month => UsageUsedRequestRange::Month,
        }
    }
}

impl From<UsedAnswerRange> for Range {
    fn from(range: UsedAnswerRange) -> Self {
        match range {
            UsedAnswerRange::Day => Range::Day,
            UsedAnswerRange::Week => Range::Week,
            UsedAnswerRange::Month => Range::Month,
        }
    }
}

/// What the Used tab's rows and lines are one of.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Split {
    Agent,
    Account,
    Computer,
    Project,
    Model,
    Source,
}

impl Split {
    pub const ALL: [Split; 6] = [Split::Agent, Split::Account, Split::Computer, Split::Project, Split::Model, Split::Source];

    pub fn word(self) -> &'static str {
        w::split_word(self)
    }

    pub fn wire(self) -> UsageUsedRequestSplit {
        match self {
            Split::Agent => UsageUsedRequestSplit::Agent,
            Split::Account => UsageUsedRequestSplit::Account,
            Split::Computer => UsageUsedRequestSplit::Computer,
            Split::Project => UsageUsedRequestSplit::Project,
            Split::Model => UsageUsedRequestSplit::Model,
            Split::Source => UsageUsedRequestSplit::Source,
        }
    }

    /// The page's ask for this split over `range`, `usedAsk`: the agents' own logs counted beside wsp's threads.
    pub fn request(self, range: Range) -> UsageUsedRequest {
        UsageUsedRequest { outside: Some(true), range: range.wire(), split: self.wire() }
    }
}

impl From<UsedAnswerSplit> for Split {
    fn from(split: UsedAnswerSplit) -> Self {
        match split {
            UsedAnswerSplit::Agent => Split::Agent,
            UsedAnswerSplit::Account => Split::Account,
            UsedAnswerSplit::Computer => Split::Computer,
            UsedAnswerSplit::Project => Split::Project,
            UsedAnswerSplit::Model => Split::Model,
            UsedAnswerSplit::Source => Split::Source,
        }
    }
}

pub fn tokens_of(row: &Row) -> f64 {
    row.tokens.input + row.tokens.output
}

/// The rows' sum of a figure, or nothing where no row carries it: a stat cell with nothing to sum is left out.
pub fn sum_of(rows: &[Row], pick: impl Fn(&Row) -> Option<f64>) -> Option<f64> {
    rows.iter().filter_map(pick).reduce(|a, b| a + b)
}

/// A row's API estimate: the host's list price over every token where it sends one, else what the row carried.
pub fn estimate_of(row: &Row) -> Option<f64> {
    row.estimate.or_else(|| match (row.cost_reported, row.cost_list) {
        (None, None) => None,
        (reported, list) => Some(reported.unwrap_or(0.0) + list.unwrap_or(0.0)),
    })
}

/// The stat strip's figures over every row of the answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Totals {
    pub tokens: f64,
    pub estimate: Option<f64>,
    pub turns: Option<i64>,
    pub cache_hit: String,
    pub saved: Option<f64>,
    /// Whether the range counted the agents' own logs beside wsp's threads.
    pub outside: bool,
}

/// One cell of the stat strip.
#[derive(Clone, Debug, PartialEq)]
pub struct Stat {
    pub key: &'static str,
    pub label: &'static str,
    pub value: String,
    pub note: Option<String>,
}

impl Totals {
    pub fn of(used: &UsedAnswer) -> Totals {
        let input = sum_of(&used.rows, |r| Some(r.tokens.input)).unwrap_or(0.0);
        let cached = sum_of(&used.rows, |r| Some(r.tokens.cached)).unwrap_or(0.0);
        let output = sum_of(&used.rows, |r| Some(r.tokens.output)).unwrap_or(0.0);
        Totals {
            tokens: input + output,
            estimate: sum_of(&used.rows, estimate_of),
            turns: used.rows.iter().filter_map(|r| r.turns).reduce(|a, b| a + b),
            cache_hit: w::percent(cached, input),
            saved: sum_of(&used.rows, |r| r.saved),
            outside: used.logs.is_some(),
        }
    }

    /// The strip's cells in order, two to four of them.
    pub fn stats(&self) -> Vec<Stat> {
        let mut stats = vec![Stat {
            key: "stat-tokens",
            label: w::TOKENS,
            value: w::fmt_tokens(self.tokens),
            note: Some(w::counts(self.outside).to_string()),
        }];
        if let Some(estimate) = self.estimate {
            stats.push(Stat {
                key: "stat-estimate",
                label: w::ESTIMATE,
                value: w::fmt_cost(estimate),
                note: Some(w::AT_LIST_PRICE.to_string()),
            });
        }
        if let Some(turns) = self.turns {
            stats.push(Stat { key: "stat-turns", label: w::TURNS, value: w::fmt_count(turns), note: Some(w::TURNS_NOTE.to_string()) });
        }
        stats.push(Stat {
            key: "stat-cache",
            label: w::CACHE_HIT,
            value: self.cache_hit.clone(),
            note: self.saved.map(|saved| w::saved(&w::fmt_cost(saved))),
        });
        stats
    }
}

/// A part of the token mix bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MixPart {
    Fresh,
    CacheWrite,
    Cached,
    Out,
}

impl MixPart {
    pub fn key(self) -> &'static str {
        match self {
            MixPart::Fresh => "fresh",
            MixPart::CacheWrite => "cache-write",
            MixPart::Cached => "cached",
            MixPart::Out => "out",
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            MixPart::Fresh => w::FRESH,
            MixPart::CacheWrite => w::CACHE_WRITE,
            MixPart::Cached => w::CACHED,
            MixPart::Out => w::OUT,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MixSlice {
    pub part: MixPart,
    pub n: f64,
    /// The slice's share of the bar, 0 to 1.
    pub share: f64,
}

/// Where the tokens went, fresh, written to the cache where any row says, read from it, and out; nothing where the
/// range used none, and the card then stands empty.
pub fn mix(used: &UsedAnswer) -> Option<Vec<MixSlice>> {
    let input = sum_of(&used.rows, |r| Some(r.tokens.input)).unwrap_or(0.0);
    let cached = sum_of(&used.rows, |r| Some(r.tokens.cached)).unwrap_or(0.0);
    let write = sum_of(&used.rows, |r| r.tokens.cache_write);
    let out = sum_of(&used.rows, |r| Some(r.tokens.output)).unwrap_or(0.0);
    let mut parts = vec![(MixPart::Fresh, (input - cached - write.unwrap_or(0.0)).max(0.0))];
    if let Some(write) = write {
        parts.push((MixPart::CacheWrite, write));
    }
    parts.push((MixPart::Cached, cached));
    parts.push((MixPart::Out, out));
    let all: f64 = parts.iter().map(|(_, n)| n).sum();
    if all == 0.0 {
        return None;
    }
    Some(parts.into_iter().map(|(part, n)| MixSlice { part, n, share: n / all }).collect())
}

/// The split table: its first column's head, whether the turns column stands, and the rows by tokens.
#[derive(Clone, Debug)]
pub struct SplitTable<'a> {
    pub head: &'static str,
    pub turns: bool,
    pub groups: Vec<SplitGroup<'a>>,
}

/// One split value's row, its bar as its share of the top row, and by agent the models it ran under it.
#[derive(Clone, Debug)]
pub struct SplitGroup<'a> {
    pub row: &'a Row,
    pub share: f64,
    pub models: Vec<SubRow<'a>>,
}

/// A model under its agent, its share of that agent's tokens.
#[derive(Clone, Debug)]
pub struct SubRow<'a> {
    pub row: &'a Row,
    pub share: f64,
}

fn by_tokens(rows: &mut [&Row]) {
    rows.sort_by(|a, b| tokens_of(b).partial_cmp(&tokens_of(a)).unwrap_or(Ordering::Equal));
}

/// `models` is the model split of the same range, read beside an agent split; it is ignored on any other split.
pub fn split_table<'a>(used: &'a UsedAnswer, models: &'a [Row]) -> SplitTable<'a> {
    let split = Split::from(used.split);
    let mut ranked: Vec<&Row> = used.rows.iter().collect();
    by_tokens(&mut ranked);
    let top = ranked.first().map_or(0.0, |row| tokens_of(row));
    let groups = ranked
        .into_iter()
        .map(|row| {
            let mut own: Vec<&Row> = if split == Split::Agent { models.iter().filter(|m| m.agent == row.agent).collect() } else { vec![] };
            by_tokens(&mut own);
            let whole = tokens_of(row);
            SplitGroup {
                row,
                share: if top == 0.0 { 0.0 } else { whole / top },
                models: own.into_iter().map(|m| SubRow { row: m, share: if whole == 0.0 { 0.0 } else { tokens_of(m) / whole } }).collect(),
            }
        })
        .collect();
    SplitTable {
        head: if split == Split::Agent && !models.is_empty() { w::AGENT_AND_MODEL } else { split.word() },
        turns: used.rows.iter().any(|r| r.turns.is_some()),
        groups,
    }
}

/// A split row's figures as its cells write them.
#[derive(Clone, Debug, PartialEq)]
pub struct RowCells {
    pub tokens: String,
    pub cache_hit: String,
    /// Empty where the row ran no turn wsp counted.
    pub turns: String,
    pub price: String,
    /// Whether the price is a figure, drawn in the text's colour, rather than "not priced", drawn quiet.
    pub estimated: bool,
}

pub fn row_cells(row: &Row) -> RowCells {
    let estimate = estimate_of(row);
    RowCells {
        tokens: w::fmt_tokens(tokens_of(row)),
        cache_hit: w::percent(row.tokens.cached, row.tokens.input),
        turns: row.turns.map(w::fmt_count).unwrap_or_default(),
        price: estimate.map_or_else(|| w::NOT_PRICED.to_string(), w::fmt_cost),
        estimated: estimate.is_some(),
    }
}

pub use crate::ui::chart::nice_top;

#[derive(Clone, Debug, PartialEq)]
pub struct Tick {
    /// Where the tick stands across the plot, 0 to 1.
    pub at: f64,
    pub word: String,
}

/// The ticks under the chart: every sixth hour in a day, each day in a week, every fifth day in a month.
pub fn ticks<Tz: TimeZone>(used: &UsedAnswer, tz: &Tz) -> Vec<Tick> {
    let range = Range::from(used.range);
    let n = used.series.len();
    let every = match range {
        Range::Day => 6,
        Range::Week => 1,
        Range::Month => 5,
    };
    used.series
        .iter()
        .enumerate()
        .filter(|(i, _)| i % every == 0)
        .map(|(i, s)| Tick { at: if n > 1 { i as f64 / (n - 1) as f64 } else { 0.0 }, word: w::tick_word(range, s.t, tz) })
        .collect()
}

/// A chart line's colour, resolved against a theme by `resolve`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ink {
    Foreground,
    MutedForeground,
    /// `SPLIT_INKS` by rank: sky, violet, amber, pink, teal.
    Split(usize),
    /// An agent's mark ink, its second account and on mixed toward the text or the page; rank 0 to 3.
    Agent {
        agent: String,
        rank: usize,
    },
    /// An agent with no mark ink: muted, then the text at 80% and 40%; rank 0 to 2.
    Neutral(usize),
}

/// Tailwind's sky, violet, amber, pink and teal 500, none of them Claude's orange nor the done and failed colours.
const SPLIT_INKS: [u32; 5] = [0x00a6f4, 0x8e51ff, 0xfe9a00, 0xf6339a, 0x00bba7];

fn agent_ink(agent: &str, rank: usize) -> Ink {
    if catalog::ink(agent).is_some() {
        Ink::Agent { agent: agent.to_string(), rank: rank.min(3) }
    } else {
        Ink::Neutral(rank.min(2))
    }
}

/// The theme colours an ink is made of.
#[derive(Clone, Copy, Debug)]
pub struct InkTheme {
    pub foreground: Hsla,
    pub background: Hsla,
    pub muted_foreground: Hsla,
    pub dark: bool,
}

impl Ink {
    pub fn resolve(&self, theme: &InkTheme) -> Hsla {
        match self {
            Ink::Foreground => theme.foreground,
            Ink::MutedForeground => theme.muted_foreground,
            Ink::Split(i) => SPLIT_INKS.get(*i).map_or(theme.muted_foreground, |hex| rgb(*hex).into()),
            Ink::Neutral(0) => theme.muted_foreground,
            Ink::Neutral(1) => theme.foreground.opacity(0.8),
            Ink::Neutral(_) => theme.foreground.opacity(0.4),
            Ink::Agent { agent, rank } => {
                let Some((light, dark)) = catalog::ink(agent) else { return theme.muted_foreground };
                let line = if theme.dark { dark } else { light };
                let mixed = match rank {
                    0 => line,
                    1 => mix_oklab(line, 0.5, theme.foreground.into()),
                    2 => mix_oklab(line, 0.55, theme.background.into()),
                    _ => mix_oklab(line, 0.75, theme.foreground.into()),
                };
                mixed.into()
            }
        }
    }
}

fn to_linear(c: f32) -> f64 {
    let c = c as f64;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(c: f64) -> f32 {
    let c = if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    c.clamp(0.0, 1.0) as f32
}

fn to_oklab(c: Rgba) -> [f64; 3] {
    let (r, g, b) = (to_linear(c.r), to_linear(c.g), to_linear(c.b));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    ]
}

fn from_oklab([lightness, a, b]: [f64; 3]) -> Rgba {
    let l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s = (lightness - 0.089_484_177_5 * a - 1.291_485_548_0 * b).powi(3);
    Rgba {
        r: from_linear(4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s),
        g: from_linear(-1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s),
        b: from_linear(-0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s),
        a: 1.0,
    }
}

/// CSS `color-mix(in oklab, x p, y)` of two opaque colours.
pub fn mix_oklab(x: Rgba, p: f64, y: Rgba) -> Rgba {
    let (x, y) = (to_oklab(x), to_oklab(y));
    from_oklab([0, 1, 2].map(|i| x[i] * p + y[i] * (1.0 - p)))
}

/// One line of the chart, its legend entry and its ink.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartLine {
    pub key: String,
    pub label: String,
    pub points: Vec<f64>,
    pub ink: Ink,
    /// The agent every turn of the line ran on, whose mark leads its legend entry.
    pub agent: Option<String>,
}

/// The chart's lines: one per split value, or one total line where the answer carries none per value.
#[derive(Clone, Debug, PartialEq)]
pub struct Chart {
    pub lines: Vec<ChartLine>,
    /// False for the one total line, whose legend reads `all_of(split)` and whose ink no split row takes.
    pub per_split: bool,
}

impl Chart {
    /// `known_projects` are the project ids this host has: use filed under any other is drawn muted.
    pub fn of(used: &UsedAnswer, known_projects: &[impl AsRef<str>]) -> Chart {
        // The host always sends lines, empty only where nothing was used; an answer without them draws the total.
        if used.lines.is_empty() {
            let points = used.series.iter().map(|s| s.tokens).collect();
            let line = ChartLine { key: "tokens".into(), label: w::TOKENS.into(), points, ink: Ink::Foreground, agent: None };
            return Chart { lines: vec![line], per_split: false };
        }
        let split = Split::from(used.split);
        let agent_of = |key: &str| used.rows.iter().find(|r| r.key == key).and_then(|r| r.agent.clone());
        let mut next = 0;
        let mut ranks: Vec<(String, usize)> = vec![];
        let lines = used
            .lines
            .iter()
            .map(|line| {
                let agent = agent_of(&line.key);
                let unowned = split == Split::Project && !known_projects.iter().any(|p| p.as_ref() == line.key);
                let ink = if unowned {
                    Ink::MutedForeground
                } else if let Some(agent) = &agent {
                    let rank = match ranks.iter_mut().find(|(a, _)| a == agent) {
                        Some((_, n)) => {
                            *n += 1;
                            *n - 1
                        }
                        None => {
                            ranks.push((agent.clone(), 1));
                            0
                        }
                    };
                    agent_ink(agent, rank)
                } else {
                    let rank = next;
                    next += 1;
                    if rank < SPLIT_INKS.len() {
                        Ink::Split(rank)
                    } else {
                        Ink::MutedForeground
                    }
                };
                ChartLine { key: line.key.clone(), label: line.label.clone(), points: line.points.clone(), ink, agent }
            })
            .collect();
        Chart { lines, per_split: true }
    }

    /// The ink a split row's bar takes: its line's, or none where it has no line, and it is drawn at foreground/45.
    pub fn ink_of(&self, key: &str) -> Option<&Ink> {
        if !self.per_split {
            return None;
        }
        self.lines.iter().find(|l| l.key == key).map(|l| &l.ink)
    }

    /// The axis's top, every line on the scale of the largest single point.
    pub fn top(&self) -> f64 {
        nice_top(self.lines.iter().flat_map(|l| l.points.iter().copied()).fold(0.0, f64::max))
    }
}

/// How long each window runs, so its pace is the share of it already gone.
pub fn window_ms(kind: Kind) -> Option<f64> {
    match kind {
        Kind::Session => Some(5.0 * 3_600_000.0),
        Kind::Week | Kind::WeekOpus | Kind::WeekSonnet => Some(7.0 * 86_400_000.0),
        Kind::Month => Some(30.0 * 86_400_000.0),
        Kind::Overage => None,
    }
}

pub const POOLED_KINDS: [Kind; 5] = [Kind::Session, Kind::Week, Kind::WeekOpus, Kind::WeekSonnet, Kind::Month];

/// One account's part of a pool's window.
#[derive(Clone, Debug)]
pub struct Segment<'a> {
    pub account: &'a Account,
    pub used: f64,
    pub resets_at: Option<f64>,
    /// The share of the window gone by now, 0 to 1, where its reset is known.
    pub elapsed: Option<f64>,
    /// When the reading was taken, where it no longer says how the window stands: its reset has passed, or it is older
    /// than the window runs.
    pub stale_at: Option<f64>,
}

pub fn segment_of(account: &Account, kind: Kind, now: f64) -> Option<Segment<'_>> {
    let window = account.windows.iter().find(|x| x.kind == kind)?;
    let length = window_ms(kind);
    let read_at = account.read_at.unwrap_or(now);
    let stale = match window.resets_at {
        Some(at) => at <= now,
        None => length.is_some_and(|length| now - read_at > length),
    };
    let elapsed = match (stale, window.resets_at, length) {
        (false, Some(at), Some(length)) => Some((1.0 - (at - now) / length).clamp(0.0, 1.0)),
        _ => None,
    };
    Some(Segment { account, used: window.used_percent, resets_at: window.resets_at, elapsed, stale_at: stale.then_some(read_at) })
}

/// How a verdict is drawn: in the warning's colour, quiet, or in the text's at 80%.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Warn,
    Quiet,
    Plain,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub word: String,
    pub tone: Tone,
}

/// The pool's one sentence: as of when it was read where no reading says how the window stands now, else reached, when
/// it runs out at this pace, or how it stands against an even pace, off the readings that still stand.
pub fn verdict_of<Tz: TimeZone>(all: &[Segment], kind: Kind, now: f64, tz: &Tz) -> Verdict {
    let verdict = |word: String, tone: Tone| Verdict { word, tone };
    let segments: Vec<&Segment> = all.iter().filter(|s| s.stale_at.is_none()).collect();
    if segments.is_empty() {
        let read_at = all.iter().filter_map(|s| s.stale_at).fold(f64::INFINITY, f64::min);
        return verdict(w::state_word(&w::as_of_word(read_at, now, tz)), Tone::Quiet);
    }
    if segments.iter().all(|s| s.used >= 100.0) {
        return verdict(w::REACHED.to_string(), Tone::Warn);
    }
    let soonest = segments.iter().filter_map(|s| s.resets_at).reduce(f64::min);
    let resets = soonest.map(|at| w::resets_word(at, now, tz)).unwrap_or_default();
    let paced: Vec<(f64, f64, f64)> =
        segments.iter().filter_map(|s| Some((s.used, s.resets_at?, s.elapsed?))).filter(|(_, _, elapsed)| *elapsed > 0.02).collect();
    let Some(length) = window_ms(kind).filter(|_| !paced.is_empty()) else { return verdict(w::state_word(&resets), Tone::Plain) };
    // The pool runs out when its last account does, each at the rate it has used its window so far.
    let out_of = |used: f64, resets_at: f64| {
        let start = resets_at - length;
        if used <= 0.0 {
            f64::INFINITY
        } else {
            start + (now - start) * (100.0 / used)
        }
    };
    let out = paced.iter().map(|&(used, resets_at, _)| out_of(used, resets_at)).fold(f64::NEG_INFINITY, f64::max);
    let last_reset = paced.iter().map(|&(_, resets_at, _)| resets_at).fold(f64::NEG_INFINITY, f64::max);
    if out < last_reset {
        return verdict(w::runs_out(&w::runs_out_when(out, now, tz)), Tone::Warn);
    }
    let ratio = paced.iter().map(|&(used, _, elapsed)| used / 100.0 / elapsed).sum::<f64>() / paced.len() as f64;
    if ratio > 1.05 {
        verdict(w::ahead_of_pace(&resets), Tone::Warn)
    } else if ratio < 0.95 {
        verdict(w::under_pace(&resets), Tone::Quiet)
    } else {
        verdict(w::on_pace(&resets), Tone::Plain)
    }
}

/// What a pool's window has used: its accounts' use, each at most 100, averaged and rounded.
pub fn pool_used(segments: &[Segment]) -> f64 {
    w::js_round(segments.iter().map(|s| s.used.min(100.0)).sum::<f64>() / segments.len() as f64)
}

/// The window's big figure, what is left: "38%".
pub fn left_word(used: f64) -> String {
    format!("{}%", (100.0 - used).max(0.0))
}

/// One window of a pool, ready to draw.
#[derive(Clone, Debug)]
pub struct PoolWindow<'a> {
    pub kind: Kind,
    pub word: String,
    /// Soonest reset first, an unknown reset last.
    pub segments: Vec<Segment<'a>>,
    pub left: String,
    pub verdict: Verdict,
    /// Every reading is stale, so the figure is drawn quiet.
    pub stale: bool,
}

/// One agent's subscription accounts, drawn as one card.
#[derive(Clone, Debug)]
pub struct Pool<'a> {
    pub agent: &'a str,
    pub accounts: Vec<&'a Account>,
}

impl<'a> Pool<'a> {
    /// The card's head.
    pub fn head(&self) -> String {
        w::agent_name(self.agent)
    }

    pub fn title(&self) -> String {
        match self.accounts.as_slice() {
            [one] => one.label.clone(),
            many => w::accounts(many.len(), &w::agent_name(self.agent)),
        }
    }

    /// The computers the accounts are signed in on, each once, as the line under the title.
    pub fn line(&self) -> String {
        let mut computers: Vec<&str> = vec![];
        for c in self.accounts.iter().flat_map(|a| &a.computers) {
            if !computers.contains(&c.as_str()) {
                computers.push(c);
            }
        }
        w::list_words(&computers)
    }

    /// What the running threads draw on the pool now; nothing where none ran in the last fifteen minutes.
    pub fn burn_word(&self) -> Option<String> {
        let burn: f64 = self.accounts.iter().filter_map(|a| a.burn.as_ref()).map(|b| b.tokens_per_minute).sum();
        let threads: i64 = self.accounts.iter().filter_map(|a| a.burn.as_ref()).map(|b| b.threads).sum();
        (burn > 0.0).then(|| w::burn(&w::fmt_tokens(burn), threads))
    }

    /// When its oldest reading was taken: "checked 3 min ago".
    pub fn checked(&self, now: f64) -> String {
        let read_at = self.accounts.iter().map(|a| a.read_at.unwrap_or(now)).fold(f64::INFINITY, f64::min);
        w::checked(&w::read_when((now - read_at).max(0.0)))
    }

    pub fn kinds(&self) -> Vec<Kind> {
        POOLED_KINDS.into_iter().filter(|kind| self.accounts.iter().any(|a| a.windows.iter().any(|x| x.kind == *kind))).collect()
    }

    pub fn segments(&self, kind: Kind, now: f64) -> Vec<Segment<'a>> {
        let mut segments: Vec<Segment<'a>> = self.accounts.iter().filter_map(|a| segment_of(a, kind, now)).collect();
        let key = |s: &Segment| s.resets_at.unwrap_or(f64::INFINITY);
        segments.sort_by(|x, y| key(x).partial_cmp(&key(y)).unwrap_or(Ordering::Equal));
        segments
    }

    pub fn windows<Tz: TimeZone>(&self, now: f64, tz: &Tz) -> Vec<PoolWindow<'a>> {
        self.kinds()
            .into_iter()
            .map(|kind| {
                let segments = self.segments(kind, now);
                PoolWindow {
                    kind,
                    word: w::window_word(kind),
                    left: left_word(pool_used(&segments)),
                    verdict: verdict_of(&segments, kind, now, tz),
                    stale: segments.iter().all(|s| s.stale_at.is_some()),
                    segments,
                }
            })
            .collect()
    }
}

/// An account's banked resets, the line under its pool's head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResetLine {
    pub banked: String,
    /// Whether Use reset stands: one is banked.
    pub can_use: bool,
    pub question: String,
}

pub fn reset_line(account: &Account, now: f64) -> Option<ResetLine> {
    let credits = account.credits.as_ref()?;
    Some(ResetLine {
        banked: w::credits_word(credits.count, credits.next_expires_at, now),
        can_use: credits.count > 0,
        question: w::reset_question(&account.label, Some(credits.count)),
    })
}

/// An account with no window to pool: its title, where it is signed in, and its state word.
#[derive(Clone, Debug)]
pub struct AccountLine<'a> {
    pub account: &'a Account,
    /// Empty where the title already names its one computer.
    pub line: String,
    pub state: String,
    pub warn: bool,
}

impl<'a> AccountLine<'a> {
    pub fn of(account: &'a Account) -> AccountLine<'a> {
        // A computer's own login names its computer in its title already, so the line under it would only say it again.
        let own = matches!(account.computers.as_slice(), [one] if account.label == w::signed_in_on(&w::agent_name(&account.agent), one));
        AccountLine {
            account,
            line: if own { String::new() } else { w::list_words(&account.computers) },
            state: w::state_word(&w::account_state(account.status, account.note)),
            warn: account.status == Some(AccountsAnswerAccountsItemStatus::Reached),
        }
    }
}

/// The Limits tab: a pool per agent with windows, then one card of the accounts with none to pool, under it the agents
/// that report no plan limit named once.
#[derive(Clone, Debug)]
pub struct Limits<'a> {
    pub pools: Vec<Pool<'a>>,
    pub quiet: Vec<AccountLine<'a>>,
    /// The agents that report no limit at all, by name, each once.
    pub unlimited: Vec<String>,
}

impl Limits<'_> {
    /// The line under the no-plan-limit card, where any agent reports none.
    pub fn no_limit_word(&self) -> Option<String> {
        (!self.unlimited.is_empty()).then(|| w::no_limit(&self.unlimited))
    }

    /// Whether the no-plan-limit card stands at all.
    pub fn has_quiet_card(&self) -> bool {
        !self.quiet.is_empty() || !self.unlimited.is_empty()
    }
}

/// The row a reset answer sends back, as the accounts answer's row: one schema the generator named twice.
pub fn account_row(row: &ResetAnswerAccount) -> Option<Account> {
    serde_json::to_value(row).ok().and_then(|v| serde_json::from_value(v).ok())
}

/// Where `accounts` is empty the tab says `NO_ACCOUNTS` instead.
pub fn limits(accounts: &[Account]) -> Limits<'_> {
    let windowed = |a: &Account| a.windows.iter().any(|x| POOLED_KINDS.contains(&x.kind));
    let unlimited = |a: &Account| a.note == Some(AccountsAnswerAccountsItemNote::ReportsNoPlanLimit) && a.windows.is_empty();
    let mut pools: Vec<Pool> = vec![];
    for account in accounts.iter().filter(|a| windowed(a)) {
        match pools.iter_mut().find(|p| p.agent == account.agent) {
            Some(pool) => pool.accounts.push(account),
            None => pools.push(Pool { agent: &account.agent, accounts: vec![account] }),
        }
    }
    let mut names: Vec<String> = vec![];
    for name in accounts.iter().filter(|a| unlimited(a)).map(|a| w::agent_name(&a.agent)) {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    Limits { pools, quiet: accounts.iter().filter(|a| !windowed(a) && !unlimited(a)).map(AccountLine::of).collect(), unlimited: names }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;
    use serde_json::json;

    /// Sat 17 Oct 2026 14:05 UTC.
    const NOW: f64 = 1_792_245_900_000.0;
    const HOUR: f64 = 3_600_000.0;
    const DAY: f64 = 86_400_000.0;

    fn utc() -> FixedOffset {
        FixedOffset::east_opt(0).unwrap()
    }

    fn used(value: serde_json::Value) -> UsedAnswer {
        let mut answer = json!({ "range": "week", "split": "agent", "rows": [], "series": [], "since": 0, "until": 0 });
        answer.as_object_mut().unwrap().extend(value.as_object().unwrap().clone());
        serde_json::from_value(answer).unwrap()
    }

    fn row(key: &str, input: f64, output: f64, extra: serde_json::Value) -> serde_json::Value {
        let mut row = json!({ "key": key, "label": key, "tokens": { "input": input, "output": output, "cached": 0 }, "priced": true });
        row.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        row
    }

    fn account(value: serde_json::Value) -> Account {
        let mut account = json!({ "key": "k", "agent": "codex", "label": "Codex", "computers": ["Boat"] });
        account.as_object_mut().unwrap().extend(value.as_object().unwrap().clone());
        serde_json::from_value(account).unwrap()
    }

    #[test]
    fn ranges_and_splits_ask_as_the_web_does() {
        assert_eq!(Range::ALL.map(Range::word), ["Today", "7 days", "30 days"]);
        assert_eq!(Split::ALL.map(Split::word), ["Agent", "Account", "Computer", "Project", "Model", "Source"]);
        let ask = serde_json::to_value(Split::Source.request(Range::Week)).unwrap();
        assert_eq!(ask, json!({ "range": "week", "split": "source", "outside": true }));
    }

    #[test]
    fn estimates_fall_back_to_what_the_row_carried() {
        let of = |extra| estimate_of(&serde_json::from_value(row("a", 1.0, 1.0, extra)).unwrap());
        assert_eq!(of(json!({ "estimate": 2.5, "costReported": 9 })), Some(2.5));
        assert_eq!(of(json!({ "costReported": 1.5 })), Some(1.5));
        assert_eq!(of(json!({ "costList": 0.25, "costReported": 1 })), Some(1.25));
        assert_eq!(of(json!({})), None);
    }

    #[test]
    fn totals_leave_out_what_no_row_carries() {
        let bare = used(json!({ "rows": [row("a", 2_400_000.0, 31_000.0, json!({})), row("b", 800_000.0, 12_000.0, json!({}))] }));
        let totals = Totals::of(&bare);
        assert_eq!(totals.tokens, 3_243_000.0);
        assert_eq!((totals.estimate, totals.turns, totals.saved), (None, None, None));
        let stats = totals.stats();
        assert_eq!(stats.iter().map(|s| s.key).collect::<Vec<_>>(), ["stat-tokens", "stat-cache"]);
        assert_eq!(stats[0].value, "3.24M");
        assert_eq!(stats[0].note.as_deref(), Some("in wsp threads only"));
        assert_eq!(stats[1].note, None);

        let mut a = row("a", 2_400_000.0, 31_000.0, json!({ "estimate": 4301.745, "turns": 1200, "saved": 0.5 }));
        a["tokens"]["cached"] = json!(2_100_000.0);
        let b = row("b", 800_000.0, 12_000.0, json!({ "costReported": 0, "turns": 34 }));
        let full = used(json!({ "rows": [a, b], "logs": { "agents": ["Codex"], "computers": ["Boat"] } }));
        let stats = Totals::of(&full).stats();
        assert_eq!(stats.iter().map(|s| s.key).collect::<Vec<_>>(), ["stat-tokens", "stat-estimate", "stat-turns", "stat-cache"]);
        assert_eq!(stats[0].note.as_deref(), Some("in wsp threads and outside wsp"));
        assert_eq!(stats[1].value, "$4,301.75");
        assert_eq!(stats[2].value, "1,234");
        assert_eq!(stats[3].value, "65.6%");
        assert_eq!(stats[3].note.as_deref(), Some("$0.50 saved"));
    }

    #[test]
    fn the_mix_shows_cache_write_only_where_a_row_says() {
        let mut a = row("a", 1000.0, 100.0, json!({}));
        a["tokens"]["cached"] = json!(600.0);
        let parts = mix(&used(json!({ "rows": [a.clone()] }))).unwrap();
        assert_eq!(
            parts.iter().map(|p| (p.part, p.n)).collect::<Vec<_>>(),
            [(MixPart::Fresh, 400.0), (MixPart::Cached, 600.0), (MixPart::Out, 100.0)]
        );
        a["tokens"]["cacheWrite"] = json!(300.0);
        let parts = mix(&used(json!({ "rows": [a] }))).unwrap();
        assert_eq!(parts.iter().map(|p| p.n).collect::<Vec<_>>(), [100.0, 300.0, 600.0, 100.0]);
        assert_eq!(parts[1].part.word(), "Cache write");
        assert!((parts.iter().map(|p| p.share).sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(mix(&used(json!({ "rows": [row("z", 0.0, 0.0, json!({}))] }))).is_none());
    }

    #[test]
    fn the_split_table_ranks_rows_and_nests_each_agents_models() {
        let rows = json!([
            row("codex", 100.0, 0.0, json!({ "agent": "codex" })),
            row("claude", 300.0, 100.0, json!({ "agent": "claude", "turns": 3 })),
            row("pi", 100.0, 0.0, json!({ "agent": "pi" })),
        ]);
        let answer = used(json!({ "rows": rows }));
        let models: Vec<Row> = serde_json::from_value(json!([
            row("claude:sonnet", 50.0, 0.0, json!({ "agent": "claude" })),
            row("codex:gpt", 100.0, 0.0, json!({ "agent": "codex" })),
            row("claude:opus", 350.0, 0.0, json!({ "agent": "claude" })),
        ]))
        .unwrap();
        let table = split_table(&answer, &models);
        assert_eq!(table.head, "Agent and model");
        assert!(table.turns);
        assert_eq!(table.groups.iter().map(|g| g.row.key.as_str()).collect::<Vec<_>>(), ["claude", "codex", "pi"]);
        assert_eq!(table.groups.iter().map(|g| g.share).collect::<Vec<_>>(), [1.0, 0.25, 0.25]);
        let claude = &table.groups[0].models;
        assert_eq!(
            claude.iter().map(|m| (m.row.key.as_str(), m.share)).collect::<Vec<_>>(),
            [("claude:opus", 0.875), ("claude:sonnet", 0.125)]
        );
        assert!(table.groups[2].models.is_empty());

        let bare = split_table(&answer, &[]);
        assert_eq!(bare.head, "Agent");
        let by_project = used(json!({ "split": "project", "rows": [row("p", 1.0, 0.0, json!({}))] }));
        let table = split_table(&by_project, &models);
        assert_eq!((table.head, table.turns, table.groups[0].models.len()), ("Project", false, 0));
    }

    #[test]
    fn row_cells_write_the_figures() {
        let mut a: Row = serde_json::from_value(row("a", 2_400_000.0, 31_000.0, json!({ "turns": 1234, "costList": 3.2 }))).unwrap();
        a.tokens.cached = 2_100_000.0;
        let cells = row_cells(&a);
        assert_eq!(
            cells,
            RowCells { tokens: "2.43M".into(), cache_hit: "87.5%".into(), turns: "1,234".into(), price: "$3.20".into(), estimated: true }
        );
        let b: Row = serde_json::from_value(row("b", 0.0, 0.0, json!({}))).unwrap();
        assert_eq!(
            row_cells(&b),
            RowCells { tokens: "0".into(), cache_hit: "0%".into(), turns: "".into(), price: "not priced".into(), estimated: false }
        );
    }

    #[test]
    fn nice_top_is_the_next_round_figure() {
        for (max, top) in [
            (0.0, 0.0),
            (-3.0, 0.0),
            (1.0, 1.0),
            (7.0, 8.0),
            (1000.0, 1000.0),
            (1100.0, 1200.0),
            (2_430_000.0, 2_500_000.0),
            (9.1e9, 1e10),
            (0.03, 0.03),
        ] {
            assert_eq!(nice_top(max), top, "{max}");
        }
    }

    #[test]
    fn ticks_stand_every_sixth_hour_each_day_and_every_fifth_day() {
        let tz = utc();
        let midnight = NOW - 14.0 * HOUR - 5.0 * 60_000.0;
        let series =
            |n: usize, step: f64| json!((0..n).map(|i| json!({ "t": midnight + i as f64 * step, "tokens": 0 })).collect::<Vec<_>>());
        let day = ticks(&used(json!({ "range": "day", "series": series(24, HOUR) })), &tz);
        assert_eq!(day.iter().map(|t| t.word.as_str()).collect::<Vec<_>>(), ["00", "06", "12", "18"]);
        assert_eq!(day[1].at, 6.0 / 23.0);
        let week = ticks(&used(json!({ "range": "week", "series": series(7, DAY) })), &tz);
        assert_eq!(week.iter().map(|t| t.word.as_str()).collect::<Vec<_>>(), ["Sat", "Sun", "Mon", "Tue", "Wed", "Thu", "Fri"]);
        assert_eq!((week[0].at, week[6].at), (0.0, 1.0));
        let month = ticks(&used(json!({ "range": "month", "series": series(30, DAY) })), &tz);
        assert_eq!(month.iter().map(|t| t.word.as_str()).collect::<Vec<_>>(), ["17 Oct", "22 Oct", "27 Oct", "1 Nov", "6 Nov", "11 Nov"]);
        assert_eq!(ticks(&used(json!({ "range": "week", "series": series(1, DAY) })), &tz)[0].at, 0.0);
    }

    fn line(key: &str) -> serde_json::Value {
        json!({ "key": key, "label": key, "points": [1, 2] })
    }

    #[test]
    fn lines_take_agent_shades_split_hues_or_muted() {
        let rows = json!([
            row("claude:a", 1.0, 0.0, json!({ "agent": "claude" })),
            row("claude:b", 1.0, 0.0, json!({ "agent": "claude" })),
            row("codex:a", 1.0, 0.0, json!({ "agent": "codex" })),
            row("codex:b", 1.0, 0.0, json!({ "agent": "codex" })),
            row("claude:c", 1.0, 0.0, json!({ "agent": "claude" })),
            row("claude:d", 1.0, 0.0, json!({ "agent": "claude" })),
            row("claude:e", 1.0, 0.0, json!({ "agent": "claude" })),
        ]);
        let keys = ["claude:a", "claude:b", "codex:a", "codex:b", "claude:c", "claude:d", "claude:e"];
        let answer = used(json!({ "split": "account", "rows": rows, "lines": keys.map(line) }));
        let chart = Chart::of(&answer, &[] as &[&str]);
        let claude = |rank| Ink::Agent { agent: "claude".into(), rank };
        assert_eq!(
            chart.lines.iter().map(|l| l.ink.clone()).collect::<Vec<_>>(),
            [claude(0), claude(1), Ink::Neutral(0), Ink::Neutral(1), claude(2), claude(3), claude(3)]
        );
        assert_eq!(chart.ink_of("codex:b"), Some(&Ink::Neutral(1)));

        let keys = ["a", "b", "c", "d", "e", "f", "g"];
        let rows: Vec<_> = keys.iter().map(|k| row(k, 1.0, 0.0, json!({}))).collect();
        let answer = used(json!({ "split": "computer", "rows": rows, "lines": keys.map(line) }));
        let inks: Vec<Ink> = Chart::of(&answer, &[] as &[&str]).lines.into_iter().map(|l| l.ink).collect();
        assert_eq!(
            inks,
            [Ink::Split(0), Ink::Split(1), Ink::Split(2), Ink::Split(3), Ink::Split(4), Ink::MutedForeground, Ink::MutedForeground]
        );

        let answer = used(
            json!({ "split": "project", "rows": [row("p1", 1.0, 0.0, json!({})), row("", 1.0, 0.0, json!({}))], "lines": [line(""), line("p1"), line("p2")] }),
        );
        let inks: Vec<Ink> = Chart::of(&answer, &["p1", "p2"]).lines.into_iter().map(|l| l.ink).collect();
        assert_eq!(inks, [Ink::MutedForeground, Ink::Split(0), Ink::Split(1)]);
    }

    #[test]
    fn an_answer_without_lines_draws_one_total_line() {
        let answer =
            used(json!({ "rows": [row("a", 1.0, 0.0, json!({}))], "series": [{ "t": 0, "tokens": 5 }, { "t": 1, "tokens": 1250 }] }));
        let chart = Chart::of(&answer, &[] as &[&str]);
        assert!(!chart.per_split);
        assert_eq!(chart.lines.len(), 1);
        assert_eq!(
            (chart.lines[0].label.as_str(), chart.lines[0].ink.clone(), chart.lines[0].points.clone()),
            ("Tokens", Ink::Foreground, vec![5.0, 1250.0])
        );
        assert_eq!(chart.ink_of("tokens"), None);
        assert_eq!(chart.top(), 1500.0);
    }

    fn hex(c: Hsla) -> [i32; 3] {
        let c = Rgba::from(c);
        [c.r, c.g, c.b].map(|v| (v * 255.0).round() as i32)
    }

    fn theme(fg: u32, bg: u32, muted: u32, dark: bool) -> InkTheme {
        InkTheme { foreground: rgb(fg).into(), background: rgb(bg).into(), muted_foreground: rgb(muted).into(), dark }
    }

    /// The colours Chrome resolves the claude shades to on Paper and Graphite.
    #[test]
    fn claude_shades_match_the_browsers_oklab_mixes() {
        let paper = theme(0x27272a, 0xfcfcfc, 0x6c6c76, false);
        let graphite = theme(0xf5f5f5, 0x0a0a0a, 0x818181, true);
        for (theme, want) in [(paper, [0xc4643f, 0x724638, 0xe1a994, 0x9a553c]), (graphite, [0xd97757, 0xebb7a6, 0x734333, 0xe3977f])] {
            for (rank, want) in want.into_iter().enumerate() {
                let got = hex(Ink::Agent { agent: "claude".into(), rank }.resolve(&theme));
                let want = [(want >> 16) & 0xff, (want >> 8) & 0xff, want & 0xff];
                assert!(got.iter().zip(want).all(|(g, w)| (g - w).abs() <= 1), "rank {rank}: {got:x?} against {want:x?}");
            }
        }
        assert_eq!(hex(Ink::Split(0).resolve(&paper)), [0x00, 0xa6, 0xf4]);
        assert_eq!(hex(Ink::Split(9).resolve(&paper)), [0x6c, 0x6c, 0x76]);
        assert_eq!(hex(Ink::Neutral(0).resolve(&graphite)), [0x81, 0x81, 0x81]);
        assert_eq!(Ink::Neutral(1).resolve(&graphite).a, 0.8);
        assert_eq!(Ink::Neutral(2).resolve(&graphite).a, 0.4);
    }

    fn windowed(key: &str, kind: &str, used: f64, resets_at: Option<f64>, read_at: f64) -> Account {
        let mut window = json!({ "kind": kind, "usedPercent": used });
        if let Some(at) = resets_at {
            window["resetsAt"] = json!(at);
        }
        account(json!({ "key": key, "windows": [window], "readAt": read_at }))
    }

    fn verdict(kind: Kind, accounts: &[Account]) -> (Verdict, f64) {
        let pool = Pool { agent: "codex", accounts: accounts.iter().collect() };
        let segments = pool.segments(kind, NOW);
        (verdict_of(&segments, kind, NOW, &utc()), pool_used(&segments))
    }

    /// Expected words and figures from the page's own `segmentOf`, `verdictOf` and pool figure, run in node with TZ=UTC.
    #[test]
    fn verdicts_take_every_branch_as_the_web_does() {
        let s = |key, used, resets: Option<f64>, read| windowed(key, "session", used, resets.map(|r| NOW + r), read);
        let wk = |key, used, resets: Option<f64>, read| windowed(key, "week", used, resets.map(|r| NOW + r), read);
        let v = |word: &str, tone| Verdict { word: word.into(), tone };
        for (name, kind, accounts, want, figure) in [
            ("on pace", Kind::Session, vec![s("a", 50.0, Some(2.5 * HOUR), NOW)], v("On pace, resets in 2 h", Tone::Plain), 50.0),
            (
                "ahead",
                Kind::Session,
                vec![s("a", 80.0, Some(2.5 * HOUR), NOW), s("b", 45.0, Some(2.5 * HOUR), NOW)],
                v("Ahead of pace, resets in 2 h", Tone::Warn),
                63.0,
            ),
            ("under", Kind::Session, vec![s("a", 20.0, Some(2.5 * HOUR), NOW)], v("Under pace, resets in 2 h", Tone::Quiet), 20.0),
            ("runs out today", Kind::Session, vec![s("a", 80.0, Some(2.5 * HOUR), NOW)], v("Runs out 14:42", Tone::Warn), 80.0),
            ("runs out later", Kind::Week, vec![wk("a", 60.0, Some(3.5 * DAY), NOW)], v("Runs out Mon 22:05", Tone::Warn), 60.0),
            (
                "stale",
                Kind::Session,
                vec![s("a", 30.0, Some(-HOUR), NOW - DAY), s("b", 30.0, Some(-2.0 * HOUR), NOW - 2.0 * DAY)],
                v("As of Thu 14:05", Tone::Quiet),
                30.0,
            ),
            ("stale with no reset", Kind::Week, vec![wk("a", 30.0, None, NOW - 8.0 * DAY)], v("As of Fri 14:05", Tone::Quiet), 30.0),
            (
                "reached",
                Kind::Session,
                vec![s("a", 100.0, Some(HOUR), NOW), s("b", 120.0, Some(2.0 * HOUR), NOW)],
                v("Limit reached", Tone::Warn),
                100.0,
            ),
            ("just reset", Kind::Session, vec![s("a", 1.0, Some(4.95 * HOUR), NOW)], v("Resets in 4 h", Tone::Plain), 1.0),
            ("no reset", Kind::Session, vec![s("a", 40.0, None, NOW)], v("", Tone::Plain), 40.0),
            (
                "a stale reading set aside",
                Kind::Session,
                vec![s("a", 100.0, Some(-HOUR), NOW - DAY), s("b", 20.0, Some(2.5 * HOUR), NOW)],
                v("Under pace, resets in 2 h", Tone::Quiet),
                60.0,
            ),
            ("nothing used", Kind::Session, vec![s("a", 0.0, Some(2.5 * HOUR), NOW)], v("Under pace, resets in 2 h", Tone::Quiet), 0.0),
        ] {
            assert_eq!(verdict(kind, &accounts), (want, figure), "{name}");
        }
    }

    #[test]
    fn segments_know_their_pace_and_staleness() {
        let fresh = windowed("a", "session", 50.0, Some(NOW + 2.5 * HOUR), NOW);
        let seg = segment_of(&fresh, Kind::Session, NOW).unwrap();
        assert_eq!((seg.elapsed, seg.stale_at), (Some(0.5), None));
        assert!(segment_of(&fresh, Kind::Week, NOW).is_none());
        let old = windowed("b", "week", 10.0, None, NOW - 8.0 * DAY);
        assert_eq!(segment_of(&old, Kind::Week, NOW).unwrap().stale_at, Some(NOW - 8.0 * DAY));
        let unread = account(json!({ "windows": [{ "kind": "week", "usedPercent": 10 }] }));
        let seg = segment_of(&unread, Kind::Week, NOW).unwrap();
        assert_eq!((seg.elapsed, seg.stale_at), (None, None));
        assert_eq!(left_word(38.0), "62%");
        assert_eq!(left_word(130.0), "0%");
    }

    #[test]
    fn pools_sort_segments_by_reset_and_name_themselves() {
        let a = windowed("a", "session", 10.0, None, NOW - 60_000.0 * 3.0);
        let mut b = windowed("b", "session", 20.0, Some(NOW + HOUR), NOW);
        b.computers = vec!["Boat".into(), "Laptop".into()];
        b.burn = serde_json::from_value(json!({ "tokensPerMinute": 4270, "threads": 2 })).unwrap();
        let mut c = windowed("c", "week", 30.0, Some(NOW + 2.0 * DAY), NOW);
        c.agent = "claude".into();
        let accounts = [a, b, c];
        let limits = limits(&accounts);
        assert_eq!(limits.pools.len(), 2);
        let codex = &limits.pools[0];
        assert_eq!(
            (codex.head(), codex.title(), codex.line()),
            ("Codex".to_string(), "2 Codex accounts".to_string(), "Boat and Laptop".to_string())
        );
        assert_eq!(codex.burn_word().as_deref(), Some("Using 4.27k tokens a minute across 2 threads"));
        assert_eq!(codex.checked(NOW), "checked 3 min ago");
        assert_eq!(codex.kinds(), [Kind::Session]);
        let windows = codex.windows(NOW, &utc());
        assert_eq!(windows[0].segments.iter().map(|s| s.account.key.as_str()).collect::<Vec<_>>(), ["b", "a"]);
        assert_eq!((windows[0].word.as_str(), windows[0].left.as_str()), ("5-hour", "85%"));
        let claude = &limits.pools[1];
        assert_eq!((claude.title(), claude.burn_word()), ("Codex".to_string(), None));
        assert_eq!(claude.windows(NOW, &utc())[0].word, "Week");
    }

    #[test]
    fn limits_group_pools_quiet_rows_and_unlimited_agents() {
        let accounts = [
            windowed("codex:plus", "session", 62.0, Some(NOW + HOUR), NOW),
            windowed("codex:over", "overage", 5.0, None, NOW),
            account(
                json!({ "key": "claude:vault-key", "agent": "claude", "label": "Claude Code with an API key", "computers": ["Boat"], "note": "pays per token, no plan limit" }),
            ),
            account(
                json!({ "key": "opencode@here", "agent": "opencode", "label": "OpenCode signed in on Boat", "computers": ["Boat"], "note": "reports no plan limit" }),
            ),
            account(
                json!({ "key": "opencode@lap", "agent": "opencode", "label": "OpenCode signed in on Lap", "computers": ["Lap"], "note": "reports no plan limit" }),
            ),
            account(
                json!({ "key": "cursor@here", "agent": "cursor", "label": "Cursor signed in on Boat", "computers": ["Boat"], "note": "reports no plan limit" }),
            ),
            account(
                json!({ "key": "claude@lap", "agent": "claude", "label": "Claude Code signed in on Lap", "computers": ["Lap"], "status": "reached", "note": "not read yet: shows after its next turn" }),
            ),
        ];
        let limits = limits(&accounts);
        assert_eq!(limits.pools.iter().map(|p| p.accounts.len()).collect::<Vec<_>>(), [1]);
        assert_eq!(limits.unlimited, ["OpenCode", "Cursor"]);
        assert_eq!(limits.no_limit_word().as_deref(), Some("OpenCode and Cursor report no plan limit."));
        let quiet: Vec<(&str, &str, &str, bool)> =
            limits.quiet.iter().map(|l| (l.account.key.as_str(), l.line.as_str(), l.state.as_str(), l.warn)).collect();
        assert_eq!(
            quiet,
            [
                ("codex:over", "Boat", "", false),
                ("claude:vault-key", "Boat", "Pays per token, no plan limit", false),
                ("claude@lap", "", "Limit reached", true),
            ]
        );
        assert!(limits.has_quiet_card());
        let none = super::limits(&[]);
        assert!(none.pools.is_empty() && !none.has_quiet_card() && none.no_limit_word().is_none());
    }

    #[test]
    fn a_reset_answers_row_reads_as_an_accounts_row() {
        let sent: ResetAnswerAccount = serde_json::from_value(json!({
            "key": "codex:plus", "agent": "codex", "label": "Codex with ChatGPT Plus", "computers": ["Boat"],
            "windows": [{ "kind": "session", "usedPercent": 0, "resetsAt": NOW + 5.0 * HOUR }], "readAt": NOW,
            "credits": { "count": 1, "readAt": NOW }
        }))
        .unwrap();
        let row = account_row(&sent).unwrap();
        assert_eq!((row.key.as_str(), row.windows[0].kind, row.credits.map(|c| c.count)), ("codex:plus", Kind::Session, Some(1)));
    }

    #[test]
    fn reset_lines_stand_where_credits_are_known() {
        let plain = account(json!({}));
        assert_eq!(reset_line(&plain, NOW), None);
        let banked = account(
            json!({ "label": "Codex with ChatGPT Plus", "credits": { "count": 2, "nextExpiresAt": NOW + 21.0 * DAY, "readAt": NOW } }),
        );
        let line = reset_line(&banked, NOW).unwrap();
        assert_eq!(line.banked, "2 banked, first expires in 21 d");
        assert!(line.can_use);
        assert!(line.question.starts_with("Use one of the 2 resets banked on Codex with ChatGPT Plus?"));
        let spent = account(json!({ "credits": { "count": 0, "readAt": NOW } }));
        assert_eq!(reset_line(&spent, NOW).map(|l| (l.banked, l.can_use)), Some(("none banked".to_string(), false)));
    }
}
