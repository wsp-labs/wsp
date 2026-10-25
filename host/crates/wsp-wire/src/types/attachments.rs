// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/attachments.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;

/// TS: packages/protocol/src/attachments.ts:27 at a3b870a6e.
pub const IMAGE_MAX_BYTES: u64 = 10_485_760;

/// TS: packages/protocol/src/attachments.ts:31 at a3b870a6e.
pub const FILE_MAX_BYTES: u64 = 10_485_760;

/// TS: packages/protocol/src/attachments.ts:34 at a3b870a6e.
pub const FILES_MAX: u64 = 5;

/// TS: packages/protocol/src/attachments.ts:46 at a3b870a6e.
pub const UNTYPED_FILE: &str = "application/octet-stream";

/// TS: packages/protocol/src/attachments.ts:75 at a3b870a6e.
pub const IMAGE_TYPE_WORDS: &str = "PNG, JPEG, GIF or WebP";

/// TS: packages/protocol/src/attachments.ts:79 at a3b870a6e.
pub const IMAGE_MAX_WORDS: &str = "10 MB";

/// TS: packages/protocol/src/attachments.ts:80 at a3b870a6e.
pub const FILE_MAX_WORDS: &str = "10 MB";

/// TS: packages/protocol/src/attachments.ts:190 at a3b870a6e.
pub const FILES_DIR: &str = ".wsp-files";

/// TS: packages/protocol/src/attachments.ts:38 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub media_type: String,
    pub bytes: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub name: Option<String>,
}

impl Check for Attachment {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "mediaType", &mut out);
        crate::types::keys::req_key::<String>(obj, "bytes", &mut out);
        crate::types::keys::opt_key::<String>(obj, "name", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "mediaType" => <String as Check>::describe(rest),
                "bytes" => <String as Check>::describe(rest),
                "name" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/attachments.ts:61 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentRecord {
    pub media_type: String,
    pub bytes: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub name: Option<String>,
}

impl Check for AttachmentRecord {
    /// Rules: bytes: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::num_at_least(self.bytes.0 .0, 0.0).map_err(|e| e.under("bytes"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "mediaType", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "bytes", &mut out, Self::rule_bytes);
        crate::types::keys::opt_key::<String>(obj, "name", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "mediaType" => <String as Check>::describe(rest),
                "bytes" => <i64 as Check>::describe(rest),
                "name" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl AttachmentRecord {
    fn rule_bytes(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least((*x).0 .0, 0.0)?;
        Ok(())
    }
}

/// TS: packages/protocol/src/attachments.ts:166 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeptAttachment {
    pub media_type: String,
    pub bytes: String,
}

impl Check for KeptAttachment {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "mediaType", &mut out);
        crate::types::keys::req_key::<String>(obj, "bytes", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "mediaType" => <String as Check>::describe(rest),
                "bytes" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
