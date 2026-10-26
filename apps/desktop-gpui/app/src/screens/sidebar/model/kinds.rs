// SPDX-License-Identifier: AGPL-3.0-only
//! Each thread's status kind and the slot, word and times it draws.

use std::borrow::Cow;

use super::{Part, Shape, Status, SubState, Subagent, SubagentState, Thread, Tile};

/// The registry of status kinds, in the order a thread is read against them; the first that matches wins.
/// `Finished` is never read off a thread: a lead's Finished fold gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    NeedsYou,
    Waiting,
    Starting,
    Working,
    Limited,
    Resuming,
    Failed,
    Released,
    Expired,
    Stopped,
    NotPushed,
    Done,
    Resting,
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Input,
    Working,
    Failed,
    Done,
    /// The web's `limited` token, inked in warning-foreground.
    Warning,
}

/// What the one status slot draws. A tile draws Working's crab at its title's end rather than in the slot
/// (ThreadTile.tsx:427,452); Starting's crab stands in the slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub glyph: Option<&'static str>,
    pub crab: bool,
    pub tone: Option<Tone>,
    pub text: Option<String>,
}

pub fn kind_of(t: &Thread, now_ms: f64) -> Kind {
    let sub = t.sub.as_ref().map(|s| s.state);
    let failed = t.status == Status::Failed;
    if t.asks {
        Kind::NeedsYou
    } else if t.running() && (t.capped || sub == Some(SubState::Waiting)) {
        Kind::Waiting
    } else if sub == Some(SubState::Starting) && !failed {
        Kind::Starting
    } else if t.running() {
        Kind::Working
    } else if failed && t.limit && t.resume_at.is_none() {
        Kind::Limited
    } else if failed && t.limit {
        Kind::Resuming
    } else if failed {
        Kind::Failed
    } else if sub == Some(SubState::Released) && matches!(t.status, Status::Completed | Status::Interrupted) && !t.unread {
        Kind::Released
    } else if t.disk_kept_until.is_some_and(|keep| keep <= now_ms) && !t.unread {
        Kind::Expired
    } else if t.status == Status::Interrupted {
        Kind::Stopped
    } else if t.held() {
        Kind::NotPushed
    } else if t.unread {
        Kind::Done
    } else {
        Kind::Resting
    }
}

/// A subagent's kind, read as a thread's would be off its own state; a settled one keeps its age alone.
pub fn subagent_kind(s: &Subagent, part: Part) -> Kind {
    match (part, s.state) {
        (Part::Settled, _) => Kind::Resting,
        (_, SubagentState::Running) => Kind::Working,
        (_, SubagentState::Failed) => Kind::Failed,
        (_, SubagentState::Stopped) => Kind::Stopped,
        (Part::Finished, SubagentState::Done) => Kind::Finished,
        (Part::Live, SubagentState::Done) => Kind::Resting,
    }
}

/// The kind a drawn tile's slot shows: a settled row or a snoozed root rests, a finished row read already ends in
/// Finished's muted check.
pub fn tile_kind(t: &Thread, tile: &Tile, now_ms: f64) -> Kind {
    match tile.shape {
        Shape::Settled => Kind::Resting,
        _ if tile.snoozed_working.is_some() => Kind::Resting,
        Shape::Finished => match kind_of(t, now_ms) {
            Kind::Resting if t.status == Status::Completed => Kind::Finished,
            kind => kind,
        },
        Shape::Full => kind_of(t, now_ms),
    }
}

/// Whether a full tile's title takes the muted ink for this kind, unless it is the one open (ThreadTile.tsx:361).
/// Slim rows always draw it muted.
pub fn recedes(kind: Kind) -> bool {
    matches!(kind, Kind::Resting | Kind::Working | Kind::Resuming | Kind::Stopped | Kind::Released | Kind::Expired)
}

/// The slot for a kind. `age_at` is the moment a resting row's age counts from, as `Tile::age_at` gives it.
pub fn slot(t: &Thread, kind: Kind, now_ms: f64, age_at: Option<f64>) -> Slot {
    let (glyph, tone) = kind_slot(kind);
    let text = match kind {
        Kind::Resting => age_at.map(|at| resting_age(at, now_ms)),
        Kind::Working => Some(running_time(now_ms - t.started_at.unwrap_or(now_ms))),
        _ => None,
    };
    Slot { glyph, crab: matches!(kind, Kind::Starting | Kind::Working), tone, text }
}

/// A kind's slot glyph and tone.
pub fn kind_slot(kind: Kind) -> (Option<&'static str>, Option<Tone>) {
    match kind {
        Kind::NeedsYou => (Some("message-circle-question"), Some(Tone::Input)),
        Kind::Waiting => (Some("hourglass"), None),
        Kind::Starting | Kind::Resting => (None, (kind == Kind::Starting).then_some(Tone::Working)),
        Kind::Working => (None, Some(Tone::Working)),
        Kind::Limited => (Some("gauge"), Some(Tone::Warning)),
        Kind::Resuming => (Some("hourglass"), None),
        Kind::Failed => (Some("circle-alert"), Some(Tone::Failed)),
        Kind::Released => (Some("cloud-off"), None),
        Kind::Expired => (Some("calendar-x-2"), None),
        Kind::Stopped => (Some("circle-stop"), None),
        Kind::NotPushed => (Some("upload"), Some(Tone::Input)),
        Kind::Done => (Some("circle-check"), Some(Tone::Done)),
        Kind::Finished => (Some("circle-check"), None),
    }
}

/// A drawn tile's slot: a snoozed root's count of working threads, else "Refused" on a full tile at rest or failed
/// whose agent cannot start there (its reason is the slot's tooltip), else its kind's.
pub fn tile_slot(t: &Thread, tile: &Tile, now_ms: f64) -> Slot {
    let kind = tile_kind(t, tile, now_ms);
    if let Some(working) = tile.snoozed_working {
        return Slot { glyph: Some("alarm-clock"), crab: false, tone: None, text: Some(format!("{working} working")) };
    }
    if tile.shape == Shape::Full && t.setup_refusal.is_some() && matches!(kind, Kind::Resting | Kind::Failed) {
        return Slot { glyph: None, crab: false, tone: None, text: Some("Refused".to_owned()) };
    }
    slot(t, kind, now_ms, tile.age_at)
}

/// The slot's word, for its tooltip and its label; Resting has none. `utc_offset_ms` places a reset days away on
/// the reader's weekday.
pub fn word(t: &Thread, kind: Kind, now_ms: f64, utc_offset_ms: f64) -> Cow<'static, str> {
    match kind {
        Kind::Resuming => Cow::Owned(resets_word(t.resume_at.unwrap_or(0.0), now_ms, utc_offset_ms)),
        _ => Cow::Borrowed(kind_word(kind)),
    }
}

/// A kind's slot word; Resuming's reads its reset, which `word` says.
pub fn kind_word(kind: Kind) -> &'static str {
    match kind {
        Kind::NeedsYou => "Needs you",
        Kind::Waiting => "Waiting",
        Kind::Starting => "Starting",
        Kind::Working => "Working",
        Kind::Limited => "At limit",
        Kind::Resuming => "",
        Kind::Failed => "Failed",
        Kind::Released => "Released",
        Kind::Expired => "Expired",
        Kind::Stopped => "Stopped",
        Kind::NotPushed => "Not pushed",
        Kind::Done | Kind::Finished => "Done",
        Kind::Resting => "",
    }
}

/// A turn's running time as the slot shows it: seconds in its first minute, then minutes: "42s", "17m", "1h 5m".
pub fn running_time(ms: f64) -> String {
    let ms = if ms.is_finite() && ms > 0.0 { ms } else { 0.0 };
    if ms < 60_000.0 {
        return format!("{}s", (ms / 1_000.0).floor());
    }
    let minutes = (ms / 60_000.0).floor() as u64;
    let hours = minutes / 60;
    if minutes < 60 {
        format!("{minutes}m")
    } else if hours < 24 {
        format!("{hours}h {}m", minutes % 60)
    } else {
        format!("{}d {}h", hours / 24, hours % 24)
    }
}

/// How long ago a moment was, as a resting row says it: "now", "5m", "3h", "2d".
pub fn resting_age(at_ms: f64, now_ms: f64) -> String {
    let seconds = ((now_ms - at_ms) / 1_000.0).floor();
    if seconds < 60.0 {
        return "now".to_owned();
    }
    let minutes = (seconds / 60.0).floor();
    let hours = (minutes / 60.0).floor();
    if minutes < 60.0 {
        format!("{minutes}m")
    } else if hours < 24.0 {
        format!("{hours}h")
    } else {
        format!("{}d", (hours / 24.0).floor())
    }
}

/// When a usage limit resets, as a slot words it (usage.ts resetsWord).
pub fn resets_word(resets_at: f64, now_ms: f64, utc_offset_ms: f64) -> String {
    let left = resets_at - now_ms;
    if left <= 0.0 {
        return "reset".to_owned();
    }
    if left < 3_600_000.0 {
        return format!("resets in {} min", (left / 60_000.0).round().max(1.0));
    }
    if left < 86_400_000.0 {
        return format!("resets in {} h", (left / 3_600_000.0).floor());
    }
    const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    let day = ((resets_at + utc_offset_ms) / 86_400_000.0).floor().rem_euclid(7.0) as usize;
    format!("resets on {}", DAYS[day])
}
