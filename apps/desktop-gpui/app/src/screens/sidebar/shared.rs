// SPDX-License-Identifier: AGPL-3.0-only
//! The host's session rows held once for the whole app. The sidebar, the menu bar's feed and Activity each follow the
//! host on a road of their own; every row they read goes through [`share`], which hands back the copy already held for
//! that session when it reads the same, so the three keep one copy of each row and a list of pointers each.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use serde_json::{Map, Value};
use wsp_host_client::{Failure, HostClient};

use super::model::Row;

/// The size the table may reach before its gone rows are first swept.
const SWEEP_FLOOR: usize = 256;

struct Held {
    /// Every copy of a session's row some reader may hold: readers take pushes at their own pace, so for a moment two
    /// of them can hold two states of one row.
    rows: HashMap<String, Vec<Weak<Row>>>,
    /// The size at which the sessions no reader holds are swept next: twice what was held after the last sweep.
    sweep_at: usize,
}

static HELD: Mutex<Option<Held>> = Mutex::new(None);

/// The one copy of `row`: the one a reader already holds where it reads the same, else this one, held from now on.
pub fn share(row: Row) -> Arc<Row> {
    let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    let held = held.get_or_insert_with(|| Held { rows: HashMap::new(), sweep_at: SWEEP_FLOOR });
    if held.rows.len() >= held.sweep_at {
        held.rows.retain(|_, copies| copies.iter().any(|row| row.strong_count() > 0));
        held.sweep_at = (held.rows.len() * 2).max(SWEEP_FLOOR);
    }
    let copies = held.rows.entry(row.id.clone()).or_default();
    copies.retain(|copy| copy.strong_count() > 0);
    if let Some(kept) = copies.iter().filter_map(Weak::upgrade).find(|kept| **kept == row) {
        return kept;
    }
    let row = Arc::new(row);
    copies.push(Arc::downgrade(&row));
    row
}

/// `sessions.list`'s rows as the host sent them, each as the sidebar keeps it ([`Row::kept`]) and shared; a row with
/// no id or no workspace is left out.
pub fn rows_of(sent: Vec<Value>) -> Vec<Arc<Row>> {
    sent.into_iter().filter_map(Row::owned).map(|row| share(row.kept())).collect()
}

/// `sessions.list` asked with `params`, read straight off the host's words into the shared rows: the list is the
/// biggest thing the host sends, so it never stands as a typed copy and a JSON one at once.
pub async fn sessions(host: &HostClient, params: Map<String, Value>) -> Result<Vec<Arc<Row>>, Failure> {
    match host.call_raw("sessions.list", params).await?.remove("sessions") {
        Some(Value::Array(sent)) => Ok(rows_of(sent)),
        _ => Err(Failure::new("sessions.list answered no sessions")),
    }
}

/// The rows of one workspace whole, as the open thread's view reads them ([`Row::kept`] cuts a settled one): the
/// sidebar's alone, never shared, since no other reader holds a row whole.
pub async fn whole(host: &HostClient, workspace: &str) -> Result<Vec<Row>, Failure> {
    let params = Map::from_iter([("workspaceId".to_owned(), Value::from(workspace))]);
    match host.call_raw("sessions.list", params).await?.remove("sessions") {
        Some(Value::Array(sent)) => Ok(sent.into_iter().filter_map(Row::owned).collect()),
        _ => Err(Failure::new("sessions.list answered no sessions")),
    }
}

/// Whether `row` is the copy every reader shares.
pub fn holds(row: &Arc<Row>) -> bool {
    let held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    held.as_ref().and_then(|held| held.rows.get(&row.id)).is_some_and(|copies| copies.iter().any(|kept| kept.as_ptr() == Arc::as_ptr(row)))
}

/// How many sessions some reader holds a row of now.
pub fn held() -> usize {
    HELD.lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map_or(0, |held| held.rows.values().filter(|copies| copies.iter().any(|row| row.strong_count() > 0)).count())
}
