// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

/// TS: packages/protocol/src/pull-request.ts:435 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestComment {
    pub id: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub node_id: Option<String>,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub association: Option<String>,
    pub bot: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub avatar: Option<String>,
    pub body: String,
    pub url: String,
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reactions: Option<Vec<PullRequestReaction>>,
}
impl Check for PullRequestComment {
    /// Rules: id: >= 0.
    /// Nested checks: reactions.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.id.0 .0, 0.0).map_err(|e| e.under("id"))?;
        self.reactions.check().map_err(|e| e.under("reactions"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsInt>(obj, "id", &mut out, Self::rule_id);
        crate::types::keys::opt_key::<String>(obj, "nodeId", &mut out);
        crate::types::keys::req_key::<String>(obj, "author", &mut out);
        crate::types::keys::opt_key::<String>(obj, "association", &mut out);
        crate::types::keys::req_key::<bool>(obj, "bot", &mut out);
        crate::types::keys::opt_key::<String>(obj, "avatar", &mut out);
        crate::types::keys::req_key::<String>(obj, "body", &mut out);
        crate::types::keys::req_key::<String>(obj, "url", &mut out);
        crate::types::keys::req_key::<String>(obj, "at", &mut out);
        crate::types::keys::opt_key::<Vec<PullRequestReaction>>(obj, "reactions", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <i64 as Check>::describe(rest),
                "nodeId" => <Option<String> as Check>::describe(rest),
                "author" => <String as Check>::describe(rest),
                "association" => <Option<String> as Check>::describe(rest),
                "bot" => <bool as Check>::describe(rest),
                "avatar" => <Option<String> as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                "url" => <String as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                "reactions" => <Option<Vec<PullRequestReaction>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PullRequestComment {
    fn rule_id(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/pull-request.ts:453 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestReviewComment {
    pub id: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub node_id: Option<String>,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub line: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub side: Option<String>,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub association: Option<String>,
    pub bot: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub avatar: Option<String>,
    pub body: String,
    pub url: String,
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub hunk: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reply_to: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub review_id: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub resolved: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reactions: Option<Vec<PullRequestReaction>>,
}
impl Check for PullRequestReviewComment {
    /// Rules: id: >= 0; line: >= 0; replyTo: >= 0; reviewId: >= 0.
    /// Nested checks: reactions.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.id.0 .0, 0.0).map_err(|e| e.under("id"))?;
        if let Some(v) = self.line {
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("line"))?;
        }
        if let Some(v) = self.reply_to {
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("replyTo"))?;
        }
        if let Some(v) = self.review_id {
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("reviewId"))?;
        }
        self.reactions.check().map_err(|e| e.under("reactions"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsInt>(obj, "id", &mut out, Self::rule_id);
        crate::types::keys::opt_key::<String>(obj, "nodeId", &mut out);
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "line", &mut out, Self::rule_line);
        crate::types::keys::opt_key::<String>(obj, "side", &mut out);
        crate::types::keys::req_key::<String>(obj, "author", &mut out);
        crate::types::keys::opt_key::<String>(obj, "association", &mut out);
        crate::types::keys::req_key::<bool>(obj, "bot", &mut out);
        crate::types::keys::opt_key::<String>(obj, "avatar", &mut out);
        crate::types::keys::req_key::<String>(obj, "body", &mut out);
        crate::types::keys::req_key::<String>(obj, "url", &mut out);
        crate::types::keys::req_key::<String>(obj, "at", &mut out);
        crate::types::keys::opt_key::<String>(obj, "hunk", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "replyTo", &mut out, Self::rule_reply_to);
        crate::types::keys::opt_key_with::<JsInt>(obj, "reviewId", &mut out, Self::rule_review_id);
        crate::types::keys::opt_key::<bool>(obj, "resolved", &mut out);
        crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::opt_key::<Vec<PullRequestReaction>>(obj, "reactions", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <i64 as Check>::describe(rest),
                "nodeId" => <Option<String> as Check>::describe(rest),
                "path" => <String as Check>::describe(rest),
                "line" => <Option<i64> as Check>::describe(rest),
                "side" => <Option<String> as Check>::describe(rest),
                "author" => <String as Check>::describe(rest),
                "association" => <Option<String> as Check>::describe(rest),
                "bot" => <bool as Check>::describe(rest),
                "avatar" => <Option<String> as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                "url" => <String as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                "hunk" => <Option<String> as Check>::describe(rest),
                "replyTo" => <Option<i64> as Check>::describe(rest),
                "reviewId" => <Option<i64> as Check>::describe(rest),
                "resolved" => <Option<bool> as Check>::describe(rest),
                "threadId" => <Option<String> as Check>::describe(rest),
                "reactions" => <Option<Vec<PullRequestReaction>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PullRequestReviewComment {
    fn rule_id(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
    fn rule_line(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
        }
        Ok(())
    }
    fn rule_reply_to(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
        }
        Ok(())
    }
    fn rule_review_id(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrViewReplyLatestReviewsItem {
    pub author: String,
    pub state: String,
    pub at: String,
}
impl Check for GitPrViewReplyLatestReviewsItem {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "author", &mut out);
        crate::types::keys::req_key::<String>(obj, "state", &mut out);
        crate::types::keys::req_key::<String>(obj, "at", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "author" => <String as Check>::describe(rest),
                "state" => <String as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrViewReplyFilesItem {
    pub path: String,
    pub additions: JsInt,
    pub deletions: JsInt,
}
impl Check for GitPrViewReplyFilesItem {
    /// Rules: additions: >= 0; deletions: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.additions.0 .0, 0.0).map_err(|e| e.under("additions"))?;
        crate::types::check::num_at_least(self.deletions.0 .0, 0.0).map_err(|e| e.under("deletions"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "path", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "additions", &mut out, Self::rule_additions);
        crate::types::keys::req_key_with::<JsInt>(obj, "deletions", &mut out, Self::rule_deletions);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "additions" => <i64 as Check>::describe(rest),
                "deletions" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl GitPrViewReplyFilesItem {
    fn rule_additions(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
    fn rule_deletions(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrViewReplyCut {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub commits: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reviews: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub threads: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub comments: Option<bool>,
}
impl Check for GitPrViewReplyCut {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<bool>(obj, "commits", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "reviews", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "threads", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "comments", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "commits" => <Option<bool> as Check>::describe(rest),
                "reviews" => <Option<bool> as Check>::describe(rest),
                "threads" => <Option<bool> as Check>::describe(rest),
                "comments" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:479 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrViewReply {
    pub title: String,
    pub body: String,
    pub author: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub closed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub merged_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub merged_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub merge_commit: Option<String>,
    pub labels: Vec<PullRequestLabel>,
    pub review_requests: Vec<PullRequestReviewRequest>,
    pub latest_reviews: Vec<GitPrViewReplyLatestReviewsItem>,
    pub assignees: Vec<String>,
    pub commits: Vec<PullRequestCommit>,
    pub reviews: Vec<PullRequestReview>,
    pub comments: Vec<PullRequestComment>,
    pub review_comments: Vec<PullRequestReviewComment>,
    pub files: Vec<GitPrViewReplyFilesItem>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cut: Option<GitPrViewReplyCut>,
}
impl Check for GitPrViewReply {
    /// Nested checks: labels, reviewRequests, latestReviews, commits, reviews, comments, reviewComments, files, cut.
    fn check(&self) -> Result<(), SchemaError> {
        self.labels.check().map_err(|e| e.under("labels"))?;
        self.review_requests.check().map_err(|e| e.under("reviewRequests"))?;
        self.latest_reviews.check().map_err(|e| e.under("latestReviews"))?;
        self.commits.check().map_err(|e| e.under("commits"))?;
        self.reviews.check().map_err(|e| e.under("reviews"))?;
        self.comments.check().map_err(|e| e.under("comments"))?;
        self.review_comments.check().map_err(|e| e.under("reviewComments"))?;
        self.files.check().map_err(|e| e.under("files"))?;
        self.cut.check().map_err(|e| e.under("cut"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "title", &mut out);
        crate::types::keys::req_key::<String>(obj, "body", &mut out);
        crate::types::keys::req_key::<String>(obj, "author", &mut out);
        crate::types::keys::req_key::<String>(obj, "createdAt", &mut out);
        crate::types::keys::req_key::<String>(obj, "updatedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "closedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "mergedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "mergedBy", &mut out);
        crate::types::keys::opt_key::<String>(obj, "mergeCommit", &mut out);
        crate::types::keys::req_key::<Vec<PullRequestLabel>>(obj, "labels", &mut out);
        crate::types::keys::req_key::<Vec<PullRequestReviewRequest>>(obj, "reviewRequests", &mut out);
        crate::types::keys::req_key::<Vec<GitPrViewReplyLatestReviewsItem>>(obj, "latestReviews", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "assignees", &mut out);
        crate::types::keys::req_key::<Vec<PullRequestCommit>>(obj, "commits", &mut out);
        crate::types::keys::req_key::<Vec<PullRequestReview>>(obj, "reviews", &mut out);
        crate::types::keys::req_key::<Vec<PullRequestComment>>(obj, "comments", &mut out);
        crate::types::keys::req_key::<Vec<PullRequestReviewComment>>(obj, "reviewComments", &mut out);
        crate::types::keys::req_key::<Vec<GitPrViewReplyFilesItem>>(obj, "files", &mut out);
        crate::types::keys::opt_key::<GitPrViewReplyCut>(obj, "cut", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "title" => <String as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                "author" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "updatedAt" => <String as Check>::describe(rest),
                "closedAt" => <Option<String> as Check>::describe(rest),
                "mergedAt" => <Option<String> as Check>::describe(rest),
                "mergedBy" => <Option<String> as Check>::describe(rest),
                "mergeCommit" => <Option<String> as Check>::describe(rest),
                "labels" => <Vec<PullRequestLabel> as Check>::describe(rest),
                "reviewRequests" => <Vec<PullRequestReviewRequest> as Check>::describe(rest),
                "latestReviews" => <Vec<GitPrViewReplyLatestReviewsItem> as Check>::describe(rest),
                "assignees" => <Vec<String> as Check>::describe(rest),
                "commits" => <Vec<PullRequestCommit> as Check>::describe(rest),
                "reviews" => <Vec<PullRequestReview> as Check>::describe(rest),
                "comments" => <Vec<PullRequestComment> as Check>::describe(rest),
                "reviewComments" => <Vec<PullRequestReviewComment> as Check>::describe(rest),
                "files" => <Vec<GitPrViewReplyFilesItem> as Check>::describe(rest),
                "cut" => <Option<GitPrViewReplyCut> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:511 at a3b870a6e.
/// A pull request read: the pull request, the step it last saw, and whether nothing changed since.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrReadReply {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pr: Option<PullRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub seen: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub unchanged: Option<bool>,
}
impl Check for GitPrReadReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.pr.check().map_err(|e| e.under("pr"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<PullRequest>(obj, "pr", &mut out);
        crate::types::keys::opt_key::<String>(obj, "seen", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "unchanged", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "pr" => <Option<PullRequest> as Check>::describe(rest),
                "seen" => <Option<String> as Check>::describe(rest),
                "unchanged" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:523 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitUpdateReply {
    pub base: String,
    pub merged: bool,
    pub commits: JsInt,
    pub conflicts: Vec<String>,
}
impl Check for GitUpdateReply {
    /// Rules: commits: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.commits.0 .0, 0.0).map_err(|e| e.under("commits"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "base", &mut out);
        crate::types::keys::req_key::<bool>(obj, "merged", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "commits", &mut out, Self::rule_commits);
        crate::types::keys::req_key::<Vec<String>>(obj, "conflicts", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "base" => <String as Check>::describe(rest),
                "merged" => <bool as Check>::describe(rest),
                "commits" => <i64 as Check>::describe(rest),
                "conflicts" => <Vec<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl GitUpdateReply {
    fn rule_commits(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PullRequestItemKind {
    #[serde(rename = "comment")]
    Comment,
    #[serde(rename = "review")]
    Review,
    #[serde(rename = "reviewComment")]
    ReviewComment,
}
impl Check for PullRequestItemKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["comment", "review", "reviewComment"]))
    }
}

/// TS: packages/protocol/src/pull-request.ts:528 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestItem {
    pub kind: PullRequestItemKind,
    pub id: JsInt,
}
impl Check for PullRequestItem {
    /// Rules: id: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        crate::types::check::num_at_least(self.id.0 .0, 0.0).map_err(|e| e.under("id"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<PullRequestItemKind>(obj, "kind", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "id", &mut out, Self::rule_id);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "kind" => <PullRequestItemKind as Check>::describe(rest),
                "id" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PullRequestItem {
    fn rule_id(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/pull-request.ts:532 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestSent {
    pub kind: PullRequestItemKind,
    pub id: JsInt,
    pub at: JsNumber,
}
impl Check for PullRequestSent {
    /// Rules: id: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        crate::types::check::num_at_least(self.id.0 .0, 0.0).map_err(|e| e.under("id"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<PullRequestItemKind>(obj, "kind", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "id", &mut out, Self::rule_id);
        crate::types::keys::req_key::<JsNumber>(obj, "at", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "kind" => <PullRequestItemKind as Check>::describe(rest),
                "id" => <i64 as Check>::describe(rest),
                "at" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PullRequestSent {
    fn rule_id(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0 .0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/pull-request.ts:538 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestPage {
    pub title: String,
    pub body: String,
    pub author: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub closed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub merged_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub merged_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub merge_commit: Option<String>,
    pub labels: Vec<PullRequestLabel>,
    pub review_requests: Vec<PullRequestReviewRequest>,
    pub latest_reviews: Vec<GitPrViewReplyLatestReviewsItem>,
    pub assignees: Vec<String>,
    pub commits: Vec<PullRequestCommit>,
    pub reviews: Vec<PullRequestReview>,
    pub comments: Vec<PullRequestComment>,
    pub review_comments: Vec<PullRequestReviewComment>,
    pub files: Vec<GitPrViewReplyFilesItem>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cut: Option<GitPrViewReplyCut>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub merge: Option<GitRepoReadReply>,
    pub sent: Vec<PullRequestSent>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub posts_as_you: Option<bool>,
}
impl Check for PullRequestPage {
    /// Nested checks: labels, reviewRequests, commits, reviews, comments, reviewComments, merge, sent.
    fn check(&self) -> Result<(), SchemaError> {
        self.labels.check().map_err(|e| e.under("labels"))?;
        self.review_requests.check().map_err(|e| e.under("reviewRequests"))?;
        self.latest_reviews.check().map_err(|e| e.under("latestReviews"))?;
        self.commits.check().map_err(|e| e.under("commits"))?;
        self.reviews.check().map_err(|e| e.under("reviews"))?;
        self.comments.check().map_err(|e| e.under("comments"))?;
        self.review_comments.check().map_err(|e| e.under("reviewComments"))?;
        self.files.check().map_err(|e| e.under("files"))?;
        self.cut.check().map_err(|e| e.under("cut"))?;
        self.merge.check().map_err(|e| e.under("merge"))?;
        self.sent.check().map_err(|e| e.under("sent"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "title", &mut out);
        crate::types::keys::req_key::<String>(obj, "body", &mut out);
        crate::types::keys::req_key::<String>(obj, "author", &mut out);
        crate::types::keys::req_key::<String>(obj, "createdAt", &mut out);
        crate::types::keys::req_key::<String>(obj, "updatedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "closedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "mergedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "mergedBy", &mut out);
        crate::types::keys::opt_key::<String>(obj, "mergeCommit", &mut out);
        crate::types::keys::req_key::<Vec<PullRequestLabel>>(obj, "labels", &mut out);
        crate::types::keys::req_key::<Vec<PullRequestReviewRequest>>(obj, "reviewRequests", &mut out);
        crate::types::keys::req_key::<Vec<GitPrViewReplyLatestReviewsItem>>(obj, "latestReviews", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "assignees", &mut out);
        crate::types::keys::req_key::<Vec<PullRequestCommit>>(obj, "commits", &mut out);
        crate::types::keys::req_key::<Vec<PullRequestReview>>(obj, "reviews", &mut out);
        crate::types::keys::req_key::<Vec<PullRequestComment>>(obj, "comments", &mut out);
        crate::types::keys::req_key::<Vec<PullRequestReviewComment>>(obj, "reviewComments", &mut out);
        crate::types::keys::req_key::<Vec<GitPrViewReplyFilesItem>>(obj, "files", &mut out);
        crate::types::keys::opt_key::<GitPrViewReplyCut>(obj, "cut", &mut out);
        crate::types::keys::opt_key::<GitRepoReadReply>(obj, "merge", &mut out);
        crate::types::keys::req_key::<Vec<PullRequestSent>>(obj, "sent", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "postsAsYou", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "title" => <String as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                "author" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "updatedAt" => <String as Check>::describe(rest),
                "closedAt" => <Option<String> as Check>::describe(rest),
                "mergedAt" => <Option<String> as Check>::describe(rest),
                "mergedBy" => <Option<String> as Check>::describe(rest),
                "mergeCommit" => <Option<String> as Check>::describe(rest),
                "labels" => <Vec<PullRequestLabel> as Check>::describe(rest),
                "reviewRequests" => <Vec<PullRequestReviewRequest> as Check>::describe(rest),
                "latestReviews" => <Vec<GitPrViewReplyLatestReviewsItem> as Check>::describe(rest),
                "assignees" => <Vec<String> as Check>::describe(rest),
                "commits" => <Vec<PullRequestCommit> as Check>::describe(rest),
                "reviews" => <Vec<PullRequestReview> as Check>::describe(rest),
                "comments" => <Vec<PullRequestComment> as Check>::describe(rest),
                "reviewComments" => <Vec<PullRequestReviewComment> as Check>::describe(rest),
                "files" => <Vec<GitPrViewReplyFilesItem> as Check>::describe(rest),
                "cut" => <Option<GitPrViewReplyCut> as Check>::describe(rest),
                "merge" => <Option<GitRepoReadReply> as Check>::describe(rest),
                "sent" => <Vec<PullRequestSent> as Check>::describe(rest),
                "postsAsYou" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PullRequestSendResultOutcome {
    #[serde(rename = "steered")]
    Steered,
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "started")]
    Started,
    #[serde(rename = "held")]
    Held,
}
impl Check for PullRequestSendResultOutcome {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["steered", "queued", "started", "held"]))
    }
}

/// TS: packages/protocol/src/pull-request.ts:550 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestSendResult {
    pub outcome: PullRequestSendResultOutcome,
    pub thread_id: String,
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub capped: Option<ThreadCapWait>,
    pub sent: Vec<PullRequestSent>,
}
impl Check for PullRequestSendResult {
    /// Nested checks: capped, sent.
    fn check(&self) -> Result<(), SchemaError> {
        self.outcome.check().map_err(|e| e.under("outcome"))?;
        self.capped.check().map_err(|e| e.under("capped"))?;
        self.sent.check().map_err(|e| e.under("sent"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<PullRequestSendResultOutcome>(obj, "outcome", &mut out);
        crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::req_key::<String>(obj, "agent", &mut out);
        crate::types::keys::opt_key::<ThreadCapWait>(obj, "capped", &mut out);
        crate::types::keys::req_key::<Vec<PullRequestSent>>(obj, "sent", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "outcome" => <PullRequestSendResultOutcome as Check>::describe(rest),
                "threadId" => <String as Check>::describe(rest),
                "agent" => <String as Check>::describe(rest),
                "capped" => <Option<ThreadCapWait> as Check>::describe(rest),
                "sent" => <Vec<PullRequestSent> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:561 at a3b870a6e.
/// The comment a reply made, on the conversation or on a line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrReplyReply {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub comment: Option<PullRequestComment>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub review_comment: Option<PullRequestReviewComment>,
}
impl Check for GitPrReplyReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.comment.check().map_err(|e| e.under("comment"))?;
        self.review_comment.check().map_err(|e| e.under("reviewComment"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<PullRequestComment>(obj, "comment", &mut out);
        crate::types::keys::opt_key::<PullRequestReviewComment>(obj, "reviewComment", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "comment" => <Option<PullRequestComment> as Check>::describe(rest),
                "reviewComment" => <Option<PullRequestReviewComment> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/pull-request.ts:569 at a3b870a6e.
/// A reaction added or taken off as the person: the item's node id and every reaction on it now.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrReactReply {
    pub subject: String,
    pub reactions: Vec<PullRequestReaction>,
}
impl Check for GitPrReactReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.reactions.check().map_err(|e| e.under("reactions"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "subject", &mut out);
        crate::types::keys::req_key::<Vec<PullRequestReaction>>(obj, "reactions", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "subject" => <String as Check>::describe(rest),
                "reactions" => <Vec<PullRequestReaction> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
