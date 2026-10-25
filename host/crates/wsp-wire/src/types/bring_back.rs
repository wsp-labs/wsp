// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/bring-back.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::pull_request::PullRequest;

/// TS: packages/protocol/src/bring-back.ts:11 at a3b870a6e.
/// The push a bring back made: the branch, its base and remote, how far it is ahead and how many files are uncommitted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPushReply {
    pub branch: String,
    pub base: String,
    pub remote: String,
    pub ahead: i64,
    pub uncommitted: i64,
    pub stat: Vec<String>,
}

impl Check for GitPushReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "branch" => <String as Check>::describe(rest),
                "base" => <String as Check>::describe(rest),
                "remote" => <String as Check>::describe(rest),
                "ahead" => <i64 as Check>::describe(rest),
                "uncommitted" => <i64 as Check>::describe(rest),
                "stat" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/bring-back.ts:22 at a3b870a6e.
/// The branch's pull request, and whether this call is what opened it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrReply {
    pub pr: PullRequest,
    pub created: bool,
}

impl Check for GitPrReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.pr.check().map_err(|e| e.under("pr"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "pr" => <PullRequest as Check>::describe(rest),
                "created" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/bring-back.ts:29 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BringBackResult {
    pub branch: String,
    pub base: String,
    pub ahead: i64,
    pub uncommitted: i64,
    pub stat: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pr: Option<PullRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub refused: Option<String>,
}

impl Check for BringBackResult {
    /// Nested checks: pr.
    fn check(&self) -> Result<(), SchemaError> {
        self.pr.check().map_err(|e| e.under("pr"))?;
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "branch" => <String as Check>::describe(rest),
                "base" => <String as Check>::describe(rest),
                "ahead" => <i64 as Check>::describe(rest),
                "uncommitted" => <i64 as Check>::describe(rest),
                "stat" => <Vec<String> as Check>::describe(rest),
                "pr" => <Option<PullRequest> as Check>::describe(rest),
                "note" => <Option<String> as Check>::describe(rest),
                "refused" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
