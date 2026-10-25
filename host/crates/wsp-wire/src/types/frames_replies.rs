// SPDX-License-Identifier: AGPL-3.0-only
//! The daemon replies and the review request that wsp-frames declares with the keys zod has: each one reads its keys in
//! the schema's order and lists every issue, which wsp-frames' derived `Deserialize` does not (it answers the first key
//! the input holds that fails). start.ts:92, 100; tree.ts:12; pull-request.ts:390, 514, 517, 520, 565.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::check::{not_object, Check, SchemaError, Zod};
use crate::types::keys;
use wsp_frames::MergeMethod;

macro_rules! reply {
    ($(#[$meta:meta])* $name:ident { $($field:ident: $ty:ty = $key:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct $name {
            $(pub $field: $ty,)+
        }

        impl Check for $name {
            fn pass(value: &Value) -> Option<Vec<SchemaError>> {
                let Value::Object(obj) = value else {
                    return Some(vec![not_object(value)]);
                };
                let mut out = Vec::new();
                $(keys::req_key::<$ty>(obj, $key, &mut out);)+
                Some(out)
            }

            fn describe(path: &[Value]) -> Option<Zod> {
                match path.split_first() {
                    None => Some(Zod::Word("object")),
                    Some((key, rest)) => match key.as_str()? {
                        $($key => <$ty as Check>::describe(rest),)+
                        _ => None,
                    },
                }
            }
        }
    };
}

reply!(
    /// TS: packages/protocol/src/start.ts:92 at a3b870a6e.
    GitPrDiffReply { diff: String = "diff", truncated: bool = "truncated", left: Vec<String> = "left" }
);

reply!(
    /// TS: packages/protocol/src/start.ts:100 at a3b870a6e.
    GitPrReviewReply { url: String = "url", folded: Vec<String> = "folded" }
);

reply!(
    /// TS: packages/protocol/src/tree.ts:12 at a3b870a6e.
    GitStartOnReply { branch: String = "branch", oid: String = "oid" }
);

reply!(
    /// TS: packages/protocol/src/pull-request.ts:390 at a3b870a6e.
    PullRequestReviewRequest { name: String = "name", team: bool = "team" }
);

reply!(
    /// TS: packages/protocol/src/pull-request.ts:514 at a3b870a6e.
    GitRunLogReply { lines: Vec<String> = "lines", truncated: bool = "truncated" }
);

reply!(
    /// TS: packages/protocol/src/pull-request.ts:517 at a3b870a6e.
    GitPrMergeReply { merged: bool = "merged", auto_armed: bool = "autoArmed" }
);

reply!(
    /// TS: packages/protocol/src/pull-request.ts:520 at a3b870a6e.
    GitRepoReadReply { methods: Vec<MergeMethod> = "methods", default_method: MergeMethod = "defaultMethod", auto_merge: bool = "autoMerge" }
);

reply!(
    /// TS: packages/protocol/src/pull-request.ts:565 at a3b870a6e.
    GitPrResolveReply { thread_id: String = "threadId", resolved: bool = "resolved" }
);
