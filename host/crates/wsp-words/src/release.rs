// SPDX-License-Identifier: AGPL-3.0-only
//! The newest release as the host last read it off GitHub, and the words a row says about an update. Port of the functions of packages/protocol/src/release.ts at
//! the cut (383708c78); its shapes and flat constants are wsp_wire::types::release. See spec/release.md.

use std::cmp::Ordering;

use wsp_wire::types::release::{ReleaseState, ReleaseView};

use crate::semver::compare_versions;

// One published release: its number, the tag that carries it, the page that holds its notes and downloads.
// release.ts:9
// in wsp_wire::types::release: ReleaseLatest

// release.ts:10
// in wsp_wire::types::release: ReleaseLatest

// `checking` while an ask is out or before any was made, `read` where the last ask was answered, `unreached` where
// it failed, `off` under the switch.
// release.ts:14
// in wsp_wire::types::release: ReleaseState

// release.ts:15
// in wsp_wire::types::release: ReleaseState

// How this host came up, which decides whether a restart brings it back: one of the three roads the lock records.
// release.ts:18
// in wsp_wire::types::release: HostShape

// release.ts:19
// in wsp_wire::types::release: HostShape

// `latest` stands whenever the host ever read one, whatever the last ask did, and is absent under `off`, so a person
// who turned checks off is offered nothing off a stale number. `checkedAt` is the last answer, `triedAt` the last ask.
// `installed` is the version the files this host was started from carry now, present only once it differs from the
// running one, which is what a reinstall under a running host looks like. `update` is the line that moves this host
// onto `latest` on the road it was installed by, present only while `latest` is above it. `restartRefusal` is the
// sentence host.restart answers where it would not bring this host back, in the words of the road it came up on, and
// absent where it does. `shape` is absent on a host none of the three roads brought up.
// release.ts:28
// in wsp_wire::types::release: ReleaseView

// release.ts:38
// in wsp_wire::types::release: ReleaseView

// The host's reading changed; every socket gets the whole view.
// release.ts:41
// in wsp_wire::types::release: ReleaseChangedEvent

// release.ts:42
// in wsp_wire::types::release: ReleaseChangedEvent

/// Whether the newest release is above any of the versions that run, the app's and the host's: a build ahead of it,
/// a prerelease or a checkout's, reads level.
// release.ts:46
// export const releaseAbove = (view: Pick<ReleaseView, "latest">, ...running: string[]): boolean => view.latest !== undefined && running.some(version => compareVersions(view.latest!.version, version) > 0);
pub fn release_above(view: &ReleaseView, running: &[impl AsRef<str>]) -> bool {
    view.latest
        .as_ref()
        .is_some_and(|latest| running.iter().any(|version| compare_versions(&latest.version, version.as_ref()) == Ordering::Greater))
}

/// The one word a reading says wherever it is shown: the number whenever one was read, else the state.
// release.ts:49
// export const releaseWord = (view: Pick<ReleaseView, "state" | "latest">): string => view.latest?.version ?? view.state;
pub fn release_word(view: &ReleaseView) -> String {
    match &view.latest {
        Some(latest) => latest.version.clone(),
        None => match view.state {
            ReleaseState::Checking => "checking",
            ReleaseState::Read => "read",
            ReleaseState::Unreached => "unreached",
            ReleaseState::Off => "off",
        }
        .to_string(),
    }
}

// release.ts:52
// in wsp_wire::types::release: UP_RESTART_LINE

// release.ts:55
// in wsp_wire::types::release: HOST_NO_RESTART_LINE

// release.ts:58
// in wsp_wire::types::release: HOST_RESTART_TICKET_REFUSAL
