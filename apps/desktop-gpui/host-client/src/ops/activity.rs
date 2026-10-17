// SPDX-License-Identifier: AGPL-3.0-only
use serde::Deserialize;

use crate::protocol;

ops! {
    "activity.end" => ActivityEndRequest => protocol::ActivityEnded,
    "appProcs.watch" => AppProcsWatchRequest => AppProcs,
    "appProcs.unwatch" => AppProcsUnwatchRequest => crate::Done,
}

#[derive(Debug, Deserialize)]
pub struct AppProcs {
    pub procs: protocol::AppProcList,
}
