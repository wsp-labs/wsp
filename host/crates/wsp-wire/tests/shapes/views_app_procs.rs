// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of views/app-procs.ts against what zod answered for the same inputs. Generated.

use super::common::{cases, cases_tagged};
use wsp_wire::views::app_procs::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/views_app_procs.json");

/// 10 cases: what zod's parse of AppProc wrote, or the first issue it raised.
#[test]
fn app_proc_reads_and_writes_as_zod_does() {
    cases::<AppProc>(FIXTURES, "AppProc");
}

/// 8 cases: what zod's parse of AppProcKind wrote, or the first issue it raised.
#[test]
fn app_proc_kind_reads_and_writes_as_zod_does() {
    cases::<AppProcKind>(FIXTURES, "AppProcKind");
}

/// 2 cases: what zod's parse of AppProcList wrote, or the first issue it raised.
#[test]
fn app_proc_list_reads_and_writes_as_zod_does() {
    cases::<AppProcList>(FIXTURES, "AppProcList");
}

/// 4 cases: what zod's parse of AppProcsEvent wrote, or the first issue it raised.
#[test]
fn app_procs_event_reads_and_writes_as_zod_does() {
    cases_tagged::<AppProcsEvent>(FIXTURES, "AppProcsEvent", "type", AppProcsEvent::TYPE);
}

/// 4 cases: what zod's parse of AppProcsWantedEvent wrote, or the first issue it raised.
#[test]
fn app_procs_wanted_event_reads_and_writes_as_zod_does() {
    cases_tagged::<AppProcsWantedEvent>(FIXTURES, "AppProcsWantedEvent", "type", AppProcsWantedEvent::TYPE);
}
