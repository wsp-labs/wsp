// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/sources.ts at 65f6b89c7: the sources a slate reads, each with its declared paths and their shapes.
use std::sync::LazyLock;

use indexmap::IndexMap;
use wsp_wire::types::pull_request::CHECK_STATE_WORDS;
use wsp_wire::types::usage::LIMIT_KINDS;

use super::expr::{SlateScopeProblem, SlateType, SlateTypeName};
use super::problems::slate_nearest;
use super::PathSeg;

/// A declared path's shape: a scalar, an enum, a list of a shape, a record of fields, or a record keyed by any name.
#[derive(Clone, Debug, PartialEq)]
pub enum SlateShape {
    Number,
    String,
    Boolean,
    Time,
    Any,
    Enum(Vec<&'static str>),
    List(Box<SlateShape>),
    Fields(IndexMap<&'static str, SlateShape>),
    /// mcp.<server>: a record keyed by any name, each of this shape.
    Keyed(Box<SlateShape>),
}

/// One source in the registry (SLATE_SOURCES): its level, what it says, how it updates and what it costs.
#[derive(Clone, Debug, PartialEq)]
pub struct SlateSourceModule {
    pub name: &'static str,
    pub level: &'static str,
    pub purpose: &'static str,
    pub update: &'static str,
    pub cost: &'static str,
    pub scope: &'static str,
    pub shape: SlateShape,
    /// Paths the registry marks as time series, which bars warns on.
    pub series: Vec<&'static str>,
    /// The fields the catalog's index names for a core source; its own entry has the rest.
    pub indexed: Option<Vec<&'static str>>,
    /// Notes per path for the catalog: which agents report it, when it is null.
    pub notes: Vec<(&'static str, &'static str)>,
    pub example: &'static str,
}

fn rec<const N: usize>(entries: [(&'static str, SlateShape); N]) -> SlateShape {
    SlateShape::Fields(entries.into_iter().collect())
}

fn list(of: SlateShape) -> SlateShape {
    SlateShape::List(Box::new(of))
}

fn one_of(values: &[&'static str]) -> SlateShape {
    SlateShape::Enum(values.to_vec())
}

fn points() -> SlateShape {
    list(rec([("x", SlateShape::Number), ("y", SlateShape::Number)]))
}

/// The head is name, level, purpose, update, cost and scope, in that order.
fn module(head: [&'static str; 6], shape: SlateShape, example: &'static str) -> SlateSourceModule {
    let [name, level, purpose, update, cost, scope] = head;
    SlateSourceModule { name, level, purpose, update, cost, scope, shape, series: Vec::new(), indexed: None, notes: Vec::new(), example }
}

/// The sources' registry, in the order the TS lists them.
pub static SLATE_SOURCES: LazyLock<IndexMap<&'static str, SlateSourceModule>> = LazyLock::new(|| {
    let check_states: Vec<&'static str> = CHECK_STATE_WORDS.iter().map(|(state, _)| *state).collect();
    let mut sources = IndexMap::new();

    let mut thread = module(
        [
            "thread",
            "core",
            "This thread: status, last turn, context, cost, tokens, changes, plan.",
            "push",
            "none beyond today",
            "the slate's own thread",
        ],
        rec([
            ("id", SlateShape::String),
            ("title", SlateShape::String),
            ("agent", SlateShape::String),
            ("model", SlateShape::String),
            ("effort", SlateShape::String),
            ("status", one_of(&["starting", "working", "needs-you", "failed", "done", "resting"])),
            ("access", SlateShape::String),
            ("computer", SlateShape::String),
            ("project", SlateShape::String),
            ("folder", SlateShape::String),
            ("turns", SlateShape::Number),
            (
                "lastTurn",
                rec([
                    ("status", SlateShape::String),
                    ("durationMs", SlateShape::Number),
                    ("waitedMs", SlateShape::Number),
                    ("startedAt", SlateShape::Time),
                    ("endedAt", SlateShape::Time),
                    ("model", SlateShape::String),
                ]),
            ),
            ("cost", rec([("usd", SlateShape::Number)])),
            (
                "tokens",
                rec([
                    ("input", SlateShape::Number),
                    ("output", SlateShape::Number),
                    ("cached", SlateShape::Number),
                    ("cacheWrite", SlateShape::Number),
                    ("reasoning", SlateShape::Number),
                ]),
            ),
            (
                "context",
                rec([
                    ("used", SlateShape::Number),
                    ("window", SlateShape::Number),
                    ("free", SlateShape::Number),
                    ("percent", SlateShape::Number),
                ]),
            ),
            (
                "changes",
                rec([
                    (
                        "files",
                        list(rec([
                            ("path", SlateShape::String),
                            ("kind", SlateShape::String),
                            ("additions", SlateShape::Number),
                            ("deletions", SlateShape::Number),
                        ])),
                    ),
                    ("from", SlateShape::String),
                    ("to", SlateShape::String),
                    ("moved", list(SlateShape::Any)),
                    ("shared", SlateShape::Boolean),
                ]),
            ),
            (
                "plan",
                rec([
                    ("steps", list(rec([("text", SlateShape::String), ("state", one_of(&["pending", "working", "done"]))]))),
                    ("text", SlateShape::String),
                ]),
            ),
            ("waitingOn", SlateShape::String),
            (
                "subagents",
                list(rec([
                    ("id", SlateShape::String),
                    ("title", SlateShape::String),
                    ("state", SlateShape::String),
                    ("startedAt", SlateShape::Time),
                    ("endedAt", SlateShape::Time),
                ])),
            ),
            (
                "messages",
                rec([
                    ("recent", list(rec([("who", SlateShape::String), ("at", SlateShape::Time), ("text", SlateShape::String)]))),
                    ("last", SlateShape::String),
                ]),
            ),
        ]),
        "percent(thread.context.used / thread.context.window)",
    );
    thread.indexed = Some(
        [
            "id",
            "title",
            "status",
            "agent",
            "model",
            "turns",
            "lastTurn",
            "cost",
            "tokens",
            "context",
            "changes",
            "plan",
            "waitingOn",
            "subagents",
        ]
        .to_vec(),
    );
    thread.notes = vec![
        ("thread.context", "the latest turn that reports it; null on an agent that does not"),
        ("thread.changes.files", "the latest session.changes"),
    ];
    sources.insert("thread", thread);

    let tree = module(
        ["tree", "working", "The thread's root and every thread under it.", "push", "none beyond today", "the thread's tree"],
        rec([
            (
                "root",
                rec([
                    ("id", SlateShape::String),
                    ("title", SlateShape::String),
                    ("status", SlateShape::String),
                    ("agent", SlateShape::String),
                    ("computer", SlateShape::String),
                ]),
            ),
            (
                "parent",
                rec([
                    ("id", SlateShape::String),
                    ("title", SlateShape::String),
                    ("status", SlateShape::String),
                    ("agent", SlateShape::String),
                    ("computer", SlateShape::String),
                ]),
            ),
            (
                "children",
                list(rec([
                    ("id", SlateShape::String),
                    ("title", SlateShape::String),
                    ("status", SlateShape::String),
                    ("agent", SlateShape::String),
                    ("computer", SlateShape::String),
                    ("costUsd", SlateShape::Number),
                    ("startedAt", SlateShape::Time),
                    ("endedAt", SlateShape::Time),
                ])),
            ),
            (
                "all",
                list(rec([
                    ("id", SlateShape::String),
                    ("title", SlateShape::String),
                    ("status", SlateShape::String),
                    ("agent", SlateShape::String),
                    ("computer", SlateShape::String),
                    ("depth", SlateShape::Number),
                ])),
            ),
            (
                "counts",
                rec([
                    ("working", SlateShape::Number),
                    ("needsYou", SlateShape::Number),
                    ("done", SlateShape::Number),
                    ("failed", SlateShape::Number),
                    ("resting", SlateShape::Number),
                ]),
            ),
        ]),
        "tree.counts.working",
    );
    sources.insert("tree", tree);

    let mut usage = module(
        [
            "usage",
            "core",
            "The plan windows of the account the thread runs on.",
            "push",
            "one event per turn end",
            "the account the thread runs on",
        ],
        rec([
            (
                "account",
                rec([
                    ("key", SlateShape::String),
                    ("label", SlateShape::String),
                    ("agent", SlateShape::String),
                    ("plan", SlateShape::String),
                    ("address", SlateShape::String),
                    ("computers", list(SlateShape::String)),
                ]),
            ),
            ("windows", list(rec([("kind", one_of(LIMIT_KINDS)), ("usedPercent", SlateShape::Number), ("resetsAt", SlateShape::Time)]))),
            ("session", rec([("percent", SlateShape::Number), ("resetsAt", SlateShape::Time)])),
            ("week", rec([("percent", SlateShape::Number), ("resetsAt", SlateShape::Time)])),
            ("status", one_of(&["ok", "warning", "reached"])),
            ("note", SlateShape::String),
            ("burn", rec([("tokensPerMinute", SlateShape::Number), ("threads", SlateShape::Number)])),
            ("credits", rec([("count", SlateShape::Number), ("nextExpiresAt", SlateShape::Time)])),
            ("readAt", SlateShape::Time),
        ]),
        "pct(usage.week.percent)",
    );
    usage.indexed = Some(["account", "windows", "session", "week", "status", "note"].to_vec());
    usage.notes = vec![("usage.windows", "Claude Code and Codex report them; a keyed sign-in has none and usage.note says why")];
    sources.insert("usage", usage);

    let mut cost = module(
        [
            "cost",
            "core",
            "What the thread's computer costs: the rate now and what it has run up.",
            "push",
            "none beyond today",
            "the computer the thread runs on",
        ],
        rec([("rateUsdPerHour", SlateShape::Number), ("accruedUsd", SlateShape::Number)]),
        "usd(cost.accruedUsd)",
    );
    cost.indexed = Some(["rateUsdPerHour", "accruedUsd"].to_vec());
    cost.notes = vec![("cost.rateUsdPerHour", "0 on this computer, which usd() reads as free")];
    sources.insert("cost", cost);

    let mut time = module(
        ["time", "core", "The clock, so a countdown or an age moves on its own.", "tick", "one re-evaluation a tick", "none"],
        rec([("now", SlateShape::Number), ("today", SlateShape::String), ("zone", SlateShape::String)]),
        "until(usage.week.resetsAt)",
    );
    time.indexed = Some(["now", "today", "zone"].to_vec());
    time.notes = vec![("time.now", "ms epoch; ticks each second while a visible piece reads it")];
    sources.insert("time", time);

    let mut git = module(
        [
            "git",
            "core",
            "The thread's checkout: branch, head, counts against upstream.",
            "push",
            "none for the checkout; status entries poll 15 s while bound",
            "the thread's folder",
        ],
        rec([
            ("branch", SlateShape::String),
            ("head", SlateShape::String),
            ("ahead", SlateShape::Number),
            ("behind", SlateShape::Number),
            ("changed", SlateShape::Number),
            ("stashes", SlateShape::Number),
            ("editsUnread", SlateShape::Boolean),
            ("countsUnknown", SlateShape::Boolean),
            ("readAt", SlateShape::Time),
            (
                "status",
                rec([("entries", list(rec([("xy", SlateShape::String), ("path", SlateShape::String), ("origPath", SlateShape::String)])))]),
            ),
        ]),
        "git.branch",
    );
    git.indexed = Some(["branch", "head", "ahead", "behind", "changed"].to_vec());
    git.notes = vec![("git.branch", "null on a folder that is not a checkout")];
    sources.insert("git", git);

    let mut pr = module(
        [
            "pr",
            "core",
            "The pull request of the thread's branch and its checks.",
            "push",
            "one gh read every 3 min; every 30 s while a bound check is pending",
            "the thread's folder",
        ],
        rec([
            ("number", SlateShape::Number),
            ("url", SlateShape::String),
            ("state", one_of(&["open", "merged", "closed"])),
            ("draft", SlateShape::Boolean),
            ("base", SlateShape::String),
            ("branch", SlateShape::String),
            ("headOid", SlateShape::String),
            ("headSubject", SlateShape::String),
            ("mergeable", one_of(&["mergeable", "conflicting", "unknown"])),
            ("review", one_of(&["none", "approved", "changes_asked", "required"])),
            (
                "checks",
                list(rec([
                    ("name", SlateShape::String),
                    ("workflow", SlateShape::String),
                    ("state", SlateShape::Enum(check_states)),
                    ("link", SlateShape::String),
                    ("description", SlateShape::String),
                    ("startedAt", SlateShape::Time),
                    ("completedAt", SlateShape::Time),
                ])),
            ),
            ("additions", SlateShape::Number),
            ("deletions", SlateShape::Number),
            ("changedFiles", SlateShape::Number),
            ("commits", SlateShape::Number),
            ("behindBase", SlateShape::Number),
            ("author", SlateShape::String),
            ("readAt", SlateShape::Time),
            ("word", SlateShape::String),
            ("unread", SlateShape::String),
        ]),
        "pr.checks | where(item.state == 'fail') | count",
    );
    pr.indexed = Some(["number", "url", "state", "draft", "branch", "headSubject", "mergeable", "review", "checks", "word"].to_vec());
    pr.notes = vec![
        ("pr.number", "null while the branch has no pull request"),
        ("pr.headSubject", "the head commit's subject; there is no pr.title"),
    ];
    sources.insert("pr", pr);

    let mut machine = module(
        [
            "machine",
            "working",
            "The thread's computer: load, memory, disk and their last minute.",
            "subscription",
            "one sample a second while bound",
            "the thread's computer",
        ],
        rec([
            ("name", SlateShape::String),
            ("cpu", SlateShape::Number),
            ("load1", SlateShape::Number),
            ("mem", rec([("used", SlateShape::Number), ("total", SlateShape::Number)])),
            ("disk", rec([("used", SlateShape::Number), ("total", SlateShape::Number)])),
            ("at", SlateShape::Time),
            ("history", rec([("cpu", points()), ("load1", points()), ("mem", points())])),
        ]),
        "bytes(machine.mem.used)",
    );
    machine.series = vec!["machine.history.cpu", "machine.history.load1", "machine.history.mem"];
    sources.insert("machine", machine);

    sources.insert(
        "processes",
        module(
            [
                "processes",
                "working",
                "The thread's computer's processes.",
                "subscription",
                "one snapshot every 2 s while bound",
                "the thread's computer",
            ],
            rec([
                (
                    "list",
                    list(rec([
                        ("pid", SlateShape::Number),
                        ("name", SlateShape::String),
                        ("cpu", SlateShape::Number),
                        ("mem", SlateShape::Number),
                        ("cmd", SlateShape::String),
                    ])),
                ),
                ("total", SlateShape::Number),
                ("at", SlateShape::Time),
            ]),
            "processes.list | sortBy(item.cpu, 'desc') | take(5)",
        ),
    );

    sources.insert(
        "ports",
        module(
            ["ports", "working", "Ports open on the thread's computer.", "push", "none beyond today", "the thread's computer"],
            rec([("list", list(rec([("port", SlateShape::Number), ("process", SlateShape::String), ("url", SlateShape::String)])))]),
            "ports.list",
        ),
    );

    sources.insert(
        "notifications",
        module(
            [
                "notifications",
                "working",
                "The last 20 notices for this thread and its account.",
                "push",
                "none beyond today",
                "this thread and its account",
            ],
            rec([("recent", list(rec([("kind", SlateShape::String), ("text", SlateShape::String), ("at", SlateShape::Time)])))]),
            "notifications.recent | first",
        ),
    );

    sources.insert(
        "mcp",
        module(
            [
                "mcp",
                "working",
                "The thread's agent's MCP servers: consent state, tools, resources.",
                "push",
                "a listing on connect",
                "servers configured for the thread's agent",
            ],
            SlateShape::Keyed(Box::new(rec([
                ("state", one_of(&["unasked", "asked", "allowed", "refused", "down"])),
                (
                    "tools",
                    list(rec([
                        ("name", SlateShape::String),
                        ("title", SlateShape::String),
                        ("description", SlateShape::String),
                        ("inputSchema", SlateShape::Any),
                        ("annotations", SlateShape::Any),
                        ("ui", SlateShape::String),
                    ])),
                ),
                (
                    "resources",
                    list(rec([
                        ("uri", SlateShape::String),
                        ("name", SlateShape::String),
                        ("mimeType", SlateShape::String),
                        ("description", SlateShape::String),
                    ])),
                ),
            ]))),
            "mcp.linear.state",
        ),
    );

    sources
});

/// The names of the sources, in the registry's order.
pub static SLATE_SOURCE_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| SLATE_SOURCES.keys().copied().collect());

pub fn is_slate_source(name: &str) -> bool {
    SLATE_SOURCES.contains_key(name)
}

fn ty(t: SlateTypeName) -> SlateType {
    SlateType { t, of: None, fields: None }
}

/// A shape as a checker type.
pub fn slate_shape_type(shape: &SlateShape) -> SlateType {
    match shape {
        SlateShape::Number => ty(SlateTypeName::Number),
        SlateShape::Time | SlateShape::Any | SlateShape::Keyed(_) => ty(SlateTypeName::Any),
        SlateShape::String | SlateShape::Enum(_) => ty(SlateTypeName::String),
        SlateShape::Boolean => ty(SlateTypeName::Boolean),
        SlateShape::List(of) => SlateType { t: SlateTypeName::List, of: Some(Box::new(slate_shape_type(of))), fields: None },
        SlateShape::Fields(fields) => SlateType {
            t: SlateTypeName::Record,
            of: None,
            fields: Some(fields.iter().map(|(name, shape)| (name.to_string(), slate_shape_type(shape))).collect()),
        },
    }
}

fn segment_text(seg: &PathSeg) -> String {
    match seg {
        PathSeg::Key(key) => format!(".{key}"),
        PathSeg::Index(index) => format!("[{index}]"),
    }
}

/// A source path's type, or the problem naming the nearest declared path.
pub fn slate_source_type(head: &str, segs: &[PathSeg]) -> Result<SlateType, SlateScopeProblem> {
    let Some(source) = SLATE_SOURCES.get(head) else {
        let names: Vec<String> = SLATE_SOURCES.keys().map(|name| name.to_string()).collect();
        let fix = slate_nearest(head, &names);
        let message = match &fix {
            Some(fix) => format!("{head} is not a path; did you mean {fix}?"),
            None => format!("{head} is not a path; sources are {}, and the slate's own values are $name", names.join(", ")),
        };
        return Err(SlateScopeProblem { code: "X401", message, fix });
    };
    let mut shape = &source.shape;
    let mut walked = head.to_string();
    for (at, seg) in segs.iter().enumerate() {
        match seg {
            PathSeg::Index(index) => match shape {
                SlateShape::List(of) => {
                    shape = of;
                    walked.push_str(&format!("[{index}]"));
                }
                _ => return Ok(ty(SlateTypeName::Any)),
            },
            PathSeg::Key(key) => {
                if let SlateShape::Keyed(of) = shape {
                    shape = of;
                    walked.push_str(&format!(".{key}"));
                    continue;
                }
                let SlateShape::Fields(fields) = shape else {
                    return Ok(ty(SlateTypeName::Any));
                };
                let Some(next) = fields.get(key.as_str()) else {
                    let names: Vec<String> = fields.keys().map(|name| name.to_string()).collect();
                    let fix = slate_nearest(key, &names);
                    let rest: String = segs[at + 1..].iter().map(segment_text).collect();
                    let fixed = fix.map(|fix| format!("{walked}.{fix}{rest}"));
                    let message = match &fixed {
                        Some(fixed) => format!("{walked}.{key}{rest} is not a path. Did you mean {fixed}?"),
                        None => format!("{walked}.{key}{rest} is not a path; {walked} has {}", names.join(", ")),
                    };
                    return Err(SlateScopeProblem { code: "X401", message, fix: fixed });
                };
                shape = next;
                walked.push_str(&format!(".{key}"));
            }
        }
    }
    Ok(slate_shape_type(shape))
}

/// Whether a source path is a registered time series.
pub fn slate_is_series(path: &str) -> bool {
    SLATE_SOURCES.values().any(|source| source.series.contains(&path))
}

/// A shape as one short phrase, for the catalog and the checker's messages.
pub fn slate_shape_text(shape: &SlateShape) -> String {
    match shape {
        SlateShape::Number => "number".to_string(),
        SlateShape::String => "string".to_string(),
        SlateShape::Boolean => "boolean".to_string(),
        SlateShape::Time => "time".to_string(),
        SlateShape::Any => "any".to_string(),
        SlateShape::Enum(values) => values.join("|"),
        SlateShape::List(of) => format!("{}[]", slate_shape_text(of)),
        SlateShape::Keyed(of) => format!("<name>: {}", slate_shape_text(of)),
        SlateShape::Fields(fields) => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, shape)| match shape {
                    SlateShape::Number | SlateShape::String | SlateShape::Boolean | SlateShape::Time => name.to_string(),
                    _ => format!("{name}: {}", slate_shape_text(shape)),
                })
                .collect();
            format!("{{ {} }}", parts.join(", "))
        }
    }
}
