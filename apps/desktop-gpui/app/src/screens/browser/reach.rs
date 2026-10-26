// SPDX-License-Identifier: AGPL-3.0-only
//! The preview route for a port of a workspace off this computer (`browser/reach.ts`), asked again before it expires,
//! and what one fetch of it answered (`browser/refusal.ts`), which names the host check that refused the page.

use std::time::Duration;

use gpui_kit::Task;

use super::refusal::{explain_refusal, Refusal};

/// `REACH_REFRESH_WITH_MS_LEFT`: the runtime hands its cached route back while more than ten minutes remain, so asking
/// with nine left is what makes it remint.
pub const REFRESH_WITH_LEFT: Duration = Duration::from_secs(9 * 60);
/// `REACH_REASK_FLOOR_MS`: the host's clock and this one's differ, so never sooner than this.
pub const FLOOR: Duration = Duration::from_secs(30);
/// `ANSWER_POLL_MS`: a port off the list is fetched again this often while it does not answer.
pub const ANSWER_POLL: Duration = Duration::from_secs(2);

/// How long to wait before asking for the route again, with `left` of its life to go: at nine minutes left, or
/// halfway for a short one, never under the floor.
pub fn next_ask(left: Duration) -> Duration {
    left.saturating_sub(REFRESH_WITH_LEFT.min(left / 2)).max(FLOOR)
}

/// `PortReach`: the route while it is being minted, once it is, or why it could not be.
#[derive(Clone, Debug, PartialEq)]
pub enum Reach {
    Minting,
    Ready(String),
    Failed(String),
}

/// One port's route and its last probe, kept while a tab shows that port.
#[derive(Default)]
pub struct Route {
    pub reach: Option<Reach>,
    pub refusal: Option<Refusal>,
    /// Whether the port answered its last fetch: None until the first.
    pub answering: Option<bool>,
    /// The load the first 401 reminted for, so a second 401 on it is said.
    pub reminted_for: Option<u64>,
    /// The load the last probe was for.
    pub probed_for: Option<u64>,
    pub(super) ask: Option<Task<()>>,
    pub(super) probe: Option<Task<()>>,
}

/// The route's host, as the refusal names it.
pub fn host_of(route: &str) -> String {
    ::url::Url::parse(route).ok().and_then(|u| u.host_str().map(str::to_owned)).unwrap_or_default()
}

/// What one fetch answered, said for a person; a first 401 of a load is a remint, not a sentence.
pub fn read_probe(status: i64, body: &str, port: u16, route: &str) -> Option<Refusal> {
    explain_refusal(status.clamp(0, i64::from(u16::MAX)) as u16, body, port, &host_of(route))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_hour_long_route_is_asked_again_with_nine_minutes_left() {
        assert_eq!(next_ask(Duration::from_secs(3600)), Duration::from_secs(3600 - 9 * 60));
    }

    #[test]
    fn a_short_route_is_asked_again_halfway() {
        assert_eq!(next_ask(Duration::from_secs(600)), Duration::from_secs(300));
    }

    #[test]
    fn a_route_already_due_waits_the_floor() {
        assert_eq!(next_ask(Duration::ZERO), FLOOR);
        assert_eq!(next_ask(Duration::from_secs(40)), FLOOR);
    }
}
