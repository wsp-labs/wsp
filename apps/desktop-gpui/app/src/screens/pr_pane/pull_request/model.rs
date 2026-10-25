// SPDX-License-Identifier: AGPL-3.0-only
//! What the pane draws, read off the status and the page the host sends by the protocol's names
//! (`packages/protocol/src/pull-request.ts`), and the readings today's pane makes of them: the one word and its tone,
//! the checks line and what holds the merge, the conversation in time order with its threads, the commits by day.

use std::collections::HashMap;

use chrono::{DateTime, Datelike, Local, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::words::{self, PR_WORDS};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum CheckState {
    Pass,
    Fail,
    #[default]
    Pending,
    Skipped,
    Cancelled,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub name: String,
    pub workflow: Option<String>,
    pub state: CheckState,
    pub link: Option<String>,
    pub description: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

impl MergeMethod {
    pub fn wire(self) -> &'static str {
        match self {
            MergeMethod::Merge => "merge",
            MergeMethod::Squash => "squash",
            MergeMethod::Rebase => "rebase",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum State {
    #[default]
    Open,
    Merged,
    Closed,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mergeable {
    Mergeable,
    Conflicting,
    #[default]
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Review {
    #[default]
    None,
    Approved,
    ChangesAsked,
    Required,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AutoMerge {
    pub method: MergeMethod,
}

/// `PullRequestFact`: the pull request as the host last read it.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Fact {
    pub number: u64,
    pub url: String,
    pub state: State,
    #[serde(default)]
    pub draft: bool,
    pub base: String,
    pub branch: String,
    pub head_oid: String,
    #[serde(default)]
    pub head_subject: String,
    #[serde(default)]
    pub mergeable: Mergeable,
    #[serde(default)]
    pub review: Review,
    #[serde(default)]
    pub checks: Vec<Check>,
    #[serde(default)]
    pub changed_files: u64,
    #[serde(default)]
    pub commits: u64,
    pub behind_base: Option<u64>,
    pub author: Option<String>,
    pub auto_merge: Option<AutoMerge>,
}

/// `PullRequestKept`: what the record kept of a settled one.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Kept {
    pub number: u64,
    pub url: String,
    pub state: State,
    pub base: String,
    pub branch: Option<String>,
    pub merged_at: Option<f64>,
    pub closed_at: Option<f64>,
    pub checks: Option<Vec<Check>>,
}

/// `PullRequestSeen`: the fact, the kept record, or the sentence saying why none could be read.
#[derive(Clone, Debug, PartialEq)]
pub enum Seen {
    Fact(Fact),
    Kept(Kept),
    Unread(String),
}

impl Seen {
    pub fn read(value: &Value) -> Option<Seen> {
        if value.get("headOid").is_some() {
            return serde_json::from_value(value.clone()).ok().map(Seen::Fact);
        }
        if value.get("number").is_some() {
            return serde_json::from_value(value.clone()).ok().map(Seen::Kept);
        }
        value.get("why").and_then(Value::as_str).map(|why| Seen::Unread(why.to_owned()))
    }

    pub fn fact(&self) -> Option<&Fact> {
        match self {
            Seen::Fact(f) => Some(f),
            _ => None,
        }
    }

    pub fn number(&self) -> Option<u64> {
        match self {
            Seen::Fact(f) => Some(f.number),
            Seen::Kept(k) => Some(k.number),
            Seen::Unread(_) => None,
        }
    }

    pub fn url(&self) -> &str {
        match self {
            Seen::Fact(f) => &f.url,
            Seen::Kept(k) => &k.url,
            Seen::Unread(_) => "",
        }
    }

    pub fn state(&self) -> State {
        match self {
            Seen::Fact(f) => f.state,
            Seen::Kept(k) => k.state,
            Seen::Unread(_) => State::Open,
        }
    }

    pub fn base(&self) -> &str {
        match self {
            Seen::Fact(f) => &f.base,
            Seen::Kept(k) => &k.base,
            Seen::Unread(_) => "",
        }
    }

    pub fn checks(&self) -> &[Check] {
        match self {
            Seen::Fact(f) => &f.checks,
            Seen::Kept(k) => k.checks.as_deref().unwrap_or_default(),
            Seen::Unread(_) => &[],
        }
    }

    /// What of it says the page moved: where it stands, its head, its review and each check's state.
    pub fn mark(&self) -> String {
        let (head, review) = match self {
            Seen::Fact(f) => (f.head_oid.as_str(), format!("{:?}", f.review)),
            _ => ("", String::new()),
        };
        let checks: Vec<String> = self.checks().iter().map(|c| format!("{}:{:?}", c.name, c.state)).collect();
        format!("{:?}|{head}|{review}|{}", self.state(), checks.join(","))
    }
}

/// The copy's checkout as the status carries it.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Checkout {
    pub branch: String,
    #[serde(default)]
    pub ahead: i64,
    #[serde(default)]
    pub behind: i64,
    #[serde(default)]
    pub changed: i64,
}

/// Where the work came from, as the head's source link says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum From {
    Issue { number: u64, title: String, url: String },
    PullRequest { number: u64, url: String },
    Review,
}

/// The parts of a workspace's status the pane and the git button read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    pub name: String,
    pub phase: String,
    pub machine_state: String,
    pub project_id: String,
    pub checkout: Option<Checkout>,
    pub pr: Option<Seen>,
    pub from: Option<From>,
    pub opens_pr: bool,
    pub kind: String,
}

impl Status {
    pub fn read(value: &Value) -> Status {
        let text = |key: &str| value.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
        let from = value.get("from").and_then(|f| {
            let n = f.get("number").and_then(Value::as_u64).unwrap_or_default();
            let url = f.get("url").and_then(Value::as_str).unwrap_or_default().to_owned();
            match f.get("kind").and_then(Value::as_str)? {
                "issue" => {
                    Some(From::Issue { number: n, title: f.get("title").and_then(Value::as_str).unwrap_or_default().to_owned(), url })
                }
                "pullRequest" | "pr" => Some(From::PullRequest { number: n, url }),
                "review" => Some(From::Review),
                _ => None,
            }
        });
        Status {
            name: text("name"),
            phase: text("phase"),
            machine_state: text("machineState"),
            kind: text("kind"),
            project_id: value.pointer("/project/id").and_then(Value::as_str).unwrap_or_default().to_owned(),
            checkout: value.get("checkout").and_then(|c| serde_json::from_value(c.clone()).ok()),
            pr: value.get("pr").and_then(Seen::read),
            from,
            opens_pr: value.get("parentThreadId").is_none_or(Value::is_null),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Reaction {
    pub content: String,
    pub count: u64,
    pub mine: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ReviewRequest {
    pub name: String,
    #[serde(default)]
    pub team: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct LatestReview {
    pub author: String,
    pub state: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub oid: String,
    pub subject: String,
    #[serde(default)]
    pub body: String,
    pub at: String,
    #[serde(default)]
    pub author: String,
    pub parents: Option<u64>,
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PageReview {
    pub id: Option<u64>,
    pub node_id: Option<String>,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub at: String,
    pub reactions: Option<Vec<Reaction>>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: u64,
    pub node_id: Option<String>,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub bot: bool,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub at: String,
    pub reactions: Option<Vec<Reaction>>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LineComment {
    pub id: u64,
    pub node_id: Option<String>,
    pub path: String,
    pub line: Option<u64>,
    pub side: Option<String>,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub bot: bool,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub at: String,
    pub hunk: Option<String>,
    pub reply_to: Option<u64>,
    pub review_id: Option<u64>,
    pub resolved: Option<bool>,
    pub thread_id: Option<String>,
    pub reactions: Option<Vec<Reaction>>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct PageFile {
    pub path: String,
    #[serde(default)]
    pub additions: u64,
    #[serde(default)]
    pub deletions: u64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Cut {
    #[serde(default)]
    pub commits: bool,
    #[serde(default)]
    pub reviews: bool,
    #[serde(default)]
    pub threads: bool,
    #[serde(default)]
    pub comments: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RepoMerge {
    pub methods: Vec<MergeMethod>,
    pub default_method: MergeMethod,
    #[serde(default)]
    pub auto_merge: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    Comment,
    Review,
    ReviewComment,
}

/// `PullRequestItem`: one item a send to the agent names.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct Item {
    pub kind: ItemKind,
    pub id: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Sent {
    pub kind: ItemKind,
    pub id: u64,
    pub at: f64,
}

/// `PullRequestPage`: what the pane reads.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub created_at: String,
    pub closed_at: Option<String>,
    pub merged_at: Option<String>,
    pub merged_by: Option<String>,
    pub merge_commit: Option<String>,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub review_requests: Vec<ReviewRequest>,
    #[serde(default)]
    pub latest_reviews: Vec<LatestReview>,
    #[serde(default)]
    pub commits: Vec<Commit>,
    #[serde(default)]
    pub reviews: Vec<PageReview>,
    #[serde(default)]
    pub comments: Vec<Comment>,
    #[serde(default)]
    pub review_comments: Vec<LineComment>,
    #[serde(default)]
    pub files: Vec<PageFile>,
    pub cut: Option<Cut>,
    pub merge: Option<RepoMerge>,
    #[serde(default)]
    pub sent: Vec<Sent>,
    #[serde(default)]
    pub posts_as_you: bool,
}

/// The ink a state wears, off the theme's own status tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Ok,
    Bad,
    Warn,
    Run,
    Merged,
    Quiet,
}

/// Where a pull request stands, the one key its word and its ink are read off (`pullRequestKey`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Merged,
    Closed,
    Conflicts,
    Failed,
    Running,
    ChangesAsked,
    Approved,
    Draft,
    Open,
    Unread,
}

pub fn key_of(seen: &Seen) -> Key {
    match seen {
        Seen::Unread(_) => Key::Unread,
        _ if seen.state() == State::Merged => Key::Merged,
        _ if seen.state() == State::Closed => Key::Closed,
        Seen::Kept(_) => Key::Open,
        Seen::Fact(f) if f.mergeable == Mergeable::Conflicting => Key::Conflicts,
        Seen::Fact(f) if f.checks.iter().any(|c| c.state == CheckState::Fail) => Key::Failed,
        Seen::Fact(f) if f.checks.iter().any(|c| c.state == CheckState::Pending) => Key::Running,
        Seen::Fact(f) if f.review == Review::ChangesAsked => Key::ChangesAsked,
        Seen::Fact(f) if f.review == Review::Approved => Key::Approved,
        Seen::Fact(f) if f.draft => Key::Draft,
        Seen::Fact(_) => Key::Open,
    }
}

/// The word, its tone and whether its mark is hollow (`pullRequestTone`).
pub fn tone_of(seen: &Seen) -> (String, Tone, bool) {
    let key = key_of(seen);
    let word = match key {
        Key::Conflicts => format!("conflicts with {}", seen.base()),
        Key::Open => "open".into(),
        Key::Draft => "draft".into(),
        Key::Running => "checks running".into(),
        Key::Failed => "checks failed".into(),
        Key::ChangesAsked => "changes asked for".into(),
        Key::Approved => "approved".into(),
        Key::Merged => "merged".into(),
        Key::Closed => "closed".into(),
        Key::Unread => "not read".into(),
    };
    let tone = match key {
        Key::Merged => Tone::Merged,
        Key::Closed | Key::Conflicts | Key::Failed => Tone::Bad,
        Key::Running => Tone::Run,
        Key::ChangesAsked => Tone::Warn,
        Key::Approved | Key::Open => Tone::Ok,
        Key::Draft | Key::Unread => Tone::Quiet,
    };
    (capitalised(&word), tone, matches!(key, Key::Draft | Key::Unread))
}

pub fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// Whether a merge can be offered now: open, mergeable, no check failed (`pullRequestMergeable`).
pub fn mergeable(fact: &Fact) -> bool {
    fact.state == State::Open && fact.mergeable == Mergeable::Mergeable && !fact.checks.iter().any(|c| c.state == CheckState::Fail)
}

const COUNT_ORDER: [CheckState; 5] = [CheckState::Pass, CheckState::Fail, CheckState::Pending, CheckState::Skipped, CheckState::Cancelled];

pub fn count_word(state: CheckState) -> &'static str {
    match state {
        CheckState::Pass => "passed",
        CheckState::Fail => "failed",
        CheckState::Pending => "running",
        CheckState::Skipped => "skipped",
        CheckState::Cancelled => "cancelled",
    }
}

/// The worst of the checks, which the checks line wears.
pub fn worst_check(checks: &[Check]) -> CheckState {
    if checks.iter().any(|c| c.state == CheckState::Fail) {
        CheckState::Fail
    } else if checks.iter().any(|c| c.state == CheckState::Pending) {
        CheckState::Pending
    } else if checks.iter().all(|c| c.state == CheckState::Pass) {
        CheckState::Pass
    } else {
        CheckState::Skipped
    }
}

/// "3 passed, 1 failed, 1 running".
pub fn checks_line(checks: &[Check]) -> String {
    COUNT_ORDER
        .iter()
        .filter_map(|state| {
            let n = checks.iter().filter(|c| c.state == *state).count();
            (n > 0).then(|| format!("{n} {}", count_word(*state)))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Each reviewer's standing verdict where it approved or asked for changes: (author, approved).
pub fn verdicts_of(latest: &[LatestReview]) -> Vec<(String, bool)> {
    latest
        .iter()
        .filter_map(|r| match r.state.to_lowercase().as_str() {
            "approved" => Some((r.author.clone(), true)),
            "changes_requested" => Some((r.author.clone(), false)),
            _ => None,
        })
        .collect()
}

pub fn review_sentence(verdicts: &[(String, bool)]) -> String {
    verdicts
        .iter()
        .map(|(who, approved)| format!("{} {}", author_name(who), if *approved { "approved" } else { "asked for changes" }))
        .collect::<Vec<_>>()
        .join(", ")
}

/// What holds the merge, in one sentence (`holdLine`).
pub fn hold_line(fact: &Fact) -> String {
    let w = &PR_WORDS.hold;
    if fact.mergeable == Mergeable::Conflicting {
        return w.conflicts.into();
    }
    if let Some(auto) = &fact.auto_merge {
        return words::merges_when_checks_pass(auto.method);
    }
    if fact.checks.iter().any(|c| matches!(c.state, CheckState::Fail | CheckState::Pending)) {
        return w.checks.into();
    }
    if fact.draft {
        return w.draft.into();
    }
    if matches!(fact.review, Review::Required | Review::ChangesAsked) {
        return w.review.into();
    }
    if fact.mergeable == Mergeable::Unknown {
        return w.unknown.into();
    }
    w.ready.into()
}

/// The methods the repository allows, its default first.
pub fn methods(repo: Option<&RepoMerge>) -> Vec<MergeMethod> {
    let Some(repo) = repo else { return Vec::new() };
    std::iter::once(repo.default_method).chain(repo.methods.iter().copied().filter(|m| *m != repo.default_method)).collect()
}

/// Whether Merge when checks pass stands beside Merge: checks run, the repository merges by itself, nothing is armed.
pub fn waits(fact: &Fact, repo: Option<&RepoMerge>) -> bool {
    fact.checks.iter().any(|c| c.state == CheckState::Pending)
        && repo.is_some_and(|r| r.auto_merge)
        && fact.auto_merge.is_none()
        && fact.mergeable != Mergeable::Conflicting
}

pub fn parse_time(at: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(at).ok().map(|t| t.with_timezone(&Utc))
}

pub fn now() -> DateTime<Utc> {
    Utc::now()
}

/// How long something took, as a row says it: "16 min", "1 h 5 min", "40 s".
pub fn span_word(ms: i64) -> String {
    let s = ((ms as f64) / 1000.).round().max(0.) as i64;
    if s < 60 {
        return format!("{s} s");
    }
    let m = ((s as f64) / 60.).round() as i64;
    if m < 60 {
        return format!("{m} min");
    }
    let h = m / 60;
    if m % 60 == 0 {
        format!("{h} h")
    } else {
        format!("{h} h {} min", m % 60)
    }
}

/// "3h ago", as `formatRelativeTimeLabel` writes it; nothing for a time that does not read.
pub fn ago(at: &str, now: DateTime<Utc>) -> String {
    let Some(then) = parse_time(at) else { return String::new() };
    crate::screens::composer::model::ago((now - then).to_std().unwrap_or_default())
}

/// The same with the figure apart from its unit: "3 h ago".
pub fn spaced_ago(at: &str, now: DateTime<Utc>) -> String {
    spaced(&ago(at, now))
}

/// What a settled pull request's rows say (`Status.tsx`): who merged it, how long ago it settled, and for a merge the
/// commit it landed as on its base.
#[derive(Clone, Debug, PartialEq)]
pub struct Settled {
    pub merged: bool,
    pub by: Option<String>,
    pub ago: Option<String>,
    pub landed: Option<(String, String)>,
}

/// The settled rows' words; none while the pull request is open. The page's times win over what the host kept.
pub fn settled(seen: &Seen, page: Option<&Page>, now: DateTime<Utc>) -> Option<Settled> {
    let merged = match seen.state() {
        State::Merged => true,
        State::Closed => false,
        _ => return None,
    };
    let kept = match seen {
        Seen::Kept(k) => if merged { k.merged_at } else { k.closed_at }.map(ms_to_iso),
        _ => None,
    };
    let read = page.and_then(|p| if merged { p.merged_at.clone() } else { p.closed_at.clone() }).filter(|s| !s.is_empty());
    let by = page.and_then(|p| p.merged_by.clone()).filter(|b| merged && !b.is_empty());
    let landed = page
        .and_then(|p| p.merge_commit.clone())
        .filter(|s| merged && !s.is_empty())
        .map(|sha| (words::landed_as(seen.base()), sha.chars().take(7).collect()));
    Some(Settled { merged, by, ago: read.or(kept).map(|at| spaced_ago(&at, now)), landed })
}

pub fn spaced(ago: &str) -> String {
    let digits = ago.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 || digits == ago.len() {
        return ago.to_owned();
    }
    let (n, rest) = ago.split_at(digits);
    if rest.starts_with(|c: char| c.is_ascii_lowercase()) {
        format!("{n} {rest}")
    } else {
        ago.to_owned()
    }
}

pub fn ms_to_iso(ms: f64) -> String {
    Utc.timestamp_millis_opt(ms as i64).single().map(|t| t.to_rfc3339()).unwrap_or_default()
}

/// Under a failed check: its workflow and how long it ran, where the host read when.
pub fn failed_note(c: &Check) -> Option<String> {
    if let (Some(start), Some(end)) = (c.started_at.as_deref().and_then(parse_time), c.completed_at.as_deref().and_then(parse_time)) {
        return Some(words::failed_after(c.workflow.as_deref(), &span_word((end - start).num_milliseconds())));
    }
    c.workflow.clone().or_else(|| c.description.clone())
}

/// An author as the pane names them: the login, or a deleted account where GitHub names none.
pub fn author_name(login: &str) -> String {
    if login.trim().is_empty() {
        PR_WORDS.deleted_account.into()
    } else {
        login.to_owned()
    }
}

/// A thread on a line: its first comment and the replies to it, oldest first.
#[derive(Clone, Debug, PartialEq)]
pub struct Thread {
    pub key: String,
    pub path: String,
    pub line: Option<u64>,
    pub hunk: Option<String>,
    pub resolved: bool,
    pub thread_id: Option<String>,
    pub review_id: Option<u64>,
    pub comments: Vec<LineComment>,
}

fn by_time(a: &str, b: &str) -> std::cmp::Ordering {
    let t = |s: &str| parse_time(s).map(|t| t.timestamp_millis()).unwrap_or(0);
    t(a).cmp(&t(b))
}

/// The comments on lines as threads, a reply under the comment it answers (`threadsOf`).
pub fn threads_of(comments: &[LineComment]) -> Vec<Thread> {
    let mut sorted: Vec<&LineComment> = comments.iter().collect();
    sorted.sort_by(|a, b| by_time(&a.at, &b.at).then(a.id.cmp(&b.id)));
    let mut order: Vec<u64> = Vec::new();
    let mut roots: HashMap<u64, Vec<LineComment>> = HashMap::new();
    for c in sorted {
        let root = match c.reply_to {
            Some(r) if roots.contains_key(&r) => r,
            _ => c.id,
        };
        if !roots.contains_key(&root) {
            order.push(root);
        }
        roots.entry(root).or_default().push(c.clone());
    }
    order
        .into_iter()
        .map(|id| {
            let held = roots.remove(&id).unwrap_or_default();
            let first = held[0].clone();
            Thread {
                key: format!("thread:{id}"),
                path: first.path.clone(),
                line: first.line,
                hunk: first.hunk.clone(),
                resolved: held.iter().any(|c| c.resolved == Some(true)),
                thread_id: held.iter().find_map(|c| c.thread_id.clone()),
                review_id: first.review_id,
                comments: held,
            }
        })
        .collect()
}

/// The reviews the timeline draws: an empty commented review that holds no thread is left out.
fn reviews_said<'a>(page: &'a Page, threads: &[Thread]) -> Vec<(usize, &'a PageReview)> {
    page.reviews
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            r.state.to_lowercase() != "commented"
                || !r.body.trim().is_empty()
                || threads.iter().any(|t| t.review_id.is_some() && t.review_id == r.id)
        })
        .collect()
}

/// What the Conversation tab counts: every comment and every review it draws.
pub fn conversation_count(page: &Page) -> usize {
    count_with(page, &threads_of(&page.review_comments))
}

fn count_with(page: &Page, threads: &[Thread]) -> usize {
    page.comments.len() + reviews_said(page, threads).len()
}

/// The key a review's entry and its body are held by: its id, else its place in the page's reviews.
pub fn review_key(id: Option<u64>, ix: usize) -> String {
    format!("review:{}", id.map_or(format!("at{ix}"), |id| id.to_string()))
}

/// Every body the page draws, by the key its entry looks it up under.
pub fn bodies_of(page: &Page) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![("description".into(), page.body.clone())];
    out.extend(page.comments.iter().map(|c| (format!("comment:{}", c.id), c.body.clone())));
    out.extend(page.reviews.iter().enumerate().map(|(i, r)| (review_key(r.id, i), r.body.clone())));
    out.extend(page.review_comments.iter().map(|c| (format!("reviewComment:{}", c.id), c.body.clone())));
    out
}

/// What the page reads as, worked out once each time it changes rather than on every frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Derived {
    pub threads: Vec<Thread>,
    pub entries: Vec<Entry>,
    pub open: (Vec<Item>, &'static str),
    pub count: usize,
}

pub fn derive(page: &Page) -> Derived {
    let threads = threads_of(&page.review_comments);
    Derived { entries: conversation_with(page, &threads), open: open_with(page, &threads), count: count_with(page, &threads), threads }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Entry {
    Comment { key: String, at: String, comment: Comment },
    Review { key: String, at: String, review: PageReview, threads: Vec<Thread> },
    Thread { key: String, at: String, thread: Thread },
    Push { key: String, at: String, commits: Vec<Commit>, authors: Vec<String> },
}

impl Entry {
    pub fn key(&self) -> &str {
        match self {
            Entry::Comment { key, .. } | Entry::Review { key, .. } | Entry::Thread { key, .. } | Entry::Push { key, .. } => key,
        }
    }

    fn at(&self) -> &str {
        match self {
            Entry::Comment { at, .. } | Entry::Review { at, .. } | Entry::Thread { at, .. } | Entry::Push { at, .. } => at,
        }
    }
}

/// The timeline oldest first, the commits pushed between two entries folded into one (`conversationOf`).
pub fn conversation_of(page: &Page) -> Vec<Entry> {
    conversation_with(page, &threads_of(&page.review_comments))
}

fn conversation_with(page: &Page, threads: &[Thread]) -> Vec<Entry> {
    let review_ids: Vec<u64> = page.reviews.iter().filter_map(|r| r.id).collect();
    let mut said: Vec<Entry> = Vec::new();
    for (ix, review) in reviews_said(page, threads) {
        let own = threads.iter().filter(|t| t.review_id.is_some() && t.review_id == review.id).cloned().collect();
        let key = review_key(review.id, ix);
        said.push(Entry::Review { key, at: review.at.clone(), review: review.clone(), threads: own });
    }
    for comment in &page.comments {
        said.push(Entry::Comment { key: format!("comment:{}", comment.id), at: comment.at.clone(), comment: comment.clone() });
    }
    for thread in threads.iter().filter(|t| t.review_id.is_none_or(|id| !review_ids.contains(&id))) {
        said.push(Entry::Thread { key: thread.key.clone(), at: thread.comments[0].at.clone(), thread: thread.clone() });
    }
    said.sort_by(|a, b| by_time(a.at(), b.at()));
    let mut commits = page.commits.clone();
    commits.sort_by(|a, b| by_time(&a.at, &b.at));
    let millis = |s: &str| parse_time(s).map(|t| t.timestamp_millis()).unwrap_or(0);
    let mut out = Vec::new();
    let mut next = 0;
    let mut push_until = |until: i64, out: &mut Vec<Entry>| {
        let mut run = Vec::new();
        while next < commits.len() && millis(&commits[next].at) <= until {
            run.push(commits[next].clone());
            next += 1;
        }
        if run.is_empty() {
            return;
        }
        let mut authors: Vec<String> = Vec::new();
        for c in &run {
            if !authors.contains(&c.author) {
                authors.push(c.author.clone());
            }
        }
        out.push(Entry::Push { key: format!("push:{}", run[0].oid), at: run[run.len() - 1].at.clone(), commits: run, authors });
    };
    for entry in said {
        push_until(millis(entry.at()), &mut out);
        out.push(entry);
    }
    push_until(i64::MAX, &mut out);
    out
}

/// A review's state as its head says it (`reviewVerdict`).
pub fn review_verdict(state: &str) -> (String, Tone) {
    match state.to_lowercase().as_str() {
        "changes_requested" => ("Changes asked for".into(), Tone::Warn),
        "approved" => ("Approved".into(), Tone::Ok),
        "commented" => ("Commented".into(), Tone::Quiet),
        "dismissed" => ("Dismissed".into(), Tone::Quiet),
        "pending" => ("Pending".into(), Tone::Quiet),
        other => (capitalised(&other.replace('_', " ")), Tone::Quiet),
    }
}

/// When an item was sent to the agent, where it was.
pub fn sent_at(page: &Page, item: Item) -> Option<f64> {
    page.sent.iter().find(|s| s.kind == item.kind && s.id == item.id).map(|s| s.at)
}

/// What the Comments row sends and the word it counts them by (`openComments`).
pub fn open_comments(page: &Page) -> (Vec<Item>, &'static str) {
    open_with(page, &threads_of(&page.review_comments))
}

fn open_with(page: &Page, threads: &[Thread]) -> (Vec<Item>, &'static str) {
    let items = threads
        .iter()
        .filter(|t| !t.resolved)
        .flat_map(|t| t.comments.iter())
        .filter(|c| !c.bot)
        .map(|c| Item { kind: ItemKind::ReviewComment, id: c.id })
        .filter(|item| sent_at(page, *item).is_none())
        .collect();
    let word = if page.review_comments.iter().any(|c| c.resolved.is_some()) { "unresolved" } else { "unsent" };
    (items, word)
}

/// GitHub's eight reactions in its picker's order: the REST name, the emoji, its name.
pub const REACTIONS: [(&str, &str, &str); 8] = [
    ("+1", "👍", "Thumbs up"),
    ("-1", "👎", "Thumbs down"),
    ("laugh", "😄", "Laugh"),
    ("hooray", "🎉", "Hooray"),
    ("confused", "😕", "Confused"),
    ("heart", "❤️", "Heart"),
    ("rocket", "🚀", "Rocket"),
    ("eyes", "👀", "Eyes"),
];

pub fn emoji_of(content: &str) -> &str {
    REACTIONS.iter().find(|r| r.0 == content).map_or(content, |r| r.1)
}

pub fn reaction_label(content: &str) -> &str {
    REACTIONS.iter().find(|r| r.0 == content).map_or(content, |r| r.2)
}

/// The reactions with the person's own put on or taken off, as GitHub would count them (`toggled`).
pub fn toggled(reactions: &[Reaction], content: &str, on: bool) -> Vec<Reaction> {
    let held = reactions.iter().position(|r| r.content == content);
    let mut out = reactions.to_vec();
    match (on, held) {
        (true, None) => out.push(Reaction { content: content.into(), count: 1, mine: true }),
        (true, Some(i)) if !out[i].mine => {
            out[i].count += 1;
            out[i].mine = true;
        }
        (false, Some(i)) if out[i].mine => {
            if out[i].count <= 1 {
                out.remove(i);
            } else {
                out[i].count -= 1;
                out[i].mine = false;
            }
        }
        _ => {}
    }
    out
}

/// The reactions of the item a node id names, wherever it stands on the page.
pub fn reactions_of(page: &Page, subject: &str) -> Option<Vec<Reaction>> {
    let named = |id: &Option<String>| id.as_deref() == Some(subject);
    page.comments
        .iter()
        .find(|c| named(&c.node_id))
        .map(|c| c.reactions.clone())
        .or_else(|| page.reviews.iter().find(|r| named(&r.node_id)).map(|r| r.reactions.clone()))
        .or_else(|| page.review_comments.iter().find(|c| named(&c.node_id)).map(|c| c.reactions.clone()))
        .map(Option::unwrap_or_default)
}

/// The page with one item's reactions given, the item named by its node id (`withReactions`).
pub fn set_reactions(page: &mut Page, subject: &str, reactions: Vec<Reaction>) {
    let named = |id: &Option<String>| id.as_deref() == Some(subject);
    for c in page.comments.iter_mut().filter(|c| named(&c.node_id)) {
        c.reactions = Some(reactions.clone());
    }
    for r in page.reviews.iter_mut().filter(|r| named(&r.node_id)) {
        r.reactions = Some(reactions.clone());
    }
    for c in page.review_comments.iter_mut().filter(|c| named(&c.node_id)) {
        c.reactions = Some(reactions.clone());
    }
}

/// A quote reply's words: who it answers, then the item's first three lines as a markdown quote (`quoteOf`).
pub fn quote_of(author: &str, body: &str) -> String {
    let lines: Vec<String> = body.trim().split('\n').filter(|l| !l.trim().is_empty()).take(3).map(|l| format!("> {l}")).collect();
    format!("@{author}\n\n{}\n\n", lines.join("\n"))
}

/// One row of the Commits tab.
#[derive(Clone, Debug, PartialEq)]
pub struct CommitRow {
    pub oid: String,
    pub sha: String,
    pub subject: String,
    pub author: String,
    pub at: String,
    pub merge: bool,
    pub from: Option<String>,
    pub lines: Option<(u64, u64)>,
    pub body: String,
}

/// A merge commit read off its subject, with the branch it brought in where it names one; None for any other.
pub fn merge_from(subject: &str) -> Option<Option<String>> {
    let quoted = ["Merge remote-tracking branch '", "Merge branch '"];
    for lead in quoted {
        if let Some(rest) = subject.strip_prefix(lead) {
            if let Some(end) = rest.find('\'') {
                return Some(Some(rest[..end].trim_start_matches("origin/").to_owned()));
            }
        }
    }
    let lower = subject.to_lowercase();
    if lower.starts_with("merge") {
        let rest = &subject[5..];
        let rest = rest.strip_prefix(':').unwrap_or(rest);
        if let Some(rest) = rest.strip_prefix(' ') {
            let mut words = rest.splitn(2, ' ');
            if let (Some(branch), Some(tail)) = (words.next(), words.next()) {
                if !branch.is_empty() && !branch.contains(char::is_whitespace) && tail.to_lowercase().starts_with("into ") {
                    return Some(Some(branch.trim_start_matches("origin/").to_owned()));
                }
            }
        }
    }
    if subject.starts_with("Merge pull request #") && subject.contains(" from ") {
        return Some(None);
    }
    None
}

pub fn is_merge_commit(c: &Commit) -> bool {
    match c.parents {
        Some(n) => n >= 2,
        None => merge_from(&c.subject).is_some(),
    }
}

fn day_word(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let (at, now) = (at.with_timezone(&Local).date_naive(), now.with_timezone(&Local).date_naive());
    match (now - at).num_days() {
        0 => "Today".into(),
        1 => "Yesterday".into(),
        _ => format!("{} {}", MONTHS[at.month0() as usize], at.day()),
    }
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// The commits by local day, newest first (`commitDays`).
pub fn commit_days(commits: &[Commit], now: DateTime<Utc>) -> Vec<(String, Vec<CommitRow>)> {
    let mut newest: Vec<(usize, &Commit)> = commits.iter().enumerate().collect();
    newest.sort_by(|(ia, a), (ib, b)| by_time(&b.at, &a.at).then(ib.cmp(ia)));
    let mut days: Vec<(String, Vec<CommitRow>)> = Vec::new();
    for (_, c) in newest {
        let day = parse_time(&c.at).map(|t| day_word(t, now)).unwrap_or_default();
        let merge = is_merge_commit(c);
        let row = CommitRow {
            oid: c.oid.clone(),
            sha: c.oid.chars().take(7).collect(),
            subject: c.subject.clone(),
            author: c.author.clone(),
            at: c.at.clone(),
            merge,
            from: if merge { merge_from(&c.subject).flatten() } else { None },
            lines: if merge { None } else { c.additions.zip(c.deletions) },
            body: c.body.clone(),
        };
        match days.last_mut() {
            Some((d, rows)) if *d == day => rows.push(row),
            _ => days.push((day, vec![row])),
        }
    }
    days
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn page(v: Value) -> Page {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn the_word_is_the_first_that_holds() {
        let fact = |v: Value| Seen::read(&v).unwrap();
        let base = json!({ "number": 1, "url": "u", "state": "open", "draft": false, "base": "main", "branch": "w", "headOid": "a", "mergeable": "mergeable", "review": "none", "checks": [] });
        let mut v = base.clone();
        v["checks"] = json!([{ "name": "a", "state": "pass" }, { "name": "b", "state": "pending" }]);
        assert_eq!(tone_of(&fact(v.clone())), ("Checks running".into(), Tone::Run, false));
        v["checks"][0]["state"] = json!("fail");
        assert_eq!(tone_of(&fact(v.clone())).0, "Checks failed");
        v["mergeable"] = json!("conflicting");
        assert_eq!(tone_of(&fact(v)).0, "Conflicts with main");
        let mut d = base.clone();
        d["draft"] = json!(true);
        assert_eq!(tone_of(&fact(d)), ("Draft".into(), Tone::Quiet, true));
        let kept = json!({ "number": 1, "url": "u", "state": "merged", "base": "main", "mergedAt": 1.0, "readAt": 2.0 });
        assert_eq!(tone_of(&fact(kept)), ("Merged".into(), Tone::Merged, false));
        assert_eq!(Seen::read(&json!({ "why": "no gh", "readAt": 1 })), Some(Seen::Unread("no gh".into())));
    }

    #[test]
    fn the_hold_line_names_the_first_thing_holding_the_merge() {
        let mut f = Fact { mergeable: Mergeable::Mergeable, ..Fact::default() };
        assert_eq!(hold_line(&f), "Ready to merge");
        f.review = Review::Required;
        assert_eq!(hold_line(&f), "Held until a review approves it");
        f.draft = true;
        assert_eq!(hold_line(&f), "Held while it is a draft");
        f.checks = vec![Check { name: "t".into(), state: CheckState::Pending, ..Check::default() }];
        assert_eq!(hold_line(&f), "Held until the checks pass");
        f.auto_merge = Some(AutoMerge { method: MergeMethod::Squash });
        assert_eq!(hold_line(&f), "Merges by squash when checks pass");
        f.mergeable = Mergeable::Conflicting;
        assert_eq!(hold_line(&f), "Held until the conflicts are fixed");
        assert_eq!(hold_line(&Fact::default()), "GitHub is still working out whether it merges");
    }

    #[test]
    fn checks_count_in_one_order_and_methods_put_the_default_first() {
        let c = |s| Check { name: "x".into(), state: s, ..Check::default() };
        let checks = [c(CheckState::Pending), c(CheckState::Pass), c(CheckState::Pass), c(CheckState::Fail)];
        assert_eq!(checks_line(&checks), "2 passed, 1 failed, 1 running");
        assert_eq!(worst_check(&checks), CheckState::Fail);
        let repo = RepoMerge {
            methods: vec![MergeMethod::Merge, MergeMethod::Squash, MergeMethod::Rebase],
            default_method: MergeMethod::Squash,
            auto_merge: true,
        };
        assert_eq!(methods(Some(&repo)), vec![MergeMethod::Squash, MergeMethod::Merge, MergeMethod::Rebase]);
        assert_eq!(span_word(40_000), "40 s");
        assert_eq!(span_word(16 * 60_000), "16 min");
        assert_eq!(span_word(65 * 60_000), "1 h 5 min");
        assert_eq!(spaced("3h ago"), "3 h ago");
        assert_eq!(spaced("just now"), "just now");
    }

    #[test]
    fn a_reply_joins_its_thread_and_pushes_fold_between_entries() {
        let p = page(json!({
            "reviews": [{ "id": 7, "author": "ben", "state": "CHANGES_REQUESTED", "body": "fix it", "at": "2026-10-18T10:00:00Z" },
                        { "id": 8, "author": "octo", "state": "COMMENTED", "body": "", "at": "2026-10-18T12:00:00Z" }],
            "comments": [{ "id": 1, "author": "ana", "bot": false, "body": "hi", "url": "", "at": "2026-10-18T09:00:00Z" }],
            "reviewComments": [
                { "id": 11, "path": "a.rs", "line": 3, "author": "ben", "bot": false, "body": "x", "url": "", "at": "2026-10-18T10:00:00Z", "reviewId": 7, "resolved": false, "threadId": "T1" },
                { "id": 12, "path": "a.rs", "line": 3, "author": "octo", "bot": false, "body": "y", "url": "", "at": "2026-10-18T12:00:00Z", "replyTo": 11, "reviewId": 8 }],
            "commits": [{ "oid": "c1", "subject": "one", "at": "2026-10-18T08:00:00Z", "author": "octo" },
                        { "oid": "c2", "subject": "two", "at": "2026-10-18T11:00:00Z", "author": "octo" }]
        }));
        let threads = threads_of(&p.review_comments);
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].comments.len(), 2);
        assert_eq!(threads[0].thread_id.as_deref(), Some("T1"));
        let entries = conversation_of(&p);
        let keys: Vec<&str> = entries.iter().map(|e| e.key()).map(|k| k.split(':').next().unwrap()).collect::<Vec<_>>();
        assert_eq!(keys, ["push", "comment", "review", "push"]);
        assert_eq!(conversation_count(&p), 2, "the empty commented review is not counted");
        let (items, word) = open_comments(&p);
        assert_eq!((items.len(), word), (2, "unresolved"));
    }

    #[test]
    fn a_review_s_entry_finds_its_body_whether_or_not_it_has_an_id() {
        let p = page(json!({
            "reviews": [{ "author": "ana", "state": "APPROVED", "body": "older, no id", "at": "2026-10-18T09:00:00Z" },
                        { "id": 9, "author": "ben", "state": "COMMENTED", "body": "", "at": "2026-10-18T09:30:00Z" },
                        { "author": "cam", "state": "CHANGES_REQUESTED", "body": "also no id", "at": "2026-10-18T10:00:00Z" }]
        }));
        let bodies: Vec<String> = bodies_of(&p).into_iter().map(|(k, _)| k).collect();
        let entries = derive(&p).entries;
        let keys: Vec<&str> = entries.iter().map(Entry::key).collect();
        assert_eq!(keys.len(), 2, "the empty commented review is left out");
        for key in keys {
            assert!(bodies.iter().any(|b| b == key), "{key} has its body under the same key: {bodies:?}");
        }
    }

    #[test]
    fn a_reaction_toggles_as_github_counts_it() {
        let r = vec![Reaction { content: "+1".into(), count: 2, mine: true }];
        assert_eq!(toggled(&r, "+1", false), vec![Reaction { content: "+1".into(), count: 1, mine: false }]);
        assert_eq!(toggled(&r, "heart", true).len(), 2);
        assert!(toggled(&[Reaction { content: "+1".into(), count: 1, mine: true }], "+1", false).is_empty());
    }

    #[test]
    fn merges_read_off_their_subject() {
        assert_eq!(merge_from("Merge branch 'main' into work"), Some(Some("main".into())));
        assert_eq!(merge_from("merge: origin/main into work"), Some(Some("main".into())));
        assert_eq!(merge_from("Merge pull request #3 from a/b"), Some(None));
        assert_eq!(merge_from("Teach the pane"), None);
        assert_eq!(quote_of("ana", "one\n\ntwo\nthree\nfour"), "@ana\n\n> one\n> two\n> three\n\n");
    }
}
