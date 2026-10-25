// SPDX-License-Identifier: AGPL-3.0-only
//! The sentences of a joined computer that still holds what wsp installed, and of one forgotten. Port of the functions of packages/protocol/src/wire/place-link.ts that wsp-wire left to this crate, at
//! a3b870a6e; that file's shapes are wsp-wire's. See spec/wire_place_link.md.

/// What a remove says about a place that was not linked when it ran: the records here are gone and the daemon on
/// that computer is not, since nothing could reach it to sweep.
// place-link.ts:253
// export const placeStillInstalledLine = (name: string): string => `${name} is off this host, but the daemon on it is still installed; run ${PLACE_LEAVE_LINE} on that computer when it is back`;
pub fn place_still_installed_line(name: &str) -> String {
    format!("{name} is off this host, but the daemon on it is still installed; run wsp leave on that computer when it is back")
}

/// What a forget says about a computer whose forks and projects went here with no road to it: nothing was done
/// there, and the one line that clears what wsp may have left on it.
// place-link.ts:257
// export const placeForgottenLine = (name: string, forks: boolean): string =>
//   `${name} is forgotten here and nothing was done on it; whatever of wsp's is still there${forks ? ", the copies its forks ran in among it," : ""} comes off with ${PLACE_LEAVE_LINE} run on that computer`;
pub fn place_forgotten_line(name: &str, forks: bool) -> String {
    let copies = if forks { ", the copies its forks ran in among it," } else { "" };
    format!("{name} is forgotten here and nothing was done on it; whatever of wsp's is still there{copies} comes off with wsp leave run on that computer")
}

/// The refusal a second join on one computer gets: a place file is the one wsp this computer belongs to.
// place-link.ts:481
// export const ALREADY_JOINED_LINE = `this computer is already a place in a wsp; ${PLACE_LEAVE_LINE} first`;
/// `this computer is already a place in a wsp; ${PLACE_LEAVE_LINE} first`, written out.
pub const ALREADY_JOINED_LINE: &str = "this computer is already a place in a wsp; wsp leave first";
