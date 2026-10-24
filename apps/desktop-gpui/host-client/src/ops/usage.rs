// SPDX-License-Identifier: AGPL-3.0-only
use serde::Deserialize;

use crate::protocol;

ops! {
    "usage.used" => UsageUsedRequest => Used,
    "usage.accounts" => UsageAccountsRequest => protocol::AccountsAnswer,
    "usage.reset" => UsageResetRequest => protocol::ResetAnswer,
    "places.readings" => PlacesReadingsRequest => protocol::ReadingsAnswer,
    "cost.history" => CostHistoryRequest => CostHistory,
}

#[derive(Debug, Deserialize)]
pub struct Used {
    pub used: protocol::UsedAnswer,
}

#[derive(Debug, Deserialize)]
pub struct CostHistory {
    pub points: Vec<protocol::WorkspaceCostEvent>,
}
