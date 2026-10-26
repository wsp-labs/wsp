// SPDX-License-Identifier: AGPL-3.0-only
use serde::Deserialize;

use crate::protocol;

ops! {
    "activity.end" => ActivityEndRequest => protocol::ActivityEnded,
    "appProcs.watch" => AppProcsWatchRequest => AppProcs,
    "appProcs.unwatch" => AppProcsUnwatchRequest => crate::Done,
    "appProcs.offer" => AppProcsOfferRequest => Offered,
    "appProcs.set" => AppProcsSetRequest => crate::Done,
}

#[derive(Debug, Deserialize)]
pub struct AppProcs {
    pub procs: protocol::AppProcList,
}

/// Whether a page watches the processes the app offered; the host pushes app.procs.wanted each time that turns.
#[derive(Debug, Deserialize)]
pub struct Offered {
    pub wanted: bool,
}
