// SPDX-License-Identifier: AGPL-3.0-only
//! A line and a host of two releases. The host names its release in the answer that lets a socket in, and a line of
//! another release refuses there in the sentence the node command line says, filled from the record: both releases,
//! and what to run on the older end.

use std::cmp::Ordering;

use crate::failure::Failure;
use crate::record::{self, fill};

/// Which host the sentence names: the one serving this computer's state file, or one by its alias or its address,
/// which `here` says is on this computer all the same, as a turn's launch pair is.
pub enum ReleaseHost {
    Here { state: String },
    At { at: String, here: bool },
}

/// Which releases of a host a line goes on with past the answer to its token: its own alone, as the command line and
/// the tool server hold it, or its own and any newer, as the desktop app attaches to a newer host
/// (`desktop:host-lifecycle.ts`). An older host is refused here, the one serving this state file in today's desktop
/// app's words for a host of another release it keeps serving; today's app offers to replace one first. A refusal of
/// the lock beside the state is said in the words of the caller it names.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseRule {
    #[default]
    Own,
    OwnOrNewer,
}

/// This binary's own release, the record's.
pub fn own() -> String {
    record::server().version
}

/// The refusal for a host that answered `theirs` and named `road` beside it, or nothing where `rule` takes it.
pub fn refusal(theirs: &str, road: Option<&str>, host: &ReleaseHost, rule: ReleaseRule) -> Option<Failure> {
    let mine = own();
    if theirs == mine || (rule == ReleaseRule::OwnOrNewer && order(&mine, theirs) == Ordering::Less) {
        return None;
    }
    if let (ReleaseRule::OwnOrNewer, ReleaseHost::Here { state }) = (rule, host) {
        // `KeptOtherRelease` and `replacePrompt` in desktop:host-lifecycle.ts.
        return Some(Failure::usage(format!("wsp {theirs} keeps serving {state}: this app opens only on wsp {mine}")));
    }
    let words = &record::words().release;
    let named = match host {
        ReleaseHost::Here { state } => fill(&words.host_here, &[("state", state)]),
        ReleaseHost::At { at, .. } => fill(&words.host_at, &[("where", at)]),
    };
    let said = fill(&words.said, &[("mine", &mine), ("host", &named), ("theirs", theirs)]);
    let fix = if order(&mine, theirs) == Ordering::Less {
        let exe = std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        let line = words.roads.get(road_of(&exe)).map(|l| fill(l, &[("version", theirs)])).unwrap_or_default();
        fill(&words.update_here, &[("line", &line), ("theirs", theirs)])
    } else if matches!(host, ReleaseHost::At { here: false, .. }) {
        fill(&words.update_there, &[("mine", &mine)])
    } else {
        // What brings an older host here level, off the road it names, as `releaseHereFix` in
        // packages/host/src/verbs/client.ts reads it.
        match road {
            Some("app") => fill(&words.reopen_app, &[("mine", &mine)]),
            Some("up") => words.restart_up.clone(),
            Some("init") => words.init_finish.clone(),
            _ => words.restart.clone(),
        }
    };
    Some(Failure::usage(format!("{said} {fix}")))
}

/// The road this binary was installed by, off its own path, as `roadOf` in packages/host/src/daemon-fix.ts reads the
/// node line's: through a node_modules folder is an npm install, inside an app bundle moves with the app, anything
/// else is a checkout. The record's roadSamples hold the two readings to one answer.
fn road_of(path: &str) -> &'static str {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.contains(&"node_modules") {
        "npm"
    } else if parts.iter().any(|p| p.ends_with(".app")) {
        "app"
    } else {
        "checkout"
    }
}

/// Semver order as `compareVersions` in packages/protocol/src/semver.mjs reads it: the dotted core, a missing part
/// read as 0, then a release above its own prereleases; build metadata carries none. The record's orderSamples hold
/// the two to one answer.
pub fn order(a: &str, b: &str) -> Ordering {
    let parts = |v: &str| -> (Vec<String>, Option<Vec<String>>) {
        let rest = v.split('+').next().unwrap_or("");
        match rest.split_once('-') {
            Some((core, pre)) => (core.split('.').map(str::to_owned).collect(), Some(pre.split('.').map(str::to_owned).collect())),
            None => (rest.split('.').map(str::to_owned).collect(), None),
        }
    };
    let ident = |a: &str, b: &str| -> Ordering {
        let num = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
        match (num(a), num(b)) {
            (true, true) => a.parse::<u128>().unwrap_or(0).cmp(&b.parse::<u128>().unwrap_or(0)),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => a.cmp(b),
        }
    };
    let ((lc, lp), (rc, rp)) = (parts(a), parts(b));
    for i in 0..lc.len().max(rc.len()) {
        let o = ident(lc.get(i).map_or("0", String::as_str), rc.get(i).map_or("0", String::as_str));
        if o != Ordering::Equal {
            return o;
        }
    }
    match (lp, rp) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(l), Some(r)) => {
            for i in 0..l.len().max(r.len()) {
                match (l.get(i), r.get(i)) {
                    (None, _) => return Ordering::Less,
                    (_, None) => return Ordering::Greater,
                    (Some(x), Some(y)) => {
                        let o = ident(x, y);
                        if o != Ordering::Equal {
                            return o;
                        }
                    }
                }
            }
            Ordering::Equal
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_road_off_a_path_as_the_node_line_does() {
        for (path, road) in &record::words().release.road_samples {
            assert_eq!(road_of(path), road, "{path}");
        }
    }

    #[test]
    fn orders_releases_as_the_node_line_does() {
        for (a, b, sign) in &record::words().release.order_samples {
            assert_eq!(order(a, b) as i32, sign.signum() as i32, "{a} against {b}");
        }
    }

    #[test]
    fn says_what_to_run_on_the_older_end() {
        let mine = record::server().version;
        let words = &record::words().release;
        let here = ReleaseHost::Here { state: "/s/state.json".into() };
        assert!(refusal(&mine, Some("service"), &here, ReleaseRule::Own).is_none());
        let fix = |road: Option<&str>, host: &ReleaseHost| {
            let failure = refusal("0.0.1", road, host, ReleaseRule::Own).unwrap();
            assert_eq!(failure.kind.as_deref(), Some("usage"));
            failure.message
        };
        let said = format!("this line runs wsp {mine} and the host serving /s/state.json runs wsp 0.0.1:");
        assert_eq!(fix(Some("service"), &here), format!("{said} {}", words.restart));
        assert_eq!(fix(Some("verb"), &here), format!("{said} {}", words.restart));
        assert_eq!(fix(Some("up"), &here), format!("{said} {}", words.restart_up));
        assert_eq!(fix(Some("init"), &here), format!("{said} {}", words.init_finish));
        assert_eq!(fix(Some("app"), &here), format!("{said} {}", fill(&words.reopen_app, &[("mine", &mine)])));
        let pair = ReleaseHost::At { at: "http://127.0.0.1:4000".into(), here: true };
        assert!(fix(Some("service"), &pair).ends_with(&words.restart));
        let there = ReleaseHost::At { at: "attic".into(), here: false };
        assert!(fix(Some("service"), &there).ends_with(&fill(&words.update_there, &[("mine", &mine)])));
        let newer = refusal("999.0.0", Some("service"), &there, ReleaseRule::Own).unwrap();
        assert!(newer.message.ends_with("on this computer brings this line to 999.0.0."), "{}", newer.message);
    }

    /// Catches: the desktop's rule refusing a newer host or taking an older one; an older host here refused in the
    /// command line's "this line runs" rather than today's app's words, or one elsewhere in anything but the line's,
    /// which is what today's app says of an account host; and the rule a line gets by default being anything but its
    /// own release.
    #[test]
    fn the_desktop_rule_takes_a_newer_host_and_refuses_an_older_one_in_todays_app_words() {
        assert_eq!(ReleaseRule::default(), ReleaseRule::Own);
        let mine = own();
        let here = ReleaseHost::Here { state: "/s/state.json".into() };
        let newer = |v: &str| order(&mine, v) == Ordering::Less;
        for theirs in [mine.as_str(), "999.0.0", "0.0.1"] {
            assert_eq!(
                refusal(theirs, Some("service"), &here, ReleaseRule::OwnOrNewer).is_none(),
                theirs == mine || newer(theirs),
                "{theirs}"
            );
        }
        assert!(refusal("999.0.0", Some("service"), &here, ReleaseRule::Own).is_some(), "the line's own rule takes its own release alone");
        let core = mine.split(['-', '+']).next().unwrap().to_owned();
        let (pre_of_mine, pre_of_next) = (format!("{core}-rc.1"), format!("{}-rc.1", next_minor(&core)));
        assert!(refusal(&pre_of_next, Some("service"), &here, ReleaseRule::OwnOrNewer).is_none(), "{pre_of_next} is newer than {mine}");
        let there = ReleaseHost::At { at: "attic".into(), here: false };
        for older in ["0.0.1", pre_of_mine.as_str()] {
            for road in [Some("service"), Some("app"), Some("up"), None] {
                let app = refusal(older, road, &here, ReleaseRule::OwnOrNewer).unwrap();
                assert_eq!(app.message, format!("wsp {older} keeps serving /s/state.json: this app opens only on wsp {mine}"), "{road:?}");
                assert_eq!(app.kind.as_deref(), Some("usage"));
                let line = refusal(older, road, &there, ReleaseRule::Own).unwrap();
                assert_eq!(refusal(older, road, &there, ReleaseRule::OwnOrNewer), Some(line), "{older} on {road:?}");
            }
        }
    }

    fn next_minor(core: &str) -> String {
        let parts: Vec<u64> = core.split('.').map(|p| p.parse().unwrap()).collect();
        format!("{}.{}.0", parts[0], parts[1] + 1)
    }
}
